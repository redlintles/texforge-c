"""Validate the Conan recipe, the vcpkg port and the offline source release end to end.

Run before a release and whenever CMakeLists.txt, cmake/, build.rs, packaging/
or the release scripts change. Steps:

1. Consistency: recipe/port versions match Cargo.toml and the vcpkg versions
   database records the current git-tree of the port.
2. Build a source release (`make_source_release.py --allow-dirty`) into a
   temporary directory.
3. Offline CMake build from that archive with an empty CARGO_HOME and an
   unreachable proxy, static and shared, then build tests/package against each
   install in Debug and Release.
4. Conan (if `conan` is installed): `conan create` static and shared, with the
   archive served from a local HTTP server instead of the release URL.
5. vcpkg (if VCPKG_ROOT points to a bootstrapped vcpkg): install the overlay
   port for x64-linux and x64-linux-dynamic, consume it through the vcpkg
   toolchain, and consume it again through a git registry built from
   packaging/vcpkg.

The recipes are never modified: hashes are substituted only in temporary copies.

Usage: python3 scripts/check_packaging.py
"""

from __future__ import annotations

import functools
import hashlib
import http.server
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXPECTED = "Hello, world 🌍!\n"
OFFLINE_ENV = {"HTTPS_PROXY": "http://127.0.0.1:9", "HTTP_PROXY": "http://127.0.0.1:9",
               "https_proxy": "http://127.0.0.1:9", "http_proxy": "http://127.0.0.1:9"}


def run(command: list[str], combined: bool = False, **kwargs) -> str:
    """Run a command and return its stdout (or stdout + stderr), failing loudly."""
    result = subprocess.run(command, capture_output=True, text=True, **kwargs)
    if result.returncode != 0:
        raise SystemExit(f"FAILED: {' '.join(command)}\n{result.stdout[-4000:]}\n{result.stderr[-4000:]}")
    return result.stdout + result.stderr if combined else result.stdout


def version() -> str:
    return re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M)[1]


def check_consistency() -> None:
    release = version()
    port = json.loads((ROOT / "packaging/vcpkg/ports/textforge-c/vcpkg.json").read_text())
    assert port["version"] == release, "vcpkg.json version differs from Cargo.toml"
    assert f'"{release}":' in (ROOT / "packaging/conan/recipes/textforge-c/config.yml").read_text()
    assert f'"{release}":' in (ROOT / "packaging/conan/recipes/textforge-c/all/conandata.yml").read_text()
    sys.path.insert(0, str(ROOT / "scripts"))
    from update_recipes import port_tree  # noqa: E402  (same computation as the writer)
    history = json.loads((ROOT / "packaging/vcpkg/versions/t-/textforge-c.json").read_text())["versions"]
    assert history[0]["version"] == release and history[0]["git-tree"] == port_tree(), (
        "packaging/vcpkg/versions is stale; run scripts/update_recipes.py with the release hashes")
    print("Recipe versions and the vcpkg versions database are consistent")


def consumer(prefix_args: list[str], build: Path, config: str, env: dict | None = None) -> None:
    run(["cmake", "-S", str(ROOT / "tests/package"), "-B", str(build), "-G", "Ninja",
         f"-DCMAKE_BUILD_TYPE={config}", *prefix_args], env=env)
    run(["cmake", "--build", str(build)], env=env)
    assert run([str(build / "quickstart")]) == EXPECTED


def check_cmake(archive: Path, work: Path) -> None:
    run(["tar", "xzf", str(archive), "-C", str(work)])
    source = work / archive.name.removesuffix(".tar.gz")
    cargo_home = work / "empty-cargo-home"
    cargo_home.mkdir()
    env = {**os.environ, **OFFLINE_ENV, "CARGO_HOME": str(cargo_home)}
    cargo = shutil.which("cargo")
    for shared in ("OFF", "ON"):
        build, prefix = work / f"build-{shared}", work / f"install-{shared}"
        run(["cmake", "-S", str(source), "-B", str(build), "-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release",
             f"-DBUILD_SHARED_LIBS={shared}", f"-DCMAKE_INSTALL_PREFIX={prefix}",
             f"-DCARGO_EXECUTABLE={cargo}"], env=env)
        run(["cmake", "--build", str(build)], env=env)
        run(["cmake", "--install", str(build)])
        for config in ("Debug", "Release"):
            consumer([f"-DCMAKE_PREFIX_PATH={prefix}"], work / f"consumer-{shared}-{config}", config)
    print("Offline CMake build from the source release: static and shared, Debug and Release consumers")


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args) -> None:
        pass


def check_conan(archive: Path, work: Path) -> None:
    if shutil.which("conan") is None:
        print("conan not found; skipping the Conan recipe check")
        return
    server = http.server.ThreadingHTTPServer(
        ("127.0.0.1", 0), functools.partial(QuietHandler, directory=str(archive.parent)))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        recipe = work / "conan-recipe"
        shutil.copytree(ROOT / "packaging/conan/recipes/textforge-c", recipe)
        data = recipe / "all/conandata.yml"
        sha256 = hashlib.sha256(archive.read_bytes()).hexdigest()
        url = f"http://127.0.0.1:{server.server_port}/{archive.name}"
        text = re.sub(r'url: "[^"]*"', f'url: "{url}"', data.read_text(), count=1)
        data.write_text(re.sub(r'sha256: "[^"]*"', f'sha256: "{sha256}"', text, count=1))
        env = {**os.environ, "NO_PROXY": "127.0.0.1", "no_proxy": "127.0.0.1",
               "CONAN_HOME": str(work / "conan-home")}
        run(["conan", "profile", "detect", "--force"], env=env)
        for shared in ("False", "True"):
            output = run(["conan", "create", str(recipe / "all"), "--version", version(), "-nr",
                          "-o", f"textforge-c/*:shared={shared}", "--build=missing"], combined=True, env=env)
            assert "TEXTFORGE!" in output, output[-2000:]
    finally:
        server.shutdown()
    print("Conan: conan create (static and shared) with test_package")


def check_vcpkg(archive: Path, work: Path) -> None:
    root = os.environ.get("VCPKG_ROOT")
    if not root or not (Path(root) / "vcpkg").exists():
        print("VCPKG_ROOT not set to a bootstrapped vcpkg; skipping the vcpkg port check")
        return
    vcpkg = str(Path(root) / "vcpkg")
    port = work / "vcpkg-ports"
    shutil.copytree(ROOT / "packaging/vcpkg/ports", port)
    portfile = port / "textforge-c/portfile.cmake"
    sha512 = hashlib.sha512(archive.read_bytes()).hexdigest()
    portfile.write_text(re.sub(r"SHA512 \S+", f"SHA512 {sha512}", portfile.read_text()))
    downloads = work / "vcpkg-downloads"
    downloads.mkdir()
    shutil.copy2(archive, downloads / archive.name)
    common = [f"--x-buildtrees-root={work / 'vcpkg-buildtrees'}", f"--x-install-root={work / 'vcpkg-installed'}",
              f"--downloads-root={downloads}", "--binarysource=clear"]
    toolchain = f"-DCMAKE_TOOLCHAIN_FILE={Path(root) / 'scripts/buildsystems/vcpkg.cmake'}"
    for triplet in ("x64-linux", "x64-linux-dynamic"):
        output = run([vcpkg, "install", f"textforge-c:{triplet}", f"--overlay-ports={port}", "--classic", *common],
                     combined=True)
        assert "warning" not in output.lower(), output[-3000:]
        for config in ("Debug", "Release"):
            consumer([toolchain, f"-DVCPKG_TARGET_TRIPLET={triplet}", f"-DVCPKG_INSTALLED_DIR={work / 'vcpkg-installed'}",
                      "-DVCPKG_MANIFEST_MODE=OFF"], work / f"vcpkg-consumer-{triplet}-{config}", config)
    print("vcpkg: overlay port for x64-linux and x64-linux-dynamic, Debug and Release consumers")
    check_vcpkg_registry(vcpkg, root, port, work, common)


def check_vcpkg_registry(vcpkg: str, root: str, port: Path, work: Path, common: list[str]) -> None:
    """Consume the port through a git registry created from packaging/vcpkg."""
    registry = work / "vcpkg-registry"
    shutil.copytree(ROOT / "packaging/vcpkg", registry)
    shutil.rmtree(registry / "ports")
    shutil.copytree(port, registry / "ports")  # same port with the test archive hash
    sys.path.insert(0, str(ROOT / "scripts"))
    git = ["git", "-c", "user.name=check", "-c", "user.email=check@localhost"]
    run([*git, "init", "-q"], cwd=registry)
    run([*git, "add", "."], cwd=registry)
    tree = run(["git", "write-tree", "--prefix=ports/textforge-c/"], cwd=registry).strip()
    history = registry / "versions/t-/textforge-c.json"
    entries = json.loads(history.read_text())
    entries["versions"][0]["git-tree"] = tree
    history.write_text(json.dumps(entries, indent=2) + "\n")
    run([*git, "add", "."], cwd=registry)
    run([*git, "commit", "-qm", "registry"], cwd=registry)
    baseline = run(["git", "rev-parse", "HEAD"], cwd=registry).strip()
    project = work / "vcpkg-manifest-consumer"
    shutil.copytree(ROOT / "tests/package", project)
    shutil.copy2(ROOT / "examples/c/quickstart.c", project / "quickstart.c")
    lists = project / "CMakeLists.txt"
    lists.write_text(lists.read_text().replace("../../examples/c/quickstart.c", "quickstart.c"))
    (project / "vcpkg.json").write_text(json.dumps({"dependencies": ["textforge-c"]}) + "\n")
    default_baseline = run(["git", "rev-parse", "HEAD"], cwd=root).strip()
    configuration = {
        "default-registry": {"kind": "git", "repository": "https://github.com/microsoft/vcpkg",
                             "baseline": default_baseline},
        "registries": [{"kind": "git", "repository": registry.as_uri(), "baseline": baseline,
                        "packages": ["textforge-c"]}],
    }
    (project / "vcpkg-configuration.json").write_text(json.dumps(configuration, indent=2) + "\n")
    downloads = next(arg for arg in common if arg.startswith("--downloads-root=")).split("=", 1)[1]
    env = {**os.environ, "VCPKG_DOWNLOADS": downloads, "VCPKG_BINARY_SOURCES": "clear"}
    consumer([f"-DCMAKE_TOOLCHAIN_FILE={Path(root) / 'scripts/buildsystems/vcpkg.cmake'}"],
             project / "build", "Release", env)
    print("vcpkg: manifest-mode consumer through a git registry built from packaging/vcpkg")


def main() -> None:
    check_consistency()
    with tempfile.TemporaryDirectory(prefix="textforge-c-packaging-") as scratch:
        work = Path(scratch)
        run([sys.executable, str(ROOT / "scripts/make_source_release.py"), "--allow-dirty", "--output",
             str(work / "dist")])
        archive = next((work / "dist").glob("textforge-c-*.tar.gz"))
        check_cmake(archive, work)
        check_conan(archive, work)
        check_vcpkg(archive, work)
    print("Packaging checks passed")


if __name__ == "__main__":
    main()

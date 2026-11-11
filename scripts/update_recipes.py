"""Point the Conan recipe and the vcpkg port at a source release.

Normally called by `scripts/make_source_release.py --update-recipes`. Writes, for
the version in Cargo.toml and the URL template in packaging/source.json:

  packaging/conan/recipes/textforge-c/config.yml        version -> folder "all"
  packaging/conan/recipes/textforge-c/all/conandata.yml url + sha256 (older entries kept)
  packaging/vcpkg/ports/textforge-c/vcpkg.json          version
  packaging/vcpkg/ports/textforge-c/portfile.cmake      URL + SHA512
  packaging/vcpkg/versions/baseline.json                baseline version
  packaging/vcpkg/versions/t-/textforge-c.json          version -> git-tree of the port

The git-tree is the object id of packaging/vcpkg/ports/textforge-c as it will be
committed, which is identical in a vcpkg registry repository created from
packaging/vcpkg (see docs/packaging.md).

Usage: python3 scripts/update_recipes.py --sha256 <hex> --sha512 <hex>
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONAN = ROOT / "packaging/conan/recipes/textforge-c"
VCPKG = ROOT / "packaging/vcpkg"
PORT = VCPKG / "ports/textforge-c"


def version() -> str:
    return re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M)[1]


def update_conan(release: str, url: str, sha256: str) -> None:
    config = CONAN / "config.yml"
    versions = re.findall(r'^  "([^"]+)":', config.read_text(), re.M)
    versions = sorted(set(versions) | {release}, key=lambda v: [int(p) for p in v.split(".")], reverse=True)
    config.write_text("versions:\n" + "".join(f'  "{v}":\n    folder: all\n' for v in versions))

    data = CONAN / "all/conandata.yml"
    entries = dict(re.findall(r'^  "([^"]+)":\n((?:    .*\n)+)', data.read_text(), re.M))
    entries[release] = f'    url: "{url}"\n    sha256: "{sha256}"\n'
    ordered = sorted(entries, key=lambda v: [int(p) for p in v.split(".")], reverse=True)
    data.write_text("sources:\n" + "".join(f'  "{v}":\n{entries[v]}' for v in ordered))


def port_tree() -> str:
    """Git tree id of the port directory with its current working-tree content."""
    with tempfile.TemporaryDirectory() as scratch:
        env = {**os.environ, "GIT_INDEX_FILE": str(Path(scratch) / "index")}
        relative = PORT.relative_to(ROOT).as_posix()
        subprocess.run(["git", "add", "--", relative], cwd=ROOT, env=env, check=True)
        return subprocess.run(["git", "write-tree", f"--prefix={relative}/"], cwd=ROOT, env=env, check=True,
                              capture_output=True, text=True).stdout.strip()


def update_vcpkg(release: str, url: str, sha512: str) -> None:
    manifest_path = PORT / "vcpkg.json"
    manifest = json.loads(manifest_path.read_text())
    if manifest["version"] != release:
        manifest["version"] = release
        manifest.pop("port-version", None)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    port_version = manifest.get("port-version", 0)

    portfile = PORT / "portfile.cmake"
    text = portfile.read_text()
    text = re.sub(r'URLS "[^"]*"', f'URLS "{url.replace("{version}", "${VERSION}")}"', text)
    text = re.sub(r"SHA512 \S+", f"SHA512 {sha512}", text)
    portfile.write_text(text)

    (VCPKG / "versions/t-").mkdir(parents=True, exist_ok=True)
    baseline = {"default": {"textforge-c": {"baseline": release, "port-version": port_version}}}
    (VCPKG / "versions/baseline.json").write_text(json.dumps(baseline, indent=2) + "\n")
    history_path = VCPKG / "versions/t-/textforge-c.json"
    history = json.loads(history_path.read_text()) if history_path.exists() else {"versions": []}
    entries = [e for e in history["versions"] if (e["version"], e["port-version"]) != (release, port_version)]
    entries.insert(0, {"git-tree": port_tree(), "version": release, "port-version": port_version})
    history_path.write_text(json.dumps({"versions": entries}, indent=2) + "\n")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--sha512", required=True)
    args = parser.parse_args()
    release = version()
    template = json.loads((ROOT / "packaging/source.json").read_text())["url"]
    update_conan(release, template.format(version=release), args.sha256)
    update_vcpkg(release, template, args.sha512)
    print(f"Recipes now point at textforge-c {release}")


if __name__ == "__main__":
    main()

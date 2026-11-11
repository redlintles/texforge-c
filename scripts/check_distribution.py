"""Build the distributable package from a clean source tree and test the installed artifacts.

1. Copy the tracked source (what a source release contains) to a temporary
   directory outside this checkout.
2. Run `make` and `make install DESTDIR=<stage> PREFIX=/usr` there (the
   Makefile drives the CMake build in both linkages).
3. Check the installed layout: header identical to include/textforge.h,
   versioned shared library + SONAME symlinks, static library, pkg-config file,
   CMake package, and that every function declared in the header is exported.
4. Build examples/c/quickstart.c with `pkg-config` against the staged install,
   both dynamically and statically, and run both.
5. Compile and run every C test suite against the installed header and library.

Usage: python3 scripts/check_distribution.py
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_OUTPUT = "Hello, world 🌍!\n"


def tracked_files() -> list[str]:
    output = subprocess.run(["git", "ls-files", "-co", "--exclude-standard"], cwd=ROOT, check=True,
                            capture_output=True, text=True).stdout
    return [line for line in output.splitlines() if (ROOT / line).is_file()]


def run(command: list[str], **kwargs) -> str:
    return subprocess.run(command, check=True, capture_output=True, text=True, **kwargs).stdout


def main() -> None:
    # The source tree path contains spaces to catch unquoted paths in the build;
    # the install stage does not, because pkg-config's sysroot support cannot
    # express such paths.
    with tempfile.TemporaryDirectory(prefix="textforge c source ") as scratch, \
            tempfile.TemporaryDirectory(prefix="textforge-c-stage-") as staging:
        work = Path(staging)
        source = Path(scratch) / "source tree"
        for relative in tracked_files():
            destination = source / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, destination)
        stage = work / "stage"
        env = {**os.environ, "CARGO_TARGET_DIR": str(source / "target")}
        subprocess.run(["make", "-C", str(source)], check=True, env=env)
        subprocess.run(["make", "-C", str(source), "install", f"DESTDIR={stage}", "PREFIX=/usr"], check=True, env=env)

        # GNUInstallDirs may choose a multiarch directory (e.g. lib/x86_64-linux-gnu).
        lib = next(stage.glob("usr/lib*/**/libtextforge_c.a")).parent
        include = stage / "usr/include"
        version = re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M)[1]
        major, minor, _ = version.split(".")
        abi = f"0.{minor}" if major == "0" else major
        assert (include / "textforge.h").read_bytes() == (ROOT / "include/textforge.h").read_bytes()
        assert (lib / f"libtextforge_c.so.{version}").is_file()
        assert os.readlink(lib / f"libtextforge_c.so.{abi}") == f"libtextforge_c.so.{version}"
        assert os.readlink(lib / "libtextforge_c.so") == f"libtextforge_c.so.{abi}"
        assert (lib / "libtextforge_c.a").is_file()
        assert (lib / "cmake/textforge-c/textforge-c-config.cmake").is_file()
        soname = run(["readelf", "-d", str(lib / f"libtextforge_c.so.{version}")])
        assert f"Library soname: [libtextforge_c.so.{abi}]" in soname, soname

        header = (include / "textforge.h").read_text(encoding="utf-8")
        declared = set(re.findall(r"\b(tf_\w+)\(", header))
        exported = set(run(["nm", "-D", "--defined-only", str(lib / "libtextforge_c.so")]).split()[2::3])
        missing = sorted(name for name in declared if name not in exported)
        assert not missing, f"declared but not exported: {missing}"

        pc_env = {**os.environ, "PKG_CONFIG_PATH": str(lib / "pkgconfig"), "PKG_CONFIG_SYSROOT_DIR": str(stage)}
        cflags = run(["pkg-config", "--cflags", "textforge-c"], env=pc_env).split()
        libs = run(["pkg-config", "--libs", "textforge-c"], env=pc_env).split()
        static_libs = run(["pkg-config", "--static", "--libs-only-other", "--libs-only-l", "textforge-c"],
                          env=pc_env).split()
        assert run(["pkg-config", "--modversion", "textforge-c"], env=pc_env).strip() == version
        consumer = work / "consumer"
        consumer.mkdir()
        quickstart = ROOT / "examples/c/quickstart.c"
        cc = os.environ.get("CC", "cc")
        run([cc, "-std=c11", "-Wall", "-Wextra", "-Werror", *cflags, str(quickstart), "-o",
             str(consumer / "dynamic"), *libs])
        private = [flag for flag in static_libs if flag != "-ltextforge_c"]
        run([cc, "-std=c11", *cflags, str(quickstart), "-o", str(consumer / "static"), f"-L{lib}",
             "-Wl,-Bstatic", "-ltextforge_c", "-Wl,-Bdynamic", *private])
        dynamic_env = {**os.environ, "LD_LIBRARY_PATH": str(lib)}
        assert run([str(consumer / "dynamic")], env=dynamic_env) == EXPECTED_OUTPUT
        assert run([str(consumer / "static")]) == EXPECTED_OUTPUT
        assert "libtextforge_c" not in run(["ldd", str(consumer / "static")])

        cmake_build = work / "cmake-consumer"
        run(["cmake", "-S", str(ROOT / "tests/package"), "-B", str(cmake_build),
             f"-DCMAKE_PREFIX_PATH={stage / 'usr'}"])
        run(["cmake", "--build", str(cmake_build)])
        assert run([str(cmake_build / "quickstart")], env=dynamic_env) == EXPECTED_OUTPUT

        tests = sorted((ROOT / "tests/c").glob("test_*.c"))
        for test in tests:
            binary = consumer / test.stem
            run([cc, "-std=c11", "-pthread", *cflags, f"-I{ROOT / 'tests/c'}", str(test), "-o", str(binary), *libs])
            run([str(binary)], env=dynamic_env, cwd=consumer)
        print(f"Installed package verified: {len(declared)} exported functions, dynamic, static and "
              f"CMake consumers, {len(tests)} C suites against the installed library")


if __name__ == "__main__":
    main()

"""Build the library and run every C test program and header example.

Each program is compiled as strict C11 (-Wall -Wextra -Wpedantic -Werror) and
run three ways:
  plain     linked against the shared library
  sanitized AddressSanitizer + UndefinedBehaviorSanitizer (incl. leak detection)
  valgrind  memcheck; definite/indirect leaks and invalid accesses fail the run

Sources:
  tests/c/test_*.c         hand-written and generated test suites
  include/textforge.h      every ```c example in the documentation comments
                           (the C equivalent of doctests)

Usage: python3 scripts/run_c_tests.py [--no-valgrind] [--no-sanitizers] [--release]
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HEADER = ROOT / "include/textforge.h"
TESTS = ROOT / "tests/c"
FLAGS = ["-std=c11", "-Wall", "-Wextra", "-Wpedantic", "-Werror", "-g", "-O1"]
EXAMPLE_PRELUDE = """#define _POSIX_C_SOURCE 200809L
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "textforge.h"
"""


def doc_examples() -> list[tuple[str, str]]:
    """Extract ```c blocks from the header; file-scope helpers precede main()."""
    header = HEADER.read_text(encoding="utf-8")
    examples = []
    for index, match in enumerate(re.finditer(r"```c\n(.*?)\n\s*\*\s*```", header, re.S)):
        lines = [re.sub(r"^\s*\* ?", "", line) for line in match[1].splitlines()]
        file_scope: list[str] = []
        if lines and lines[0].startswith("static "):
            end = max(i for i, line in enumerate(lines) if line == "}")
            file_scope, lines = lines[: end + 1], lines[end + 1 :]
        following = header[match.end():]
        name = re.search(r"\b(tf_\w+|Tf\w+|TF_\w+)\b", following)
        label = f"doc_{index:02d}_{name[1] if name else 'example'}"
        body = "\n".join("    " + line for line in lines)
        source = EXAMPLE_PRELUDE + "\n".join(file_scope) + f"\n\nint main(void) {{\n{body}\n    return 0;\n}}\n"
        examples.append((label, source))
    return examples


def link_soname(library: Path) -> None:
    """Let in-tree programs resolve the versioned SONAME the build embeds."""
    if sys.platform.startswith("linux"):
        output = subprocess.run(["readelf", "-d", str(library / "libtextforge_c.so")],
                                capture_output=True, text=True, check=True).stdout
        soname = re.search(r"Library soname: \[([^\]]+)\]", output)
        if soname:
            link = library / soname[1]
            if not link.exists():
                link.symlink_to("libtextforge_c.so")


def compile_program(source: Path, output: Path, library: Path, extra: list[str]) -> None:
    command = [
        os.environ.get("CC", "cc"), *FLAGS, *extra, f"-I{ROOT / 'include'}", f"-I{TESTS}",
        str(source), "-o", str(output), f"-L{library}", "-ltextforge_c", f"-Wl,-rpath,{library}",
    ]
    subprocess.run(command, check=True)


def run(label: str, command: list[str], cwd: Path, env: dict[str, str]) -> bool:
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"FAIL {label}\n{result.stdout}{result.stderr}", file=sys.stderr)
        return False
    return True


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--no-valgrind", action="store_true")
    parser.add_argument("--no-sanitizers", action="store_true")
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    profile = ["--release"] if args.release else []
    subprocess.run(["cargo", "build", "--locked", "--lib", *profile], cwd=ROOT, check=True)
    library = ROOT / "target" / ("release" if args.release else "debug")
    link_soname(library)
    use_valgrind = not args.no_valgrind and shutil.which("valgrind") is not None
    if not args.no_valgrind and not use_valgrind:
        print("valgrind not found; skipping memcheck runs", file=sys.stderr)

    with tempfile.TemporaryDirectory(prefix="textforge-c-tests-") as scratch:
        work = Path(scratch)
        programs = [(path.stem, path) for path in sorted(TESTS.glob("test_*.c"))]
        for label, source in doc_examples():
            path = work / f"{label}.c"
            path.write_text(source, encoding="utf-8")
            programs.append((label, path))
        env = {**os.environ, "ASAN_OPTIONS": "detect_leaks=1:abort_on_error=0",
               "UBSAN_OPTIONS": "halt_on_error=1:print_stacktrace=1"}
        failures = 0
        for label, source in programs:
            plain = work / label
            compile_program(source, plain, library, [])
            failures += not run(label, [str(plain)], work, env)
            if not args.no_sanitizers:
                sanitized = work / f"{label}.asan"
                compile_program(source, sanitized, library, ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"])
                failures += not run(f"{label} (sanitizers)", [str(sanitized)], work, env)
            if use_valgrind:
                memcheck = ["valgrind", "--quiet", "--error-exitcode=99", "--leak-check=full",
                            "--errors-for-leak-kinds=definite,indirect", "--show-leak-kinds=definite,indirect",
                            str(plain)]
                failures += not run(f"{label} (valgrind)", memcheck, work, env)
        print(f"C programs: {len(programs)} ({len(programs) - len(list(TESTS.glob('test_*.c')))} header examples), "
              f"failures: {failures}")
        if failures:
            raise SystemExit(1)


if __name__ == "__main__":
    main()

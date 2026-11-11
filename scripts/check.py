"""Run the complete pre-commit validation for the C binding. Never commits or publishes.

Mirrors AGENTS.md section 14 for this crate, plus the binding checks required by
section 17 (core provenance, generated-code freshness, parity, C tests under
sanitizers/valgrind and the installed-package check).

Usage: python3 scripts/check.py [--quick]
--quick skips valgrind and the distribution build (for fast local iteration).
"""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PY = sys.executable
quick = "--quick" in sys.argv

checks = [
    ["cargo", "fmt", "--check"],
    ["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"],
    ["cargo", "clippy", "--locked", "--all-targets", "--features", "test-reference", "--", "-D", "warnings"],
    ["cargo", "test", "--locked"],
    ["cargo", "test", "--locked", "--features", "test-reference"],
    [PY, "scripts/check_core.py"],
    [PY, "scripts/generate_builder.py", "--check"],
    [PY, "scripts/generate_reference.py", "--check"],
    [PY, "scripts/generate_processor_reference.py", "--check"],
    [PY, "scripts/check_parity.py"],
    [PY, "scripts/run_c_tests.py", *(["--no-valgrind"] if quick else [])],
]
if not quick:
    checks.append([PY, "scripts/check_distribution.py"])

for command in checks:
    print("Running " + " ".join(command), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)
print("All checks passed")

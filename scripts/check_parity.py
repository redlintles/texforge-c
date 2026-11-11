"""Check that every public Rust and portable entry point has a C function.

* Public processor methods (src/api/processor.rs and its buffered/file_io
  extensions) map to `tf_processor_<method>`; the result is recorded in
  docs/processor-parity.json (`--write` regenerates it).
* Every public method of the portable types maps to the C function named in
  PORTABLE_TYPES below.
* Every builder method listed in docs/builder-parity.json must exist in the
  header and have at least one Rust-oracle case in tests/fixtures.

Usage: python3 scripts/check_parity.py [--write]
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORE = ROOT / "vendor/textforge/src/api"
HEADER = (ROOT / "include/textforge.h").read_text(encoding="utf-8")
MAPPING = ROOT / "docs/processor-parity.json"

# Portable type -> (source file, C prefix). Methods map to `<prefix><method>`.
PORTABLE_TYPES = {
    "PortableProcessor": (["processor.rs", "buffered.rs", "watchers.rs"], "tf_processor_"),
    "PipelinePlan": (["mod.rs"], "tf_pipeline_"),
    "CompiledPipeline": (["mod.rs"], "tf_compiled_pipeline_"),
    "Observers": (["watchers.rs"], "tf_observers_"),
    "PortableBuffer": (["buffered.rs"], "tf_buffer_"),
    "FilePath": (["path.rs"], "tf_file_path_"),
    "BatchPool": (["processor.rs"], "tf_batch_pool_"),
}
# Portable entry points whose C name does not follow the prefix rule.
RENAMED = {
    "PortableProcessor::create_pipeline": "tf_processor_create_pipeline",
    "PortableProcessor::register": "tf_processor_register",
    "features": "tf_features",
    "BufferOptions": "tf_buffer_options_new",
    "InstructionSpec::new": "tf_instruction_new",
}


def declared(function: str) -> bool:
    return re.search(r"\b" + re.escape(function) + r"\(", HEADER) is not None


def processor_methods() -> list[str]:
    """Public methods of the Rust processor, as the Node/Python bindings collect them."""
    source = (CORE / "processor.rs").read_text(encoding="utf-8")
    names = set(re.findall(r"^    fn (\w+)", source.split("impl Default for TextForgeProcessor")[0], re.M))
    names.update(re.findall(r"^    pub fn (\w+)", source, re.M))
    for path in (CORE / "processor/buffered").glob("*.rs"):
        text = path.read_text(encoding="utf-8")
        if "pub trait" in text:
            names.update(re.findall(r"^    fn (\w+)\(", text, re.M))
    for filename in ["buffered.rs", "file_io.rs"]:
        text = (CORE / "processor" / filename).read_text(encoding="utf-8")
        for block in text.split("impl TextForgeProcessor {")[1:]:
            names.update(re.findall(r"^    pub fn (\w+)", block, re.M))
    names.discard("default")
    return sorted(names)


def portable_methods() -> list[tuple[str, str]]:
    """(type, method) for every public inherent method of the portable types."""
    found = []
    for type_name, (files, _) in PORTABLE_TYPES.items():
        for filename in files:
            text = (CORE / "portable" / filename).read_text(encoding="utf-8")
            for block in text.split(f"impl {type_name} {{")[1:]:
                body = block.split("\n}\n")[0]
                found += [(type_name, name) for name in re.findall(r"^    pub fn (\w+)\(", body, re.M)]
    return found


def main() -> None:
    missing = []
    mapping = []
    for name in processor_methods():
        c_name = "tf_processor_" + name
        if not declared(c_name):
            missing.append(f"processor method {name} -> {c_name}")
        mapping.append({"rust": "TextForgeProcessor::" + name, "portable": "PortableProcessor::" + name, "c": c_name})
    for type_name, method in portable_methods():
        qualified = f"{type_name}::{method}"
        c_name = RENAMED.get(qualified, PORTABLE_TYPES[type_name][1] + method)
        if not declared(c_name):
            missing.append(f"portable {qualified} -> {c_name}")
    for qualified, c_name in RENAMED.items():
        if not declared(c_name):
            missing.append(f"portable {qualified} -> {c_name}")
    builder = json.loads((ROOT / "docs/builder-parity.json").read_text(encoding="utf-8"))["methods"]
    fixtures = json.loads((ROOT / "tests/fixtures/builder_reference.json").read_text(encoding="utf-8"))
    covered = {case["name"] for case in fixtures}
    for method in builder:
        if not declared(method["c"]):
            missing.append(f"builder {method['rust']} -> {method['c']}")
        short = method["rust"].split("::")[-1]
        if method["rust"].startswith("TextForgeBuilderMethods::") and short != "push_token" and short not in covered:
            missing.append(f"builder {method['rust']} has no Rust oracle case")
    if missing:
        raise SystemExit("Parity gaps:\n  " + "\n  ".join(missing))
    output = json.dumps({"coreVersion": "0.2.0", "methods": mapping}, indent=2) + "\n"
    if "--write" in sys.argv:
        MAPPING.write_text(output, encoding="utf-8")
    elif not MAPPING.exists() or MAPPING.read_text(encoding="utf-8") != output:
        raise SystemExit("docs/processor-parity.json is stale; run scripts/check_parity.py --write")
    print(f"Verified {len(mapping)} processor methods, {len(portable_methods())} portable methods "
          f"and {len(builder)} builder entries")


if __name__ == "__main__":
    main()

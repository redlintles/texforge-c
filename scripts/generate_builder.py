"""Generate the typed C builder functions and their Rust parity mapping.

Reads the pinned core's public builder trait (vendor/textforge/src/api/mod.rs)
and instruction registry (vendor/textforge/src/globals/table.rs). Each Rust
`TextForgeBuilderMethods` method becomes one exported function
`tf_pipeline_<method>` that appends the corresponding registry mnemonic through
the portable API. Hand-written group/map functions live in src/pipeline.rs and
are listed in the mapping too.

Outputs (never edit by hand):
  src/builder.rs              exported functions
  docs/builder-parity.json    Rust method -> C function -> mnemonic mapping

Usage: python3 scripts/generate_builder.py [--check]
--check fails when an output is stale (used by scripts/check.py and CI).
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORE = ROOT / "vendor/textforge"
BUILDER = ROOT / "src/builder.rs"
MAPPING = ROOT / "docs/builder-parity.json"

KIND_TEXT = {
    "String": "text or variable",
    "Usize": "number or variable",
    "Token": "nested instruction or variable",
}
# The public Rust builder accepts numbers for assignment values and stores
# them as decimal text; the portable layer applies the same normalization.
VALUE_PARAMETERS = {("val", "var_value"), ("var", "var_value"), ("mut_var", "var_value")}

# Builder surface implemented by hand in src/pipeline.rs.
MANUAL = [
    {"rust": "TextForgeBuilderMethods::define_replace_map", "c": "tf_pipeline_define_replace_map",
     "portable": "PipelinePlan::define_replace_map"},
    {"rust": "ReplaceMapBuilder::add_replace", "c": "tf_replace_map_builder_add_replace",
     "portable": "PipelinePlan::define_replace_map rules"},
    {"rust": "ReplaceMapBuilder::end", "c": "tf_pipeline_define_replace_map",
     "portable": "implicit: passing the builder ends configuration"},
    {"rust": "TextForgeBuilderMethods::push_token", "c": "tf_pipeline_push_token",
     "portable": "PipelinePlan::add"},
    {"rust": "TextForgeBlockMethods::block_assoc", "c": "tf_pipeline_block_assoc",
     "portable": "PipelinePlan::block"},
    {"rust": "TextForgeBlockMethods::call_block", "c": "tf_pipeline_call_block",
     "portable": "PipelinePlan::call_block"},
    {"rust": "TextForgeConditionalMethods::if_do_contains_each", "c": "tf_pipeline_if_do_contains_each",
     "portable": "PipelinePlan::contains_each"},
    {"rust": "TextForgeBuilder::build", "c": "tf_pipeline_build",
     "portable": "PortableProcessor::register"},
]


def registry_syntax(registry: str) -> dict[str, list[str]]:
    """Map each mnemonic to its declared parameter kinds."""
    pattern = r'(?s)\(\s*"(\w+)",\s*0x[0-9a-fA-F]+,.*?\[([^\]]*)\]\s*,?\s*\)'
    return {
        match[1]: re.findall(r"SyntaxToken::(String|Usize|Token)\b", match[2])
        for match in re.finditer(pattern, registry)
    }


def summary(docs: str) -> list[str]:
    """First descriptive paragraphs of a Rust doc comment, without links/examples."""
    lines: list[str] = []
    for line in docs.splitlines():
        text = line.strip()
        if text.startswith(("See Also", "#", "```")):
            break
        lines.append(re.sub(r"\[`?([^`\]]+)`?\]\([^)]*\)", r"`\1`", text))
    while lines and not lines[-1]:
        lines.pop()
    while lines and not lines[0]:
        lines.pop(0)
    return lines


def methods(api: str, syntax: dict[str, list[str]]) -> list[dict]:
    """Extract every simple fluent method of TextForgeBuilderMethods."""
    trait = api.split("pub trait TextForgeBuilderMethods")[1].split("pub trait TextForgeConditionalMethods")[0]
    pattern = r"(?ms)((?:^    ///[^\n]*\n|^[ \t]*\n)*)^    fn (\w+)\(([^;]*?)\) -> Result<&mut Self, TextForgeError> \{\n(.*?)^    }"
    result = []
    for match in re.finditer(pattern, trait):
        raw_docs, name, signature, body = match.groups()
        token = re.search(r"Box::new\((\w+)::", body)
        if token is None:
            raise ValueError(f"cannot find the instruction built by {name}")
        mnemonic = token[1]
        parameters = re.findall(r"(\w+): impl Into<ValType>", signature)
        kinds = syntax[mnemonic]
        if len(parameters) != len(kinds):
            raise ValueError(f"{name}: {len(parameters)} parameters but {mnemonic} declares {len(kinds)}")
        docs = re.sub(r"(?m)^\s*/// ?", "", raw_docs)
        result.append({
            "rust": name,
            "mnemonic": mnemonic,
            "parameters": list(zip(parameters, kinds, strict=True)),
            "summary": summary(docs),
        })
    return result


def describe(method: dict, name: str, kind: str) -> str:
    if (method["rust"], name) in VALUE_PARAMETERS:
        return "text, number or variable"
    return KIND_TEXT[kind]


def function(method: dict) -> str:
    """Render one exported builder function."""
    c_name = "tf_pipeline_" + method["rust"]
    doc = [*method["summary"]]
    if doc:
        doc.append("")
    doc.append(f"Mirrors Rust `TextForgeBuilderMethods::{method['rust']}` (mnemonic `{method['mnemonic']}`).")
    if method["parameters"]:
        listed = ", ".join(f"`{name}` ({describe(method, name, kind)})" for name, kind in method["parameters"])
        doc += ["", f"Arguments: {listed}."]
    doc += [
        "",
        "Returns `pipeline` for chaining (NULL only when `pipeline` is NULL);",
        "failures are kept as the pipeline's sticky error.",
        "",
        "# Safety",
        "",
        "`pipeline` is NULL or a live pipeline; arguments obey the `TfArg` contract.",
    ]
    parameters = "".join(f", {name}: TfArg" for name, _ in method["parameters"])
    names = ", ".join(name for name, _ in method["parameters"])
    lines = [("/// " + line).rstrip() for line in doc]
    return (
        "\n".join(lines)
        + "\n#[unsafe(no_mangle)]\n"
        + f"pub unsafe extern \"C\" fn {c_name}(pipeline: *mut TfPipeline{parameters}) -> *mut TfPipeline {{\n"
        + "    // SAFETY: forwarded caller contract.\n"
        + f"    unsafe {{ append(pipeline, \"{method['mnemonic']}\", &[{names}]) }}\n"
        + "}\n"
    )


def main() -> None:
    api = (CORE / "src/api/mod.rs").read_text(encoding="utf-8")
    registry = (CORE / "src/globals/table.rs").read_text(encoding="utf-8")
    found = methods(api, registry_syntax(registry))
    if len(found) < 70:
        raise ValueError("unexpected builder shape; review the generator")
    source = (
        "//! Typed builder functions, one per public Rust builder method.\n"
        "//!\n"
        "//! Generated by `python3 scripts/generate_builder.py` from the bundled core;\n"
        "//! do not edit by hand. CI runs the generator with `--check`.\n\n"
        "use crate::{instruction::TfArg, pipeline::{TfPipeline, append}};\n\n"
        + "\n".join(function(method) for method in found)
    )
    formatted = subprocess.run(
        ["rustfmt", "--edition", "2024", "--emit", "stdout"],
        input=source, text=True, capture_output=True, check=True, cwd=ROOT,
    ).stdout
    mapping = {
        "coreVersion": "0.2.0",
        "apiSha256": hashlib.sha256(api.encode()).hexdigest(),
        "registrySha256": hashlib.sha256(registry.encode()).hexdigest(),
        "methods": [
            {"rust": "TextForgeBuilderMethods::" + m["rust"], "c": "tf_pipeline_" + m["rust"],
             "mnemonic": m["mnemonic"], "parameters": [list(p) for p in m["parameters"]]}
            for m in found
        ] + MANUAL,
    }
    outputs = {BUILDER: formatted, MAPPING: json.dumps(mapping, indent=2) + "\n"}
    for path, content in outputs.items():
        if "--check" in sys.argv:
            if not path.exists() or path.read_text(encoding="utf-8") != content:
                raise SystemExit(f"Stale generated file: {path.relative_to(ROOT)}")
        else:
            path.write_text(content, encoding="utf-8")
    print(f"{'Verified' if '--check' in sys.argv else 'Generated'} {len(found)} builder functions")


if __name__ == "__main__":
    main()

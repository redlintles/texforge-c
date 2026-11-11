"""Capture outputs of the real public Rust builder and generate the C parity test.

1. Sample calls come from the pinned Rust builder's own doctests, plus explicit
   logical cases (variables, iteration, blocks, conditionals, replace maps).
2. examples/builder_reference.rs runs those calls with the public Rust builder
   (feature `test-reference`) and prints each result; no portable mapping is
   used to compute expected outputs.
3. tests/fixtures/builder_reference.json stores the cases and Rust results.
4. tests/c/test_builder_parity.c replays every case through the C functions
   listed in docs/builder-parity.json and compares results and error codes.

Usage: python3 scripts/generate_reference.py [--check]
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/fixtures/builder_reference.json"
EXAMPLE = ROOT / "examples/builder_reference.rs"
C_TEST = ROOT / "tests/c/test_builder_parity.c"
MAPPING = ROOT / "docs/builder-parity.json"


def cases() -> list[dict[str, Any]]:
    """Read simple public-builder doctests and supply explicit logical cases."""
    api = (ROOT / "vendor/textforge/src/api/mod.rs").read_text(encoding="utf-8")
    trait = api.split("pub trait TextForgeConditionalMethods")[0]
    pattern = r"(?ms)^    fn (\w+)\(([^;]*?)\) -> Result<&mut Self, TextForgeError> \{\n(.*?)^    }"
    explicit = {
        "normalize_unicode": ("Ａﬁ²", [["normalize_unicode", ["nfkc"]]]),
        "add_replace_rule": ("cat", [["add_replace_rule", ["animals", "cat", "dog"]]]),
        "execute_replace_map": (
            "cat",
            [["add_replace_rule", ["animals", "cat", "dog"]], ["execute_replace_map", ["animals"]]],
        ),
    }
    skipped = {"val", "var", "mut_var", "eval", "iter", "end", "remove_pattern"}
    result = []
    for match in re.finditer(pattern, trait):
        name = match[1]
        if name in explicit:
            input_text, steps = explicit[name]
            result.append({"name": name, "input": input_text, "steps": steps})
            continue
        if name in skipped:
            continue
        docs_match = re.search(r"((?:    ///[^\n]*\n|[ \t]*\n)*)$", trait[: match.start()])
        docs = re.sub(r"(?m)^    /// ?", "", docs_match[0]) if docs_match else ""
        call = re.search(r"\." + name + r"\(([^)]*)\)\s*\.unwrap", docs)
        input_match = re.search(r'let input = ("(?:[^"\\]|\\.)*");', docs)
        if call is None or input_match is None:
            raise ValueError(f"No supported Rust example for {name}")
        result.append({
            "name": name,
            "input": json.loads(input_match[1]),
            "steps": [[name, json.loads("[" + call[1] + "]")]],
        })
    suffix_block = ["block_assoc", ["suffix", {"body": [["add_to_end", ["!"]]]}]]
    result += [
        {"name": "val", "input": "é", "steps": [["val", ["n", 2]], ["repeat", [{"variable": "n"}]]]},
        {"name": "var", "input": "é", "steps": [["var", ["n", 2]], ["repeat", [{"variable": "n"}]]]},
        {"name": "mut_var", "input": "é", "steps": [
            ["var", ["n", 1]], ["mut_var", ["n", 2]], ["repeat", [{"variable": "n"}]]]},
        {"name": "eval", "input": "é", "steps": [["eval", ["2 + 3", "n"]], ["repeat", [{"variable": "n"}]]]},
        {"name": "iter", "input": "é", "steps": [
            ["iter", [2, {"instruction": ["add_to_beginning", ["🍌"]]}]]]},
        {"name": "remove_pattern", "input": "bánáná🍌", "steps": [["remove_pattern", ["á"]]]},
        {"name": "end", "input": "é", "steps": [["end", []]]},
        {"name": "block_assoc", "input": "é", "steps": [suffix_block, ["call_block", ["suffix"]]]},
        {"name": "call_block", "input": "é", "steps": [
            suffix_block, ["call_block", ["suffix"]], ["call_block", ["suffix"]]]},
        {"name": "if_do_contains_each", "input": "x", "steps": [[
            "if_do_contains_each", ["x", {"body": [["remove_pattern", ["x"]], ["add_to_end", ["skipped"]]]}]]]},
        {"name": "define_replace_map", "input": "cat wolf", "steps": [
            ["define_replace_map", ["animals", {"map": [["cat", "wolf"], ["wolf", "dog"]]}]],
            ["execute_replace_map", ["animals"]]]},
        # Error parity: the same core error code must surface through C.
        {"name": "undefined_variable", "input": "é", "steps": [["repeat", [{"variable": "missing"}]]]},
        {"name": "invalid_regex", "input": "abc", "steps": [["replace_all_with", ["(", "x"]]]},
        {"name": "missing_block", "input": "é", "steps": [["call_block", ["nowhere"]]]},
    ]
    return result


# --------------------------------------------------------------------- Rust oracle

def rust_argument(value: object) -> str:
    """Render a literal or an explicit public Rust reference/group argument."""
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, int):
        return f"{value}_usize"
    if not isinstance(value, dict):
        raise TypeError(f"Unsupported fixture argument: {value!r}")
    if "variable" in value:
        return f"get_var({rust_argument(value['variable'])})"
    if "map" in value:
        rules = "".join(f"map.add_replace({rust_argument(a)}, {rust_argument(b)})?; " for a, b in value["map"])
        return "|map| { " + rules + "map.end() }"
    if "body" in value:
        return "|body| { " + rust_chain(value["body"], "body") + "; Ok(()) }"
    if "instruction" in value:
        if value["instruction"] != ["add_to_beginning", ["🍌"]]:
            raise ValueError("Unsupported nested Rust fixture; extend its explicit template")
        return """ValType::Literal(TextForgeParamTypes::Token(TokenWrapper::new(
            Box::new(textforge::tokens::transforms::atb::Atb::default()),
            Some(vec!["🍌".into()]),
        )))"""
    raise ValueError(f"Unsupported fixture value: {value!r}")


def rust_chain(steps: list[list[Any]], root: str) -> str:
    """Render calls to named Rust builder methods, in their declared argument order."""
    return root + "".join(f".{name}({', '.join(rust_argument(arg) for arg in args)})?" for name, args in steps)


def program(data: list[dict[str, Any]]) -> str:
    """Produce a standalone executable oracle using only existing Rust dependencies."""
    result = """//! Reference outputs from the public Rust builder for the C parity test.
//! Generated by scripts/generate_reference.py; not a second engine.
use textforge::{
    api::{get_var, processor::{TextForgeProcessor, TextForgeProcessorMethods},
        TextForgeBuilderMethods, TextForgeBlockMethods, TextForgeConditionalMethods},
    parser::{params::TextForgeParamTypes, resolve_var::{TokenWrapper, ValType}},
    utils::errors::TextForgeError,
};

fn record(name: &str, result: Result<String, TextForgeError>) {
    match result {
        Ok(text) => {
            let encoded: String = text.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect();
            println!("{name}\\tok\\t{encoded}");
        }
        Err(error) => println!("{name}\\terror\\t{}", error.error_code.get_error_code()),
    }
}

fn main() {
"""
    for case in data:
        expression = rust_chain(case["steps"], "processor.create_pipeline()")
        result += f'''    record("{case["name"]}", (|| {{
        let mut processor = TextForgeProcessor::new();
        let id = {expression}.build()?;
        processor.process_all(&id, {rust_argument(case["input"])})
    }})());
'''
    return result + "}\n"


# ------------------------------------------------------------------------- C test

def c_string(text: str) -> str:
    """C string literal; non-ASCII bytes use octal escapes (no hex run-on)."""
    out = []
    for byte in text.encode("utf-8"):
        char = chr(byte)
        if char in '"\\':
            out.append("\\" + char)
        elif 0x20 <= byte < 0x7F:
            out.append(char)
        else:
            out.append(f"\\{byte:03o}")
    return '"' + "".join(out) + '"'


class CEmitter:
    """Translate fixture steps into calls to the generated C functions."""

    def __init__(self, functions: dict[str, str], mnemonics: dict[str, str]) -> None:
        self.functions = functions
        self.mnemonics = mnemonics
        self.counter = 0
        self.lines: list[str] = []
        self.cleanup: list[str] = []

    def fresh(self, prefix: str) -> str:
        self.counter += 1
        return f"{prefix}{self.counter}"

    def argument(self, value: object) -> str:
        if isinstance(value, str):
            return f"tf_text_n({c_string(value)}, {len(value.encode())})"
        if isinstance(value, int):
            return f"tf_number({value})"
        assert isinstance(value, dict)
        if "variable" in value:
            return f"tf_var({c_string(value['variable'])})"
        if "instruction" in value:
            method, arguments = value["instruction"]
            name = self.fresh("nested")
            args = self.fresh("nested_args")
            self.lines.append(f"TfArg {args}[] = {{{', '.join(self.argument(a) for a in arguments)}}};")
            self.lines.append(f"TfInstruction *{name} = NULL;")
            self.lines.append(
                f"CHECK_OK(tf_instruction_new({c_string(self.mnemonics[method])}, TF_NTS, {args}, "
                f"{len(arguments)}, &{name}, NULL));"
            )
            self.cleanup.append(f"tf_instruction_free({name});")
            return f"tf_nested({name})"
        raise ValueError(f"Unsupported argument {value!r}")

    def steps(self, plan: str, steps: list[list[Any]]) -> None:
        for method, arguments in steps:
            if method in {"block_assoc", "if_do_contains_each"}:
                label, body = arguments
                body_plan = self.fresh("body")
                self.lines.append(f"TfPipeline *{body_plan} = tf_pipeline_new();")
                self.steps(body_plan, body["body"])
                self.lines.append(
                    f"{self.functions[method]}({plan}, {c_string(label)}, TF_NTS, {body_plan});  /* consumes body */"
                )
            elif method == "define_replace_map":
                label, rules = arguments
                map_name = self.fresh("map")
                self.lines.append(f"TfReplaceMapBuilder *{map_name} = tf_replace_map_builder_new();")
                for pattern, replacement in rules["map"]:
                    self.lines.append(
                        f"tf_replace_map_builder_add_replace({map_name}, {self.argument(pattern)}, "
                        f"{self.argument(replacement)});"
                    )
                self.lines.append(f"tf_pipeline_define_replace_map({plan}, {self.argument(label)}, {map_name});")
            elif method == "call_block":
                self.lines.append(f"tf_pipeline_call_block({plan}, {c_string(arguments[0])}, TF_NTS);")
            else:
                rendered = "".join(", " + self.argument(argument) for argument in arguments)
                self.lines.append(f"{self.functions[method]}({plan}{rendered});")


def c_test(data: list[dict[str, Any]]) -> str:
    mapping = json.loads(MAPPING.read_text(encoding="utf-8"))["methods"]
    functions: dict[str, str] = {}
    for entry in mapping:  # Builder-trait methods come first and win over ReplaceMapBuilder::end.
        functions.setdefault(entry["rust"].split("::")[-1], entry["c"])
    mnemonics = {entry["rust"].split("::")[-1]: entry["mnemonic"] for entry in mapping if "mnemonic" in entry}
    body = []
    for case in data:
        emitter = CEmitter(functions, mnemonics)
        emitter.lines.append("TfPipeline *plan = tf_pipeline_new();")
        emitter.steps("plan", case["steps"])
        expected = case["result"]
        if "ok" in expected:
            check = f"expect_text(plan, {c_string(case['input'])}, {c_string(expected['ok'])}, " \
                    f"{len(expected['ok'].encode())});"
        else:
            check = f"expect_error(plan, {c_string(case['input'])}, {expected['error']});"
        statements = emitter.lines + [check, "tf_pipeline_free(plan);"] + emitter.cleanup
        body.append(
            f"static void case_{case['name']}(void) {{\n"
            + "".join(f"    {line}\n" for line in statements)
            + "}\n"
        )
    calls = "".join(f"    RUN(case_{case['name']});\n" for case in data)
    return f"""/* Builder parity against the public Rust builder.
 *
 * Generated by scripts/generate_reference.py from tests/fixtures/builder_reference.json;
 * do not edit by hand. Each case is built with the C builder functions listed in
 * docs/builder-parity.json, registered with tf_pipeline_build and executed with
 * tf_processor_process_all, mirroring the Rust oracle.
 */
#include "support.h"

/* Register the plan, run it and compare the output bytes. */
static void expect_text(const TfPipeline *plan, const char *input, const char *expected, size_t length) {{
    TfProcessor *processor = tf_processor_new();
    TfString id = {{0}};
    CHECK_OK(tf_pipeline_build(plan, processor, &id, NULL));
    TfString out = {{0}};
    CHECK_OK(tf_processor_process_all(processor, id.data, id.length, input, TF_NTS, &out, NULL));
    CHECK(out.length == length && memcmp(out.data, expected, length) == 0);
    tf_string_free(&out);
    tf_string_free(&id);
    tf_processor_free(processor);
}}

/* The Rust builder failed with core code `code`; C must report the same code. */
static void expect_error(const TfPipeline *plan, const char *input, unsigned code) {{
    TfProcessor *processor = tf_processor_new();
    TfString id = {{0}};
    TfError *error = NULL;
    if (tf_pipeline_build(plan, processor, &id, &error) == TF_STATUS_OK) {{
        TfString out = {{0}};
        CHECK(tf_processor_process_all(processor, id.data, id.length, input, TF_NTS, &out, &error) != TF_STATUS_OK);
        tf_string_free(&id);
    }}
    CHECK(error != NULL && tf_error_code(error) == code);
    tf_error_free(error);
    tf_processor_free(processor);
}}

{chr(10).join(body)}
int main(void) {{
{calls}    return finish("builder parity");
}}
"""


def main() -> None:
    """Regenerate fixtures, or verify the saved outputs against real Rust execution."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    data = cases()
    formatted = subprocess.check_output(
        ["rustfmt", "--edition", "2024", "--emit", "stdout"], input=program(data), text=True
    )
    if args.check:
        if EXAMPLE.read_text(encoding="utf-8") != formatted:
            raise SystemExit("Rust reference is stale: run scripts/generate_reference.py")
    else:
        EXAMPLE.write_text(formatted, encoding="utf-8")
    output = subprocess.check_output(
        ["cargo", "run", "--quiet", "--locked", "--features", "test-reference", "--example", "builder_reference"],
        cwd=ROOT, text=True,
    )
    rows = {parts[0]: parts[1:] for line in output.splitlines() if (parts := line.split("\t"))}
    for case in data:
        status, payload = rows[case["name"]]
        case["result"] = (
            {"ok": bytes.fromhex(payload).decode("utf-8")} if status == "ok" else {"error": int(payload)}
        )
    fixture = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
    test = c_test(data)
    for path, content in ((FIXTURE, fixture), (C_TEST, test)):
        if args.check:
            if not path.exists() or path.read_text(encoding="utf-8") != content:
                raise SystemExit(f"Stale generated file: {path.relative_to(ROOT)}; review and regenerate")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
    print(f"{'Verified' if args.check else 'Generated'} {len(data)} public Rust builder reference cases")


if __name__ == "__main__":
    main()

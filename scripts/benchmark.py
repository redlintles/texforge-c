"""Benchmark one complex pipeline through the C binding and through the Rust crate.

The pipeline uses every public builder method (77 TextForgeBuilderMethods plus
blocks, a conditional, a scoped replacement map and nested iteration). A single
step list is rendered twice:

* Rust: a temporary cargo project depending on vendor/textforge (same core,
  same release profile), built with the public TextForgeBuilder and run with
  TextForgeProcessor::process_all. It also enables `test_access`, which only
  makes internal modules public (needed to build the nested `iter` argument, as
  in the parity oracle) and adds test modules; it changes no execution code.
* C: a program linked against the release static library, built with the
  generated tf_pipeline_* functions and run through tf_compiled_pipeline_process
  and tf_processor_process_all.

Both process the same deterministic multi-line UTF-8 corpus (accents, emoji,
HTML/JSON-sensitive characters) at several sizes. Outputs must be byte-identical
across all three paths. Each measurement runs warm-up iterations, then records
every iteration; the report gives median, mean, standard deviation and min, plus
the build/compile time and the peak RSS of each process.

Usage: python3 scripts/benchmark.py [--sizes 100K,1M,8M] [--iterations 15] [--markdown FILE]
"""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from generate_reference import CEmitter, rust_chain  # noqa: E402  (shared renderers)

# Every step: [rust_method, [arguments]]; see scripts/generate_reference.py for the
# argument encoding ({"variable": ...}, {"body": [...]}, {"map": [...]}, {"instruction": ...}).
# Size-preserving steps first, line operations next, shrinking steps last, so most
# steps process the full input (word operations collapse line breaks; see below).
STEPS = [
    ["val", ["n", 2]],
    ["var", ["limit", 3]],
    ["mut_var", ["limit", 4]],
    ["eval", ["2 + 3", "k"]],
    ["normalize_unicode", ["nfc"]],
    ["add_replace_rule", ["animals", "gato", "cão"]],
    ["define_replace_map", ["words", {"map": [["casa", "lar"], ["rio", "mar"]]}]],
    ["execute_replace_map", ["animals"]],
    ["execute_replace_map", ["words"]],
    ["trim_both_sides", []],
    ["trim_left_side", []],
    ["trim_right_side", []],
    ["add_to_beginning", ["<<início>>\n"]],
    ["add_to_end", ["\n<<fim>>"]],
    ["delete_first", []],
    ["delete_last", []],
    ["replace_all_with", ["foo", "bar"]],
    ["replace_first_with", ["bar", "baz"]],
    ["replace_last_with", ["bar", "qux"]],
    ["replace_nth_with", ["bar", "nth", 2]],
    ["replace_count_with", ["bar", "cnt", 3]],
    ["rotate_left", [5]],
    ["rotate_right", [5]],
    ["to_uppercase_all", []],
    ["to_lowercase_all", []],
    ["to_uppercase_single", [0]],
    ["to_lowercase_single", [0]],
    ["to_uppercase_chunk", [0, 100]],
    ["to_lowercase_chunk", [0, 50]],
    ["capitalize_first_word", []],
    ["capitalize_last_word", []],
    ["capitalize_chunk", [0, 200]],
    ["insert", [10, "[ins]"]],
    ["delete_single", [3]],
    ["delete_chunk", [1, 3]],
    ["pad_left", ["*", 10]],
    ["pad_right", ["*", 10]],
    ["to_html_escaped", []],
    ["to_html_unescaped", []],
    ["to_url_encoded", []],
    ["to_url_decoded", []],
    ["to_json_escaped", []],
    ["to_json_unescaped", []],
    ["to_reverse", []],
    ["to_reverse", []],
    ["remove_pattern", ["XYZ"]],
    ["repeat", [{"variable": "n"}]],
    ["iter", [2, {"instruction": ["add_to_beginning", ["🍌"]]}]],
    ["block_assoc", ["mark", {"body": [["add_to_end", ["!"]], ["add_to_beginning", ["#"]]]}]],
    ["call_block", ["mark"]],
    ["if_do_contains_each", ["linha", {"body": [["add_to_end", ["?"]], ["remove_pattern", ["\\?$"]]]}]],
    ["append_line", ["última"]],
    ["prepend_line", ["primeira"]],
    ["insert_line", [2, "inserida"]],
    ["replace_line", [3, "trocada"]],
    ["delete_line", [4]],
    ["delete_empty_lines", []],
    ["unique_lines", []],
    ["reverse_lines", []],
    ["reverse_lines", []],
    ["keep_lines_matching", ["."]],
    ["delete_lines_matching", ["^zzz$"]],
    ["delete_lines_interval", [2, 3]],
    ["select_lines", [1, 1000]],
    ["delete_lines_before", [2]],
    ["delete_lines_after", [500]],
    ["join_lines", [" | "]],
    ["remove_line_breaks", []],
    # Word operations rebuild the text with single spaces, so they run after the line phase.
    ["capitalize_range", [0, 50]],
    ["capitalize_single_word", [1]],
    ["to_uppercase_word", [2]],
    ["to_lowercase_word", [2]],
    ["split_characters", []],
    ["join_to_kebab_case", []],
    ["join_to_snake_case", []],
    ["join_to_camel_case", []],
    ["join_to_pascal_case", []],
    ["remove_whitespace", []],
    ["extract_matches_joined", ["[a-z]+", "-"]],
    ["select", [0, 40]],
    ["delete_after", [30]],
    ["delete_before", [2]],
    ["split_select", ["-", 1]],
    ["end", []],
]
WORDS = ("gato casa rio foo linha texto pipeline ação coração João 🍌 <b>&amp;</b> \"aspas\" "
         "café über naïve résumé 東京 Ａﬁ² é tab\tseparado").split(" ")


def corpus(size: int) -> str:
    """Deterministic multi-line UTF-8 text of roughly `size` bytes; lines are unique."""
    rng = random.Random(20261111)
    lines, total, number = [], 0, 0
    while total < size:
        number += 1
        line = f"linha {number}: " + " ".join(rng.choice(WORDS) for _ in range(rng.randint(6, 18)))
        if number % 50 == 0:
            line = ""  # some empty lines for delete_empty_lines
        lines.append(line)
        total += len(line.encode()) + 1
    return "\n".join(lines)


def parse_size(text: str) -> int:
    match = re.fullmatch(r"(\d+)([KM]?)", text.upper())
    if match is None:
        raise SystemExit(f"invalid size {text}")
    return int(match[1]) * {"": 1, "K": 1024, "M": 1024 * 1024}[match[2]]


def rust_program(iterations: int, warmup: int) -> str:
    chain = rust_chain(STEPS, "processor.create_pipeline()")
    return f"""//! Generated by scripts/benchmark.py.
use std::{{borrow::Cow, time::Instant}};
use textforge::api::{{get_var, processor::{{TextForgeProcessor, TextForgeProcessorMethods}},
    TextForgeBuilderMethods, TextForgeBlockMethods, TextForgeConditionalMethods}};
use textforge::parser::{{params::TextForgeParamTypes, resolve_var::{{TokenWrapper, ValType}}}};

fn main() -> Result<(), Box<dyn std::error::Error>> {{
    let args: Vec<String> = std::env::args().collect();
    let input = std::fs::read_to_string(&args[1])?;
    let started = Instant::now();
    let mut processor = TextForgeProcessor::new();
    let id = (|| -> Result<String, textforge::utils::errors::TextForgeError> {{
        Ok({chain}.build()?)
    }})()?;
    println!("build_ns {{}}", started.elapsed().as_nanos());
    let mut output = String::new();
    for iteration in 0..{warmup + iterations} {{
        let started = Instant::now();
        output = processor.process_all(&id, &input)?;
        let elapsed = started.elapsed().as_nanos();
        if iteration >= {warmup} {{ println!("rust_process_all_ns {{elapsed}}"); }}
    }}
    // run_transform(&self): the path CompiledPipeline (tf_compiled_pipeline_process) uses.
    for iteration in 0..{warmup + iterations} {{
        let started = Instant::now();
        let result = processor.run_transform(&id, Cow::Borrowed(input.as_str()))?.into_owned();
        let elapsed = started.elapsed().as_nanos();
        assert!(result == output);
        if iteration >= {warmup} {{ println!("rust_run_transform_ns {{elapsed}}"); }}
    }}
    // Identity pipeline: isolates the per-call cost that remains without instructions.
    let empty = processor.create_pipeline().build()?;
    for iteration in 0..{warmup + iterations} {{
        let started = Instant::now();
        let result = processor.run_transform(&empty, Cow::Borrowed(input.as_str()))?.into_owned();
        let elapsed = started.elapsed().as_nanos();
        assert!(result.len() == input.len());
        if iteration >= {warmup} {{ println!("rust_identity_ns {{elapsed}}"); }}
    }}
    std::fs::write(&args[2], output.as_bytes())?;
    Ok(())
}}
"""


def c_program(iterations: int, warmup: int) -> str:
    mapping = json.loads((ROOT / "docs/builder-parity.json").read_text())["methods"]
    functions: dict[str, str] = {}
    for entry in mapping:
        functions.setdefault(entry["rust"].split("::")[-1], entry["c"])
    mnemonics = {e["rust"].split("::")[-1]: e["mnemonic"] for e in mapping if "mnemonic" in e}
    emitter = CEmitter(functions, mnemonics)
    emitter.lines.append("TfPipeline *plan = tf_pipeline_new();")
    emitter.steps("plan", STEPS)
    build = "".join(f"    {line}\n" for line in emitter.lines)
    cleanup = "".join(f"    {line}\n" for line in emitter.cleanup)
    return f"""/* Generated by scripts/benchmark.py. */
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "textforge.h"

#define CHECK_OK(call) do {{ TfError *e_ = NULL; (void)e_; if ((call) != TF_STATUS_OK) {{ \\
    fprintf(stderr, "failed: %s\\n", #call); exit(1); }} }} while (0)

static long long now_ns(void) {{
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (long long)t.tv_sec * 1000000000LL + t.tv_nsec;
}}

static char *read_all(const char *path, size_t *length) {{
    FILE *file = fopen(path, "rb");
    if (!file) exit(1);
    fseek(file, 0, SEEK_END);
    *length = (size_t)ftell(file);
    fseek(file, 0, SEEK_SET);
    char *data = malloc(*length + 1);
    if (!data || fread(data, 1, *length, file) != *length) exit(1);
    fclose(file);
    return data;
}}

static void report(const char *label, const TfError *error) {{
    fprintf(stderr, "%s: %s (%s / %s)\\n", label, tf_error_message(error, NULL),
            tf_error_instruction(error, NULL), tf_error_input(error, NULL));
}}

int main(int argc, char **argv) {{
    if (argc < 3) return 2;
    size_t length = 0;
    char *input = read_all(argv[1], &length);

    long long started = now_ns();
{build}    TfCompiledPipeline *pipeline = NULL;
    TfError *error = NULL;
    if (tf_pipeline_compile(plan, &pipeline, &error) != TF_STATUS_OK) {{ report("compile", error); return 1; }}
    printf("build_ns %lld\\n", now_ns() - started);

    TfProcessor *processor = tf_processor_new();
    TfString id = {{0}};
    CHECK_OK(tf_pipeline_build(plan, processor, &id, NULL));

    TfString out = {{0}};
    for (int i = 0; i < {warmup + iterations}; i++) {{
        tf_string_free(&out);
        long long t = now_ns();
        if (tf_compiled_pipeline_process(pipeline, input, length, &out, &error) != TF_STATUS_OK) {{
            report("process", error); return 1;
        }}
        long long elapsed = now_ns() - t;
        if (i >= {warmup}) printf("c_compiled_ns %lld\\n", elapsed);
    }}
    TfString via_processor = {{0}};
    for (int i = 0; i < {warmup + iterations}; i++) {{
        tf_string_free(&via_processor);
        long long t = now_ns();
        if (tf_processor_process_all(processor, id.data, id.length, input, length, &via_processor, &error)
                != TF_STATUS_OK) {{ report("process_all", error); return 1; }}
        long long elapsed = now_ns() - t;
        if (i >= {warmup}) printf("c_process_all_ns %lld\\n", elapsed);
    }}
    if (via_processor.length != out.length || memcmp(via_processor.data, out.data, out.length) != 0) {{
        fprintf(stderr, "compiled and processor outputs differ\\n");
        return 1;
    }}
    /* Identity pipeline: the per-call adapter cost without instructions. */
    TfPipeline *empty_plan = tf_pipeline_new();
    TfCompiledPipeline *empty = NULL;
    CHECK_OK(tf_pipeline_compile(empty_plan, &empty, NULL));
    TfString same = {{0}};
    for (int i = 0; i < {warmup + iterations}; i++) {{
        tf_string_free(&same);
        long long t = now_ns();
        CHECK_OK(tf_compiled_pipeline_process(empty, input, length, &same, NULL));
        long long elapsed = now_ns() - t;
        if (i >= {warmup}) printf("c_identity_ns %lld\\n", elapsed);
    }}
    if (same.length != length) return 1;
    tf_string_free(&same);
    tf_compiled_pipeline_free(empty);
    tf_pipeline_free(empty_plan);

    FILE *file = fopen(argv[2], "wb");
    fwrite(out.data, 1, out.length, file);
    fclose(file);

    tf_string_free(&via_processor);
    tf_string_free(&out);
    tf_string_free(&id);
    tf_processor_free(processor);
    tf_compiled_pipeline_free(pipeline);
    tf_pipeline_free(plan);
{cleanup}    free(input);
    return 0;
}}
"""


def run(command: list[str], **kwargs) -> subprocess.CompletedProcess:
    result = subprocess.run(command, capture_output=True, text=True, **kwargs)
    if result.returncode != 0:
        raise SystemExit(f"FAILED: {' '.join(map(str, command))}\n{result.stdout[-3000:]}\n{result.stderr[-3000:]}")
    return result


def measure(command: list[str]) -> tuple[dict[str, list[int]], int]:
    """Run a benchmark binary; return its samples and its peak RSS in KiB (from wait4)."""
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    stdout, stderr = process.stdout.read(), process.stderr.read()
    _, status, usage = os.wait4(process.pid, 0)
    process.returncode = os.waitstatus_to_exitcode(status)
    if process.returncode != 0:
        raise SystemExit(f"FAILED: {' '.join(command)}\n{stdout[-3000:]}\n{stderr[-3000:]}")
    samples: dict[str, list[int]] = {}
    for line in stdout.splitlines():
        name, _, value = line.partition(" ")
        samples.setdefault(name, []).append(int(value))
    return samples, usage.ru_maxrss


def summary(samples: list[int]) -> dict[str, float]:
    ms = [sample / 1e6 for sample in samples]
    return {"median": statistics.median(ms), "mean": statistics.fmean(ms),
            "stdev": statistics.stdev(ms) if len(ms) > 1 else 0.0, "min": min(ms)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--sizes", default="100K,1M,8M")
    parser.add_argument("--iterations", type=int, default=15)
    parser.add_argument("--warmup", type=int, default=3)
    parser.add_argument("--rounds", type=int, default=4, help="process runs per side, alternating order")
    parser.add_argument("--markdown", type=Path)
    args = parser.parse_args()
    sizes = [parse_size(size) for size in args.sizes.split(",")]
    methods = {step[0] for step in STEPS} | {"add_to_end", "add_to_beginning", "remove_pattern"}
    builder = {e["rust"].split("::")[-1] for e in json.loads((ROOT / "docs/builder-parity.json").read_text())["methods"]
               if e["rust"].startswith("TextForgeBuilderMethods::") and e["rust"] != "TextForgeBuilderMethods::push_token"}
    missing = sorted(builder - methods)
    if missing:
        raise SystemExit(f"pipeline does not cover: {missing}")

    with tempfile.TemporaryDirectory(prefix="textforge-c-bench-") as scratch:
        work = Path(scratch)
        # Rust reference: same vendored core, same features, release profile, locked versions.
        crate = work / "rust-bench"
        (crate / "src").mkdir(parents=True)
        (crate / "Cargo.toml").write_text(f"""[package]
name = "rust-bench"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
textforge = {{ path = "{ROOT / 'vendor/textforge'}", features = ["bytecode", "watchers", "buffered", "test_access"] }}

[workspace]
""")
        (crate / "Cargo.lock").write_bytes((ROOT / "Cargo.lock").read_bytes())
        (crate / "src/main.rs").write_text(rust_program(args.iterations, args.warmup))
        run(["cargo", "build", "--release", "--quiet"], cwd=crate)
        rust_binary = crate / "target/release/rust-bench"

        run(["cargo", "build", "--release", "--locked", "--lib", "--quiet"], cwd=ROOT)
        c_source = work / "bench.c"
        c_source.write_text(c_program(args.iterations, args.warmup))
        c_binary = work / "c-bench"
        run([os.environ.get("CC", "cc"), "-std=c11", "-O2", "-Wall", "-Werror", f"-I{ROOT / 'include'}", str(c_source),
             "-o", str(c_binary), str(ROOT / "target/release/libtextforge_c.a"),
             "-lm", "-lrt", "-lpthread", "-lgcc_s", "-lutil", "-ldl"])

        rows = []
        for size in sizes:
            text = corpus(size)
            source = work / f"input-{size}.txt"
            source.write_text(text, encoding="utf-8")
            rust_out, c_out = work / f"rust-{size}.out", work / f"c-{size}.out"
            rust_samples: dict[str, list[int]] = {}
            c_samples: dict[str, list[int]] = {}
            rust_rss = c_rss = 0
            for round_number in range(args.rounds):  # alternate the order to cancel drift
                order = ("rust", "c") if round_number % 2 == 0 else ("c", "rust")
                for which in order:
                    if which == "rust":
                        samples, rss = measure([str(rust_binary), str(source), str(rust_out)])
                        target, rust_rss = rust_samples, max(rust_rss, rss)
                    else:
                        samples, rss = measure([str(c_binary), str(source), str(c_out)])
                        target, c_rss = c_samples, max(c_rss, rss)
                    for key, values in samples.items():
                        target.setdefault(key, []).extend(values)
                assert rust_out.read_bytes() == c_out.read_bytes(), f"outputs differ for {size} bytes"
            rows.append({
                "bytes": len(text.encode()), "lines": text.count("\n") + 1, "output_bytes": rust_out.stat().st_size,
                "rust": summary(rust_samples["rust_process_all_ns"]),
                "rust_run": summary(rust_samples["rust_run_transform_ns"]),
                "c_compiled": summary(c_samples["c_compiled_ns"]),
                "rust_identity": summary(rust_samples["rust_identity_ns"]),
                "c_identity": summary(c_samples["c_identity_ns"]),
                "c_process_all": summary(c_samples["c_process_all_ns"]),
                "rust_build_ms": statistics.median(rust_samples["build_ns"]) / 1e6,
                "c_build_ms": statistics.median(c_samples["build_ns"]) / 1e6,
                "rust_rss_mib": rust_rss / 1024, "c_rss_mib": c_rss / 1024,
            })
            print(f"{len(text.encode()):>10} B  rust run_transform {rows[-1]['rust_run']['median']:9.2f} ms  "
                  f"c compiled {rows[-1]['c_compiled']['median']:9.2f} ms  "
                  f"rust process_all {rows[-1]['rust']['median']:9.2f} ms  "
                  f"c process_all {rows[-1]['c_process_all']['median']:9.2f} ms  outputs identical", flush=True)

    lines = [
        f"Pipeline: {len(STEPS)} steps covering all {len(builder)} builder methods plus a block, a conditional, "
        f"a replacement map and nested iteration. Per side: {args.rounds} process runs in alternating order, "
        f"each with {args.warmup} warm-ups and {args.iterations} measured iterations "
        f"({args.rounds * args.iterations} samples). Times in ms (median ± standard deviation); "
        "outputs byte-identical in every path. Δ compares the C path with the matching Rust call.",
        "",
        "| Input | Lines | Rust `run_transform` | C `tf_compiled_pipeline_process` | Δ | Rust `process_all` | "
        "C `tf_processor_process_all` | Δ | Peak RSS Rust / C |",
        "| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for row in rows:
        def cell(key: str) -> str:
            return f"{row[key]['median']:.2f} ± {row[key]['stdev']:.2f}"
        def delta(key: str, base: str) -> str:
            return f"{(row[key]['median'] / row[base]['median'] - 1) * 100:+.1f}%"
        lines.append(f"| {row['bytes'] / 1024:,.0f} KiB | {row['lines']:,} | {cell('rust_run')} | {cell('c_compiled')} | "
                     f"{delta('c_compiled', 'rust_run')} | {cell('rust')} | {cell('c_process_all')} | "
                     f"{delta('c_process_all', 'rust')} | {row['rust_rss_mib']:.1f} / {row['c_rss_mib']:.1f} MiB |")
    lines += ["", "Identity pipeline (no instructions; isolates the per-call boundary cost). The C path "
              "validates the input as UTF-8 on every call; the Rust program validated it once when reading "
              "the file, outside the timing. Median ms:", "",
              "| Input | Rust `run_transform` | C `tf_compiled_pipeline_process` | Difference |",
              "| ---: | ---: | ---: | ---: |"]
    for row in rows:
        difference = row["c_identity"]["median"] - row["rust_identity"]["median"]
        lines.append(f"| {row['bytes'] / 1024:,.0f} KiB | {row['rust_identity']['median']:.3f} | "
                     f"{row['c_identity']['median']:.3f} | {difference:+.3f} ms |")
    lines += ["", "Pipeline construction (builder calls + validation, median): " + ", ".join(
        f"Rust {row['rust_build_ms']:.3f} ms, C {row['c_build_ms']:.3f} ms" for row in rows[:1])]
    report = "\n".join(lines) + "\n"
    print("\n" + report)
    if args.markdown:
        args.markdown.write_text(report, encoding="utf-8")


if __name__ == "__main__":
    main()

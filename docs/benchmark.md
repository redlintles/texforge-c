# Benchmark: C binding versus the Rust crate

`python3 scripts/benchmark.py` runs one complex pipeline through the C library
and through the Rust crate on the same inputs and compares the results.

## Method

* **Pipeline:** 84 steps that cover all 78 `TextForgeBuilderMethods` (the 77
  generated functions plus `define_replace_map`), with a block and its calls, a
  conditional (`if_do_contains_each`), a scoped replacement map, variables
  (`val`/`var`/`mut_var`/`eval`) and nested iteration. One step list is
  rendered into both programs by the same code that generates the parity
  tests, so both build exactly the same pipeline.
* **Order:** size-preserving steps first, then line operations, then the
  shrinking steps (selection, joins, extraction). Word operations rebuild the
  text with single spaces, which removes line breaks (core behavior), so they
  run after the line phase. About 65 of the 84 steps process the full input;
  the last ~20 run on the 500 lines kept by `delete_lines_after`.
* **Input:** deterministic multi-line UTF-8 text with accents, CJK, emoji,
  combining characters, tabs, HTML/JSON-sensitive characters and empty lines.
* **Rust:** a temporary cargo project on the same `vendor/textforge`, same
  release profile and locked dependency versions, using the public
  `TextForgeBuilder`. It enables `test_access`, which only makes internal
  modules public (needed for the nested `iter` argument, as in the parity
  oracle); it changes no execution code.
* **C:** a `-O2` program linked statically against the release library, using
  the generated `tf_pipeline_*` functions.
* **Paths compared:** `tf_compiled_pipeline_process` against Rust
  `run_transform` (the call `CompiledPipeline` makes), and
  `tf_processor_process_all` against Rust `process_all`.
* **Statistics:** per side, 6 process runs in alternating order (Rust→C, C→Rust,
  …) to cancel drift, each with 3 warm-ups and 10 measured iterations, giving
  60 samples per cell. Outputs are byte-identical in every path for every size;
  the script fails otherwise.

## Results (2026-11-11)

Environment: 2 shared vCPUs (Intel Xeon @ 2.10 GHz), Linux, rustc 1.97.0, GCC
13.3. Shared CPUs make the standard deviation large (≈ 5–15 %); differences
below that are noise.

| Input | Lines | Rust `run_transform` | C `tf_compiled_pipeline_process` | Δ | Rust `process_all` | C `tf_processor_process_all` | Δ | Peak RSS Rust / C |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 100 KiB | 1,074 | 18.82 ± 1.98 | 19.87 ± 2.78 | +5.6% | 19.71 ± 4.13 | 20.42 ± 3.35 | +3.6% | 18.0 / 18.0 MiB |
| 1,024 KiB | 11,063 | 123.42 ± 11.61 | 123.97 ± 14.76 | +0.4% | 125.89 ± 9.94 | 125.78 ± 12.49 | -0.1% | 24.3 / 24.3 MiB |
| 8,192 KiB | 87,319 | 974.87 ± 51.54 | 961.75 ± 62.65 | -1.3% | 990.13 ± 62.00 | 998.05 ± 68.40 | +0.8% | 77.1 / 78.0 MiB |

Times in ms, median ± standard deviation. Pipeline construction (84 builder
calls and validation): Rust 1.27 ms, C 1.34 ms.

**Boundary cost.** A pipeline without instructions isolates what the C
boundary adds per call:

| Input | Rust `run_transform` | C `tf_compiled_pipeline_process` | Difference |
| ---: | ---: | ---: | ---: |
| 100 KiB | 0.002 ms | 0.193 ms | +0.19 ms |
| 1,024 KiB | 0.051 ms | 1.892 ms | +1.8 ms |
| 8,192 KiB | 0.687 ms | 15.683 ms | +15.0 ms |

Measured separately on the 8 MiB input: UTF-8 validation of the input takes
15.4 ms; copying the result into the returned `TfString` (with its terminator)
takes 0.7 ms, the same as Rust's own `into_owned`. The difference is therefore
the input validation. It is required: the core works on `&str`, so C input must
be valid UTF-8. The Rust program performs the same validation once when it reads
the file (`read_to_string`), outside the timed region. On this corpus, which is
rich in multi-byte characters, it costs about 1.9 ms/MiB, roughly 1.6 % of the
full pipeline; pure ASCII validates much faster.

An attempted change to the result copy (reserving the terminator slot before
pushing it) showed no measurable difference and was not kept.

## Conclusions

* The binding runs the pipeline at the same speed as the Rust crate: every
  median is within the run-to-run noise, at all sizes, through both paths.
* Memory use is the same (peak RSS within 1 MiB).
* The only systematic cost is UTF-8 validation of each input (≈ 2 ms per MiB of
  multi-byte text here). A faster SIMD validator would need a new dependency
  (maintainer approval) and is only worthwhile for workloads dominated by very
  cheap pipelines on large inputs.

## Running it

```sh
python3 scripts/benchmark.py                       # 100K, 1M, 8M; 15 iterations, 4 rounds
python3 scripts/benchmark.py --sizes 1M,32M --iterations 10 --rounds 6 --markdown result.md
```

Run it on an idle machine with dedicated cores for tighter deviations. The
script builds both programs in a temporary directory and leaves no files
behind.

# TextForge

> A sequential text-transformation DSL built in Rust — with blocks, conditionals, and variables. Write pipelines once, run them anywhere.

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%203.0-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
![Status: Pre-release](https://img.shields.io/badge/status-pre--release-orange)

> TextForge was previously known as **ATP (Advanced Text Processor)**. The crate, binary, and file formats have been renamed; some file extensions and example pipelines in this repo may still be catching up.

---

## What is TextForge?

TextForge is a text-processing DSL that executes **sequential pipelines of single-purpose instructions**. Each instruction performs exactly one transformation on the input text, and instructions chain one after another — the output of each step becomes the input of the next.

```
"  hello world  " → tbs → raw world Rust → tua → "HELLO RUST"
```

On top of that linear pipeline, TextForge also has a small amount of **control flow**:

- **Blocks** (`blk` / `cblk`) — group a set of instructions under a name and invoke them together, as many times as you want.
- **Conditionals** (`ifdc`) — run a group of instructions only if the input contains a given value.
- **Variables** (`val`) — store an immutable value in the pipeline's execution context so other instructions can reference it instead of a hardcoded literal.

Pipelines can be written in human-readable `.textforge` files, compiled to an optimized `.textforgebc` binary format, or composed directly in Rust via the `TextForgeBuilder` API. Language bindings are maintained in independent projects; see the [Python binding status](docs/python-bindings.md) for its separately pinned core and package versions.

---

## Why TextForge?

- **Portable pipelines** — write a pipeline once, share it as a `.textforge` file, version it with Git, and audit it instruction by instruction.
- **Optimized binary format** — `.textforgebc` eliminates parsing overhead for production use cases.
- **Extensible by design** — adding a new instruction touches a small, fixed set of files and never modifies the core. The instruction set can scale to thousands without architectural changes.
- **Fluent Rust API** — compose pipelines directly in code via `TextForgeBuilder`/`TextForgeProcessor` without writing `.textforge` files.
- **Batch processing** — run a pipeline over many input files in parallel (powered by `rayon`).
- **Built-in observability (optional)** — the `watchers` feature lets you attach probes to a pipeline run and export a JSON report of what happened at each step.

---

## File Formats

| Format | Extension | Use case |
|--------|-----------|----------|
| Text pipeline | `.textforge` | Human-readable, editable, versionable |
| Binary pipeline | `.textforgebc` | Optimized for performance and distribution (`bytecode` feature) |

### `.textforge` Syntax

```textforge
// Comments start with //
// One instruction per line, ending with ;

tbs;
raw world Rust;
tua;
```

Arguments are split the same way a shell splits a command line, so quoting is optional unless an argument contains whitespace:

```textforge
atb "banana";
raw "banana" "laranja";
```

#### Control flow

```textforge
// Define a block named "block_1" by adding instructions to it, one `blk` line at a time
blk block_1 assoc dlf;
blk block_1 assoc dll;
blk block_1 assoc ins 5 "banana";

// Run every instruction in "block_1", in the order they were added
cblk block_1;
```

`val` declares an immutable variable in the execution context; other instructions can then reference it (by variable name) instead of a hardcoded literal. `ifdc` runs a group of instructions only if the current text contains a given value. Both are easiest to use through the Rust builder API for now — see [`TextForgeConditionalMethods`](src/api/mod.rs) and [`TextForgeBlockMethods`](src/api/mod.rs).

---

## Quick Start

### As a Rust library

TextForge isn't on crates.io yet — add it as a git dependency:

```toml
[dependencies]
textforge = { git = "https://github.com/redlintles/textforge" }
```

Use the builder API through a `TextForgeProcessor`:

```rust
use textforge::api::{
    processor::{ TextForgeProcessor, TextForgeProcessorMethods },
    TextForgeBuilderMethods,
};

let mut processor = TextForgeProcessor::new();

let id = processor
    .create_pipeline()
    .trim_both_sides()?
    .replace_all_with("world", "Rust")?
    .to_uppercase_all()?
    .build()?;

let result = processor.process_all(&id, "  hello world  ")?;
assert_eq!(result, "HELLO RUST");
```

### From a `.textforge` file (CLI)

```bash
atp run pipeline.textforge --input "  hello world  "
```

> **Note:** The CLI is currently under development and not yet available. (The binary is still named `atp` internally; a couple of empty binary stubs for a future shell/game also exist but do nothing yet.)

---

## Instruction Reference

### String Manipulation

| Mnemonic | Description | Args | Example |
|----------|-------------|------|---------|
| `atb` | Add to beginning | `<text>` | `"world"` → `"helloworld"` |
| `ate` | Add to end | `<text>` | `"hello"` → `"hello!"` |
| `ins` | Insert at index | `<index> <text>` | Insert after nth char |
| `rev` | Reverse | — | `"abc"` → `"cba"` |
| `rpt` | Repeat | `<times>` | `"hi"` × 3 → `"hihihi"` |
| `rmws` | Remove whitespace | — | Removes all whitespace |
| `splc` | Split characters | — | `"abc"` → `"a b c"` |

### Delete Operations

| Mnemonic | Description | Args |
|----------|-------------|------|
| `dlf` | Delete first char | — |
| `dll` | Delete last char | — |
| `dls` | Delete single char | `<index>` |
| `dlc` | Delete chunk | `<start> <end>` |
| `dla` | Delete after index | `<index>` |
| `dlb` | Delete before index | `<index>` |

### Replace Operations

| Mnemonic | Description | Args |
|----------|-------------|------|
| `raw` | Replace all occurrences | `<pattern> <replacement>` |
| `rfw` | Replace first occurrence | `<pattern> <replacement>` |
| `rlw` | Replace last occurrence | `<pattern> <replacement>` |
| `rnw` | Replace nth occurrence | `<pattern> <replacement> <n>` |
| `rcw` | Replace up to N occurrences | `<pattern> <replacement> <count>` |

### Trim & Select

| Mnemonic | Description | Args |
|----------|-------------|------|
| `tbs` | Trim both sides | — |
| `tls` | Trim left side | — |
| `trs` | Trim right side | — |
| `slt` | Select substring | `<start> <end>` |
| `sslt` | Split and select | `<pattern> <index>` |

### Case Conversion

| Mnemonic | Description | Args |
|----------|-------------|------|
| `tua` | Uppercase all | — |
| `tla` | Lowercase all | — |
| `tucs` | Uppercase single char | `<index>` |
| `tlcs` | Lowercase single char | `<index>` |
| `tucc` | Uppercase chunk | `<start> <end>` |
| `tlcc` | Lowercase chunk | `<start> <end>` |
| `tucw` | Uppercase word | `<index>` |
| `tlcw` | Lowercase word | `<index>` |

### Capitalize

| Mnemonic | Description | Args |
|----------|-------------|------|
| `cfw` | Capitalize first word | — |
| `clw` | Capitalize last word | — |
| `cts` | Capitalize single word | `<index>` |
| `ctc` | Capitalize chunk | `<start> <end>` |
| `ctr` | Capitalize range | `<start> <end>` |

### Rotate & Pad

| Mnemonic | Description | Args |
|----------|-------------|------|
| `rtl` | Rotate left | `<times>` |
| `rtr` | Rotate right | `<times>` |
| `padl` | Pad left | `<text> <max_len>` |
| `padr` | Pad right | `<text> <max_len>` |

### Case Formatting (Join)

| Mnemonic | Description | Example |
|----------|-------------|---------|
| `jkbc` | kebab-case | `"hello world"` → `"hello-world"` |
| `jsnc` | snake_case | `"hello world"` → `"hello_world"` |
| `jcmc` | camelCase | `"hello world"` → `"helloWorld"` |
| `jpsc` | PascalCase | `"hello world"` → `"HelloWorld"` |

### Encoding & Escaping

| Mnemonic | Description |
|----------|-------------|
| `urle` | URL encode |
| `urld` | URL decode |
| `htmle` | HTML escape |
| `htmlu` | HTML unescape |
| `jsone` | JSON escape |
| `jsonu` | JSON unescape |

### Control Flow

| Mnemonic | Description | Args |
|----------|-------------|------|
| `blk` | Add an instruction to a named block (creating it if needed) | `<block_name> assoc <instruction>` |
| `cblk` | Run every instruction previously added to a named block, in order | `<block_name>` |
| `ifdc` | Run a group of instructions only if the input contains a value | `<value>` |
| `val` | Declare an immutable variable in the execution context | `<name> <value>` |

> 🚧 An `emj` instruction (extract all regex matches, join them with a separator) has been implemented but is not yet registered in the instruction table — it isn't usable from `.textforge` files or the builder API yet.

---

## Feature Flags

| Flag | Description |
|------|-------------|
| `default` | Core library, no CLI, no bytecode |
| `bytecode` | Enables the `.textforgebc` binary protocol and the CLI binary |
| `watchers` | Enables pipeline observability probes and JSON execution reports |
| `test_access` | Enables test helpers (`rand`, `random-string`, `tempfile`) and pulls in `bytecode`/`watchers` |

---

## Batch Processing

Pipelines can be run over many files in parallel via `TextForgeProcessor::process_batch`, which reads each input file, runs a registered pipeline against it, and writes the result to an output file — backed by `rayon` under the hood.

---

## Adding a New Instruction

TextForge is designed so that adding a new instruction never modifies the core. Involved files:

- Create `src/tokens/transforms/<mnemonic>/mod.rs` — instruction struct + `InstructionMethods` impl
- Create `src/tokens/transforms/<mnemonic>/test.rs` — unit tests
- Register the mnemonic in `src/globals/table.rs`
- Add the builder method in `src/api/mod.rs` (`TextForgeBuilderMethods`)

See [`instruction-design.md`](.agents/skills/atp-project/references/instruction-design.md) for the full guide with code templates.

---

## Project Structure

```
src/
├── main.rs                    — CLI entrypoint (requires bytecode feature)
├── bin/                        — additional binaries (shell/game scaffolding, not yet implemented)
├── api/                        — Public API surface (builder, processor, block/conditional builders)
├── bytecode/                  — Binary .textforgebc protocol (feature-gated)
├── context/
│   ├── execution_context.rs   — Runtime execution state (variables, blocks)
│   └── static_context.rs      — Static/compile-time context
├── globals/
│   ├── table.rs               — Instruction registry
│   └── var.rs                 — Variable/param resolution
├── macros/                    — Internal Rust macros
├── text/
│   ├── reader.rs              — .textforge file parsing
│   └── writer.rs              — Output writing
├── tokens/
│   ├── transforms/            — One subdirectory per transform instruction
│   └── instructions/          — Control-flow tokens (blk, cblk, ifdc, val)
├── watchers/                  — Pipeline observability (feature-gated)
└── utils/                     — Shared utilities

tests/                          — Integration tests (processor, bytecode, params, benchmark)
pipelines/text/                 — Example pipelines
data/                           — Sample input files for batch-processing examples
```

---

## Roadmap

**Done**
- [x] Core library with 47+ instructions
- [x] `.textforge` text format
- [x] `.textforgebc` binary protocol
- [x] `TextForgeBuilder`/`TextForgeProcessor` Rust API
- [x] Control flow: blocks (`blk`/`cblk`), conditionals (`ifdc`), variables (`val`)
- [x] Batch processing over multiple files, in parallel (`rayon`)
- [x] Optional pipeline observability (`watchers` feature, JSON reports)

**In progress**
- [ ] Complete CLI
- [ ] `emj` instruction registration (implemented, not yet wired in)
- [ ] Error accumulation / diagnostics API (`ErrorManager` is an internal sink for now)
- [ ] Pipeline chaining (`pipeline1.textforge | pipeline2.textforge`)
- [ ] REPL and `textforge-game` (binary scaffolding exists; no behavior yet)

**Planned**
- [ ] C FFI (`cdylib`)
- [ ] JavaScript bindings (WASM / NAPI)
- [ ] Python bindings (PyO3)
- [ ] Public pipeline repository
- [ ] Project website
- [ ] Publish to crates.io

---

## Contributing

Contributions are welcome. This repo currently doesn't have a root-level `CONTRIBUTING.md` — the instruction design pattern lives in [`.agents/skills/atp-project/`](.agents/skills/atp-project/) instead. *(If you'd rather have a standalone `CONTRIBUTING.md` again, let me know and I'll draft one from that content.)*

All commits, code, and documentation should be in **English**.

Commit format: `type(scope): description` ([Conventional Commits](https://www.conventionalcommits.org/))

---

## License

[GPL-3.0](LICENSE) — forks must remain open source.
### Non-cascading replacement maps

Use `rmapd animals add 'cat' 'wolf';`, `rmapd animals add 'wolf' 'dog';`, then
`rmape animals;` to turn `cat wolf` into `wolf dog`. Definitions live in the
execution context; execution can be reused from blocks. Rust builders expose
`define_replace_map` and `execute_replace_map`. See [replacement maps](docs/replace-maps.md)
for overlap rules, capture expansion, scope, bytecode and concurrent batch reuse.

### Unicode normalization

Use `unorm nfc;`, `unorm nfd;`, `unorm nfkc;` or `unorm nfkd;` to normalize the
current text explicitly. Rust builders expose `normalize_unicode(mode)`; the Python
binding mirrors it. See [Unicode normalization](docs/unicode-normalization.md) for
examples, compatibility distinctions, Unicode version and document-boundary behavior.

### Logical lines

The line transforms use **one-based** indices and inclusive intervals. For example:

```rust
use textforge::api::{processor::{TextForgeProcessor, TextForgeProcessorMethods}, TextForgeBuilderMethods};
let mut processor = TextForgeProcessor::new();
let id = processor.create_pipeline()
    .delete_empty_lines().unwrap()
    .unique_lines().unwrap()
    .join_lines(" | ").unwrap()
    .build().unwrap();
assert_eq!(processor.process_all(&id, "é\n\né\n🍌").unwrap(), "é | 🍌");
```

See the [line semantics and complete instruction list](docs/lines.md) and the
[2.0.0 changelog](changelog.md). Python adapters are released separately as
`textforgers` 0.1.2.


## 2.0.0 release preparation and performance

The 2.0.0 candidate reduces temporary strings and reuses owned buffers while
preserving instruction results, public signatures and bytecode opcodes. Trims and
contiguous selections can return borrowed slices; word/character transforms avoid
per-item strings and intermediate vectors. Unicode contextual case conversion
and existing whitespace/line semantics are preserved.

See [implementation, compatibility evidence and benchmark methodology](docs/allocation-optimization.md)
and [raw timing, CPU and peak-memory samples](docs/benchmarks/allocations-v2.json).
The comparison uses the same large UTF-8 files and repeated workloads on both
versions, including coverage of all 81 instructions. Results are specific to the
recorded machine and workloads.

This is a core release candidate, not a publication announcement. The independent
Python package keeps its previous pinned core snapshot; 2.0.0 Python distribution
support is not claimed. See the [release checklist](docs/release-2.0.0.md).


Measured medians on the recorded Ryzen 5 4600G machine (five runs; three document
passes per process; in-memory execution):

| Input / workload | Wall seconds, baseline → 2.0.0 | Peak RSS MiB, baseline → 2.0.0 |
| --- | ---: | ---: |
| 4 MiB / coverage | 5.36 → 2.59 | 221.2 → 25.5 |
| 4 MiB / large | 1.93 → 0.20 | 220.9 → 21.3 |
| 16 MiB / coverage | 23.11 → 11.09 | 867.8 → 99.5 |
| 16 MiB / large | 8.08 → 0.90 | 867.6 → 85.5 |

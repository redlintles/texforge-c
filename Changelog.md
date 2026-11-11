# Changelog

All notable changes to the TextForge C binding.

## Unreleased

### Added

- C ABI over the TextForge 0.2.0 `api::portable` layer, with a cbindgen-generated
  C11 header (`include/textforge.h`) documented in Doxygen style.
- Fluent `TfPipeline` builder with sticky errors: 77 typed `tf_pipeline_<method>`
  functions generated from the public Rust builder, generic `tf_pipeline_add`,
  blocks, conditionals, replace maps, `TfCompiledPipeline`.
- Full processor surface (`TfProcessor`): registration, snapshots, text and
  bytecode persistence, debug runs, files, parallel batches and worker pools.
- Watchers with C callbacks and built-in metrics; spool-backed buffers with
  synchronous reader/writer callbacks; lossless `TfFilePath`.
- Structured `TfError` preserving the core code, stage, message, instruction
  and input.
- `make install` with versioned SONAME, static library and pkg-config file
  `textforge-c`.
- Validation: Rust-oracle parity tests for the builder (84 cases) and processor
  (30 cases), C suites for conventions, ownership, callbacks, threads and files,
  every header example compiled and run, all under ASan/UBSan and valgrind, and
  an installed-package check with dynamic and static consumers.
- Bundled core snapshot identical to the reviewed Node.js binding snapshot, with
  provenance verification.

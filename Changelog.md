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
- CMake build (`CMakeLists.txt`) driving cargo, one linkage per build, with a
  relocatable CMake package `textforge-c` (target `textforge-c::textforge-c`,
  per-configuration locations), pkg-config module and Rust cross targets; the
  Makefile now wraps it.
- Reproducible offline source release (`scripts/make_source_release.py`) that
  vendors every Rust crate.
- Conan recipe in the ConanCenter layout and a vcpkg port with git-registry
  metadata (`packaging/`), kept in sync by `scripts/update_recipes.py` and
  verified by `scripts/check_packaging.py` and a CI job. ConanCenter and the
  vcpkg curated registry do not currently accept Rust-built libraries; see
  `docs/packaging.md`.
- macOS shared libraries use `@rpath/libtextforge_c.dylib` as install name.

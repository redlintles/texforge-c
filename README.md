# textforge-c

C bindings for [TextForge](https://github.com/redlintles/textforge), a DSL for
text-transformation pipelines. The library is a thin adapter over the core's
language-neutral `api::portable` layer: every instruction, validation rule and
execution step comes from the bundled TextForge core, so C programs get the same
results and error codes as Rust.

- **Version:** 0.1.0 (unreleased). **Core:** TextForge 0.2.0, reviewed snapshot (see [docs/core.md](docs/core.md)).
- **Header:** [`include/textforge.h`](include/textforge.h), C11, usable from C++.
- **Library:** `libtextforge_c` (shared, SONAME `libtextforge_c.so.0.1`, and static), pkg-config module `textforge-c`.
- **Supported platform:** Linux x86_64 with glibc ([compatibility matrix](docs/compatibility.md)).
- **License:** GPL-3.0-only.

## Build and install

Requires a stable Rust toolchain (edition 2024), a C compiler, `make` and `pkg-config`.

```sh
make                                 # release shared + static libraries
sudo make install                    # PREFIX=/usr/local by default
make install DESTDIR=/tmp/pkg PREFIX=/usr   # staged install for packaging
```

`make install` puts `textforge.h` in `$(PREFIX)/include`, the libraries and
the SONAME symlinks in `$(PREFIX)/lib`, and `textforge-c.pc` in
`$(PREFIX)/lib/pkgconfig`. `make uninstall` removes them.

```sh
cc -std=c11 app.c $(pkg-config --cflags --libs textforge-c) -o app
# Static linking:
cc -std=c11 app.c $(pkg-config --cflags textforge-c) \
   -Wl,-Bstatic -ltextforge_c -Wl,-Bdynamic $(pkg-config --static --libs-only-l textforge-c | sed 's/-ltextforge_c//')
```

## Quick start

```c
#include <stdio.h>
#include <textforge.h>

int main(void) {
    TfPipeline *plan = tf_pipeline_new();
    tf_pipeline_trim_both_sides(plan);
    tf_pipeline_val(plan, tf_text("greeting"), tf_text("Hello"));
    tf_pipeline_add_to_beginning(plan, tf_text("{{greeting}}, "));
    tf_pipeline_add_to_end(plan, tf_text("!"));

    TfCompiledPipeline *pipeline = NULL;
    TfError *error = NULL;
    if (tf_pipeline_compile(plan, &pipeline, &error) != TF_STATUS_OK) {
        fprintf(stderr, "%s\n", tf_error_message(error, NULL));
        tf_error_free(error);
        tf_pipeline_free(plan);
        return 1;
    }
    tf_pipeline_free(plan);

    TfString out = {0};
    if (tf_compiled_pipeline_process(pipeline, "  world  ", TF_NTS, &out, NULL) == TF_STATUS_OK)
        printf("%s\n", out.data);  /* Hello, world! */
    tf_string_free(&out);
    tf_compiled_pipeline_free(pipeline);
    return 0;
}
```

The full program is [`examples/c/quickstart.c`](examples/c/quickstart.c). Every
public declaration in the header carries Doxygen documentation, most with a C
example; those examples are compiled and run by the test suite.

## How the API works

The rules below hold for every function; [docs/api.md](docs/api.md) has the
complete ownership, threading and callback reference.

| Topic | Rule |
| --- | --- |
| Text | UTF-8 as `(const char *text, size_t length)`. `TF_NTS` means NUL-terminated; with an explicit length NUL bytes are kept. Invalid UTF-8 is rejected, never repaired. |
| Status | Fallible functions return `TfStatus` (`TF_STATUS_OK == 0`). |
| Errors | The optional last argument `TfError **error` must point to `NULL`; on failure it receives the core code, stage, message, instruction and input. Release with `tf_error_free`. |
| Out-parameters | Written only on success. |
| Objects | `tf_<type>_new` / `tf_<type>_free`. Free functions accept `NULL`. |
| Returned data | `TfString`, `TfBytes`, `TfStringList`, ... belong to the caller; release with their `tf_*_free`, which resets them (calling it twice is harmless). Never use `free()`. |
| Consumed arguments | Bodies of `tf_pipeline_block`/`contains_each`, replace-map builders and input buffers of buffered processing are released by the call on every path. |
| Builders | `tf_pipeline_<method>` mirrors each Rust `TextForgeBuilderMethods` method and returns the pipeline. The first failure becomes a *sticky error* reported by `tf_pipeline_compile`, `tf_pipeline_build` or `tf_pipeline_status`. |
| Arguments | `TfArg` values from `tf_text`, `tf_var`, `tf_number`, `tf_nested`; they borrow their data until the receiving call returns. |
| Threads | Compiled pipelines may be used concurrently. Other objects serialize calls from different threads; re-entering an object from its own callback returns `TF_STATUS_BUSY`. |

### Main types

| C type | Rust counterpart | Purpose |
| --- | --- | --- |
| `TfPipeline` | `TextForgeBuilder` / `PipelinePlan` | Fluent plan under construction |
| `TfCompiledPipeline` | `CompiledPipeline` | Immutable executable |
| `TfProcessor` | `TextForgeProcessor` / `PortableProcessor` | Registry of pipelines by ID, files, batches, debug, bytecode |
| `TfInstruction` | `InstructionSpec` | Mnemonic + typed arguments |
| `TfReplaceMapBuilder` | `ReplaceMapBuilder` | Rules for `tf_pipeline_define_replace_map` |
| `TfObservers` | `Observers` / `WatcherList` | Watchers and JSON reports |
| `TfBuffer`, `TfBufferOptions` | `PortableBuffer`, `BufferOptions` | Spool-backed documents and streams |
| `TfFilePath` | `FilePath` | Lossless OS paths (UTF-8, Unix bytes, Windows UTF-16) |
| `TfBatchPool` | `BatchPool` | Reusable worker pool |
| `TfError` | `BindingError` | Structured error |

The complete method mapping is generated in
[docs/builder-parity.json](docs/builder-parity.json) and
[docs/processor-parity.json](docs/processor-parity.json); see [docs/parity.md](docs/parity.md).

## Development

All work happens in this repository; the core is consumed only from
`vendor/textforge` and is never edited here (see [docs/core.md](docs/core.md)).

```sh
python3 scripts/check.py          # everything below; run before each commit
python3 scripts/check.py --quick  # without valgrind and the package check
```

| Command | Purpose |
| --- | --- |
| `cargo fmt --check`, `cargo clippy --all-targets [--features test-reference] -- -D warnings` | Format and lints (no `unwrap`/`expect`/`panic`, every `unsafe` block justified) |
| `cargo test [--features test-reference]` | Rust doctest and header freshness (`TEXTFORGE_UPDATE_HEADER=1 cargo test --test header` regenerates the header with cbindgen) |
| `python3 scripts/check_core.py` | Bundled core matches the published crate plus the reviewed patch (`cargo fetch --manifest-path scripts/upstream/Cargo.toml` first) |
| `python3 scripts/generate_builder.py [--check]` | Typed builder functions from the pinned Rust builder |
| `python3 scripts/generate_reference.py [--check]` | Rust builder oracle, fixtures and the generated C parity test |
| `python3 scripts/generate_processor_reference.py [--check]` | Rust processor/error oracle and its C table |
| `python3 scripts/check_parity.py` | Every public Rust and portable entry point has a C function |
| `python3 scripts/run_c_tests.py` | C suites and header examples: strict C11, plain, ASan/UBSan and valgrind |
| `python3 scripts/check_distribution.py` | Clean source build, `make install`, installed layout, exports, dynamic/static consumers, suites against the installed library |

Generated files (`src/builder.rs`, `include/textforge.h`, `tests/c/test_builder_parity.c`,
`tests/c/processor_reference.h`, `tests/fixtures/*.json`, `docs/*-parity.json`)
must not be edited by hand.

## Status

Feature-complete against the portable API of the bundled core, with no known
parity gaps. Not yet published to any package registry; publishing needs
maintainer approval. See [Changelog.md](Changelog.md).

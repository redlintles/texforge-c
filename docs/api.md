# API reference: ownership, errors, threads and callbacks

The header documents every function. This page collects the rules that span
functions (AGENTS.md section 17, items 10 and 11).

## Ownership

| Value | Created by | Released by | Notes |
| --- | --- | --- | --- |
| `TfPipeline` | `tf_pipeline_new`, `tf_processor_create_pipeline` | `tf_pipeline_free` | Compiling or registering copies the plan; executables and IDs outlive it. |
| `TfCompiledPipeline` | `tf_pipeline_compile` | `tf_compiled_pipeline_free` | Immutable and independent of its plan. |
| `TfProcessor` | `tf_processor_new` | `tf_processor_free` | Owns every registered pipeline. |
| `TfInstruction` | `tf_instruction_new`, `tf_instruction_clone` | `tf_instruction_free` | Pipelines and processors keep copies. |
| `TfInstructionList` | `tf_processor_get_transform_vec` | `tf_instruction_list_free` | Frees every item too. |
| `TfReplaceMapBuilder` | `tf_replace_map_builder_new` | consumed by `tf_pipeline_define_replace_map`, or `tf_replace_map_builder_free` | |
| `TfObservers` | `tf_observers_new` | `tf_observers_free` | Owns callback `user_data` (see below). |
| `TfBuffer` | `tf_buffer_from_*`, buffered processing | `tf_buffer_free`, or consumed by `tf_buffer_into_string` / `tf_processor_process_*_buffered` | Freeing removes the spill file. |
| `TfBufferOptions`, `TfFilePath`, `TfBatchPool` | `*_new` / `tf_file_path_from_*` | `*_free` | Immutable. |
| `TfError` | any fallible call | `tf_error_free` | Immutable. |
| `TfString`, `TfBytes`, `TfWideString`, `TfStringList` | out-parameters | their `tf_*_free` | Struct values owned by the caller. |

* Free functions accept `NULL`. Array/string free functions reset the struct to
  `{NULL, 0}`, so a second call is harmless; handles cannot be reset, so do not
  free a handle twice.
* **Consuming functions** (`tf_pipeline_block`, `tf_pipeline_block_assoc`,
  `tf_pipeline_contains_each`, `tf_pipeline_if_do_contains_each`,
  `tf_pipeline_define_replace_map`, `tf_buffer_into_string`,
  `tf_processor_process_all_buffered`, `tf_processor_process_single_buffered`)
  release that argument on every path, including errors and a NULL target.
  Exceptions: passing a pipeline as its own body (rejected, nothing freed) and a
  buffer that is busy in one of its own callbacks (`TF_STATUS_BUSY`, nothing freed).
* Borrowed results — `tf_error_message`, `tf_instruction_name`,
  `tf_instruction_argument` views, `tf_observation_*` — stay valid while their
  owner lives (observations: only during the callback).
* `TfArg` borrows its text/instruction until the receiving call returns; every
  function copies what it keeps.

## Errors

Every failure has a `TfStatus` and, if requested, a `TfError`:

| Status | Stage | Source |
| --- | --- | --- |
| `TF_STATUS_CONVERSION` | `TF_STAGE_CONVERSION` | Unknown mnemonic, out-of-range number, excessive nesting (core) |
| `TF_STATUS_VALIDATION` | `TF_STAGE_VALIDATION` | Arity, types, block/conditional hierarchy (core, at compile/register) |
| `TF_STATUS_EXECUTION` | `TF_STAGE_EXECUTION` | Undefined variable or block, regex, I/O, streams (core) |
| `TF_STATUS_PERSISTENCE` | `TF_STAGE_PERSISTENCE` | Pipeline/report import and export (core) |
| `TF_STATUS_CALLBACK` | `TF_STAGE_CALLBACK` | A watcher callback failed (core code 27) |
| `TF_STATUS_INVALID_ARGUMENT` | `TF_STAGE_ADAPTER` | NULL pointer, invalid UTF-8, unknown `TfArg` kind, non-NULL error slot |
| `TF_STATUS_BUSY` | `TF_STAGE_ADAPTER` | Same-thread re-entry into an object in use |
| `TF_STATUS_PANIC` | `TF_STAGE_ADAPTER` | Internal panic caught at the boundary; that object may be unusable |
| `TF_STATUS_UNKNOWN` | `TF_STAGE_UNKNOWN` | Stage added by a newer core |

Core errors keep the portable code, message, instruction and input verbatim
(no colors, no rewriting); `tests/c/test_processor_parity.c` compares all five
fields with the Rust implementation. Adapter errors use code 0.

A `TfError **error` slot must hold `NULL` on entry. A non-NULL slot is refused
with `TF_STATUS_INVALID_ARGUMENT` and left untouched, so an earlier error is
never leaked by being overwritten.

**Lossless conversion.** Numbers are `uint64_t`; values that do not fit the
target's `size_t` fail with `TF_STATUS_CONVERSION` instead of being truncated.
Text is never re-encoded: invalid UTF-8 is rejected with its byte offset and
NUL bytes survive when explicit lengths are used. Paths use `TfFilePath`:
`tf_file_path_from_native_bytes` keeps arbitrary Unix bytes,
`tf_file_path_from_windows_wide` keeps unpaired UTF-16 surrogates, and a
representation that is not native to the platform fails with
`TF_STATUS_CONVERSION`. `tf_file_path_to_text` reports names that have no
exact Unicode form instead of approximating them.

## Sticky builder errors

Builder functions return the pipeline so calls can be written in sequence or
nested. When one fails (bad UTF-8, unknown mnemonic, an argument conversion
error, a consumed body with its own error), the error is stored and the call
does nothing; later builder calls on the same pipeline are skipped. The plan
itself is never partially modified. `tf_pipeline_status` reports the error
without clearing it, `tf_pipeline_clear_error` forgets it, and
`tf_pipeline_compile`/`tf_pipeline_build` fail with it. Builders return NULL
only for a NULL pipeline, which makes nested calls on a NULL pipeline safe.

## Threads

* `TfCompiledPipeline`, `TfInstruction`, `TfFilePath`, `TfBufferOptions`,
  `TfBatchPool` and `TfError` are immutable and may be used from several
  threads at once. Each `tf_compiled_pipeline_process` call uses a fresh
  execution context.
* `TfPipeline`, `TfProcessor`, `TfObservers`, `TfBuffer` and
  `TfReplaceMapBuilder` contain a lock: calls from different threads wait for
  each other. Freeing an object while another thread uses it is undefined
  behavior, as in any C library.
* Batch functions run file tasks on worker threads (the global pool or a
  `TfBatchPool`). Watcher and stream callbacks never run on those workers;
  they run on the calling thread.

## Callbacks

| Callback | Registered by | Lifetime of `user_data` |
| --- | --- | --- |
| `TfObserverFn` | `tf_observers_set` | Owned by the observers. `TfDestroyFn` (optional) runs exactly once: on replacement, on `tf_observers_free`, or immediately when `tf_observers_set` fails. |
| `TfStreamReadFn` | `tf_buffer_from_reader` | Borrowed for that call only. |
| `TfStreamWriteFn` | `tf_buffer_copy_to` | Borrowed for that call only. |

* Callbacks return `0` for success and non-zero for failure; the failure becomes
  `TF_STATUS_CALLBACK` (watchers) or `TF_STATUS_EXECUTION` (streams). A watcher
  must call `tf_observer_result_set` (or `tf_observer_result_set_error`) before
  returning.
* Callbacks must return normally. `longjmp` or a C++ exception leaving a callback
  is undefined behavior (a Rust unwind would abort the process).
* **Re-entrancy.** A callback may use any other TextForge object. Calling back
  into the object that is running it — the processor or observers of a watched
  run, or the buffer being read or copied — fails with `TF_STATUS_BUSY` instead
  of corrupting state; a busy buffer is not consumed.
* Callback `user_data` shared between threads must be thread-safe; the library
  does not synchronize it.

## Exceptions and panics

The core API is designed not to panic; every exported function still catches
panics and returns `TF_STATUS_PANIC`, so unwinding never crosses into C. A
panic while an object is locked marks it unusable: later calls on it return
`TF_STATUS_PANIC`; it can still be freed.

//! Full processor surface (portable `PortableProcessor`).

use crate::{
    error::{Failure, TfError, TfStatus},
    ffi::{self, Guarded, Out},
    instruction::{TfInstruction, TfInstructionList},
    path::{self, TfFilePath},
    pipeline::TfPipeline,
    text::{TfString, TfStringList},
};
use std::{ffi::c_char, ptr};
use textforge::api::portable::{BatchPool, FileTask, PortableProcessor};

/// Registry of pipelines addressed by string IDs (Rust `TextForgeProcessor`).
///
/// Create with `tf_processor_new`, release with `tf_processor_free`. Calls
/// from different threads are serialized; a call made from inside one of this
/// processor's own watcher or stream callbacks fails with `TF_STATUS_BUSY`.
///
/// ```c
/// TfProcessor *processor = tf_processor_new();
/// TfPipeline *plan = tf_pipeline_add_to_beginning(tf_processor_create_pipeline(processor), tf_text("🍌"));
/// TfString id = {0};
/// assert(tf_pipeline_build(plan, processor, &id, NULL) == TF_STATUS_OK);
/// tf_pipeline_free(plan);
/// TfString out = {0};
/// assert(tf_processor_process_all(processor, id.data, id.length, "é", TF_NTS, &out, NULL) == TF_STATUS_OK);
/// assert(strcmp(out.data, "🍌é") == 0);
/// tf_string_free(&out);
/// assert(tf_processor_remove_transform(processor, id.data, id.length, NULL) == TF_STATUS_OK);
/// bool exists = true;
/// assert(tf_processor_transform_exists(processor, id.data, id.length, &exists, NULL) == TF_STATUS_OK);
/// assert(!exists);
/// tf_string_free(&id);
/// tf_processor_free(processor);
/// ```
pub struct TfProcessor {
    pub(crate) inner: Guarded<PortableProcessor>,
}

/// Borrow a processor handle.
///
/// # Safety
///
/// `processor` is NULL or a live processor.
pub(crate) unsafe fn borrow<'a>(processor: *const TfProcessor) -> Result<&'a TfProcessor, Failure> {
    // SAFETY: forwarded caller contract.
    unsafe { ffi::handle(processor, "processor") }
}

/// Read a pipeline ID argument.
///
/// # Safety
///
/// `id`/`length` follow the text convention.
pub(crate) unsafe fn id<'a>(id: *const c_char, length: usize) -> Result<&'a str, Failure> {
    // SAFETY: forwarded caller contract.
    unsafe { ffi::text(id, length, "pipeline id") }
}

/// Run `operation` on a locked processor and write its text result.
///
/// # Safety
///
/// As [`ffi::call`]; `processor` is NULL or live; `out` is writable.
unsafe fn text_result(
    processor: *const TfProcessor,
    out: *mut TfString,
    error: *mut *mut TfError,
    operation: impl FnOnce(&mut PortableProcessor) -> Result<String, Failure>,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let mut processor = borrow(processor)?.inner.lock()?;
            out.write(TfString::from(operation(&mut processor)?));
            Ok(())
        })
    }
}

/// Run `operation` on a locked processor without a result value.
///
/// # Safety
///
/// As [`ffi::call`]; `processor` is NULL or live.
unsafe fn unit_result(
    processor: *const TfProcessor,
    error: *mut *mut TfError,
    operation: impl FnOnce(&mut PortableProcessor) -> Result<(), Failure>,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe { ffi::call(error, || operation(&mut *borrow(processor)?.inner.lock()?)) }
}

/// Create an empty processor. Never returns NULL.
#[unsafe(no_mangle)]
pub extern "C" fn tf_processor_new() -> *mut TfProcessor {
    ffi::into_handle(TfProcessor {
        inner: Guarded::new(PortableProcessor::new()),
    })
}

/// Release a processor and every pipeline registered in it. `NULL` is ignored.
///
/// # Safety
///
/// `processor` is NULL or live and not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_free(processor: *mut TfProcessor) {
    // SAFETY: forwarded caller contract.
    ffi::guard((), || drop(unsafe { ffi::take_handle(processor) }));
}

/// Create an empty pipeline intended for this processor (Rust
/// `create_pipeline`). Register it with `tf_pipeline_build`. Returns NULL only
/// when `processor` is NULL.
///
/// # Safety
///
/// `processor` is NULL or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_create_pipeline(processor: *const TfProcessor) -> *mut TfPipeline {
    if processor.is_null() {
        return ptr::null_mut();
    }
    crate::pipeline::tf_pipeline_new()
}

/// Validate `pipeline` and register a snapshot; the new ID goes to `*id`.
///
/// # Safety
///
/// `processor` and `pipeline` are live; `id` is writable; `error` follows the
/// error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_register(
    processor: *mut TfProcessor,
    pipeline: *const TfPipeline,
    id: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        text_result(processor, id, error, |processor| {
            ffi::handle(pipeline, "pipeline")?.with_plan(|plan| Ok(processor.register(plan)?))
        })
    }
}

/// Register a pipeline made of copies of `count` instructions.
///
/// # Safety
///
/// `processor` is live; `instructions` points to `count` live instruction
/// pointers (or is NULL with `count == 0`); `id` is writable; `error` follows
/// the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_add_transform(
    processor: *mut TfProcessor,
    instructions: *const *const TfInstruction,
    count: usize,
    id: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        text_result(processor, id, error, |processor| {
            let specs = ffi::array(instructions, count, "instructions")?
                .iter()
                .map(|&item| Ok(ffi::handle(item, "instruction")?.spec.clone()))
                .collect::<Result<Vec<_>, Failure>>()?;
            Ok(processor.add_transform(specs)?)
        })
    }
}

/// Remove a registered pipeline. Compiled executables are unaffected.
///
/// # Safety
///
/// `processor` is live; `id`/`length` follow the text convention; `error`
/// follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_remove_transform(
    processor: *mut TfProcessor,
    id: *const c_char,
    length: usize,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        unit_result(processor, error, |processor| {
            Ok(processor.remove_transform(self::id(id, length)?)?)
        })
    }
}

/// Store in `*exists` whether pipeline `id` is registered.
///
/// # Safety
///
/// `processor` is live; `id`/`length` follow the text convention; `exists`
/// is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_transform_exists(
    processor: *const TfProcessor,
    id: *const c_char,
    length: usize,
    exists: *mut bool,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let exists = Out::new(exists, "exists")?;
            let processor = borrow(processor)?.inner.lock()?;
            exists.write(processor.transform_exists(self::id(id, length)?));
            Ok(())
        })
    }
}

/// All registered IDs, sorted. Release with `tf_string_list_free`.
///
/// # Safety
///
/// `processor` is live; `ids` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_list_transforms(
    processor: *const TfProcessor,
    ids: *mut TfStringList,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let ids = Out::new(ids, "ids")?;
            ids.write(TfStringList::from(borrow(processor)?.inner.lock()?.list_transforms()));
            Ok(())
        })
    }
}

/// Print every registered pipeline to standard output (core format).
///
/// # Safety
///
/// `processor` is live; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_show_transforms(
    processor: *const TfProcessor,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        unit_result(processor, error, |processor| {
            processor.show_transforms();
            Ok(())
        })
    }
}

/// Independent, editable copies of a pipeline's instructions.
///
/// # Safety
///
/// `processor` is live; `id`/`length` follow the text convention;
/// `instructions` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_get_transform_vec(
    processor: *const TfProcessor,
    id: *const c_char,
    length: usize,
    instructions: *mut TfInstructionList,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let instructions = Out::new(instructions, "instructions")?;
            let specs = borrow(processor)?.inner.lock()?.get_transform_vec(self::id(id, length)?)?;
            let items = specs
                .into_iter()
                .map(TfInstruction::from_spec)
                .collect::<Result<Vec<_>, _>>()?;
            instructions.write(TfInstructionList::from(items));
            Ok(())
        })
    }
}

/// Canonical textual DSL form of each instruction of a pipeline.
///
/// # Safety
///
/// `processor` is live; `id`/`length` follow the text convention; `lines` is
/// writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_get_text_transform_vec(
    processor: *const TfProcessor,
    id: *const c_char,
    length: usize,
    lines: *mut TfStringList,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let lines = Out::new(lines, "lines")?;
            let text = borrow(processor)?.inner.lock()?.get_text_transform_vec(self::id(id, length)?)?;
            lines.write(TfStringList::from(text));
            Ok(())
        })
    }
}

/// Register a copy of pipeline `id`; the copy's ID goes to `*copy`.
///
/// # Safety
///
/// `processor` is live; `id`/`length` follow the text convention; `copy` is
/// writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_clone_pipeline(
    processor: *mut TfProcessor,
    id: *const c_char,
    length: usize,
    copy: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        text_result(processor, copy, error, |processor| {
            Ok(processor.clone_pipeline(self::id(id, length)?)?)
        })
    }
}

/// Import a textual pipeline file (`.textforge` / `.tft`) and register it.
///
/// # Safety
///
/// `processor` and `path` are live; `id` is writable; `error` follows the
/// error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_get_pipeline_from_file(
    processor: *mut TfProcessor,
    path: *const TfFilePath,
    id: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        text_result(processor, id, error, |processor| {
            Ok(processor.get_pipeline_from_file(path::borrow(path, "path")?)?)
        })
    }
}

/// Export pipeline `id` as textual DSL, creating missing parent directories.
///
/// Values that the text format cannot represent (literal line breaks, some
/// variable names) fail before the destination is touched; use bytecode.
///
/// # Safety
///
/// `processor` and `path` are live; `id`/`length` follow the text convention;
/// `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_write_pipeline_to_file(
    processor: *mut TfProcessor,
    id: *const c_char,
    length: usize,
    path: *const TfFilePath,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        unit_result(processor, error, |processor| {
            Ok(processor.write_pipeline_to_file(self::id(id, length)?, path::borrow(path, "path")?)?)
        })
    }
}

/// Import a big-endian bytecode file (`.textforgebc` / `.tfb`) and register it.
///
/// # Safety
///
/// `processor` and `path` are live; `id` is writable; `error` follows the
/// error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_read_from_bytecode_file(
    processor: *mut TfProcessor,
    path: *const TfFilePath,
    id: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        text_result(processor, id, error, |processor| {
            Ok(processor.read_from_bytecode_file(path::borrow(path, "path")?)?)
        })
    }
}

/// Export pipeline `id` as bytecode (lossless), creating missing parent
/// directories.
///
/// # Safety
///
/// `processor` and `path` are live; `id`/`length` follow the text convention;
/// `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_write_to_bytecode_file(
    processor: *mut TfProcessor,
    id: *const c_char,
    length: usize,
    path: *const TfFilePath,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        unit_result(processor, error, |processor| {
            Ok(processor.write_to_bytecode_file(self::id(id, length)?, path::borrow(path, "path")?)?)
        })
    }
}

/// Execute pipeline `id` without the debug or watcher machinery.
///
/// # Safety
///
/// `processor` is live; `id`/`id_length` and `input`/`input_length` follow the
/// text convention; `out` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_run_transform(
    processor: *const TfProcessor,
    id: *const c_char,
    id_length: usize,
    input: *const c_char,
    input_length: usize,
    out: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        text_result(processor, out, error, |processor| {
            Ok(processor.run_transform(self::id(id, id_length)?, ffi::text(input, input_length, "input")?)?)
        })
    }
}

/// Execute pipeline `id` on `input` (Rust `process_all`).
///
/// # Safety
///
/// As `tf_processor_run_transform`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_all(
    processor: *mut TfProcessor,
    id: *const c_char,
    id_length: usize,
    input: *const c_char,
    input_length: usize,
    out: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        text_result(processor, out, error, |processor| {
            Ok(processor.process_all(self::id(id, id_length)?, ffi::text(input, input_length, "input")?)?)
        })
    }
}

/// Like `tf_processor_process_all`, printing every step to standard output.
///
/// # Safety
///
/// As `tf_processor_run_transform`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_all_with_debug(
    processor: *mut TfProcessor,
    id: *const c_char,
    id_length: usize,
    input: *const c_char,
    input_length: usize,
    out: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        text_result(processor, out, error, |processor| {
            Ok(processor.process_all_with_debug(
                self::id(id, id_length)?,
                ffi::text(input, input_length, "input")?,
            )?)
        })
    }
}

/// Execute a single instruction on `input` (Rust `process_single`).
///
/// # Safety
///
/// `processor` and `instruction` are live; `input`/`length` follow the text
/// convention; `out` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_single(
    processor: *mut TfProcessor,
    instruction: *const TfInstruction,
    input: *const c_char,
    length: usize,
    out: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        text_result(processor, out, error, |processor| {
            let spec = ffi::handle(instruction, "instruction")?.spec.clone();
            Ok(processor.process_single(spec, ffi::text(input, length, "input")?)?)
        })
    }
}

/// Like `tf_processor_process_single`, printing processing details to standard output.
///
/// # Safety
///
/// As `tf_processor_process_single`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_single_with_debug(
    processor: *mut TfProcessor,
    instruction: *const TfInstruction,
    input: *const c_char,
    length: usize,
    out: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        text_result(processor, out, error, |processor| {
            let spec = ffi::handle(instruction, "instruction")?.spec.clone();
            Ok(processor.process_single_with_debug(spec, ffi::text(input, length, "input")?)?)
        })
    }
}

/// Read `input`, run pipeline `id` and write UTF-8 text to `output`.
///
/// # Safety
///
/// `processor`, `input` and `output` are live; `id`/`length` follow the text
/// convention; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_file(
    processor: *mut TfProcessor,
    id: *const c_char,
    length: usize,
    input: *const TfFilePath,
    output: *const TfFilePath,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        unit_result(processor, error, |processor| {
            Ok(processor.process_file(
                self::id(id, length)?,
                path::borrow(input, "input")?,
                path::borrow(output, "output")?,
            )?)
        })
    }
}

/// One file job for `tf_processor_process_batch`. All pointers are borrowed
/// for the duration of the call. Use distinct output paths.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TfFileTask {
    /// File to read.
    pub input: *const TfFilePath,
    /// Registered pipeline ID (UTF-8).
    pub pipeline_id: *const c_char,
    /// Byte length of `pipeline_id`, or `TF_NTS`.
    pub pipeline_id_length: usize,
    /// File to write.
    pub output: *const TfFilePath,
}

/// Convert C tasks into portable tasks.
///
/// # Safety
///
/// `tasks` points to `count` tasks whose pointers are valid.
unsafe fn file_tasks(tasks: *const TfFileTask, count: usize) -> Result<Vec<FileTask>, Failure> {
    // SAFETY: forwarded caller contract.
    unsafe { ffi::array(tasks, count, "tasks") }?
        .iter()
        .map(|task| {
            // SAFETY: each task's pointers are valid by contract.
            unsafe {
                Ok(FileTask {
                    input: path::borrow(task.input, "task input")?.clone(),
                    pipeline_id: id(task.pipeline_id, task.pipeline_id_length)?.into(),
                    output: path::borrow(task.output, "task output")?.clone(),
                })
            }
        })
        .collect()
}

/// Store per-task results: `NULL` for success, otherwise an owned error.
///
/// # Safety
///
/// `results` is valid for writing `outcomes.len()` pointers.
unsafe fn store_outcomes(results: *mut *mut TfError, outcomes: Vec<Result<(), textforge::api::portable::BindingError>>) {
    for (index, outcome) in outcomes.into_iter().enumerate() {
        let value = match outcome {
            Ok(()) => ptr::null_mut(),
            Err(error) => ffi::into_handle(TfError::from(Failure::from(error))),
        };
        // SAFETY: `index < count` and the array is writable by contract.
        unsafe { results.add(index).write(value) };
    }
}

/// Process files in parallel on the global worker pool.
///
/// `results` must have room for `count` pointers; every entry is overwritten
/// with NULL (task succeeded) or an owned error to release with
/// `tf_error_free`. The function itself fails only for invalid arguments, in
/// which case `results` is untouched. Tasks may use different pipeline IDs.
///
/// # Safety
///
/// `processor` is live; `tasks` points to `count` valid tasks; `results` is
/// writable for `count` pointers; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_batch(
    processor: *const TfProcessor,
    tasks: *const TfFileTask,
    count: usize,
    results: *mut *mut TfError,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        unit_result(processor, error, |processor| {
            if count > 0 && results.is_null() {
                return Err(Failure::null("results"));
            }
            let outcomes = processor.process_batch(file_tasks(tasks, count)?);
            store_outcomes(results, outcomes);
            Ok(())
        })
    }
}

/// Reusable worker pool for `tf_processor_process_batch_in_pool`.
///
/// ```c
/// TfBatchPool *pool = NULL;
/// assert(tf_batch_pool_new(2, &pool, NULL) == TF_STATUS_OK);
/// tf_batch_pool_free(pool);
/// ```
pub struct TfBatchPool {
    inner: BatchPool,
}

/// Create a pool with `threads` workers (0 selects automatic sizing).
///
/// # Safety
///
/// `out` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_batch_pool_new(
    threads: u64,
    out: *mut *mut TfBatchPool,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            out.write(ffi::into_handle(TfBatchPool {
                inner: BatchPool::new(threads)?,
            }));
            Ok(())
        })
    }
}

/// Release a pool, waiting for its idle workers to exit. `NULL` is ignored.
///
/// # Safety
///
/// `pool` is NULL or live and not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_batch_pool_free(pool: *mut TfBatchPool) {
    // SAFETY: forwarded caller contract.
    ffi::guard((), || drop(unsafe { ffi::take_handle(pool) }));
}

/// Like `tf_processor_process_batch`, on a caller-supplied pool.
///
/// # Safety
///
/// As `tf_processor_process_batch`; `pool` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_batch_in_pool(
    processor: *const TfProcessor,
    pool: *const TfBatchPool,
    tasks: *const TfFileTask,
    count: usize,
    results: *mut *mut TfError,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        unit_result(processor, error, |processor| {
            if count > 0 && results.is_null() {
                return Err(Failure::null("results"));
            }
            let pool = &ffi::handle(pool, "pool")?.inner;
            let outcomes = processor.process_batch_in_pool(pool, file_tasks(tasks, count)?);
            store_outcomes(results, outcomes);
            Ok(())
        })
    }
}

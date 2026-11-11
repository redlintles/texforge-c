//! Fluent pipeline construction and compiled executables.
//!
//! Builder functions return the pipeline they received so calls can be written
//! one after another without checking each result. The first failure is kept
//! as a *sticky error*: later builder calls on that pipeline do nothing, and
//! `tf_pipeline_compile`, `tf_pipeline_build` or `tf_pipeline_status` report it.
//! A failed call never changes the plan itself.

use crate::{
    error::{Failure, TfError, TfStatus},
    ffi::{self, Guarded, Out},
    instruction::{self, TfArg, TfInstruction},
    processor::TfProcessor,
    text::TfString,
};
use std::{ffi::c_char, ptr};
use textforge::api::portable::{Argument, CompiledPipeline, InstructionSpec, PipelinePlan};

/// Mutable pipeline plan under construction (portable `PipelinePlan`).
///
/// Create with `tf_pipeline_new` or `tf_processor_create_pipeline`, release
/// with `tf_pipeline_free`. A plan may be compiled or registered any number of
/// times; executables already created stay valid after the plan is freed.
/// Calls on one plan from several threads are serialized.
///
/// ```c
/// TfPipeline *plan = tf_pipeline_new();
/// tf_pipeline_delete_first(plan);
/// tf_pipeline_add_to_beginning(plan, tf_text("banana "));
/// tf_pipeline_delete_last(plan);
/// TfCompiledPipeline *pipeline = NULL;
/// TfError *error = NULL;
/// if (tf_pipeline_compile(plan, &pipeline, &error) != TF_STATUS_OK) {
///     fprintf(stderr, "%s\n", tf_error_message(error, NULL));
///     tf_error_free(error);
///     abort();
/// }
/// TfString out = {0};
/// assert(tf_compiled_pipeline_process(pipeline, "xfruitx", TF_NTS, &out, NULL) == TF_STATUS_OK);
/// assert(strcmp(out.data, "banana fruit") == 0);
/// tf_string_free(&out);
/// tf_compiled_pipeline_free(pipeline);
/// tf_pipeline_free(plan);
/// ```
pub struct TfPipeline {
    state: Guarded<PipelineState>,
}

#[derive(Default)]
struct PipelineState {
    plan: PipelinePlan,
    error: Option<Failure>,
}

impl TfPipeline {
    fn new() -> Self {
        Self {
            state: Guarded::new(PipelineState::default()),
        }
    }

    /// Validate structure and lower the plan, honoring the sticky error.
    pub(crate) fn with_plan<T>(
        &self,
        use_plan: impl FnOnce(&PipelinePlan) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let state = self.state.lock()?;
        if let Some(error) = &state.error {
            return Err(error.clone());
        }
        use_plan(&state.plan)
    }
}

/// Apply one builder step to `pipeline` and return it.
///
/// Returns NULL only when `pipeline` is NULL or unusable after a panic; every
/// other failure is recorded as the sticky error.
///
/// # Safety
///
/// `pipeline` is NULL or a live pipeline. The step may dereference pointers the
/// caller supplied and must uphold their contracts.
pub(crate) unsafe fn step(
    pipeline: *mut TfPipeline,
    apply: impl FnOnce(&mut PipelinePlan) -> Result<(), Failure>,
) -> *mut TfPipeline {
    ffi::guard(ptr::null_mut(), || {
        // SAFETY: forwarded caller contract.
        let Some(handle) = (unsafe { pipeline.as_ref() }) else {
            return ptr::null_mut();
        };
        let Ok(mut state) = handle.state.lock() else {
            return ptr::null_mut();
        };
        if state.error.is_none()
            && let Err(failure) = apply(&mut state.plan)
        {
            state.error = Some(failure);
        }
        pipeline
    })
}

/// Append instruction `mnemonic` with C arguments (used by generated builders).
///
/// # Safety
///
/// As [`step`]; `arguments` obey the [`TfArg`] contract.
pub(crate) unsafe fn append(
    pipeline: *mut TfPipeline,
    mnemonic: &str,
    arguments: &[TfArg],
) -> *mut TfPipeline {
    // SAFETY: forwarded caller contract.
    unsafe {
        step(pipeline, |plan| {
            let arguments = arguments
                .iter()
                .enumerate()
                .map(|(index, argument)| {
                    instruction::convert(argument, &format!("{mnemonic} argument {index}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(plan.add(InstructionSpec::new(mnemonic.into(), arguments))?)
        })
    }
}

/// Take ownership of a consumed body pipeline, rejecting self-insertion.
///
/// The body is released here on every path except self-insertion, so a
/// pipeline that already has a sticky error does not leak it.
///
/// # Safety
///
/// `body` is NULL or a live pipeline that the caller gives up.
unsafe fn take_body(
    pipeline: *mut TfPipeline,
    body: *mut TfPipeline,
) -> Result<PipelinePlan, Failure> {
    if ptr::eq(pipeline, body) && !body.is_null() {
        return Err(Failure::invalid("a pipeline cannot be its own body"));
    }
    // SAFETY: distinct from `pipeline` and given up by the caller.
    let body = unsafe { ffi::take_handle(body) }.ok_or_else(|| Failure::null("body"))?;
    let state = body.state.into_inner()?;
    match state.error {
        Some(error) => Err(error),
        None => Ok(state.plan),
    }
}

/// Run a consuming group step (`body` is always released, see [`take_body`]).
///
/// # Safety
///
/// `pipeline` is NULL or live; `body` is NULL or a live pipeline the caller gives up.
unsafe fn group(
    pipeline: *mut TfPipeline,
    body: *mut TfPipeline,
    apply: impl FnOnce(&mut PipelinePlan, PipelinePlan) -> Result<(), Failure>,
) -> *mut TfPipeline {
    // SAFETY: forwarded caller contract.
    let body = ffi::guard(Err(Failure::panic()), || unsafe { take_body(pipeline, body) });
    // SAFETY: forwarded caller contract.
    unsafe { step(pipeline, |plan| apply(plan, body?)) }
}

/// Create an empty pipeline. Never returns NULL.
#[unsafe(no_mangle)]
pub extern "C" fn tf_pipeline_new() -> *mut TfPipeline {
    ffi::into_handle(TfPipeline::new())
}

/// Release a pipeline. `NULL` is ignored. Compiled pipelines and registered
/// IDs created from it remain valid.
///
/// # Safety
///
/// `pipeline` is NULL or a live pipeline not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_free(pipeline: *mut TfPipeline) {
    // SAFETY: forwarded caller contract.
    ffi::guard((), || drop(unsafe { ffi::take_handle(pipeline) }));
}

/// Append any registered instruction by mnemonic, e.g. `"atb"`.
///
/// Typed functions such as `tf_pipeline_add_to_beginning` are usually more
/// convenient; this generic form accepts every mnemonic the core registers.
///
/// # Safety
///
/// `pipeline` is NULL or live; `name`/`length` follow the text convention;
/// `arguments` points to `count` valid arguments (or NULL with `count == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_add(
    pipeline: *mut TfPipeline,
    name: *const c_char,
    length: usize,
    arguments: *const TfArg,
    count: usize,
) -> *mut TfPipeline {
    // SAFETY: forwarded caller contract.
    unsafe {
        step(pipeline, |plan| {
            let name = ffi::text(name, length, "name")?;
            let arguments = instruction::convert_all(arguments, count)?;
            Ok(plan.add(InstructionSpec::new(name.into(), arguments))?)
        })
    }
}

/// Append a copy of an existing instruction (Rust `push_token`).
///
/// # Safety
///
/// `pipeline` is NULL or live; `instruction` is NULL or a live instruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_push_token(
    pipeline: *mut TfPipeline,
    instruction: *const TfInstruction,
) -> *mut TfPipeline {
    // SAFETY: forwarded caller contract.
    unsafe {
        step(pipeline, |plan| {
            let instruction = ffi::handle(instruction, "instruction")?;
            Ok(plan.add(instruction.spec.clone())?)
        })
    }
}

/// Define reusable block `name` whose instructions are `body` (Rust `block_assoc`).
///
/// **Consumes `body`**: it is released by this call on every path, including
/// failures, and must not be used again. The only exception is passing the
/// pipeline itself as its own body, which is recorded as an error.
///
/// ```c
/// TfPipeline *suffix = tf_pipeline_add_to_end(tf_pipeline_new(), tf_text("!"));
/// TfPipeline *plan = tf_pipeline_block(tf_pipeline_new(), "shout", TF_NTS, suffix);
/// tf_pipeline_call_block(plan, "shout", TF_NTS);
/// tf_pipeline_call_block(plan, "shout", TF_NTS);
/// TfCompiledPipeline *pipeline = NULL;
/// assert(tf_pipeline_compile(plan, &pipeline, NULL) == TF_STATUS_OK);
/// TfString out = {0};
/// assert(tf_compiled_pipeline_process(pipeline, "hey", TF_NTS, &out, NULL) == TF_STATUS_OK);
/// assert(strcmp(out.data, "hey!!") == 0);
/// tf_string_free(&out);
/// tf_compiled_pipeline_free(pipeline);
/// tf_pipeline_free(plan);
/// ```
///
/// # Safety
///
/// `pipeline` is NULL or live; `name`/`length` follow the text convention;
/// `body` is NULL or a live pipeline the caller gives up.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_block(
    pipeline: *mut TfPipeline,
    name: *const c_char,
    length: usize,
    body: *mut TfPipeline,
) -> *mut TfPipeline {
    // SAFETY: forwarded caller contract.
    unsafe {
        group(pipeline, body, |plan, body| {
            let name = ffi::text(name, length, "name")?;
            plan.block(name.into(), body);
            Ok(())
        })
    }
}

/// Alias of `tf_pipeline_block` named after Rust `TextForgeBlockMethods::block_assoc`.
///
/// # Safety
///
/// As `tf_pipeline_block`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_block_assoc(
    pipeline: *mut TfPipeline,
    name: *const c_char,
    length: usize,
    body: *mut TfPipeline,
) -> *mut TfPipeline {
    // SAFETY: forwarded caller contract.
    unsafe { tf_pipeline_block(pipeline, name, length, body) }
}

/// Run each instruction of `body` only while the text contains `pattern`
/// (Rust `if_do_contains_each`). The condition is re-evaluated before every
/// child. **Consumes `body`** exactly like `tf_pipeline_block`.
///
/// ```c
/// TfPipeline *body = tf_pipeline_new();
/// tf_pipeline_remove_pattern(body, tf_text("x"));
/// tf_pipeline_add_to_end(body, tf_text("skipped"));
/// TfPipeline *plan = tf_pipeline_contains_each(tf_pipeline_new(), "x", TF_NTS, body);
/// TfCompiledPipeline *pipeline = NULL;
/// assert(tf_pipeline_compile(plan, &pipeline, NULL) == TF_STATUS_OK);
/// TfString out = {0};
/// assert(tf_compiled_pipeline_process(pipeline, "x", TF_NTS, &out, NULL) == TF_STATUS_OK);
/// assert(out.length == 0);  // "x" is removed, so the second child is skipped.
/// tf_string_free(&out);
/// tf_compiled_pipeline_free(pipeline);
/// tf_pipeline_free(plan);
/// ```
///
/// # Safety
///
/// As `tf_pipeline_block`, with `pattern`/`length` as the text.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_contains_each(
    pipeline: *mut TfPipeline,
    pattern: *const c_char,
    length: usize,
    body: *mut TfPipeline,
) -> *mut TfPipeline {
    // SAFETY: forwarded caller contract.
    unsafe {
        group(pipeline, body, |plan, body| {
            let pattern = ffi::text(pattern, length, "pattern")?;
            plan.contains_each(pattern.into(), body);
            Ok(())
        })
    }
}

/// Alias of `tf_pipeline_contains_each` named after Rust
/// `TextForgeConditionalMethods::if_do_contains_each`.
///
/// # Safety
///
/// As `tf_pipeline_contains_each`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_if_do_contains_each(
    pipeline: *mut TfPipeline,
    pattern: *const c_char,
    length: usize,
    body: *mut TfPipeline,
) -> *mut TfPipeline {
    // SAFETY: forwarded caller contract.
    unsafe { tf_pipeline_contains_each(pipeline, pattern, length, body) }
}

/// Invoke block `name` (Rust `TextForgeBlockMethods::call_block`).
///
/// # Safety
///
/// `pipeline` is NULL or live; `name`/`length` follow the text convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_call_block(
    pipeline: *mut TfPipeline,
    name: *const c_char,
    length: usize,
) -> *mut TfPipeline {
    // SAFETY: forwarded caller contract.
    unsafe {
        step(pipeline, |plan| {
            let name = ffi::text(name, length, "name")?;
            Ok(plan.call_block(name.into())?)
        })
    }
}

/// Ordered regex/replacement rules for `tf_pipeline_define_replace_map`
/// (Rust `ReplaceMapBuilder`). Like pipelines, `add_replace` returns the
/// builder and keeps the first failure as a sticky error.
///
/// ```c
/// TfReplaceMapBuilder *map = tf_replace_map_builder_new();
/// tf_replace_map_builder_add_replace(map, tf_text("cat"), tf_text("wolf"));
/// tf_replace_map_builder_add_replace(map, tf_text("wolf"), tf_text("dog"));
/// TfPipeline *plan = tf_pipeline_define_replace_map(tf_pipeline_new(), tf_text("animals"), map);
/// tf_pipeline_execute_replace_map(plan, tf_text("animals"));
/// TfCompiledPipeline *pipeline = NULL;
/// assert(tf_pipeline_compile(plan, &pipeline, NULL) == TF_STATUS_OK);
/// TfString out = {0};
/// assert(tf_compiled_pipeline_process(pipeline, "cat wolf", TF_NTS, &out, NULL) == TF_STATUS_OK);
/// assert(strcmp(out.data, "wolf dog") == 0);
/// tf_string_free(&out);
/// tf_compiled_pipeline_free(pipeline);
/// tf_pipeline_free(plan);
/// ```
pub struct TfReplaceMapBuilder {
    state: Guarded<MapState>,
}

#[derive(Default)]
struct MapState {
    rules: Vec<(Argument, Argument)>,
    error: Option<Failure>,
}

/// Create an empty rule list. Never returns NULL.
#[unsafe(no_mangle)]
pub extern "C" fn tf_replace_map_builder_new() -> *mut TfReplaceMapBuilder {
    ffi::into_handle(TfReplaceMapBuilder {
        state: Guarded::new(MapState::default()),
    })
}

/// Append one rule (text or variable arguments) and return `map`.
///
/// Returns NULL only for a NULL or unusable `map`.
///
/// # Safety
///
/// `map` is NULL or live; both arguments obey the [`TfArg`] contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_replace_map_builder_add_replace(
    map: *mut TfReplaceMapBuilder,
    pattern: TfArg,
    replacement: TfArg,
) -> *mut TfReplaceMapBuilder {
    ffi::guard(ptr::null_mut(), || {
        // SAFETY: forwarded caller contract.
        let Some(handle) = (unsafe { map.as_ref() }) else {
            return ptr::null_mut();
        };
        let Ok(mut state) = handle.state.lock() else {
            return ptr::null_mut();
        };
        if state.error.is_none() {
            // SAFETY: forwarded TfArg contract.
            let rule = unsafe {
                instruction::convert(&pattern, "pattern").and_then(|pattern| {
                    Ok((pattern, instruction::convert(&replacement, "replacement")?))
                })
            };
            match rule {
                Ok(rule) => state.rules.push(rule),
                Err(failure) => state.error = Some(failure),
            }
        }
        map
    })
}

/// Release an unused rule list. `NULL` is ignored.
///
/// # Safety
///
/// `map` is NULL or a live builder not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_replace_map_builder_free(map: *mut TfReplaceMapBuilder) {
    // SAFETY: forwarded caller contract.
    ffi::guard((), || drop(unsafe { ffi::take_handle(map) }));
}

/// Define replacement map `name` from the rules in `map`
/// (Rust `define_replace_map(name, |map| ...)`). **Consumes `map`** on every path.
///
/// An empty map, or a map with a sticky error, becomes the pipeline's error.
///
/// # Safety
///
/// `pipeline` is NULL or live; `name` obeys the [`TfArg`] contract; `map` is
/// NULL or a live builder the caller gives up.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_define_replace_map(
    pipeline: *mut TfPipeline,
    name: TfArg,
    map: *mut TfReplaceMapBuilder,
) -> *mut TfPipeline {
    // SAFETY: the caller gave up `map`; it is released when this box drops.
    let map = unsafe { ffi::take_handle(map) };
    // SAFETY: forwarded caller contract.
    unsafe {
        step(pipeline, |plan| {
            let state = map.ok_or_else(|| Failure::null("map"))?.state.into_inner()?;
            if let Some(error) = state.error {
                return Err(error);
            }
            let name = instruction::convert(&name, "name")?;
            Ok(plan.define_replace_map(name, state.rules)?)
        })
    }
}

/// Report the sticky error, if any, without clearing it.
///
/// Returns `TF_STATUS_OK` when no builder call has failed; otherwise returns the
/// failure status and stores a copy of the error in `*error` (when not NULL).
///
/// # Safety
///
/// `pipeline` is a live pipeline; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_status(
    pipeline: *const TfPipeline,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            ffi::handle(pipeline, "pipeline")?.with_plan(|_| Ok(()))
        })
    }
}

/// Forget the sticky error so construction can continue. The plan is
/// unchanged: the failed call was never applied.
///
/// # Safety
///
/// `pipeline` is NULL or a live pipeline.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_clear_error(pipeline: *mut TfPipeline) {
    ffi::guard((), || {
        // SAFETY: forwarded caller contract.
        if let Some(handle) = unsafe { pipeline.as_ref() }
            && let Ok(mut state) = handle.state.lock()
        {
            state.error = None;
        }
    });
}

/// Validate the plan and create an independent executable in `*out`.
///
/// Fails with the sticky error, or with `TF_STATUS_VALIDATION` for invalid
/// arity, types or hierarchy. The plan is unchanged and can be compiled again.
///
/// # Safety
///
/// `pipeline` is live; `out` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_compile(
    pipeline: *const TfPipeline,
    out: *mut *mut TfCompiledPipeline,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let compiled = ffi::handle(pipeline, "pipeline")?.with_plan(|plan| Ok(plan.compile()?))?;
            out.write(ffi::into_handle(TfCompiledPipeline { inner: compiled }));
            Ok(())
        })
    }
}

/// Register a snapshot of the plan in `processor` and return its new ID in
/// `*id` (Rust `TextForgeBuilder::build`). Release the ID with `tf_string_free`.
///
/// # Safety
///
/// `pipeline` and `processor` are live; `id` is writable; `error` follows the
/// error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_pipeline_build(
    pipeline: *const TfPipeline,
    processor: *mut TfProcessor,
    id: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe { crate::processor::tf_processor_register(processor, pipeline, id, error) }
}

/// Immutable, reusable executable produced by `tf_pipeline_compile`.
///
/// Each `tf_compiled_pipeline_process` call uses a fresh execution context, so
/// one executable may be used from several threads at the same time.
///
/// ```c
/// TfPipeline *plan = tf_pipeline_repeat(tf_pipeline_new(), tf_number(2));
/// TfCompiledPipeline *pipeline = NULL;
/// assert(tf_pipeline_compile(plan, &pipeline, NULL) == TF_STATUS_OK);
/// tf_pipeline_free(plan);  // The executable is independent of its plan.
/// const char input[] = {'a', '\0', 'b'};  // Embedded NUL is preserved.
/// TfString out = {0};
/// assert(tf_compiled_pipeline_process(pipeline, input, sizeof input, &out, NULL) == TF_STATUS_OK);
/// assert(out.length == 6 && memcmp(out.data, "a\0ba\0b", 6) == 0);
/// tf_string_free(&out);
/// tf_compiled_pipeline_free(pipeline);
/// ```
pub struct TfCompiledPipeline {
    inner: CompiledPipeline,
}

/// Run the executable on UTF-8 `input` and store the result in `*out`.
///
/// # Safety
///
/// `pipeline` is live; `input`/`length` follow the text convention; `out` is
/// writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_compiled_pipeline_process(
    pipeline: *const TfCompiledPipeline,
    input: *const c_char,
    length: usize,
    out: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let pipeline = ffi::handle(pipeline, "pipeline")?;
            let input = ffi::text(input, length, "input")?;
            out.write(TfString::from(pipeline.inner.process(input)?));
            Ok(())
        })
    }
}

/// Release an executable. `NULL` is ignored.
///
/// # Safety
///
/// `pipeline` is NULL or live and not used afterwards (by any thread).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_compiled_pipeline_free(pipeline: *mut TfCompiledPipeline) {
    // SAFETY: forwarded caller contract.
    ffi::guard((), || drop(unsafe { ffi::take_handle(pipeline) }));
}

//! Watchers: per-instruction observations collected into a JSON report.
//!
//! Watchers add overhead and are meant for debugging feedback, not for
//! production runs (use `tf_processor_process_all` there).

use crate::{
    error::{Failure, TfError, TfStatus},
    ffi::{self, Guarded, Out},
    path::{self, TfFilePath},
    processor::{self, TfProcessor},
    text::{CText, TfString, TfStringList},
};
use std::ffi::{c_char, c_int, c_void};
use textforge::api::portable::{Observation, Observer, Observers};

/// Snapshot passed to a watcher callback; valid only during the callback.
pub struct TfObservation {
    current: CText,
    before: CText,
    after: Option<CText>,
    instruction: CText,
}

/// Collects a watcher callback's answer; valid only during the callback.
pub struct TfObserverResult {
    value: Option<Result<String, String>>,
}

/// Watcher callback.
///
/// Inspect `observation`, then call `tf_observer_result_set` with the metric
/// text and return 0. Return non-zero (optionally after
/// `tf_observer_result_set_error`) to fail the run with `TF_STATUS_CALLBACK`.
/// The callback must return normally: `longjmp` or C++ exceptions out of it are
/// undefined behavior. It may use other TextForge objects, but calling back into
/// the running processor or these observers fails with `TF_STATUS_BUSY`.
pub type TfObserverFn = Option<
    unsafe extern "C" fn(
        observation: *const TfObservation,
        result: *mut TfObserverResult,
        user_data: *mut c_void,
    ) -> c_int,
>;

/// Releases callback `user_data` when the registration is dropped. May be NULL.
pub type TfDestroyFn = Option<unsafe extern "C" fn(user_data: *mut c_void)>;

/// A registered C callback; owns `user_data` through `destroy`.
struct CObserver {
    callback: unsafe extern "C" fn(*const TfObservation, *mut TfObserverResult, *mut c_void) -> c_int,
    user_data: *mut c_void,
    destroy: TfDestroyFn,
}

impl Drop for CObserver {
    fn drop(&mut self) {
        if let Some(destroy) = self.destroy {
            // SAFETY: registration contract: `destroy` accepts this `user_data` once.
            unsafe { destroy(self.user_data) };
        }
    }
}

impl Observer for CObserver {
    fn observe(&self, context: Observation) -> Result<String, String> {
        let observation = TfObservation {
            current: CText::from(context.current),
            before: CText::from(context.before),
            after: context.after.map(CText::from),
            instruction: CText::from(context.instruction),
        };
        let mut result = TfObserverResult { value: None };
        // SAFETY: registration contract; both pointers outlive the call.
        let status = unsafe { (self.callback)(&observation, &mut result, self.user_data) };
        match (status, result.value) {
            (0, Some(Ok(text))) => Ok(text),
            (0, Some(Err(message))) => Err(message),
            (0, None) => Err("watcher callback returned without a result".into()),
            (_, Some(Err(message))) => Err(message),
            (code, _) => Err(format!("watcher callback failed with status {code}")),
        }
    }
}

/// Text at the moment the watcher runs (NUL-terminated UTF-8).
///
/// # Safety
///
/// `observation` is NULL or the live callback argument; `length` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observation_current(observation: *const TfObservation, length: *mut usize) -> *const c_char {
    // SAFETY: forwarded caller contract.
    match unsafe { observation.as_ref() } {
        Some(observation) => observation.current.export(length),
        None => CText::null(length),
    }
}

/// Text before the instruction ran.
///
/// # Safety
///
/// `observation` is NULL or the live callback argument; `length` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observation_before(observation: *const TfObservation, length: *mut usize) -> *const c_char {
    // SAFETY: forwarded caller contract.
    match unsafe { observation.as_ref() } {
        Some(observation) => observation.before.export(length),
        None => CText::null(length),
    }
}

/// Mnemonic of the instruction just executed.
///
/// # Safety
///
/// `observation` is NULL or the live callback argument; `length` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observation_instruction(observation: *const TfObservation, length: *mut usize) -> *const c_char {
    // SAFETY: forwarded caller contract.
    match unsafe { observation.as_ref() } {
        Some(observation) => observation.instruction.export(length),
        None => CText::null(length),
    }
}

/// Text after the instruction ran, or NULL when not available.
///
/// # Safety
///
/// `observation` is NULL or the live callback argument; `length` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observation_after(
    observation: *const TfObservation,
    length: *mut usize,
) -> *const c_char {
    // SAFETY: forwarded caller contract.
    match unsafe { observation.as_ref() }.and_then(|o| o.after.as_ref()) {
        Some(after) => after.export(length),
        None => CText::null(length),
    }
}

/// Store the watcher's metric text (copied; UTF-8). The last call wins.
///
/// # Safety
///
/// `result` is the live callback argument; `text`/`length` follow the text convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observer_result_set(
    result: *mut TfObserverResult,
    text: *const c_char,
    length: usize,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe { set_result(result, text, length, Ok) }
}

/// Store a failure message (copied; UTF-8) reported as the callback error.
///
/// # Safety
///
/// `result` is the live callback argument; `message`/`length` follow the text convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observer_result_set_error(
    result: *mut TfObserverResult,
    message: *const c_char,
    length: usize,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe { set_result(result, message, length, Err) }
}

/// Shared implementation of the two result setters.
///
/// # Safety
///
/// As the setters.
unsafe fn set_result(
    result: *mut TfObserverResult,
    text: *const c_char,
    length: usize,
    wrap: fn(String) -> Result<String, String>,
) -> TfStatus {
    ffi::guard(TfStatus::Panic, || {
        // SAFETY: forwarded caller contract.
        let Some(result) = (unsafe { result.as_mut() }) else {
            return TfStatus::InvalidArgument;
        };
        // SAFETY: forwarded caller contract.
        match unsafe { ffi::text(text, length, "text") } {
            Ok(text) => {
                result.value = Some(wrap(text.into()));
                TfStatus::Ok
            }
            Err(failure) => failure.status(),
        }
    })
}

/// Named watcher registrations and the observations they collect.
///
/// ```c
/// static int length_of(const TfObservation *observation, TfObserverResult *result, void *user_data) {
///     (void)user_data;
///     size_t length = 0;
///     tf_observation_current(observation, &length);
///     char text[32];
///     int written = snprintf(text, sizeof text, "%zu", length);
///     return tf_observer_result_set(result, text, (size_t)written) == TF_STATUS_OK ? 0 : 1;
/// }
///
/// TfObservers *observers = tf_observers_new();
/// assert(tf_observers_set(observers, "bytes", TF_NTS, length_of, NULL, NULL, NULL) == TF_STATUS_OK);
/// assert(tf_observers_set_builtin(observers, "words", TF_NTS, "word_count", TF_NTS, NULL) == TF_STATUS_OK);
/// assert(tf_observers_schedule_all(observers, "bytes", TF_NTS, NULL) == TF_STATUS_OK);
/// tf_observers_free(observers);
/// ```
pub struct TfObservers {
    inner: Guarded<Observers>,
}

/// Run `operation` on locked observers.
///
/// # Safety
///
/// As [`ffi::call`]; `observers` is NULL or live.
unsafe fn with_observers(
    observers: *const TfObservers,
    error: *mut *mut TfError,
    operation: impl FnOnce(&mut Observers) -> Result<(), Failure>,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            operation(&mut *ffi::handle(observers, "observers")?.inner.lock()?)
        })
    }
}

/// Create an empty registration set. Never returns NULL.
#[unsafe(no_mangle)]
pub extern "C" fn tf_observers_new() -> *mut TfObservers {
    ffi::into_handle(TfObservers {
        inner: Guarded::new(Observers::new()),
    })
}

/// Release observers, invoking every registration's destroy function.
/// `NULL` is ignored.
///
/// # Safety
///
/// `observers` is NULL or live and not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observers_free(observers: *mut TfObservers) {
    // SAFETY: forwarded caller contract.
    ffi::guard((), || drop(unsafe { ffi::take_handle(observers) }));
}

/// Register (or replace) custom watcher `name`.
///
/// **Ownership of `user_data` transfers on every call**: `destroy` (if not
/// NULL) runs exactly once — when the registration is replaced, when the
/// observers are freed, or immediately if this call fails.
///
/// # Safety
///
/// `observers` is live; `name`/`length` follow the text convention; `callback`
/// and `destroy` are valid for `user_data` until `destroy` runs; `error`
/// follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observers_set(
    observers: *mut TfObservers,
    name: *const c_char,
    length: usize,
    callback: TfObserverFn,
    user_data: *mut c_void,
    destroy: TfDestroyFn,
    error: *mut *mut TfError,
) -> TfStatus {
    // Own `user_data` first so every failure path below releases it.
    let guard_data = OwnedData { user_data, destroy };
    // SAFETY: forwarded caller contract.
    unsafe {
        with_observers(observers, error, move |observers| {
            let callback = callback.ok_or_else(|| Failure::null("callback"))?;
            let name = ffi::text(name, length, "name")?;
            let OwnedData { user_data, destroy } = guard_data.release();
            observers.set(
                name.into(),
                Box::new(CObserver {
                    callback,
                    user_data,
                    destroy,
                }),
            );
            Ok(())
        })
    }
}

/// `user_data` awaiting registration; destroyed if registration does not happen.
struct OwnedData {
    user_data: *mut c_void,
    destroy: TfDestroyFn,
}

impl OwnedData {
    fn release(mut self) -> Self {
        let released = Self {
            user_data: self.user_data,
            destroy: self.destroy.take(),
        };
        std::mem::forget(self);
        released
    }
}

impl Drop for OwnedData {
    fn drop(&mut self) {
        if let Some(destroy) = self.destroy {
            // SAFETY: registration contract.
            unsafe { destroy(self.user_data) };
        }
    }
}

/// Register watcher `name` computing built-in `metric` (see
/// `tf_observers_builtin_names`).
///
/// # Safety
///
/// `observers` is live; both text pairs follow the text convention; `error`
/// follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observers_set_builtin(
    observers: *mut TfObservers,
    name: *const c_char,
    name_length: usize,
    metric: *const c_char,
    metric_length: usize,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        with_observers(observers, error, |observers| {
            let name = ffi::text(name, name_length, "name")?;
            let metric = ffi::text(metric, metric_length, "metric")?;
            Ok(observers.set_builtin(name.into(), metric)?)
        })
    }
}

/// Names of all built-in metrics. Release with `tf_string_list_free`.
///
/// # Safety
///
/// `names` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observers_builtin_names(
    names: *mut TfStringList,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            Out::new(names, "names")?.write(TfStringList::from(Observers::builtin_names()));
            Ok(())
        })
    }
}

/// Run watcher `name` after every instruction.
///
/// # Safety
///
/// `observers` is live; `name`/`length` follow the text convention; `error`
/// follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observers_schedule_all(
    observers: *mut TfObservers,
    name: *const c_char,
    length: usize,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        with_observers(observers, error, |observers| {
            Ok(observers.schedule_all(ffi::text(name, length, "name")?)?)
        })
    }
}

/// Discard collected observations, keeping registrations and schedules.
///
/// # Safety
///
/// `observers` is live; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observers_reset(observers: *mut TfObservers, error: *mut *mut TfError) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        with_observers(observers, error, |observers| {
            observers.reset();
            Ok(())
        })
    }
}

/// Run the scheduled watchers once on a manual observation. `after` may be
/// NULL (with any length) for "no after text".
///
/// # Safety
///
/// `observers` is live; every non-NULL text pair follows the text convention;
/// `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observers_run(
    observers: *mut TfObservers,
    current: *const c_char,
    current_length: usize,
    before: *const c_char,
    before_length: usize,
    after: *const c_char,
    after_length: usize,
    instruction: *const c_char,
    instruction_length: usize,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        with_observers(observers, error, |observers| {
            let observation = Observation {
                current: ffi::text(current, current_length, "current")?.into(),
                before: ffi::text(before, before_length, "before")?.into(),
                after: if after.is_null() {
                    None
                } else {
                    Some(ffi::text(after, after_length, "after")?.into())
                },
                instruction: ffi::text(instruction, instruction_length, "instruction")?.into(),
            };
            Ok(observers.run(observation)?)
        })
    }
}

/// Write the collected observations as a JSON report, creating missing parent
/// directories.
///
/// # Safety
///
/// `observers` and `path` are live; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_observers_write_report(
    observers: *const TfObservers,
    path: *const TfFilePath,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        with_observers(observers, error, |observers| {
            Ok(observers.write_report(path::borrow(path, "path")?)?)
        })
    }
}

/// Execute pipeline `id` with watchers and write the JSON report to `report`.
///
/// A successful run resets earlier observations first; registrations are kept.
/// On failure an existing report file is preserved.
///
/// ```c
/// TfProcessor *processor = tf_processor_new();
/// TfPipeline *plan = tf_pipeline_to_uppercase_all(tf_pipeline_new());
/// TfString id = {0};
/// assert(tf_pipeline_build(plan, processor, &id, NULL) == TF_STATUS_OK);
/// TfObservers *observers = tf_observers_new();
/// assert(tf_observers_set_builtin(observers, "chars", TF_NTS, "current_len_chars", TF_NTS, NULL) == TF_STATUS_OK);
/// assert(tf_observers_schedule_all(observers, "chars", TF_NTS, NULL) == TF_STATUS_OK);
/// char directory[] = "/tmp/textforge-doc-XXXXXX";
/// assert(mkdtemp(directory) != NULL);
/// char report_path[64];
/// snprintf(report_path, sizeof report_path, "%s/report.json", directory);
/// TfFilePath *report = NULL;
/// assert(tf_file_path_from_text(report_path, TF_NTS, &report, NULL) == TF_STATUS_OK);
/// TfString out = {0};
/// assert(tf_processor_process_all_with_watchers(processor, id.data, id.length, "ab", TF_NTS,
///                                              observers, report, &out, NULL) == TF_STATUS_OK);
/// assert(strcmp(out.data, "AB") == 0);
/// remove(report_path);
/// rmdir(directory);
/// tf_string_free(&out);
/// tf_file_path_free(report);
/// tf_observers_free(observers);
/// tf_string_free(&id);
/// tf_pipeline_free(plan);
/// tf_processor_free(processor);
/// ```
///
/// # Safety
///
/// `processor`, `observers` and `report` are live; text pairs follow the text
/// convention; `out` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_all_with_watchers(
    processor: *mut TfProcessor,
    id: *const c_char,
    id_length: usize,
    input: *const c_char,
    input_length: usize,
    observers: *mut TfObservers,
    report: *const TfFilePath,
    out: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let id = processor::id(id, id_length)?;
            let input = ffi::text(input, input_length, "input")?;
            let report = path::borrow(report, "report")?;
            let mut observers = ffi::handle(observers, "observers")?.inner.lock()?;
            let mut processor = processor::borrow(processor)?.inner.lock()?;
            let text = processor.process_all_with_watchers(id, input, &mut observers, report)?;
            out.write(TfString::from(text));
            Ok(())
        })
    }
}

//! Status codes and structured errors.

use crate::text::CText;
use std::ffi::c_char;
use textforge::api::portable::{BindingError, ErrorStage};

/// Result of every fallible function. `TF_STATUS_OK` is zero.
///
/// The first five failure values correspond to the portable error stages; the
/// others are produced by this adapter.
///
/// ```c
/// TfError *error = NULL;
/// TfStatus status = tf_pipeline_compile(NULL, NULL, &error);
/// assert(status == TF_STATUS_INVALID_ARGUMENT);
/// assert(tf_error_status(error) == status);
/// tf_error_free(error);
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TfStatus {
    /// Success.
    Ok = 0,
    /// Unknown mnemonic, out-of-range number or excessive nesting.
    Conversion = 1,
    /// Wrong arity or type, or an invalid block/conditional hierarchy.
    Validation = 2,
    /// Runtime failure: undefined variable or block, invalid regex, I/O, ...
    Execution = 3,
    /// Pipeline import/export failure.
    Persistence = 4,
    /// A watcher callback reported a failure.
    Callback = 5,
    /// A NULL pointer, invalid UTF-8, a non-NULL error slot or similar misuse.
    InvalidArgument = 6,
    /// The object is already in use by the current thread (re-entrant call
    /// from one of its own callbacks).
    Busy = 7,
    /// An internal panic was caught. The object involved may be unusable.
    Panic = 8,
    /// An error stage introduced by a newer core that this binding does not know.
    Unknown = 9,
}

/// Where an error originated.
///
/// ```c
/// TfError *error = NULL;
/// tf_pipeline_compile(NULL, NULL, &error);
/// assert(tf_error_stage(error) == TF_STAGE_ADAPTER);
/// tf_error_free(error);
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TfStage {
    /// Converting arguments into an instruction description.
    Conversion = 1,
    /// Structural validation while compiling or registering.
    Validation = 2,
    /// Executing a pipeline.
    Execution = 3,
    /// Reading or writing pipeline files and reports.
    Persistence = 4,
    /// A user callback.
    Callback = 5,
    /// This C adapter (argument checks, re-entrancy, panics).
    Adapter = 6,
    /// A stage introduced by a newer core.
    Unknown = 7,
}

/// Internal failure carried until it is handed to C as a [`TfError`].
#[derive(Clone, Debug)]
pub(crate) enum Failure {
    Binding(BindingError),
    Adapter { status: TfStatus, message: String },
}

impl Failure {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::Adapter {
            status: TfStatus::InvalidArgument,
            message: message.into(),
        }
    }
    pub(crate) fn null(what: &str) -> Self {
        Self::invalid(format!("{what} must not be NULL"))
    }
    pub(crate) fn busy() -> Self {
        Self::Adapter {
            status: TfStatus::Busy,
            message: "object is already in use by this thread (re-entrant call)".into(),
        }
    }
    pub(crate) fn poisoned() -> Self {
        Self::Adapter {
            status: TfStatus::Panic,
            message: "object is unusable after an internal panic".into(),
        }
    }
    pub(crate) fn panic() -> Self {
        Self::Adapter {
            status: TfStatus::Panic,
            message: "internal panic caught at the C boundary".into(),
        }
    }
    pub(crate) fn status(&self) -> TfStatus {
        match self {
            Self::Binding(error) => match stage(error.stage) {
                TfStage::Conversion => TfStatus::Conversion,
                TfStage::Validation => TfStatus::Validation,
                TfStage::Execution => TfStatus::Execution,
                TfStage::Persistence => TfStatus::Persistence,
                TfStage::Callback => TfStatus::Callback,
                TfStage::Adapter | TfStage::Unknown => TfStatus::Unknown,
            },
            Self::Adapter { status, .. } => *status,
        }
    }
}

impl From<BindingError> for Failure {
    fn from(error: BindingError) -> Self {
        Self::Binding(error)
    }
}

fn stage(stage: ErrorStage) -> TfStage {
    match stage {
        ErrorStage::Conversion => TfStage::Conversion,
        ErrorStage::Validation => TfStage::Validation,
        ErrorStage::Execution => TfStage::Execution,
        ErrorStage::Persistence => TfStage::Persistence,
        ErrorStage::Callback => TfStage::Callback,
        _ => TfStage::Unknown,
    }
}

/// Owned, immutable error description. Release it with `tf_error_free`.
///
/// Fields mirror the portable `BindingError` verbatim: numeric core code,
/// stage, message, the instruction involved and diagnostic input. Adapter
/// errors use stage `TF_STAGE_ADAPTER` and code 0. All strings are
/// NUL-terminated UTF-8 owned by the error.
///
/// ```c
/// TfPipeline *plan = tf_pipeline_add(tf_pipeline_new(), "nope", TF_NTS, NULL, 0);
/// TfError *error = NULL;
/// assert(tf_pipeline_status(plan, &error) == TF_STATUS_CONVERSION);
/// size_t length = 0;
/// const char *instruction = tf_error_instruction(error, &length);
/// assert(strcmp(instruction, "nope") == 0 && length == 4);
/// tf_error_free(error);
/// tf_pipeline_free(plan);
/// ```
#[derive(Debug)]
pub struct TfError {
    status: TfStatus,
    stage: TfStage,
    code: u16,
    message: CText,
    instruction: CText,
    input: CText,
}

impl From<Failure> for TfError {
    fn from(failure: Failure) -> Self {
        let status = failure.status();
        match failure {
            Failure::Binding(error) => Self {
                status,
                stage: stage(error.stage),
                code: error.code,
                message: CText::from(error.message),
                instruction: CText::from(error.instruction),
                input: CText::from(error.input),
            },
            Failure::Adapter { message, .. } => Self {
                status,
                stage: TfStage::Adapter,
                code: 0,
                message: CText::from(message),
                instruction: CText::default(),
                input: CText::default(),
            },
        }
    }
}

fn field(error: *const TfError, length: *mut usize, pick: fn(&TfError) -> &CText) -> *const c_char {
    // SAFETY: `error` is NULL or a live error by the documented contract.
    match unsafe { error.as_ref() } {
        Some(error) => pick(error).export(length),
        None => CText::null(length),
    }
}

/// Status corresponding to this error (`TF_STATUS_INVALID_ARGUMENT` for NULL).
///
/// # Safety
///
/// `error` is NULL or a live error returned by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_error_status(error: *const TfError) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe { error.as_ref() }.map_or(TfStatus::InvalidArgument, |e| e.status)
}

/// Stage where the error originated (`TF_STAGE_ADAPTER` for NULL).
///
/// # Safety
///
/// `error` is NULL or a live error returned by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_error_stage(error: *const TfError) -> TfStage {
    // SAFETY: forwarded caller contract.
    unsafe { error.as_ref() }.map_or(TfStage::Adapter, |e| e.stage)
}

/// Numeric core error code (0 for adapter errors and NULL).
///
/// # Safety
///
/// `error` is NULL or a live error returned by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_error_code(error: *const TfError) -> u16 {
    // SAFETY: forwarded caller contract.
    unsafe { error.as_ref() }.map_or(0, |e| e.code)
}

/// Human-readable message. Valid until the error is freed.
///
/// If `length` is not NULL it receives the byte length (excluding the NUL).
/// Returns NULL when `error` is NULL.
///
/// # Safety
///
/// `error` is NULL or a live error; `length` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_error_message(error: *const TfError, length: *mut usize) -> *const c_char {
    field(error, length, |e| &e.message)
}

/// Instruction (or API location) involved; empty when not applicable.
///
/// # Safety
///
/// `error` is NULL or a live error; `length` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_error_instruction(
    error: *const TfError,
    length: *mut usize,
) -> *const c_char {
    field(error, length, |e| &e.instruction)
}

/// Raw diagnostic context (offending input, path, ...); may be empty.
///
/// # Safety
///
/// `error` is NULL or a live error; `length` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_error_input(error: *const TfError, length: *mut usize) -> *const c_char {
    field(error, length, |e| &e.input)
}

/// Release an error. `NULL` is ignored.
///
/// # Safety
///
/// `error` is NULL or a live error that is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_error_free(error: *mut TfError) {
    // SAFETY: forwarded caller contract.
    drop(unsafe { crate::ffi::take_handle(error) });
}

/// Stable, static name of a status value, e.g. `"TF_STATUS_EXECUTION"`.
///
/// Unknown values return `"TF_STATUS_UNRECOGNIZED"`. The result is never freed.
///
/// ```c
/// assert(strcmp(tf_status_name(TF_STATUS_BUSY), "TF_STATUS_BUSY") == 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn tf_status_name(status: i32) -> *const c_char {
    let name: &'static [u8] = match status {
        0 => b"TF_STATUS_OK\0",
        1 => b"TF_STATUS_CONVERSION\0",
        2 => b"TF_STATUS_VALIDATION\0",
        3 => b"TF_STATUS_EXECUTION\0",
        4 => b"TF_STATUS_PERSISTENCE\0",
        5 => b"TF_STATUS_CALLBACK\0",
        6 => b"TF_STATUS_INVALID_ARGUMENT\0",
        7 => b"TF_STATUS_BUSY\0",
        8 => b"TF_STATUS_PANIC\0",
        9 => b"TF_STATUS_UNKNOWN\0",
        _ => b"TF_STATUS_UNRECOGNIZED\0",
    };
    name.as_ptr().cast()
}

//! C ABI for TextForge, built on `textforge::api::portable`.
//!
//! The public C interface is `include/textforge.h`, generated from this crate
//! by cbindgen. Every exported function is a thin adapter: it validates C
//! pointers and UTF-8, converts values into portable types, delegates to the
//! portable API and converts results back. No instruction, validation or
//! execution logic lives here.
//!
//! Memory model (details in the header and README):
//!
//! * Objects (`TfPipeline`, `TfProcessor`, ...) are opaque handles created by
//!   `tf_*_new` and released by `tf_*_free`; free functions accept NULL.
//! * Returned strings and arrays (`TfString`, `TfStringList`, ...) are owned
//!   by the caller and released with their own `tf_*_free`.
//! * Functions documented as *consuming* release their argument on every path.
//! * Errors are returned as `TfStatus`, with optional detail in a `TfError`.
//!
//! The functions can also be called from Rust, which is how the integration
//! tests exercise the exact exported code:
//!
//! ```
//! use textforge_c::*;
//! // SAFETY: every pointer comes from this library or a C string literal.
//! unsafe {
//!     let plan = tf_pipeline_to_uppercase_all(tf_pipeline_new());
//!     let mut compiled = std::ptr::null_mut();
//!     assert_eq!(tf_pipeline_compile(plan, &mut compiled, std::ptr::null_mut()), TfStatus::Ok);
//!     let mut out = TfString { data: std::ptr::null_mut(), length: 0 };
//!     let status = tf_compiled_pipeline_process(compiled, c"é".as_ptr(), TF_NTS, &mut out, std::ptr::null_mut());
//!     assert_eq!(status, TfStatus::Ok);
//!     assert_eq!(std::ffi::CStr::from_ptr(out.data).to_str(), Ok("É"));
//!     tf_string_free(&mut out);
//!     tf_compiled_pipeline_free(compiled);
//!     tf_pipeline_free(plan);
//! }
//! ```

mod buffers;
mod builder;
mod error;
mod ffi;
mod info;
mod instruction;
mod path;
mod pipeline;
mod processor;
mod text;
mod watchers;

pub use buffers::*;
pub use builder::*;
pub use error::{
    TfError, TfStage, TfStatus, tf_error_code, tf_error_free, tf_error_input, tf_error_instruction,
    tf_error_message, tf_error_stage, tf_error_status, tf_status_name,
};
pub use ffi::TF_NTS;
pub use info::*;
pub use instruction::{
    TF_ARG_INSTRUCTION, TF_ARG_NUMBER, TF_ARG_TEXT, TF_ARG_VARIABLE, TfArg, TfArgKind,
    TfInstruction, TfInstructionList, tf_instruction_argument, tf_instruction_argument_count,
    tf_instruction_clone, tf_instruction_free, tf_instruction_list_free, tf_instruction_name,
    tf_instruction_new, tf_nested, tf_number, tf_text, tf_text_n, tf_var, tf_var_n,
};
pub use path::{
    TfFilePath, tf_file_path_free, tf_file_path_from_native_bytes, tf_file_path_from_text,
    tf_file_path_from_windows_wide, tf_file_path_to_native_bytes, tf_file_path_to_text,
    tf_file_path_to_windows_wide,
};
pub use pipeline::*;
pub use processor::*;
pub use text::*;
pub use watchers::*;

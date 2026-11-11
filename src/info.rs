//! Library metadata.

use std::ffi::c_char;

/// Optional core capabilities compiled into this library.
///
/// ```c
/// TfFeatures features = tf_features();
/// assert(features.bytecode && features.watchers && features.buffered);
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TfFeatures {
    /// Bytecode import/export.
    pub bytecode: bool,
    /// Watcher reports.
    pub watchers: bool,
    /// Spool-backed buffers and streams.
    pub buffered: bool,
}

/// Capabilities of the bundled core.
#[unsafe(no_mangle)]
pub extern "C" fn tf_features() -> TfFeatures {
    let features = textforge::api::portable::features();
    TfFeatures {
        bytecode: features.bytecode,
        watchers: features.watchers,
        buffered: features.buffered,
    }
}

/// Version of this C binding, e.g. `"0.1.0"`. Static; never freed.
///
/// ```c
/// printf("textforge-c %s (core %s)\n", tf_version(), tf_core_version());
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn tf_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// Version of the bundled TextForge core, e.g. `"0.2.0"`. Static; never freed.
#[unsafe(no_mangle)]
pub extern "C" fn tf_core_version() -> *const c_char {
    c"0.2.0".as_ptr()
}

//! Lossless operating-system paths (portable `FilePath`).

use crate::{
    error::{Failure, TfError, TfStatus},
    ffi::{self, Out},
    text::{TfBytes, TfString, TfWideString},
};
use std::ffi::c_char;
use textforge::api::portable::FilePath;

/// Immutable file-system path. Release with `tf_file_path_free`.
///
/// Use `tf_file_path_from_text` for UTF-8 paths, `tf_file_path_from_native_bytes`
/// for arbitrary Unix byte paths and `tf_file_path_from_windows_wide` for raw
/// Windows UTF-16 paths (including unpaired surrogates). Paths are never
/// normalized or lossily converted; a representation that is not native to the
/// current platform fails with `TF_STATUS_CONVERSION`. Paths may be shared
/// between threads.
///
/// ```c
/// TfFilePath *path = NULL;
/// const uint8_t raw[] = {'f', 0xFF, '.', 't', 'f', 't'};  // Not valid UTF-8.
/// assert(tf_file_path_from_native_bytes(raw, sizeof raw, &path, NULL) == TF_STATUS_OK);
/// TfString text = {0};
/// bool representable = true;
/// assert(tf_file_path_to_text(path, &text, &representable, NULL) == TF_STATUS_OK);
/// assert(!representable && text.data == NULL);
/// tf_file_path_free(path);
/// ```
#[derive(Clone, Debug)]
pub struct TfFilePath {
    pub(crate) inner: FilePath,
}

fn publish(out: Out<*mut TfFilePath>, inner: FilePath) {
    out.write(ffi::into_handle(TfFilePath { inner }));
}

/// Borrow the portable path of a C handle.
///
/// # Safety
///
/// `path` is NULL or a live path.
pub(crate) unsafe fn borrow<'a>(
    path: *const TfFilePath,
    what: &str,
) -> Result<&'a FilePath, Failure> {
    // SAFETY: forwarded caller contract.
    Ok(&unsafe { ffi::handle(path, what) }?.inner)
}

/// Create a path from UTF-8 text.
///
/// # Safety
///
/// `text`/`length` follow the text convention; `out` is writable; `error`
/// follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_file_path_from_text(
    text: *const c_char,
    length: usize,
    out: *mut *mut TfFilePath,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            publish(
                out,
                FilePath::from_text(ffi::text(text, length, "path")?.into()),
            );
            Ok(())
        })
    }
}

/// Create a path from native Unix bytes (any byte sequence).
/// Fails with `TF_STATUS_CONVERSION` on non-Unix targets.
///
/// # Safety
///
/// `bytes` is NULL with `length == 0` or valid for `length` bytes; `out` is
/// writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_file_path_from_native_bytes(
    bytes: *const u8,
    length: usize,
    out: *mut *mut TfFilePath,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let bytes = ffi::bytes(bytes, length, "path")?;
            publish(out, FilePath::from_native_bytes(bytes.to_vec())?);
            Ok(())
        })
    }
}

/// Create a path from raw Windows UTF-16 code units. Fails with
/// `TF_STATUS_CONVERSION` on non-Windows targets.
///
/// # Safety
///
/// `units` is NULL with `length == 0` or valid for `length` code units; `out`
/// is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_file_path_from_windows_wide(
    units: *const u16,
    length: usize,
    out: *mut *mut TfFilePath,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let units = ffi::array(units, length, "path")?;
            publish(out, FilePath::from_windows_wide(units.to_vec())?);
            Ok(())
        })
    }
}

/// Convert to UTF-8 text when an exact representation exists.
///
/// On success `*representable` tells whether `*text` was written; it is
/// `false` (and `*text` untouched) for native names without a Unicode form.
///
/// # Safety
///
/// `path` is live; `text` and `representable` are writable; `error` follows
/// the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_file_path_to_text(
    path: *const TfFilePath,
    text: *mut TfString,
    representable: *mut bool,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let text = Out::new(text, "text")?;
            let representable = Out::new(representable, "representable")?;
            match borrow(path, "path")?.to_text() {
                Some(value) => {
                    text.write(TfString::from(value));
                    representable.write(true);
                }
                None => representable.write(false),
            }
            Ok(())
        })
    }
}

/// Native Unix bytes of the path. Fails with `TF_STATUS_CONVERSION` elsewhere.
///
/// # Safety
///
/// `path` is live; `bytes` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_file_path_to_native_bytes(
    path: *const TfFilePath,
    bytes: *mut TfBytes,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let bytes = Out::new(bytes, "bytes")?;
            bytes.write(TfBytes::from(borrow(path, "path")?.to_native_bytes()?));
            Ok(())
        })
    }
}

/// Raw Windows UTF-16 code units. Fails with `TF_STATUS_CONVERSION` elsewhere.
///
/// # Safety
///
/// `path` is live; `units` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_file_path_to_windows_wide(
    path: *const TfFilePath,
    units: *mut TfWideString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let units = Out::new(units, "units")?;
            units.write(TfWideString::from(borrow(path, "path")?.to_windows_wide()?));
            Ok(())
        })
    }
}

/// Release a path. `NULL` is ignored.
///
/// # Safety
///
/// `path` is NULL or a live path not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_file_path_free(path: *mut TfFilePath) {
    // SAFETY: forwarded caller contract.
    ffi::guard((), || drop(unsafe { ffi::take_handle(path) }));
}

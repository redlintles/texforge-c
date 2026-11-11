//! Owned strings and byte arrays returned to C.
//!
//! Every returned array is allocated by this library, carries an explicit length
//! and has one extra zero element after the last item, so text can also be used
//! as a C string when it contains no embedded NUL. Release values with the
//! matching `*_free` function, never with `free()`. Free functions reset the
//! struct to `{NULL, 0}`, so calling them twice on the same struct is harmless.

use std::{ffi::c_char, ptr};

/// Allocate `items` followed by one `T::default()` terminator.
fn leak<T: Copy + Default>(mut items: Vec<T>) -> (*mut T, usize) {
    let length = items.len();
    items.push(T::default());
    let boxed = items.into_boxed_slice();
    (Box::into_raw(boxed).cast(), length)
}

/// Release memory produced by [`leak`].
///
/// # Safety
///
/// `data`/`length` were returned together by `leak::<T>` and not yet released.
unsafe fn release<T>(data: *mut T, length: usize) {
    if !data.is_null() {
        // SAFETY: reconstructs exactly the boxed slice `leak` produced.
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(data, length + 1)) });
    }
}

/// Internal immutable NUL-terminated text owned by a library object.
#[derive(Clone, Debug)]
pub(crate) struct CText(Box<[u8]>);

impl Default for CText {
    fn default() -> Self {
        Self(Box::new([0]))
    }
}

impl CText {
    /// Write the length (excluding the NUL) to `length` and return the text.
    pub(crate) fn export(&self, length: *mut usize) -> *const c_char {
        if !length.is_null() {
            // SAFETY: callers document `length` as NULL or writable.
            unsafe { length.write(self.0.len() - 1) };
        }
        self.0.as_ptr().cast()
    }
    /// Report "no value": NULL pointer and zero length.
    pub(crate) fn null(length: *mut usize) -> *const c_char {
        if !length.is_null() {
            // SAFETY: callers document `length` as NULL or writable.
            unsafe { length.write(0) };
        }
        ptr::null()
    }
}

impl From<String> for CText {
    fn from(text: String) -> Self {
        let mut bytes = text.into_bytes();
        bytes.push(0);
        Self(bytes.into_boxed_slice())
    }
}

impl From<&str> for CText {
    fn from(text: &str) -> Self {
        Self::from(text.to_owned())
    }
}

/// Owned UTF-8 text returned by the library. Release with `tf_string_free`.
///
/// `data[length]` is always `'\0'`. `length` is authoritative: the text may
/// contain embedded NUL bytes. Do not modify `data` or `length`.
///
/// ```c
/// TfPipeline *plan = tf_pipeline_to_uppercase_all(tf_pipeline_new());
/// TfCompiledPipeline *pipeline = NULL;
/// assert(tf_pipeline_compile(plan, &pipeline, NULL) == TF_STATUS_OK);
/// TfString out = {0};
/// assert(tf_compiled_pipeline_process(pipeline, "é", TF_NTS, &out, NULL) == TF_STATUS_OK);
/// assert(strcmp(out.data, "É") == 0);
/// tf_string_free(&out);
/// assert(out.data == NULL);
/// tf_compiled_pipeline_free(pipeline);
/// tf_pipeline_free(plan);
/// ```
#[repr(C)]
#[derive(Debug)]
pub struct TfString {
    /// UTF-8 bytes followed by a NUL terminator; NULL after release.
    pub data: *mut c_char,
    /// Byte length, excluding the terminator.
    pub length: usize,
}

impl From<String> for TfString {
    fn from(text: String) -> Self {
        let (data, length) = leak(text.into_bytes());
        Self {
            data: data.cast(),
            length,
        }
    }
}

/// Owned byte array returned by the library. Release with `tf_bytes_free`.
///
/// `data[length]` is a zero byte. Do not modify `data` or `length`.
///
/// ```c
/// TfFilePath *path = NULL;
/// assert(tf_file_path_from_text("a/b.txt", TF_NTS, &path, NULL) == TF_STATUS_OK);
/// TfBytes bytes = {0};
/// assert(tf_file_path_to_native_bytes(path, &bytes, NULL) == TF_STATUS_OK);
/// assert(bytes.length == 7 && memcmp(bytes.data, "a/b.txt", 7) == 0);
/// tf_bytes_free(&bytes);
/// tf_file_path_free(path);
/// ```
#[repr(C)]
#[derive(Debug)]
pub struct TfBytes {
    /// Bytes followed by a zero byte; NULL after release.
    pub data: *mut u8,
    /// Number of bytes, excluding the terminator.
    pub length: usize,
}

impl From<Vec<u8>> for TfBytes {
    fn from(bytes: Vec<u8>) -> Self {
        let (data, length) = leak(bytes);
        Self { data, length }
    }
}

/// Owned UTF-16 code units (Windows wide path). Release with `tf_wide_string_free`.
///
/// `data[length]` is zero. Unpaired surrogates are preserved.
///
/// ```c
/// TfFilePath *path = NULL;
/// const uint16_t wide[] = {'a', 0xD800};
/// TfStatus status = tf_file_path_from_windows_wide(wide, 2, &path, NULL);
/// #ifdef _WIN32
/// assert(status == TF_STATUS_OK);
/// TfWideString copy = {0};
/// assert(tf_file_path_to_windows_wide(path, &copy, NULL) == TF_STATUS_OK);
/// tf_wide_string_free(&copy);
/// #else
/// assert(status == TF_STATUS_CONVERSION);  // Not a native representation here.
/// #endif
/// tf_file_path_free(path);
/// ```
#[repr(C)]
#[derive(Debug)]
pub struct TfWideString {
    /// Code units followed by a zero unit; NULL after release.
    pub data: *mut u16,
    /// Number of code units, excluding the terminator.
    pub length: usize,
}

impl From<Vec<u16>> for TfWideString {
    fn from(units: Vec<u16>) -> Self {
        let (data, length) = leak(units);
        Self { data, length }
    }
}

/// Owned list of strings. Release with `tf_string_list_free`, which also
/// releases every item.
///
/// ```c
/// TfStringList names = {0};
/// assert(tf_observers_builtin_names(&names, NULL) == TF_STATUS_OK);
/// assert(names.length > 0);
/// for (size_t i = 0; i < names.length; i++) assert(names.items[i].length > 0);
/// tf_string_list_free(&names);
/// ```
#[repr(C)]
#[derive(Debug)]
pub struct TfStringList {
    /// `length` strings; NULL after release.
    pub items: *mut TfString,
    /// Number of strings.
    pub length: usize,
}

impl From<Vec<String>> for TfStringList {
    fn from(strings: Vec<String>) -> Self {
        let items: Box<[TfString]> = strings.into_iter().map(TfString::from).collect();
        let length = items.len();
        Self {
            items: Box::into_raw(items).cast(),
            length,
        }
    }
}

/// Release a string and reset it to `{NULL, 0}`. `NULL` is ignored.
///
/// # Safety
///
/// `text` is NULL or points to a `TfString` returned by this library (or one
/// already reset by this function).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_string_free(text: *mut TfString) {
    // SAFETY: forwarded caller contract.
    if let Some(text) = unsafe { text.as_mut() } {
        // SAFETY: the pair was produced by `leak` (or data is NULL).
        unsafe { release(text.data.cast::<u8>(), text.length) };
        text.data = ptr::null_mut();
        text.length = 0;
    }
}

/// Release a byte array and reset it to `{NULL, 0}`. `NULL` is ignored.
///
/// # Safety
///
/// `bytes` is NULL or points to a `TfBytes` returned by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_bytes_free(bytes: *mut TfBytes) {
    // SAFETY: forwarded caller contract.
    if let Some(bytes) = unsafe { bytes.as_mut() } {
        // SAFETY: the pair was produced by `leak` (or data is NULL).
        unsafe { release(bytes.data, bytes.length) };
        bytes.data = ptr::null_mut();
        bytes.length = 0;
    }
}

/// Release a wide string and reset it to `{NULL, 0}`. `NULL` is ignored.
///
/// # Safety
///
/// `text` is NULL or points to a `TfWideString` returned by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_wide_string_free(text: *mut TfWideString) {
    // SAFETY: forwarded caller contract.
    if let Some(text) = unsafe { text.as_mut() } {
        // SAFETY: the pair was produced by `leak` (or data is NULL).
        unsafe { release(text.data, text.length) };
        text.data = ptr::null_mut();
        text.length = 0;
    }
}

/// Release a string list and all its items, then reset it. `NULL` is ignored.
///
/// # Safety
///
/// `list` is NULL or points to a `TfStringList` returned by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_string_list_free(list: *mut TfStringList) {
    // SAFETY: forwarded caller contract.
    let Some(list) = (unsafe { list.as_mut() }) else {
        return;
    };
    if !list.items.is_null() {
        // SAFETY: reconstructs the boxed slice created in `From<Vec<String>>`.
        let items = unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(list.items, list.length)) };
        for mut item in items {
            // SAFETY: each item is a library-owned string.
            unsafe { tf_string_free(&mut item) };
        }
    }
    list.items = ptr::null_mut();
    list.length = 0;
}

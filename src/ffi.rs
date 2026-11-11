//! Raw-pointer boundary shared by every exported function.
//!
//! This module is the only place that turns C pointers into Rust references. Each
//! helper states the contract the C caller must uphold; everything above this
//! layer works with ordinary safe Rust values. The rules applied everywhere:
//!
//! * A `NULL` pointer is reported as [`TfStatus::InvalidArgument`] instead of
//!   being dereferenced (except where a function documents `NULL` as valid).
//! * Text is UTF-8, passed as pointer + byte length. [`TF_NTS`] asks the library
//!   to measure a NUL-terminated string instead. Embedded NUL bytes are preserved
//!   when an explicit length is given.
//! * Out-parameters are written only on success, so a failed call never leaves a
//!   half-initialized value behind.
//! * Every exported function catches Rust panics and reports
//!   [`TfStatus::Panic`]; unwinding never crosses into C.
//!
//! [`TfStatus::InvalidArgument`]: crate::error::TfStatus::InvalidArgument
//! [`TfStatus::Panic`]: crate::error::TfStatus::Panic

use crate::error::{Failure, TfError, TfStatus};
use std::{
    ffi::{CStr, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::NonNull,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
};

/// Length sentinel meaning "the text is NUL-terminated; measure it".
///
/// Exported to C as `#define TF_NTS SIZE_MAX` (see `after_includes` in
/// cbindgen.toml, which carries the C documentation).
///
/// cbindgen:ignore
pub const TF_NTS: usize = usize::MAX;

/// Run an exported function body: validates the error slot, catches panics and
/// converts failures into a status plus an optional heap-allocated [`TfError`].
///
/// # Safety
///
/// `error` is either `NULL` or valid for reads and writes of one pointer.
pub(crate) unsafe fn call<F>(error: *mut *mut TfError, body: F) -> TfStatus
where
    F: FnOnce() -> Result<(), Failure>,
{
    // SAFETY: the caller guarantees `error` is NULL or points to a valid slot.
    if !error.is_null() && unsafe { !(*error).is_null() } {
        // Refuse to overwrite (and leak) an error the caller has not released.
        return TfStatus::InvalidArgument;
    }
    let failure = match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(())) => return TfStatus::Ok,
        Ok(Err(failure)) => failure,
        Err(_) => Failure::panic(),
    };
    let status = failure.status();
    if !error.is_null() {
        // SAFETY: checked non-NULL above and valid by the caller's contract.
        unsafe { error.write(Box::into_raw(Box::new(TfError::from(failure)))) };
    }
    status
}

/// Run a body that cannot report a [`TfError`], mapping a panic to `fallback`.
pub(crate) fn guard<T>(fallback: T, body: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(fallback)
}

/// Borrow a UTF-8 string from a `(pointer, length)` pair.
///
/// # Safety
///
/// If `length` is [`TF_NTS`], `text` is NULL or a NUL-terminated string.
/// Otherwise `text` is NULL (only when `length == 0`) or valid for `length`
/// bytes. The bytes stay alive and unmodified for `'a`.
pub(crate) unsafe fn text<'a>(
    text: *const c_char,
    length: usize,
    what: &str,
) -> Result<&'a str, Failure> {
    let bytes = if length == TF_NTS {
        if text.is_null() {
            return Err(Failure::null(what));
        }
        // SAFETY: the caller guarantees a NUL-terminated string.
        unsafe { CStr::from_ptr(text) }.to_bytes()
    } else {
        // SAFETY: forwarded caller contract for an explicit length.
        unsafe { bytes(text.cast(), length, what) }?
    };
    std::str::from_utf8(bytes).map_err(|error| {
        Failure::invalid(format!(
            "{what} is not valid UTF-8 (byte offset {})",
            error.valid_up_to()
        ))
    })
}

/// Borrow `length` bytes. `NULL` is accepted only for an empty slice.
///
/// # Safety
///
/// `data` is NULL with `length == 0`, or valid for reads of `length` bytes that
/// stay alive and unmodified for `'a`.
pub(crate) unsafe fn bytes<'a>(
    data: *const u8,
    length: usize,
    what: &str,
) -> Result<&'a [u8], Failure> {
    // SAFETY: forwarded caller contract.
    unsafe { array(data, length, what) }
}

/// Borrow an array of `length` elements. `NULL` is accepted only when empty.
///
/// # Safety
///
/// `data` is NULL with `length == 0`, or points to `length` initialized,
/// properly aligned elements that stay alive and unmodified for `'a`.
pub(crate) unsafe fn array<'a, T>(
    data: *const T,
    length: usize,
    what: &str,
) -> Result<&'a [T], Failure> {
    if length == 0 {
        return Ok(&[]);
    }
    if data.is_null() {
        return Err(Failure::null(what));
    }
    if length
        .checked_mul(size_of::<T>())
        .is_none_or(|n| n > isize::MAX as usize)
    {
        return Err(Failure::invalid(format!("{what} length is too large")));
    }
    // SAFETY: non-NULL, size checked, and valid for `length` elements by contract.
    Ok(unsafe { std::slice::from_raw_parts(data, length) })
}

/// Borrow a handle created by this library.
///
/// # Safety
///
/// `handle` is NULL or a live pointer returned by this library for type `T`,
/// not yet freed or consumed.
pub(crate) unsafe fn handle<'a, T>(handle: *const T, what: &str) -> Result<&'a T, Failure> {
    // SAFETY: NULL is rejected; otherwise valid by contract.
    unsafe { handle.as_ref() }.ok_or_else(|| Failure::null(what))
}

/// A validated out-parameter, written only once the operation has succeeded.
pub(crate) struct Out<T>(NonNull<T>);

impl<T> Out<T> {
    /// Validate an out-parameter before doing any work.
    ///
    /// # Safety
    ///
    /// `slot` is NULL or valid for writing one `T`. The previous contents are
    /// overwritten without being dropped or freed.
    pub(crate) unsafe fn new(slot: *mut T, what: &str) -> Result<Self, Failure> {
        NonNull::new(slot)
            .map(Self)
            .ok_or_else(|| Failure::null(what))
    }

    /// Store the result.
    pub(crate) fn write(self, value: T) {
        // SAFETY: `new` validated the slot, whose validity the caller guaranteed.
        unsafe { self.0.as_ptr().write(value) };
    }
}

/// Move a value to the heap and give C an owning pointer.
pub(crate) fn into_handle<T>(value: T) -> *mut T {
    Box::into_raw(Box::new(value))
}

/// Take back ownership of a handle (for `*_free` and consuming functions).
///
/// # Safety
///
/// `handle` is NULL or a live pointer returned by [`into_handle`] for `T`; it
/// must not be used again by the caller.
pub(crate) unsafe fn take_handle<T>(handle: *mut T) -> Option<Box<T>> {
    // SAFETY: produced by `Box::into_raw` and not yet released, by contract.
    (!handle.is_null()).then(|| unsafe { Box::from_raw(handle) })
}

thread_local! {
    static THREAD_KEY: u8 = const { 0 };
}

/// A non-zero value unique to the current thread while it is alive.
fn thread_key() -> usize {
    THREAD_KEY.with(|key| std::ptr::from_ref(key) as usize)
}

/// Mutable object state shared with C.
///
/// Different threads are serialized by the mutex. A call that re-enters the
/// same object from one of its own callbacks (same thread, lock already held)
/// would otherwise create two mutable references or deadlock; it is rejected
/// with [`TfStatus::Busy`](crate::error::TfStatus::Busy) instead.
pub(crate) struct Guarded<T> {
    owner: AtomicUsize,
    value: Mutex<T>,
}

impl<T> Guarded<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            owner: AtomicUsize::new(0),
            value: Mutex::new(value),
        }
    }

    /// Acquire exclusive access, rejecting same-thread re-entry.
    pub(crate) fn lock(&self) -> Result<GuardedRef<'_, T>, Failure> {
        let key = thread_key();
        if self.owner.load(Ordering::Acquire) == key {
            return Err(Failure::busy());
        }
        let guard = self.value.lock().map_err(|_| Failure::poisoned())?;
        self.owner.store(key, Ordering::Release);
        Ok(GuardedRef {
            owner: &self.owner,
            guard,
        })
    }

    /// Recover the state from a consumed handle.
    pub(crate) fn into_inner(self) -> Result<T, Failure> {
        self.value.into_inner().map_err(|_| Failure::poisoned())
    }
}

/// Exclusive access to a [`Guarded`] value.
pub(crate) struct GuardedRef<'a, T> {
    owner: &'a AtomicUsize,
    guard: MutexGuard<'a, T>,
}

impl<T> std::ops::Deref for GuardedRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T> std::ops::DerefMut for GuardedRef<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

impl<T> Drop for GuardedRef<'_, T> {
    fn drop(&mut self) {
        // Runs before the mutex guard field is released.
        self.owner.store(0, Ordering::Release);
    }
}

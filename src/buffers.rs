//! Spool-backed text buffers and synchronous streams (portable `PortableBuffer`).

use crate::{
    error::{Failure, TfError, TfStatus},
    ffi::{self, Guarded, Out},
    path::{self, TfFilePath},
    processor::{self, TfProcessor},
    text::{TfBytes, TfString},
};
use std::ffi::{c_char, c_int, c_void};
use textforge::api::portable::{
    BufferOptions, BufferTask, FilePath, PortableBuffer, StreamReader, StreamWriter,
};

/// Default in-memory size (8 MiB) before a buffer spills to a temporary file.
pub const TF_DEFAULT_MEMORY_THRESHOLD: u64 = 8 * 1024 * 1024;

/// Spooling placement for buffers: a memory threshold and a spill directory.
/// This is not a global memory cap; some transforms materialize text.
///
/// ```c
/// TfBufferOptions *options = NULL;
/// assert(tf_buffer_options_new(TF_DEFAULT_MEMORY_THRESHOLD, NULL, &options, NULL) == TF_STATUS_OK);
/// tf_buffer_options_free(options);
/// ```
pub struct TfBufferOptions {
    inner: BufferOptions,
}

/// Create options. `spill_directory` may be NULL to use the system temporary
/// directory; it is created when the options are used.
///
/// # Safety
///
/// `spill_directory` is NULL or live; `out` is writable; `error` follows the
/// error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_options_new(
    memory_threshold: u64,
    spill_directory: *const TfFilePath,
    out: *mut *mut TfBufferOptions,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let spill_directory = if spill_directory.is_null() {
                temporary_directory()?
            } else {
                path::borrow(spill_directory, "spill_directory")?.clone()
            };
            out.write(ffi::into_handle(TfBufferOptions {
                inner: BufferOptions {
                    memory_threshold,
                    spill_directory,
                },
            }));
            Ok(())
        })
    }
}

/// The system temporary directory, represented losslessly.
fn temporary_directory() -> Result<FilePath, Failure> {
    let directory = std::env::temp_dir();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(FilePath::from_native_bytes(
            directory.as_os_str().as_bytes().to_vec(),
        )?)
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        Ok(FilePath::from_windows_wide(
            directory.as_os_str().encode_wide().collect(),
        )?)
    }
    #[cfg(not(any(unix, windows)))]
    {
        directory
            .to_str()
            .map(|text| FilePath::from_text(text.into()))
            .ok_or_else(|| Failure::invalid("temporary directory is not valid Unicode"))
    }
}

/// Release options. `NULL` is ignored.
///
/// # Safety
///
/// `options` is NULL or live and not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_options_free(options: *mut TfBufferOptions) {
    // SAFETY: forwarded caller contract.
    ffi::guard((), || drop(unsafe { ffi::take_handle(options) }));
}

/// Borrow the portable options of a C handle (cloned, as the portable API takes them by value).
///
/// # Safety
///
/// `options` is NULL or live.
unsafe fn options(options: *const TfBufferOptions) -> Result<BufferOptions, Failure> {
    // SAFETY: forwarded caller contract.
    Ok(unsafe { ffi::handle(options, "options") }?.inner.clone())
}

/// Reader callback: copy at most `maximum` bytes into `destination`, store
/// the count in `*read` (0 means end of input) and return 0. Return non-zero
/// to fail. Partial reads may split UTF-8 sequences; the core validates the
/// complete stream. The callback is used only during the receiving call.
pub type TfStreamReadFn = Option<
    unsafe extern "C" fn(
        user_data: *mut c_void,
        destination: *mut u8,
        maximum: usize,
        read: *mut usize,
    ) -> c_int,
>;

/// Writer callback: consume up to `length` bytes, store the accepted count in
/// `*written` and return 0 (the core retries partial writes). Return non-zero
/// to fail. The callback is used only during the receiving call.
pub type TfStreamWriteFn = Option<
    unsafe extern "C" fn(
        user_data: *mut c_void,
        bytes: *const u8,
        length: usize,
        written: *mut usize,
    ) -> c_int,
>;

struct CReader {
    callback: unsafe extern "C" fn(*mut c_void, *mut u8, usize, *mut usize) -> c_int,
    user_data: *mut c_void,
}

impl StreamReader for CReader {
    fn read(&mut self, maximum: u64) -> Result<Vec<u8>, String> {
        let maximum = usize::try_from(maximum).map_err(|error| error.to_string())?;
        let mut bytes = vec![0; maximum];
        let mut read = 0;
        // SAFETY: `bytes` is writable for `maximum` bytes; callback contract.
        let status =
            unsafe { (self.callback)(self.user_data, bytes.as_mut_ptr(), maximum, &mut read) };
        if status != 0 {
            return Err(format!(
                "stream reader callback failed with status {status}"
            ));
        }
        if read > maximum {
            return Err("stream reader callback reported more bytes than requested".into());
        }
        bytes.truncate(read);
        Ok(bytes)
    }
}

struct CWriter {
    callback: unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut usize) -> c_int,
    user_data: *mut c_void,
}

impl StreamWriter for CWriter {
    fn write(&mut self, bytes: Vec<u8>) -> Result<u64, String> {
        let mut written = 0;
        // SAFETY: `bytes` is readable for its length; callback contract.
        let status =
            unsafe { (self.callback)(self.user_data, bytes.as_ptr(), bytes.len(), &mut written) };
        if status != 0 {
            return Err(format!(
                "stream writer callback failed with status {status}"
            ));
        }
        u64::try_from(written).map_err(|error| error.to_string())
    }
}

/// Owned UTF-8 document kept in memory or spilled to a temporary file.
///
/// Byte offsets may split UTF-8 sequences. Functions documented as consuming
/// release the buffer on every path (except `TF_STATUS_BUSY` and a NULL buffer);
/// releasing a buffer removes its spill file immediately.
///
/// ```c
/// TfBufferOptions *options = NULL;
/// assert(tf_buffer_options_new(4, NULL, &options, NULL) == TF_STATUS_OK);
/// TfBuffer *buffer = NULL;
/// assert(tf_buffer_from_text("héllo", TF_NTS, options, &buffer, NULL) == TF_STATUS_OK);
/// bool spilled = false;
/// assert(tf_buffer_is_spilled(buffer, &spilled, NULL) == TF_STATUS_OK && spilled);
/// TfBytes head = {0};
/// assert(tf_buffer_read_at(buffer, 0, 3, &head, NULL) == TF_STATUS_OK);
/// assert(head.length == 3 && memcmp(head.data, "h\xc3\xa9", 3) == 0);
/// tf_bytes_free(&head);
/// TfString text = {0};
/// assert(tf_buffer_into_string(buffer, &text, NULL) == TF_STATUS_OK);  // Consumes buffer.
/// assert(strcmp(text.data, "héllo") == 0);
/// tf_string_free(&text);
/// tf_buffer_options_free(options);
/// ```
pub struct TfBuffer {
    inner: Guarded<PortableBuffer>,
}

fn publish(out: Out<*mut TfBuffer>, buffer: PortableBuffer) {
    out.write(ffi::into_handle(TfBuffer {
        inner: Guarded::new(buffer),
    }));
}

/// Take ownership of a consumed buffer.
///
/// # Safety
///
/// `buffer` is NULL or a live buffer the caller gives up.
unsafe fn take(buffer: *mut TfBuffer) -> Result<PortableBuffer, Failure> {
    // SAFETY: forwarded caller contract.
    let handle = unsafe { ffi::handle(buffer, "buffer") }?;
    // Refuse to free a buffer that one of its own callbacks is still using.
    drop(handle.inner.lock()?);
    // SAFETY: live, not in use by this thread, and given up by the caller.
    let owned = unsafe { ffi::take_handle(buffer) }.ok_or_else(|| Failure::null("buffer"))?;
    owned.inner.into_inner()
}

/// Run `operation` on a locked buffer.
///
/// # Safety
///
/// As [`ffi::call`]; `buffer` is NULL or live.
unsafe fn with_buffer(
    buffer: *const TfBuffer,
    error: *mut *mut TfError,
    operation: impl FnOnce(&mut PortableBuffer) -> Result<(), Failure>,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            operation(&mut *ffi::handle(buffer, "buffer")?.inner.lock()?)
        })
    }
}

/// Create a buffer holding a copy of UTF-8 `text`.
///
/// # Safety
///
/// `text`/`length` follow the text convention; `options` is live; `out` is
/// writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_from_text(
    text: *const c_char,
    length: usize,
    options: *const TfBufferOptions,
    out: *mut *mut TfBuffer,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let text = ffi::text(text, length, "text")?;
            publish(
                out,
                PortableBuffer::from_text(text, self::options(options)?)?,
            );
            Ok(())
        })
    }
}

/// Create a buffer from a UTF-8 file.
///
/// # Safety
///
/// `path` and `options` are live; `out` is writable; `error` follows the
/// error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_from_file(
    path: *const TfFilePath,
    options: *const TfBufferOptions,
    out: *mut *mut TfBuffer,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let path = path::borrow(path, "path")?;
            publish(
                out,
                PortableBuffer::from_file(path, self::options(options)?)?,
            );
            Ok(())
        })
    }
}

/// Create a buffer by draining `reader` until it reports end of input.
///
/// # Safety
///
/// `reader` is valid for `user_data` during the call; `options` is live; `out`
/// is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_from_reader(
    reader: TfStreamReadFn,
    user_data: *mut c_void,
    options: *const TfBufferOptions,
    out: *mut *mut TfBuffer,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let callback = reader.ok_or_else(|| Failure::null("reader"))?;
            let reader = Box::new(CReader {
                callback,
                user_data,
            });
            publish(
                out,
                PortableBuffer::from_reader(reader, self::options(options)?)?,
            );
            Ok(())
        })
    }
}

/// Store the size in bytes in `*length`.
///
/// # Safety
///
/// `buffer` is live; `length` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_len_bytes(
    buffer: *const TfBuffer,
    length: *mut u64,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        let length = Out::new(length, "length");
        with_buffer(buffer, error, |buffer| {
            length?.write(buffer.len_bytes());
            Ok(())
        })
    }
}

/// Store in `*empty` whether the buffer holds no bytes.
///
/// # Safety
///
/// `buffer` is live; `empty` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_is_empty(
    buffer: *const TfBuffer,
    empty: *mut bool,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        let empty = Out::new(empty, "empty");
        with_buffer(buffer, error, |buffer| {
            empty?.write(buffer.is_empty());
            Ok(())
        })
    }
}

/// Store in `*spilled` whether the content lives in a spill file.
///
/// # Safety
///
/// `buffer` is live; `spilled` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_is_spilled(
    buffer: *const TfBuffer,
    spilled: *mut bool,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        let spilled = Out::new(spilled, "spilled");
        with_buffer(buffer, error, |buffer| {
            spilled?.write(buffer.is_spilled());
            Ok(())
        })
    }
}

/// Move the content to a spill file now.
///
/// # Safety
///
/// `buffer` is live; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_spill(
    buffer: *mut TfBuffer,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe { with_buffer(buffer, error, |buffer| Ok(buffer.spill()?)) }
}

/// Copy up to `maximum` bytes starting at `offset`; empty past the end.
///
/// # Safety
///
/// `buffer` is live; `bytes` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_read_at(
    buffer: *mut TfBuffer,
    offset: u64,
    maximum: u64,
    bytes: *mut TfBytes,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        let bytes = Out::new(bytes, "bytes");
        with_buffer(buffer, error, |buffer| {
            let bytes = bytes?;
            bytes.write(TfBytes::from(buffer.read_at(offset, maximum)?));
            Ok(())
        })
    }
}

/// Stream the whole content to `writer`; the byte count goes to `*copied`.
///
/// # Safety
///
/// `buffer` is live; `writer` is valid for `user_data` during the call;
/// `copied` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_copy_to(
    buffer: *mut TfBuffer,
    writer: TfStreamWriteFn,
    user_data: *mut c_void,
    copied: *mut u64,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        let copied = Out::new(copied, "copied");
        with_buffer(buffer, error, |buffer| {
            let copied = copied?;
            let callback = writer.ok_or_else(|| Failure::null("writer"))?;
            copied.write(buffer.copy_to(Box::new(CWriter {
                callback,
                user_data,
            }))?);
            Ok(())
        })
    }
}

/// Write the content to `path`, creating missing parent directories; the byte
/// count goes to `*written`.
///
/// # Safety
///
/// `buffer` and `path` are live; `written` is writable; `error` follows the
/// error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_write_to_file(
    buffer: *mut TfBuffer,
    path: *const TfFilePath,
    written: *mut u64,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        let written = Out::new(written, "written");
        with_buffer(buffer, error, |buffer| {
            let written = written?;
            written.write(buffer.write_to_file(path::borrow(path, "path")?)?);
            Ok(())
        })
    }
}

/// Convert the whole content to a string. **Consumes `buffer`**.
///
/// # Safety
///
/// `buffer` is NULL or a live buffer the caller gives up; `text` is writable;
/// `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_into_string(
    buffer: *mut TfBuffer,
    text: *mut TfString,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        let owned = ffi::guard(Err(Failure::panic()), || take(buffer));
        ffi::call(error, || {
            let owned = owned?;
            let text = Out::new(text, "text")?;
            text.write(TfString::from(owned.into_string()?));
            Ok(())
        })
    }
}

/// Release a buffer and its spill file. `NULL` is ignored.
///
/// # Safety
///
/// `buffer` is NULL or live and not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_buffer_free(buffer: *mut TfBuffer) {
    // SAFETY: forwarded caller contract.
    ffi::guard((), || drop(unsafe { ffi::take_handle(buffer) }));
}

/// Execute pipeline `id` on a buffer, producing a new buffer in `*out`.
/// **Consumes `input`**, including on failure.
///
/// # Safety
///
/// `processor` is live; `id`/`length` follow the text convention; `input` is
/// NULL or a live buffer the caller gives up; `out` is writable; `error`
/// follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_all_buffered(
    processor: *mut TfProcessor,
    id: *const c_char,
    length: usize,
    input: *mut TfBuffer,
    out: *mut *mut TfBuffer,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        let input = ffi::guard(Err(Failure::panic()), || take(input));
        ffi::call(error, || {
            let input = input?;
            let out = Out::new(out, "out")?;
            let id = processor::id(id, length)?;
            let mut processor = processor::borrow(processor)?.inner.lock()?;
            publish(out, processor.process_all_buffered(id, input)?);
            Ok(())
        })
    }
}

/// Execute one textual DSL instruction (e.g. `"atb \"x\";"`) on a buffer.
/// **Consumes `input`**, including on failure.
///
/// # Safety
///
/// As `tf_processor_process_all_buffered`, with `instruction`/`length` as text.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_single_buffered(
    processor: *mut TfProcessor,
    instruction: *const c_char,
    length: usize,
    input: *mut TfBuffer,
    out: *mut *mut TfBuffer,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        let input = ffi::guard(Err(Failure::panic()), || take(input));
        ffi::call(error, || {
            let input = input?;
            let out = Out::new(out, "out")?;
            let instruction = ffi::text(instruction, length, "instruction")?;
            let mut processor = processor::borrow(processor)?.inner.lock()?;
            publish(out, processor.process_single_buffered(instruction, input)?);
            Ok(())
        })
    }
}

/// Execute pipeline `id` on a file through a buffer, producing `*out`.
///
/// # Safety
///
/// `processor`, `input` and `options` are live; `id`/`length` follow the text
/// convention; `out` is writable; `error` follows the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_file_buffered(
    processor: *mut TfProcessor,
    id: *const c_char,
    length: usize,
    input: *const TfFilePath,
    options: *const TfBufferOptions,
    out: *mut *mut TfBuffer,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let id = processor::id(id, length)?;
            let input = path::borrow(input, "input")?;
            let options = self::options(options)?;
            let mut processor = processor::borrow(processor)?.inner.lock()?;
            publish(out, processor.process_file_buffered(id, input, options)?);
            Ok(())
        })
    }
}

/// One job for `tf_processor_process_batch_buffered`; pointers are borrowed.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TfBufferTask {
    /// Registered pipeline ID (UTF-8).
    pub pipeline_id: *const c_char,
    /// Byte length of `pipeline_id`, or `TF_NTS`.
    pub pipeline_id_length: usize,
    /// File to read.
    pub input: *const TfFilePath,
}

/// Process files in parallel into buffers.
///
/// `outputs` and `results` must each have room for `count` pointers. For every
/// task exactly one is set: `outputs[i]` (a buffer to release) on success, or
/// `results[i]` (an error to release) on failure; the other is set to NULL.
/// The function fails only for invalid arguments, leaving both arrays untouched.
///
/// # Safety
///
/// `processor` and `options` are live; `tasks` points to `count` valid tasks;
/// `outputs` and `results` are writable for `count` pointers; `error` follows
/// the error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_processor_process_batch_buffered(
    processor: *const TfProcessor,
    tasks: *const TfBufferTask,
    count: usize,
    options: *const TfBufferOptions,
    outputs: *mut *mut TfBuffer,
    results: *mut *mut TfError,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            if count > 0 && (outputs.is_null() || results.is_null()) {
                return Err(Failure::null("outputs/results"));
            }
            let tasks = ffi::array(tasks, count, "tasks")?
                .iter()
                .map(|task| {
                    Ok(BufferTask {
                        pipeline_id: processor::id(task.pipeline_id, task.pipeline_id_length)?
                            .into(),
                        input: path::borrow(task.input, "task input")?.clone(),
                    })
                })
                .collect::<Result<Vec<_>, Failure>>()?;
            let options = self::options(options)?;
            let processor = processor::borrow(processor)?.inner.lock()?;
            for (index, outcome) in processor
                .process_batch_buffered(tasks, options)
                .into_iter()
                .enumerate()
            {
                let (output, failure) = match outcome {
                    Ok(buffer) => (
                        ffi::into_handle(TfBuffer {
                            inner: Guarded::new(buffer),
                        }),
                        std::ptr::null_mut(),
                    ),
                    Err(error) => (
                        std::ptr::null_mut(),
                        ffi::into_handle(TfError::from(Failure::from(error))),
                    ),
                };
                // SAFETY: `index < count`; both arrays are writable by contract.
                outputs.add(index).write(output);
                // SAFETY: as above.
                results.add(index).write(failure);
            }
            Ok(())
        })
    }
}

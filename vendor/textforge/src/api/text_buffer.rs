//! Experimental owned UTF-8 documents backed by memory or an unnamed temporary file.
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::{borrow::Cow, sync::Arc};

use tempfile::{SpooledData, SpooledTempFile};

use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

const COPY_SIZE: usize = 64 * 1024;

/// Storage settings for one buffer, not a global batch memory limit.
/// Choose a directory on a disk-backed filesystem to actually spill out of RAM.
///
/// ```
/// use textforge::api::TextBufferOptions;
/// let options = TextBufferOptions::new(1024 * 1024, std::env::temp_dir());
/// assert_eq!(options.memory_threshold(), 1024 * 1024);
/// ```
#[derive(Clone, Debug)]
pub struct TextBufferOptions {
    memory_threshold: usize,
    spill_directory: PathBuf,
}

impl TextBufferOptions {
    /// Configure the per-buffer spool threshold and temporary-file directory.
    pub fn new(memory_threshold: usize, spill_directory: impl Into<PathBuf>) -> Self {
        Self {
            memory_threshold,
            spill_directory: spill_directory.into(),
        }
    }

    /// The threshold is a placement decision, not a hard process memory bound.
    pub fn memory_threshold(&self) -> usize {
        self.memory_threshold
    }

    /// Directory used when the buffer first needs a temporary file.
    pub fn spill_directory(&self) -> &Path {
        &self.spill_directory
    }
}

/// An owned, valid UTF-8 document with explicit materialization.
///
/// No `Deref<str>`, implicit string conversion, raw pointer, tempfile handle, or
/// deep `Clone` is exposed. A future FFI can own this as an opaque handle and
/// provide length, byte-range reads, explicit string export and destruction.
/// Chunks and byte offsets do not define independent logical documents.
///
/// ```
/// use textforge::api::{TextBuffer, TextBufferOptions};
/// let options = TextBufferOptions::new(2, std::env::temp_dir());
/// let mut text = TextBuffer::from_text("👩🏽‍💻 é", options)?;
/// assert!(text.is_spilled());
/// let mut bytes = Vec::new();
/// text.copy_to(&mut bytes)?;
/// assert_eq!(text.into_string()?, "👩🏽‍💻 é");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Debug)]
pub struct TextBuffer {
    storage: SpooledTempFile,
    options: TextBufferOptions,
    len: u64,
    materialized: Option<MaterializedText>,
    retain_materialized: bool,
}

/// Internal owned/borrowed immutable text already required by String observers.
///
/// ```
/// use textforge::api::{TextBuffer, TextBufferOptions};
/// let buffer = TextBuffer::from_text("é", TextBufferOptions::new(1, std::env::temp_dir()))?;
/// assert_eq!(buffer.into_string()?, "é");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Debug)]
enum MaterializedText {
    Owned(String),
    Shared(Arc<str>),
}

impl MaterializedText {
    fn as_str(&self) -> &str {
        match self {
            Self::Owned(text) => text,
            Self::Shared(text) => text,
        }
    }
}

fn io_error(error: std::io::Error) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::FileReadingError(error.to_string().into()),
        "TextBuffer",
        "buffer I/O",
    )
}

fn write_error(error: std::io::Error) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::FileWritingError(error.to_string().into()),
        "TextBuffer",
        "buffer output I/O",
    )
}

fn invalid_utf8(error: impl ToString) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::InvalidUtf8Error(error.to_string().into()),
        "TextBuffer",
        "invalid UTF-8 document",
    )
}

impl TextBuffer {
    fn empty(options: TextBufferOptions) -> Self {
        Self {
            storage: SpooledTempFile::new_in(options.memory_threshold, &options.spill_directory),
            options,
            len: 0,
            materialized: None,
            retain_materialized: false,
        }
    }

    pub(crate) fn retain_materialized(mut self) -> Self {
        self.retain_materialized = true;
        self
    }

    fn from_owned(text: String, options: TextBufferOptions) -> Self {
        let mut output = Self::empty(options);
        output.len = text.len() as u64;
        output.materialized = Some(MaterializedText::Owned(text));
        output.retain_materialized = true;
        output
    }

    pub(crate) fn from_shared(text: Arc<str>, options: TextBufferOptions) -> Self {
        let mut output = Self::empty(options);
        output.len = text.len() as u64;
        output.materialized = Some(MaterializedText::Shared(text));
        output.retain_materialized = true;
        output
    }

    pub(crate) fn into_shared(mut self, previous: &Arc<str>) -> Result<Arc<str>, TextForgeError> {
        match self.materialized.take() {
            Some(MaterializedText::Shared(text)) => Ok(text),
            Some(MaterializedText::Owned(text)) if text.as_str() == previous.as_ref() => {
                Ok(Arc::clone(previous))
            }
            Some(MaterializedText::Owned(text)) => Ok(Arc::from(text)),
            None => Ok(Arc::from(self.into_string()?)),
        }
    }

    pub(crate) fn transform_with(
        mut self,
        transform: impl for<'a> FnOnce(Cow<'a, str>) -> Result<Cow<'a, str>, TextForgeError>,
    ) -> Result<Self, TextForgeError> {
        let options = self.options.clone();
        let retain = self.retain_materialized;
        let result = match self.materialized.take() {
            Some(MaterializedText::Shared(text)) => {
                let result = transform(Cow::Borrowed(text.as_ref()))?;
                if let Cow::Borrowed(value) = &result
                    && value.len() == text.len()
                    && std::ptr::eq(value.as_ptr(), text.as_ptr())
                {
                    return Ok(Self::from_shared(text, options));
                }
                result.into_owned()
            }
            Some(MaterializedText::Owned(text)) => transform(Cow::Owned(text))?.into_owned(),
            None => transform(Cow::Owned(self.into_string()?))?.into_owned(),
        };
        if retain {
            Ok(Self::from_owned(result, options))
        } else {
            Self::from_text(&result, options)
        }
    }

    /// Copy valid text into storage; large text goes directly to a tempfile.
    pub fn from_text(text: &str, options: TextBufferOptions) -> Result<Self, TextForgeError> {
        let mut output = Self::empty(options);
        output.append_bytes(text.as_bytes())?;
        Ok(output)
    }

    /// Read a single document, validating UTF-8 across arbitrary read boundaries.
    /// Incomplete code points are carried between reads; invalid UTF-8 is rejected.
    pub fn from_reader(
        mut reader: impl Read,
        options: TextBufferOptions,
    ) -> Result<Self, TextForgeError> {
        let mut output = Self::empty(options);
        let mut chunk = [0; COPY_SIZE];
        let mut pending = Vec::new();
        pending.try_reserve_exact(COPY_SIZE + 3).map_err(|error| {
            TextForgeError::new(
                TextForgeErrorCode::ValidationError(
                    "Cannot allocate UTF-8 validation buffer".into(),
                ),
                "TextBuffer::from_reader",
                error.to_string(),
            )
        })?;
        loop {
            let count = match reader.read(&mut chunk) {
                Ok(count) => count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(io_error(error)),
            };
            if count == 0 {
                break;
            }
            pending.extend_from_slice(&chunk[..count]);
            match std::str::from_utf8(&pending) {
                Ok(_) => {
                    output.append_bytes(&pending)?;
                    pending.clear();
                }
                Err(error) if error.error_len().is_none() => {
                    let valid = error.valid_up_to();
                    output.append_bytes(&pending[..valid])?;
                    pending.copy_within(valid.., 0);
                    pending.truncate(pending.len() - valid);
                }
                Err(error) => return Err(invalid_utf8(error)),
            }
        }
        if !pending.is_empty() {
            return Err(invalid_utf8("incomplete code point at EOF"));
        }
        Ok(output)
    }

    /// Open and validate an input document without mapping or reading it all at once.
    pub fn from_file(
        path: impl AsRef<Path>,
        options: TextBufferOptions,
    ) -> Result<Self, TextForgeError> {
        Self::from_reader(std::fs::File::open(path).map_err(io_error)?, options)
    }

    /// Logical length in bytes; this does not count characters or graphemes.
    pub fn len_bytes(&self) -> u64 {
        self.len
    }

    /// Whether the logical document is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the owned storage has migrated to a temporary file.
    pub fn is_spilled(&self) -> bool {
        self.storage.is_rolled()
    }

    /// Move storage to a tempfile explicitly, preserving the document and cursor.
    pub fn spill(&mut self) -> Result<(), TextForgeError> {
        if let Some(text) = &self.materialized {
            let mut storage = SpooledTempFile::new_in(
                self.options.memory_threshold,
                &self.options.spill_directory,
            );
            storage.roll().map_err(write_error)?;
            storage
                .write_all(text.as_str().as_bytes())
                .map_err(write_error)?;
            self.storage = storage;
            self.materialized = None;
        }
        self.storage.roll().map_err(write_error)
    }

    /// Read raw bytes at a byte offset. A returned range may split a code point;
    /// the complete document remains valid UTF-8. Each call sets its own offset.
    pub fn read_at(
        &mut self,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, TextForgeError> {
        if offset >= self.len || destination.is_empty() {
            return Ok(0);
        }
        if let Some(text) = &self.materialized {
            let start = usize::try_from(offset).map_err(|error| invalid_utf8(error.to_string()))?;
            let bytes = text.as_str().as_bytes();
            let count = destination.len().min(bytes.len() - start);
            destination[..count].copy_from_slice(&bytes[start..start + count]);
            return Ok(count);
        }
        self.storage
            .seek(SeekFrom::Start(offset))
            .map_err(io_error)?;
        self.storage.read(destination).map_err(io_error)
    }

    /// Copy the whole document from offset zero, regardless of earlier reads.
    pub fn copy_to(&mut self, mut destination: impl Write) -> Result<u64, TextForgeError> {
        if let Some(text) = &self.materialized {
            destination
                .write_all(text.as_str().as_bytes())
                .map_err(write_error)?;
            return Ok(self.len);
        }
        self.storage.rewind().map_err(io_error)?;
        std::io::copy(&mut self.storage, &mut destination).map_err(write_error)
    }

    /// Explicitly materialize and consume this document. Memory-backed storage
    /// transfers its byte allocation; disk-backed storage requires a full allocation.
    pub fn into_string(mut self) -> Result<String, TextForgeError> {
        if let Some(text) = self.materialized.take() {
            return match text {
                MaterializedText::Owned(text) => Ok(text),
                MaterializedText::Shared(text) => {
                    let mut output = String::new();
                    output.try_reserve_exact(text.len()).map_err(|_| {
                        io_error(std::io::Error::other("Cannot materialize TextBuffer"))
                    })?;
                    output.push_str(&text);
                    Ok(output)
                }
            };
        }
        match self.storage.into_inner() {
            SpooledData::InMemory(cursor) => {
                String::from_utf8(cursor.into_inner()).map_err(invalid_utf8)
            }
            SpooledData::OnDisk(mut file) => {
                let len = usize::try_from(self.len).map_err(|error| {
                    TextForgeError::new(
                        TextForgeErrorCode::ValidationError(
                            "TextBuffer size cannot fit a String".into(),
                        ),
                        "TextBuffer::into_string",
                        error.to_string(),
                    )
                })?;
                let mut output = String::new();
                output.try_reserve_exact(len).map_err(|error| {
                    TextForgeError::new(
                        TextForgeErrorCode::ValidationError("Cannot materialize TextBuffer".into()),
                        "TextBuffer::into_string",
                        error.to_string(),
                    )
                })?;
                file.rewind().map_err(io_error)?;
                file.read_to_string(&mut output).map_err(io_error)?;
                Ok(output)
            }
        }
    }

    pub(crate) fn options(&self) -> &TextBufferOptions {
        &self.options
    }

    fn append_bytes(&mut self, bytes: &[u8]) -> Result<(), TextForgeError> {
        if self.materialized.is_some() {
            return self.append_text(std::str::from_utf8(bytes).map_err(invalid_utf8)?);
        }
        let len = self.len.checked_add(bytes.len() as u64).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::ValidationError("TextBuffer size overflow".into()),
                "TextBuffer",
                "append",
            )
        })?;
        self.storage.seek(SeekFrom::End(0)).map_err(io_error)?;
        self.storage.write_all(bytes).map_err(write_error)?;
        self.len = len;
        Ok(())
    }

    pub(crate) fn append_text(&mut self, text: &str) -> Result<(), TextForgeError> {
        match &mut self.materialized {
            Some(MaterializedText::Owned(current)) => {
                current
                    .try_reserve(text.len())
                    .map_err(|error| io_error(std::io::Error::other(error.to_string())))?;
                current.push_str(text);
                self.len = current.len() as u64;
                Ok(())
            }
            Some(MaterializedText::Shared(current)) => {
                let size = current
                    .len()
                    .checked_add(text.len())
                    .ok_or_else(|| io_error(std::io::Error::other("TextBuffer size overflow")))?;
                let mut output = String::new();
                output
                    .try_reserve_exact(size)
                    .map_err(|error| io_error(std::io::Error::other(error.to_string())))?;
                output.push_str(current);
                output.push_str(text);
                self.len = output.len() as u64;
                self.materialized = Some(MaterializedText::Owned(output));
                Ok(())
            }
            None => self.append_bytes(text.as_bytes()),
        }
    }

    fn copy_into(&mut self, output: &mut Self) -> Result<(), TextForgeError> {
        if let Some(text) = &self.materialized {
            return output.append_text(text.as_str());
        }
        self.storage.rewind().map_err(io_error)?;
        let mut chunk = [0; COPY_SIZE];
        loop {
            let count = self.storage.read(&mut chunk).map_err(io_error)?;
            if count == 0 {
                break;
            }
            output.append_bytes(&chunk[..count])?;
        }
        Ok(())
    }

    pub(crate) fn prepend(mut self, prefix: &str) -> Result<Self, TextForgeError> {
        if let Some(text) = self.materialized.take() {
            let size = prefix
                .len()
                .checked_add(text.as_str().len())
                .ok_or_else(|| io_error(std::io::Error::other("TextBuffer size overflow")))?;
            let mut output = String::new();
            output
                .try_reserve_exact(size)
                .map_err(|error| io_error(std::io::Error::other(error.to_string())))?;
            output.push_str(prefix);
            output.push_str(text.as_str());
            return Ok(Self::from_owned(output, self.options));
        }
        let mut output = Self::from_text(prefix, self.options.clone())?;
        self.copy_into(&mut output)?;
        output.retain_materialized = self.retain_materialized;
        Ok(output)
    }

    pub(crate) fn repeat(mut self, times: usize) -> Result<Self, TextForgeError> {
        self.len.checked_mul(times as u64).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::ValidationError("Repeated output size overflow".into()),
                "rpt",
                "buffered repeat",
            )
        })?;
        if times == 1 {
            return Ok(self);
        }
        if let Some(text) = self.materialized.take() {
            let input = text.as_str();
            let size = input
                .len()
                .checked_mul(times)
                .ok_or_else(|| io_error(std::io::Error::other("Repeated output size overflow")))?;
            let mut result = String::new();
            result
                .try_reserve_exact(size)
                .map_err(|_| io_error(std::io::Error::other("Cannot allocate repeated output")))?;
            if size != 0 {
                result.push_str(input);
                while result.len() <= size / 2 {
                    result.extend_from_within(..);
                }
                result.extend_from_within(..size - result.len());
            }
            return Ok(Self::from_owned(result, self.options));
        }
        let mut output = Self::empty(self.options.clone());
        output.retain_materialized = self.retain_materialized;
        if self.is_empty() || times == 0 {
            return Ok(output);
        }
        if self.len <= COPY_SIZE as u64 {
            // Repeat small documents using a bounded block rather than one I/O per copy.
            let mut block = Vec::new();
            block.try_reserve_exact(COPY_SIZE).map_err(|error| {
                TextForgeError::new(
                    TextForgeErrorCode::ValidationError("Cannot allocate repeat block".into()),
                    "rpt",
                    error.to_string(),
                )
            })?;
            block.resize(self.len as usize, 0);
            self.storage.rewind().map_err(io_error)?;
            self.storage.read_exact(&mut block).map_err(io_error)?;
            let copies_per_block = (COPY_SIZE / block.len()).min(times);
            let block_len = block.len() * copies_per_block;
            while block.len() <= block_len / 2 {
                block.extend_from_within(..);
            }
            block.extend_from_within(..block_len - block.len());
            for _ in 0..times / copies_per_block {
                output.append_bytes(&block)?;
            }
            let remaining = (times % copies_per_block) * self.len as usize;
            output.append_bytes(&block[..remaining])?;
        } else {
            for _ in 0..times {
                self.copy_into(&mut output)?;
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
mod test;

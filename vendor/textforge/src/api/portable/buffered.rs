use super::processor::{execution, native_size};
use super::{BindingError, FilePath, PortableProcessor};
use crate::{
    api::{TextBuffer, TextBufferOptions, processor::BufferedTask},
    utils::errors::{TextForgeError, TextForgeErrorCode},
};
use std::io::{Read, Write};

/// Owned spool settings with checked native-size conversion.
/// The threshold selects storage placement, not a hard global memory limit.
///
/// ```
/// # #[cfg(feature = "buffered")] {
/// use textforge::api::portable::{BufferOptions, FilePath};
/// let options = BufferOptions { memory_threshold: 1024, spill_directory: FilePath::from_text(".".into()) };
/// assert_eq!(options.memory_threshold, 1024);
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct BufferOptions {
    pub memory_threshold: u64,
    pub spill_directory: FilePath,
}
impl BufferOptions {
    pub(super) fn core(&self) -> Result<TextBufferOptions, BindingError> {
        let threshold = native_size(self.memory_threshold, "BufferOptions")?;
        std::fs::create_dir_all(self.spill_directory.as_path()).map_err(|error| {
            execution(TextForgeError::new(
                TextForgeErrorCode::FileOpeningError(error.to_string().into()),
                "BufferOptions.spill_directory",
                self.spill_directory.as_path().display().to_string(),
            ))
        })?;
        Ok(TextBufferOptions::new(
            threshold,
            self.spill_directory.0.clone(),
        ))
    }
}

/// Owned buffered batch request; successful results remain spilled buffers.
///
/// ```
/// # #[cfg(feature = "buffered")] {
/// use textforge::api::portable::{BufferTask, FilePath};
/// let task = BufferTask { pipeline_id: "id".into(), input: FilePath::from_text("in.txt".into()) };
/// assert_eq!(task.pipeline_id, "id");
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct BufferTask {
    pub pipeline_id: String,
    pub input: FilePath,
}

/// Synchronous foreign-stream read hook. Return at most `maximum` bytes or an
/// empty vector for EOF. The layer validates counts and UTF-8, not the adapter.
///
/// ```
/// # #[cfg(feature = "buffered")] {
/// use textforge::api::portable::StreamReader;
/// let mut reader = |_maximum: u64| Ok(Vec::new());
/// assert!(reader.read(64).unwrap().is_empty());
/// # }
/// ```
pub trait StreamReader {
    fn read(&mut self, maximum: u64) -> Result<Vec<u8>, String>;
}
impl<F: FnMut(u64) -> Result<Vec<u8>, String>> StreamReader for F {
    fn read(&mut self, maximum: u64) -> Result<Vec<u8>, String> {
        self(maximum)
    }
}

/// Synchronous write hook. Partial writes are retried by the core buffer copier.
/// The result must not exceed the supplied byte count.
///
/// ```
/// # #[cfg(feature = "buffered")] {
/// use textforge::api::portable::StreamWriter;
/// let mut writer = |bytes: Vec<u8>| Ok(bytes.len() as u64);
/// assert_eq!(writer.write(vec![1, 2]).unwrap(), 2);
/// # }
/// ```
pub trait StreamWriter {
    fn write(&mut self, bytes: Vec<u8>) -> Result<u64, String>;
}
impl<F: FnMut(Vec<u8>) -> Result<u64, String>> StreamWriter for F {
    fn write(&mut self, bytes: Vec<u8>) -> Result<u64, String> {
        self(bytes)
    }
}

/// Opaque UTF-8 document backed by the existing memory/disk spool.
/// Read byte ranges without implicit full-string materialization; explicit export
/// consumes the buffer. See [`PortableProcessor::process_all_buffered`].
///
/// ```
/// # #[cfg(feature = "buffered")] {
/// use textforge::api::portable::{PortableBuffer, BufferOptions, FilePath};
/// let options = BufferOptions { memory_threshold: 1024, spill_directory: FilePath::from_text(".".into()) };
/// let buffer = PortableBuffer::from_text("é", options).unwrap();
/// assert_eq!(buffer.len_bytes(), 2);
/// assert_eq!(buffer.into_string().unwrap(), "é");
/// # }
/// ```
#[derive(Debug)]
pub struct PortableBuffer {
    core: TextBuffer,
}
impl PortableBuffer {
    pub fn from_text(text: &str, options: BufferOptions) -> Result<Self, BindingError> {
        TextBuffer::from_text(text, options.core()?)
            .map(|core| Self { core })
            .map_err(execution)
    }
    pub fn from_file(path: &FilePath, options: BufferOptions) -> Result<Self, BindingError> {
        TextBuffer::from_file(path.as_path(), options.core()?)
            .map(|core| Self { core })
            .map_err(execution)
    }
    pub fn from_reader(
        reader: Box<dyn StreamReader>,
        options: BufferOptions,
    ) -> Result<Self, BindingError> {
        TextBuffer::from_reader(ReaderBridge(reader), options.core()?)
            .map(|core| Self { core })
            .map_err(execution)
    }
    pub fn len_bytes(&self) -> u64 {
        self.core.len_bytes()
    }
    pub fn is_empty(&self) -> bool {
        self.core.is_empty()
    }
    pub fn is_spilled(&self) -> bool {
        self.core.is_spilled()
    }
    pub fn spill(&mut self) -> Result<(), BindingError> {
        self.core.spill().map_err(execution)
    }
    pub fn read_at(&mut self, offset: u64, maximum: u64) -> Result<Vec<u8>, BindingError> {
        let count = maximum.min(self.len_bytes().saturating_sub(offset));
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(native_size(count, "PortableBuffer.read_at")?)
            .map_err(|error| {
                execution(TextForgeError::new(
                    TextForgeErrorCode::GenericError(error.to_string().into()),
                    "PortableBuffer.read_at",
                    "allocation",
                ))
            })?;
        bytes.resize(native_size(count, "PortableBuffer.read_at")?, 0);
        let read = self.core.read_at(offset, &mut bytes).map_err(execution)?;
        bytes.truncate(read);
        Ok(bytes)
    }
    pub fn copy_to(&mut self, writer: Box<dyn StreamWriter>) -> Result<u64, BindingError> {
        self.core.copy_to(WriterBridge(writer)).map_err(execution)
    }
    pub fn write_to_file(&mut self, path: &FilePath) -> Result<u64, BindingError> {
        crate::utils::validations::prepare_output_path(
            path.as_path(),
            "PortableBuffer.write_to_file",
        )
        .map_err(execution)?;
        let file = std::fs::File::create(path.as_path()).map_err(|error| {
            execution(TextForgeError::new(
                TextForgeErrorCode::FileOpeningError(error.to_string().into()),
                "PortableBuffer.write_to_file",
                path.as_path().display().to_string(),
            ))
        })?;
        self.core.copy_to(file).map_err(execution)
    }
    pub fn into_string(self) -> Result<String, BindingError> {
        self.core.into_string().map_err(execution)
    }
}

impl PortableProcessor {
    pub fn process_all_buffered(
        &mut self,
        id: &str,
        input: PortableBuffer,
    ) -> Result<PortableBuffer, BindingError> {
        self.core
            .process_all_buffered(id, input.core)
            .map(|core| PortableBuffer { core })
            .map_err(execution)
    }
    pub fn process_single_buffered(
        &mut self,
        instruction: &str,
        input: PortableBuffer,
    ) -> Result<PortableBuffer, BindingError> {
        self.core
            .process_single_buffered(instruction, input.core)
            .map(|core| PortableBuffer { core })
            .map_err(execution)
    }
    pub fn process_file_buffered(
        &mut self,
        id: &str,
        input: &FilePath,
        options: BufferOptions,
    ) -> Result<PortableBuffer, BindingError> {
        self.core
            .process_file_buffered(id, input.as_path(), options.core()?)
            .map(|core| PortableBuffer { core })
            .map_err(execution)
    }
    pub fn process_batch_buffered(
        &self,
        tasks: Vec<BufferTask>,
        options: BufferOptions,
    ) -> Vec<Result<PortableBuffer, BindingError>> {
        let options = match options.core() {
            Ok(options) => options,
            Err(error) => return tasks.iter().map(|_| Err(error.clone())).collect(),
        };
        let tasks = tasks
            .into_iter()
            .map(|task| BufferedTask::new(task.pipeline_id, task.input.0))
            .collect::<Vec<_>>();
        self.core
            .process_batch_buffered(&tasks, &options)
            .into_iter()
            .map(|result| {
                result
                    .map(|core| PortableBuffer { core })
                    .map_err(execution)
            })
            .collect()
    }
}

/// Validates foreign read sizes before passing bytes to the core UTF-8 reader.
///
/// ```
/// # #[cfg(feature = "buffered")] {
/// use textforge::api::portable::StreamReader;
/// let mut eof = |_maximum| Ok(Vec::<u8>::new());
/// assert!(eof.read(1).unwrap().is_empty());
/// # }
/// ```
struct ReaderBridge(Box<dyn StreamReader>);
impl Read for ReaderBridge {
    fn read(&mut self, destination: &mut [u8]) -> std::io::Result<usize> {
        let bytes = self
            .0
            .read(destination.len() as u64)
            .map_err(std::io::Error::other)?;
        if bytes.len() > destination.len() {
            return Err(std::io::Error::other(
                "Reader exceeded requested byte count",
            ));
        }
        destination[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}

/// Validates foreign partial-write counts for the existing core copy loop.
///
/// ```
/// # #[cfg(feature = "buffered")] {
/// use textforge::api::portable::StreamWriter;
/// let mut sink = |bytes: Vec<u8>| Ok(bytes.len() as u64);
/// assert_eq!(sink.write(vec![1]).unwrap(), 1);
/// # }
/// ```
struct WriterBridge(Box<dyn StreamWriter>);
impl Write for WriterBridge {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let count = self
            .0
            .write(bytes.to_vec())
            .map_err(std::io::Error::other)?;
        let count = usize::try_from(count).map_err(std::io::Error::other)?;
        if count > bytes.len() {
            return Err(std::io::Error::other("Writer exceeded supplied byte count"));
        }
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

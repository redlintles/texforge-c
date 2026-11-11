//! Opt-in whole-document execution and adapters preserving existing method signatures.
//!
//! With `buffered`, the three standard processing methods use a 1 MiB spool
//! threshold and the system temporary directory. String-returning methods still
//! materialize their final output. Batch retains parallel scheduling. Debug logs
//! spill separately and print only after successful pipeline execution; colored
//! snapshots materialize individually to preserve embedded ANSI reset handling.
use std::path::{Path, PathBuf};

use super::TextForgeProcessor;
use crate::api::{TextBuffer, TextBufferOptions};
use crate::context::execution_context::GlobalContextMethods;
use crate::context::execution_context::GlobalExecutionContext;
use crate::utils::errors::TextForgeErrorCode;
use crate::utils::errors::{TextForgeError, token_array_not_found};
use colored::Colorize;
use rayon::prelude::*;
use std::io::{Seek, Write};

/// An owned batch input descriptor suitable for wrapping behind future FFI handles.
/// Results keep descriptor order. The descriptor contains no Rust token objects.
///
/// ```
/// use textforge::api::processor::BufferedTask;
/// let task = BufferedTask::new("registered-pipeline-id", "input.txt");
/// assert_eq!(task.pipeline_id(), "registered-pipeline-id");
/// ```
#[derive(Debug)]
pub struct BufferedTask {
    pipeline_id: String,
    input_path: PathBuf,
}

impl BufferedTask {
    /// Own a pipeline identifier and input path for later execution.
    pub fn new(pipeline_id: impl Into<String>, input_path: impl Into<PathBuf>) -> Self {
        Self {
            pipeline_id: pipeline_id.into(),
            input_path: input_path.into(),
        }
    }
    /// Registered pipeline identifier.
    pub fn pipeline_id(&self) -> &str {
        &self.pipeline_id
    }
    /// Input document path.
    pub fn input_path(&self) -> &Path {
        &self.input_path
    }
}

impl TextForgeProcessor {
    fn run_buffered(&self, id: &str, mut input: TextBuffer) -> Result<TextBuffer, TextForgeError> {
        let tokens = self
            .transforms
            .get(id)
            .ok_or_else(token_array_not_found(id))?;
        let mut context = GlobalExecutionContext::new();
        for token in tokens {
            if context.is_halted {
                break;
            }
            input = token.apply_buffered(input, &mut context)?;
        }
        Ok(input)
    }

    /// Execute one complete logical document with one execution context.
    /// Prefix, suffix, repetition, null and end use native buffered operations;
    /// other instructions materialize through the existing transform implementation.
    ///
    /// ```
    /// use textforge::api::{TextBuffer, TextBufferOptions, TextForgeBuilderMethods};
    /// use textforge::api::processor::TextForgeProcessor;
    /// let mut processor = TextForgeProcessor::new();
    /// let id = processor.create_pipeline().add_to_beginning("<")?.add_to_end(">")?.build()?;
    /// let input = TextBuffer::from_text("👩🏽‍💻", TextBufferOptions::new(1, std::env::temp_dir()))?;
    /// let output = processor.process_all_buffered(&id, input)?;
    /// assert!(output.is_spilled());
    /// assert_eq!(output.into_string()?, "<👩🏽‍💻>");
    /// # Ok::<(), textforge::utils::errors::TextForgeError>(())
    /// ```
    pub fn process_all_buffered(
        &mut self,
        id: &str,
        input: TextBuffer,
    ) -> Result<TextBuffer, TextForgeError> {
        self.run_buffered(id, input)
            .inspect_err(|error| self.errors.add_error(error.clone()))
    }

    /// Parse and execute one textual instruction against a single owned document.
    /// This avoids exposing parser/token types in the new src/api entry point.
    pub fn process_single_buffered(
        &mut self,
        instruction: &str,
        input: TextBuffer,
    ) -> Result<TextBuffer, TextForgeError> {
        let result = crate::text::reader::read_from_text(instruction)
            .and_then(|token| token.apply_buffered(input, &mut GlobalExecutionContext::new()));
        result.inspect_err(|error| self.errors.add_error(error.clone()))
    }

    /// Read a file with bounded UTF-8 validation and process it as one document.
    /// No mmap is used by this execution path. Output conversion/writing is explicit.
    pub fn process_file_buffered(
        &mut self,
        id: &str,
        path: impl AsRef<Path>,
        options: TextBufferOptions,
    ) -> Result<TextBuffer, TextForgeError> {
        let result =
            TextBuffer::from_file(path, options).and_then(|input| self.run_buffered(id, input));
        result.inspect_err(|error| self.errors.add_error(error.clone()))
    }

    /// Execute independent documents sequentially, preserving result order and
    /// continuing after errors. Completed results are forced to disk so retaining
    /// the result vector does not retain one in-memory spool per successful task.
    /// This is deliberately a conservative experiment, not the legacy parallel scheduler.
    /// File descriptors, disk space and fallback allocations are not globally capped.
    ///
    /// ```
    /// use textforge::api::{TextBufferOptions, TextForgeBuilderMethods};
    /// use textforge::api::processor::TextForgeProcessor;
    /// let mut processor = TextForgeProcessor::new();
    /// let _id = processor.create_pipeline().add_to_end("!")?.build()?;
    /// let results = processor.process_batch_buffered(&[], &TextBufferOptions::new(1024, std::env::temp_dir()));
    /// assert!(results.is_empty());
    /// # Ok::<(), textforge::utils::errors::TextForgeError>(())
    /// ```
    pub fn process_batch_buffered(
        &self,
        tasks: &[BufferedTask],
        options: &TextBufferOptions,
    ) -> Vec<Result<TextBuffer, TextForgeError>> {
        tasks
            .iter()
            .map(|task| {
                TextBuffer::from_file(&task.input_path, options.clone())
                    .and_then(|input| self.run_buffered(&task.pipeline_id, input))
                    .and_then(|mut output| {
                        output.spill()?;
                        Ok(output)
                    })
            })
            .collect()
    }
}

fn adapter_options() -> TextBufferOptions {
    TextBufferOptions::new(1024 * 1024, std::env::temp_dir())
}

fn adapter_io(error: std::io::Error) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::FileWritingError(error.to_string().into()),
        "buffered processor adapter",
        "output or debug log",
    )
}

fn write_document(
    document: &mut TextBuffer,
    writer: &mut impl Write,
    color: &str,
) -> Result<(), TextForgeError> {
    if colored::control::SHOULD_COLORIZE.should_colorize() {
        // ColoredString also restores the outer style after embedded ANSI resets.
        // Preserve that behavior; color-enabled output materializes one snapshot.
        let mut bytes = Vec::new();
        document.copy_to(&mut bytes)?;
        let text = String::from_utf8(bytes).map_err(|error| {
            TextForgeError::new(
                TextForgeErrorCode::InvalidUtf8Error(error.to_string().into()),
                "buffered debug",
                "snapshot",
            )
        })?;
        if color == "\x1b[31m" {
            write!(writer, "{}", text.red()).map_err(adapter_io)?;
        } else {
            write!(writer, "{}", text.green()).map_err(adapter_io)?;
        }
    } else {
        document.copy_to(writer)?;
    }
    Ok(())
}

impl TextForgeProcessor {
    pub(super) fn process_all_via_buffer(
        &mut self,
        id: &str,
        input: &str,
    ) -> Result<String, TextForgeError> {
        let result = TextBuffer::from_text(input, adapter_options())
            .and_then(|input| self.run_buffered(id, input.retain_materialized()))
            .and_then(TextBuffer::into_string);
        result.inspect_err(|error| self.errors.add_error(error.clone()))
    }

    // Preserve Rayon scheduling and per-task error isolation. Write each completed
    // document immediately instead of retaining a batch of in-memory strings.
    pub(super) fn process_batch_via_buffer(
        &self,
        tasks: Vec<(&Path, &str, &Path)>,
    ) -> Vec<Result<(), TextForgeError>> {
        tasks
            .into_par_iter()
            .map(|(origin, id, target)| {
                if !origin.exists() {
                    return Err(TextForgeError::new(
                        TextForgeErrorCode::FileNotFound("Origin file does not exist".into()),
                        "process_batch",
                        format!("{origin:?}"),
                    ));
                }
                if !origin.is_file() {
                    return Err(TextForgeError::new(
                        TextForgeErrorCode::ValidationError("Origin path is not a file".into()),
                        "process_batch",
                        format!("{origin:?}"),
                    ));
                }
                let file = std::fs::File::open(origin).map_err(|error| {
                    TextForgeError::new(
                        TextForgeErrorCode::FileOpeningError("Failed to open origin file".into()),
                        "process_batch",
                        format!("{origin:?} - {error}"),
                    )
                })?;
                let input = TextBuffer::from_reader(file, adapter_options())?.retain_materialized();
                let mut output = self.run_buffered(id, input)?;
                if let Some(parent) = target.parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent).map_err(adapter_io)?;
                }
                let file = std::fs::File::create(target).map_err(adapter_io)?;
                output.copy_to(file)?;
                Ok(())
            })
            .collect()
    }

    // Keep the log spooled until the whole pipeline succeeds, matching the legacy
    // no-partial-log behavior on transformation errors. No whole-log String exists.
    pub(super) fn process_debug_via_buffer(
        &mut self,
        id: &str,
        input: &str,
    ) -> Result<String, TextForgeError> {
        let result = (|| {
            let options = adapter_options();
            let mut log = tempfile::SpooledTempFile::new_in(
                options.memory_threshold(),
                options.spill_directory(),
            );
            let input = TextBuffer::from_text(input, options)?.retain_materialized();
            let output = self.run_buffered_debug(id, input, &mut log)?;
            log.rewind().map_err(adapter_io)?;
            let stdout = std::io::stdout();
            let mut stdout = stdout.lock();
            std::io::copy(&mut log, &mut stdout).map_err(adapter_io)?;
            output.into_string()
        })();
        result.inspect_err(|error| self.errors.add_error(error.clone()))
    }

    fn run_buffered_debug(
        &self,
        id: &str,
        mut input: TextBuffer,
        log: &mut impl Write,
    ) -> Result<TextBuffer, TextForgeError> {
        let tokens = self
            .transforms
            .get(id)
            .ok_or_else(token_array_not_found(id))?;
        let mut context = GlobalExecutionContext::new();
        log.write_all(b"PROCESSING STEP BY STEP:\n----------\n\n")
            .map_err(adapter_io)?;
        for (counter, token) in tokens.iter().enumerate() {
            if context.is_halted {
                break;
            }
            let bound = token.bind_for_debug(&mut context)?;
            let line = bound.to_textforge_line();
            if bound.get_string_repr() == "blk" {
                let name = match bound.get_params().first() {
                    Some(crate::parser::params::TextForgeParamTypes::String(name)) => name,
                    _ => {
                        return Err({
                            TextForgeError::new(
                                TextForgeErrorCode::IndexOutOfRange("Invalid BLK Block".into()),
                                "process_all_with_debug",
                                "",
                            )
                        });
                    }
                };
                input = bound.transform_buffered(input, Some(&mut context))?;
                write!(log, "Step: [{}] => [{}]\n{}\n\tBlock Instruction: {}\t\tBlock Name: {}\n\t\t\tCurrent instructions Associated to this Block:\n{}",
                    counter.to_string().blue(), (counter + 1).to_string().blue(),
                    "Block Declaration: ".green(), line.yellow(), name.green(),
                    context.get_formatted_block_items(name)?).map_err(adapter_io)?;
            } else {
                write!(
                    log,
                    "Step: [{}] => [{}]\nInstruction: {}\nBefore: ",
                    counter.to_string().blue(),
                    (counter + 1).to_string().blue(),
                    line.yellow()
                )
                .map_err(adapter_io)?;
                write_document(&mut input, log, "\x1b[31m")?;
                input = bound.transform_buffered(input, Some(&mut context))?;
                log.write_all(b"\nAfter: ").map_err(adapter_io)?;
                write_document(&mut input, log, "\x1b[32m")?;
                log.write_all(b"\n\n").map_err(adapter_io)?;
            }
            if counter + 1 < tokens.len() {
                log.write_all(b"----------\n\n").map_err(adapter_io)?;
            }
        }
        Ok(input)
    }
}

#[cfg(feature = "watchers")]
impl TextForgeProcessor {
    // WatcherContext owns Arc<str> snapshots. Keep the same previous/current/next
    // semantics, including after=None only at EOF and reset only after export.
    // Transformations use the experimental engine, but observers still require
    // whole-document materialization; this is not a bounded-memory watcher API.
    pub(super) fn process_watchers_via_buffer(
        &mut self,
        id: &str,
        input: &str,
        watchers: &mut crate::watchers::WatcherList,
        report_path: &Path,
    ) -> Result<String, TextForgeError> {
        use crate::watchers::WatcherContext;
        use std::sync::Arc;
        let tokens = match self
            .transforms
            .get(id)
            .ok_or_else(token_array_not_found(id))
        {
            Ok(tokens) => tokens,
            Err(error) => {
                self.errors.add_error(error.clone());
                return Err(error);
            }
        };
        let mut context = GlobalExecutionContext::new();
        let mut current: Arc<str> = Arc::from(input);
        let mut pending: Option<(Arc<str>, Arc<str>, String)> = None;
        for token in tokens {
            let before = Arc::clone(&current);
            let document = TextBuffer::from_shared(Arc::clone(&current), adapter_options());
            let output = token.apply_buffered(document, &mut context)?;
            current = output.into_shared(&before)?;
            if let Some((previous_before, previous_current, instruction)) = pending.take() {
                watchers.run_watchers(WatcherContext::new(
                    previous_current,
                    previous_before,
                    Some(Arc::clone(&current)),
                    instruction,
                ))?;
            }
            pending = Some((
                before,
                Arc::clone(&current),
                token.to_textforge_line().to_string(),
            ));
        }
        if let Some((before, current, instruction)) = pending {
            watchers.run_watchers(WatcherContext::new(
                current,
                before,
                None::<Arc<str>>,
                instruction,
            ))?;
        }
        watchers.to_json(report_path)?;
        watchers.reset();
        Ok(current.to_string())
    }
}

#[cfg(test)]
mod adapter_tests;

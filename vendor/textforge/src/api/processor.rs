#[cfg(feature = "buffered")]
mod buffered;
mod file_io;
#[cfg(feature = "buffered")]
pub use buffered::BufferedTask;

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use colored::*;
#[cfg(not(feature = "buffered"))]
use rayon::prelude::*;
use uuid::Uuid;

#[cfg(all(feature = "watchers", not(feature = "buffered")))]
use std::sync::Arc;

#[cfg(feature = "bytecode")]
use crate::bytecode::{reader::read_bytecode_from_file, writer::write_bytecode_to_file};
#[cfg(all(feature = "watchers", not(feature = "buffered")))]
use crate::watchers::WatcherContext;
#[cfg(feature = "watchers")]
use crate::watchers::WatcherList;

use crate::api::builder::TextForgeBuilder;
#[cfg(not(feature = "buffered"))]
use crate::context::execution_context::GlobalContextMethods;
use crate::context::execution_context::GlobalExecutionContext;
use crate::parser::resolve_var::TokenWrapper;
use crate::text::reader::read_from_file;
use crate::text::writer::write_to_file;
use crate::utils::apply::apply_transform;
use crate::utils::errors::{
    ErrorManager, TextForgeError, TextForgeErrorCode, token_array_not_found,
};

/// TextForge Processor
///
/// `TextForgeProcessor` is the main **execution engine** of TextForge (formerly ATP).
///
/// It stores multiple linear transformation pipelines (called **transforms**) identified
/// by a `String` ID (generated with UUID). Each transform is a `Vec<TokenWrapper>`,
/// i.e. a sequence of tokens executed from left to right.
///
/// # Core concepts
///
/// ## Transform (pipeline)
/// A *transform* is a vector of tokens executed sequentially:
///
/// 1. Start with `result = input`
/// 2. For each token in order: `result = token.transform(result)`
/// 3. Return the final `result`
///
/// This matches TextForge's **linear** nature: there is no implicit nesting at runtime.
/// Any logical grouping/abstraction lives at the builder / authoring layer.
///
/// A pipeline can end up registered in `self.transforms` through more than one path — built
/// in-memory via [`create_pipeline`](Self::create_pipeline), parsed from a `.textforge` text
/// file via [`get_pipeline_from_file`](TextForgeProcessorMethods::get_pipeline_from_file), or
/// loaded from a `.textforgebc` bytecode file via
/// `read_from_bytecode_file` — but once
/// registered, every pipeline is executed the exact same way by the `process_*` methods.
/// Reading/writing bytecode never processes input directly; it only moves pipelines in and out
/// of `self.transforms`.
///
/// ## Error accumulation (WIP)
/// `TextForgeProcessor` contains an `ErrorManager` meant to accumulate errors found during
/// reading/writing/execution. At the moment, `ErrorManager` is still under construction;
/// therefore, this should be treated as an internal error sink rather than a stable public
/// diagnostics API.
///
/// # Examples
///
/// ## 1) Build a pipeline through the processor, then run it
///
/// ```rust
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
/// use textforge::api::TextForgeBuilderMethods;
///
/// let mut processor = TextForgeProcessor::new();
///
/// // Build + register a transform; `build()` returns its ID.
/// let id = processor
///     .create_pipeline()
///     .trim_both_sides()?
///     .add_to_end("!")?
///     .build().unwrap();
///
/// let out = processor.process_all(&id, "   banana   ")?;
/// assert_eq!(out, "banana!");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
///
/// ## 2) Step-by-step debug execution (SBS)
///
/// ```rust
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
/// use textforge::api::TextForgeBuilderMethods;
///
/// let mut processor = TextForgeProcessor::new();
///
/// let id = processor
///     .create_pipeline()
///     .add_to_beginning("Banana")?
///     .add_to_end("pizza")?
///     .repeat(3)?
///     .trim_both_sides()?
///     .build().unwrap();
///
/// // Prints each step: instruction + before/after.
/// let out = processor.process_all_with_debug(&id, "Banana Laranja cheia de canja")?;
/// println!("{out}");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
///
/// ## 3) Quick single-token execution (no pipeline registration)
///
/// ```rust
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
/// use textforge::tokens::InstructionMethods;
/// use textforge::tokens::transforms::tbs;
///
/// let mut processor = TextForgeProcessor::new();
///
/// let token = (Box::new(tbs::Tbs::default()) as Box<dyn InstructionMethods>).into();
/// let out = processor.process_single(token, "   banana   ")?;
/// assert_eq!(out, "banana");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
///
/// ## 4) Benchmark-style usage (as in your tests)
///
/// This mirrors the exact usage pattern shown in your suite:
///
/// ```rust
/// use textforge::api::{
///     TextForgeBuilderMethods,
///     processor::{ TextForgeProcessor, TextForgeProcessorMethods },
/// };
/// use std::time::Instant;
///
/// # fn main() -> Result<(), textforge::utils::errors::TextForgeError> {
/// let runs = 100;
/// let mut total = 0.0;
///
/// let mut processor = TextForgeProcessor::new();
///
/// let id = processor
///     .create_pipeline()
///     .add_to_beginning("Banana")?
///     .add_to_end("pizza")?
///     .repeat(3)?
///     .delete_after(20)?
///     .delete_before(3)?
///     .delete_chunk(0, 3)?
///     .delete_first()?
///     .delete_last()?
///     .replace_all_with("a", "e")?
///     .replace_first_with("L", "coxinha")?
///     .replace_count_with("e", "carro", 3)?
///     .insert(0, "Coxinha Banana")?
///     .rotate_left(1)?
///     .rotate_right(2)?
///     .trim_both_sides()?
///     .trim_left_side()?
///     .trim_right_side()?
///     .add_to_beginning("laranjadebananavermelha")?
///     .select(3, 7)?
///     .replace_count_with("a", "b", 3)?
///     .to_uppercase_all()?
///     .to_lowercase_all()?
///     .to_uppercase_single(3)?
///     .to_lowercase_single(2)?
///     .capitalize_first_word()?
///     .capitalize_single_word(1)?
///     .capitalize_last_word()?
///     .capitalize_range(0, 3)?
///     .split_select("B", 1)?
///     .capitalize_chunk(0, 3)?
///     .replace_last_with("b", "c")?
///     .replace_nth_with("b", "d", 3)?
///     .to_url_encoded()?
///     .to_url_decoded()?
///     .to_reverse()?
///     .split_characters()?
///     .to_html_escaped()?
///     .to_html_unescaped()?
///     .to_json_escaped()?
///     .to_json_unescaped()?
///     .insert(1, "banana")?
///     .to_uppercase_chunk(1, 3)?
///     .to_lowercase_chunk(0, 5)?
///     .join_to_camel_case()?
///     .join_to_kebab_case()?
///     .join_to_pascal_case()?
///     .join_to_snake_case()?
///     .pad_left("xy", 12)?
///     .pad_right("yx", 20)?
///     .build().unwrap();
///
/// for _ in 0..runs {
///     let start = Instant::now();
///     let input = "Banana Laranja cheia de canja";
///     let _ = processor.process_all(&id, input)?;
///     total += start.elapsed().as_secs_f64();
/// }
///
/// let avg = total / runs as f64;
/// println!("Average: {avg:.6}s");
/// # Ok(())
/// # }
/// ```
///
/// # Notes
///
/// - `build()` registers a new transform entry inside the processor and returns its UUID.
/// - The pipeline is **one giant vector** of tokens; execution is deterministic and ordered.
/// - Debug methods (`*_with_debug`) only add printing; they do not change execution.
pub struct TextForgeProcessor {
    transforms: HashMap<String, Vec<TokenWrapper>>,
    errors: ErrorManager,
}

/// Operational API for `TextForgeProcessor`.
///
/// This trait defines the public "surface" of the processor: how pipelines are registered,
/// executed, persisted, inspected, and removed.
///
/// A **transform** is stored internally as:
/// `HashMap<String, Vec<TokenWrapper>>`
///
/// Where the key is a UUID string and the value is a linear sequence of tokens.
///
/// ## Error reporting
/// Most methods will:
/// - return `Err(TextForgeError)` on failure
/// - and also push a copy into the internal `ErrorManager` (where you already do that)
///
/// The exact behavior depends on the implementation (and your `ErrorManager` is still WIP).
///
/// ## Method groups
/// Methods below are grouped by responsibility:
/// - **Pipeline persistence (text)** — move a pipeline to/from a `.textforge` text file.
/// - **Pipeline registration & inspection** — register, remove, list, or clone pipelines.
/// - **Processing** — the only methods that ever run a pipeline against input text.
/// - **Bytecode persistence** (`bytecode` feature) — move a pipeline to/from a `.textforgebc`
///   binary file. Like the text persistence methods, these never process input directly; they
///   only read/write `self.transforms`. To run a bytecode-loaded pipeline, register it with
///   `read_from_bytecode_file` and then call any of the
///   `process_*` methods, the same way you would for a text- or builder-originated pipeline.
pub trait TextForgeProcessorMethods {
    // -----------------------------------------------------------------
    // Pipeline persistence (text format)
    // -----------------------------------------------------------------

    /// Writes a registered transform (pipeline) to a `.textforge` text file.
    ///
    /// Internally:
    /// - looks up `id` in `self.transforms`
    /// - if found, calls `write_to_file(path, tokens)`
    /// - if not found, returns `TokenArrayNotFound` (via `token_array_not_found`)
    ///
    /// # Parameters
    /// - `id`: Transform identifier previously returned by `add_transform()` / `build()`.
    /// - `path`: Destination path to write the textual representation.
    ///
    /// # Errors
    /// Returns `Err` if:
    /// - the transform does not exist
    /// - writing fails (I/O or serialization problems inside `write_to_file`)
    fn write_pipeline_to_file(&mut self, id: &str, path: &Path) -> Result<(), TextForgeError>;

    /// Reads a `.textforge` text file, parses it into tokens, registers it as a new transform,
    /// and returns the newly created transform ID.
    ///
    /// Internally:
    /// - reads and parses tokens via `read_from_file(path)`
    /// - generates a new UUID
    /// - inserts the parsed vector into `self.transforms`
    ///
    /// This only registers the pipeline — it does not process any input. To run it, pass the
    /// returned ID to one of the `process_*` methods.
    ///
    /// # Returns
    /// The UUID string identifying the newly registered transform.
    ///
    /// # Errors
    /// Returns `Err` if reading/parsing the file fails.
    fn get_pipeline_from_file(&mut self, path: &Path) -> Result<String, TextForgeError>;

    // -----------------------------------------------------------------
    // Pipeline registration & inspection
    // -----------------------------------------------------------------

    /// Registers a new transform (pipeline) directly from a token vector.
    ///
    /// This is the low-level "insert" API. Higher-level builder APIs typically call this.
    ///
    /// Internally:
    /// - generates a new UUID
    /// - inserts `(uuid -> tokens)` into `self.transforms`
    ///
    /// # Returns
    /// The UUID string identifying the newly registered transform.
    fn add_transform(&mut self, tokens: Vec<TokenWrapper>) -> String;

    /// Removes a transform from the processor.
    ///
    /// Internally:
    /// - performs `self.transforms.remove(id)`
    /// - returns `Ok(())` if something was removed
    /// - returns `Err(TokenNotFound)` (your custom error) if the ID does not exist
    ///
    /// # Errors
    /// Returns `Err` if the transform does not exist.
    fn remove_transform(&mut self, id: &str) -> Result<(), TextForgeError>;

    /// Displays the list of registered transform IDs.
    ///
    /// Your current implementation prints:
    /// - an index counter
    /// - the UUID key
    ///
    /// # Note
    /// This is pure side-effect (stdout). It does not return the data.
    ///
    /// The trait provides a default empty body `{}` so implementors may override it.
    fn show_transforms(&self) {}

    /// Checks whether a transform with the given `id` exists.
    ///
    /// Internally: `self.transforms.contains_key(id)`
    fn transform_exists(&self, id: &str) -> bool;

    /// Returns a **cloned** copy of the token vector for a given transform `id`.
    ///
    /// This method is useful for:
    /// - inspection
    /// - exporting
    /// - composing transforms (if you later support merging)
    ///
    /// # Returns
    /// A cloned `Vec<TokenWrapper>`.
    ///
    /// # Errors
    /// Returns `Err(TokenArrayNotFound)` if the transform does not exist.
    fn get_transform_vec(&self, id: &str) -> Result<Vec<TokenWrapper>, TextForgeError>;

    /// Returns the textual `.textforge` lines for a given transform `id`.
    ///
    /// Internally:
    /// - clones the transform vector
    /// - maps each token to `token.to_textforge_line().to_string()`
    ///
    /// This is typically what you want for:
    /// - UI display
    /// - exporting to text
    /// - debugging what the pipeline "looks like"
    ///
    /// # Errors
    /// Returns `Err(TokenArrayNotFound)` if the transform does not exist.
    fn get_text_transform_vec(&self, id: &str) -> Result<Vec<String>, TextForgeError>;

    // -----------------------------------------------------------------
    // Processing — the only methods that run a pipeline against input
    // -----------------------------------------------------------------

    /// Executes a registered transform against `input` and returns the result, without
    /// recording anything into `self.errors`.
    ///
    /// This is the shared low-level execution primitive behind
    /// [`process_all`](Self::process_all) and [`process_batch`](Self::process_batch): both
    /// look up `id`, then run each token in order over a fresh, local `ErrorManager` and
    /// `GlobalExecutionContext`.
    ///
    /// Prefer `process_all` when you want failures recorded in the processor's error log;
    /// use `run_transform` directly when you want execution without touching `self.errors`
    /// (e.g. read-only access, as in `process_batch`, which only borrows `&self`).
    ///
    /// # Errors
    /// Returns `Err` if:
    /// - the transform does not exist
    /// - any token execution fails
    fn run_transform<'a>(
        &self,
        id: &str,
        input: Cow<'a, str>,
    ) -> Result<Cow<'a, str>, TextForgeError>;

    /// Executes all tokens of a registered transform from left to right.
    ///
    /// Semantics:
    /// - `result` starts as `input`
    /// - for each token `t` in the transform:
    ///   - `result = parse_token(t, result, &mut self.errors)?`
    /// - returns the final `result`
    ///
    /// `parse_token` is used instead of calling `token.transform` directly because it can
    /// integrate with your parsing/diagnostics/error flow (and will likely be where conditional
    /// execution, blocks, etc. plug in later).
    ///
    /// # Parameters
    /// - `id`: Transform identifier.
    /// - `input`: Input string to process.
    ///
    /// # Errors
    /// Returns `Err` if:
    /// - the transform does not exist
    /// - any token execution fails (propagated from `parse_token`)
    fn process_all(&mut self, id: &str, input: &str) -> Result<String, TextForgeError>;

    /// Executes a single token over `input`, without registering it into the processor.
    ///
    /// This is a convenience method for ad-hoc transformations:
    /// - calls `token.transform(input)`
    /// - stores any encountered error in the internal error manager (in your impl)
    ///
    /// # Parameters
    /// - `token`: The token to execute once.
    /// - `input`: Input string.
    ///
    /// # Errors
    /// Returns `Err` if the token's `transform` fails.
    fn process_single(
        &mut self,
        token: TokenWrapper,
        input: &str,
    ) -> Result<String, TextForgeError>;

    /// Executes a registered transform like `process_all`, but prints each step.
    ///
    /// Intended for debugging and teaching:
    /// - prints an SBS header
    /// - for each token:
    ///   - computes `temp = parse_token(...)`
    ///   - prints: step index, instruction (`to_textforge_line()`), before, after
    /// - returns the final result
    ///
    /// # Parameters
    /// - `id`: Transform identifier.
    /// - `input`: Input string.
    ///
    /// # Errors
    /// Same error behavior as `process_all`.
    fn process_all_with_debug(&mut self, id: &str, input: &str) -> Result<String, TextForgeError>;

    /// Executes a single token like `process_single`, but prints a single SBS step.
    ///
    /// Intended for debugging token behavior in isolation.
    ///
    /// Prints:
    /// - Step 0 -> 1
    /// - Instruction (`to_textforge_line()`)
    /// - Before / After
    ///
    /// # Errors
    /// Returns `Err` if the token's `transform` fails.
    fn process_single_with_debug(
        &mut self,
        token: TokenWrapper,
        input: &str,
    ) -> Result<String, TextForgeError>;

    /// Reads an owned UTF-8 document from `file_path`, runs `pipeline_id`, and
    /// writes the result to `output_path`. No memory mapping or unsafe file-backed
    /// references are used. Output parent directories are created as needed.
    ///
    /// # Parameters
    /// - `pipeline_id`: ID of a transform previously registered via `add_transform` /
    ///   `build()` / `get_pipeline_from_file` / `read_from_bytecode_file`.
    /// - `file_path`: Path to the input file to read. Must already exist.
    /// - `output_path`: Path to write the transformed output to.
    ///
    /// # Errors
    /// Returns `Err` if:
    /// - `file_path` fails to open or read
    /// - the file's contents are not valid UTF-8
    /// - `pipeline_id` does not match any registered transform, or any token fails during
    ///   execution
    /// - `output_path`'s parent directory fails to be created
    /// - `output_path` fails to open or write
    fn process_file(
        &mut self,
        pipeline_id: &str,
        file_path: &Path,
        output_path: &Path,
    ) -> Result<(), TextForgeError>;

    /// Runs a pipeline over many `(input file, output file)` pairs in parallel.
    ///
    /// Each task is `(origin, pipeline_id, target)`:
    /// - `origin`: path to the input file to read. Must already exist.
    /// - `pipeline_id`: ID of a transform previously registered via `add_transform` /
    ///   `build()` / `get_pipeline_from_file` / `read_from_bytecode_file`.
    /// - `target`: path to write the transformed output to. Its parent directory is
    ///   created automatically if it doesn't exist yet.
    ///
    /// Tasks are distributed across a `rayon` thread pool (`into_par_iter`), so origin
    /// files are read, transformed, and written concurrently. Tasks are fully
    /// independent: one task failing does not stop or affect the others, and the
    /// returned `Vec` preserves the same order as the input `tasks` vector, so
    /// `results[i]` always corresponds to `tasks[i]`.
    ///
    /// Internally, for each task:
    /// - checks that `origin` exists and is a file (not a directory)
    /// - reads an owned UTF-8 document from `origin`
    /// - runs the registered pipeline via `run_transform(pipeline_id, &input)`
    /// - creates `target`'s parent directory if it doesn't exist yet (`create_dir_all`)
    /// - writes the result to `target` with `std::fs::write`
    ///
    /// Neither `origin` nor `target` are required to have any particular file
    /// extension.
    ///
    /// # Parameters
    /// - `tasks`: the `(origin, pipeline_id, target)` triples to process.
    ///
    /// # Returns
    /// One `Result<(), TextForgeError>` per task, in the same order as `tasks`.
    ///
    /// # Errors
    /// A task's `Err` can come from:
    /// - `origin` not existing (`FileNotFound`) or not being a file (`ValidationError`)
    /// - `origin` failing to read (permissions, invalid UTF-8, ...) (`FileReadingError`)
    /// - `pipeline_id` not matching any registered transform (`TokenArrayNotFound`)
    /// - any token in the pipeline failing during execution
    /// - `target`'s parent directory failing to be created (`FileWritingError`)
    /// - `target` failing to write (`FileWritingError`)
    fn process_batch(&self, tasks: Vec<(&Path, &str, &Path)>) -> Vec<Result<(), TextForgeError>>;

    /// Executes a registered transform like [`process_all`](Self::process_all), but also runs
    /// `watcher_list`'s diagnostics after each step and exports the resulting report as JSON.
    ///
    /// For every step after the first, a [`WatcherContext`](crate::watchers::WatcherContext) is built from the *previous* step's
    /// `before`/`current` values plus the *current* step's output (as that previous step's
    /// `after`), then passed to `watcher_list.run_watchers`. The final step has no following
    /// step to supply an `after`, so its context is run with `after = None`. Once every step has
    /// been processed, the report is written to `report_path` via `watcher_list.to_json`, and
    /// `watcher_list` is reset so it can be reused for the next call.
    ///
    /// Available only with the `watchers` feature.
    ///
    /// # Parameters
    /// - `id`: Transform identifier.
    /// - `input`: Input string to process.
    /// - `watcher_list`: The watchers to run after each step.
    /// - `report_path`: Destination path for the exported JSON report.
    ///
    /// # Errors
    /// Returns `Err` if the transform does not exist, any token execution fails, a watcher run
    /// fails, or the report fails to serialize/write.
    #[cfg(feature = "watchers")]
    fn process_all_with_watchers(
        &mut self,
        id: &str,
        input: &str,
        watcher_list: &mut WatcherList,
        report_path: &Path,
    ) -> Result<String, TextForgeError>;

    // -----------------------------------------------------------------
    // Bytecode persistence (feature = "bytecode")
    //
    // These two methods only move a pipeline in and out of the binary `.textforgebc`
    // format — they never process input directly. Once a pipeline is registered (from
    // bytecode, text, or built in-memory), any `process_*` method above can run it.
    // -----------------------------------------------------------------

    /// Writes a registered transform to a `.textforgebc` bytecode file.
    ///
    /// Available only with the `bytecode` feature.
    ///
    /// Internally:
    /// - looks up `id` in `self.transforms`
    /// - calls `write_bytecode_to_file(path, tokens.to_vec())`
    ///
    /// # Errors
    /// Returns `Err` if the transform does not exist or bytecode writing fails.
    #[cfg(feature = "bytecode")]
    fn write_to_bytecode_file(&mut self, id: &str, path: &Path) -> Result<(), TextForgeError>;

    /// Reads a `.textforgebc` bytecode file and registers it as a new transform, returning
    /// its ID.
    ///
    /// Available only with the `bytecode` feature.
    ///
    /// Internally:
    /// - parses tokens via `read_bytecode_from_file(path)`
    /// - registers them using `add_transform`
    ///
    /// This only registers the pipeline — it does not process any input. To run it, pass the
    /// returned ID to one of the `process_*` methods (e.g. `process_all`,
    /// `process_all_with_debug`).
    ///
    /// # Returns
    /// The UUID string identifying the newly registered transform.
    ///
    /// # Errors
    /// Returns `Err` if bytecode reading/parsing fails.
    #[cfg(feature = "bytecode")]
    fn read_from_bytecode_file(&mut self, path: &Path) -> Result<String, TextForgeError>;
}

/// Creates an empty processor using the standard default-construction trait.
///
/// ```
/// use textforge::api::{processor::TextForgeProcessor, TextForgeBuilderMethods};
/// let mut processor = TextForgeProcessor::default();
/// let id = processor.create_pipeline().trim_both_sides().unwrap().build().unwrap();
/// assert!(!id.is_empty());
/// ```
impl Default for TextForgeProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl TextForgeProcessor {
    /// Creates a new empty processor.
    ///
    /// - No transforms are registered initially.
    /// - The internal `ErrorManager` is initialized with `Default`.
    pub fn new() -> Self {
        TextForgeProcessor {
            transforms: HashMap::new(),
            errors: ErrorManager::default(),
        }
    }

    /// Creates a `TextForgeBuilder` bound to this processor.
    ///
    /// The builder accumulates tokens and, when `build()` is called, it registers a new
    /// transform entry inside this processor and returns the corresponding transform ID.
    ///
    /// # Example
    ///
    /// ```rust
    /// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
    /// use textforge::api::TextForgeBuilderMethods;
    ///
    /// let mut processor = TextForgeProcessor::new();
    ///
    /// let id = processor
    ///     .create_pipeline()
    ///     .trim_both_sides()?
    ///     .add_to_end("!")?
    ///     .build().unwrap();
    ///
    /// let out = processor.process_all(&id, "   banana   ")?;
    /// assert_eq!(out, "banana!");
    /// # Ok::<(), textforge::utils::errors::TextForgeError>(())
    /// ```
    pub fn create_pipeline(&mut self) -> TextForgeBuilder<'_> {
        TextForgeBuilder::new(self)
    }
}

impl TextForgeProcessorMethods for TextForgeProcessor {
    // -- Pipeline persistence (text format) --------------------------

    fn write_pipeline_to_file(&mut self, id: &str, path: &Path) -> Result<(), TextForgeError> {
        let tokens = match self
            .transforms
            .get(id)
            .ok_or_else(token_array_not_found(id))
        {
            Ok(x) => x,
            Err(e) => {
                self.errors.add_error(e.clone());
                return Err(e);
            }
        };

        write_to_file(Path::new(path), tokens)
    }

    fn get_pipeline_from_file(&mut self, path: &Path) -> Result<String, TextForgeError> {
        let tokens = match read_from_file(Path::new(path)) {
            Ok(x) => x,
            Err(e) => {
                self.errors.add_error(e.clone());
                return Err(e);
            }
        };

        Ok(self.add_transform(tokens))
    }

    // -- Pipeline registration & inspection ---------------------------

    fn add_transform(&mut self, mut tokens: Vec<TokenWrapper>) -> String {
        for token in &mut tokens {
            token.prepare_constant();
        }
        let identifier = Uuid::new_v4().to_string();
        self.transforms.insert(identifier.clone(), tokens);
        identifier
    }

    fn remove_transform(&mut self, id: &str) -> Result<(), TextForgeError> {
        match self.transforms.remove(id).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::TokenNotFound("Transformation not found".into()),
                "remove_transform",
                id.to_string(),
            )
        }) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn show_transforms(&self) {
        for (i, k) in self.transforms.keys().enumerate() {
            println!("{} - {}", i, k);
        }
    }

    fn transform_exists(&self, id: &str) -> bool {
        self.transforms.contains_key(id)
    }

    fn get_transform_vec(&self, id: &str) -> Result<Vec<TokenWrapper>, TextForgeError> {
        Ok(self
            .transforms
            .get(id)
            .ok_or_else(|| {
                TextForgeError::new(
                    TextForgeErrorCode::TokenArrayNotFound("Transform not found".into()),
                    "get_transform_vec".to_string(),
                    id.to_string(),
                )
            })?
            .clone())
    }

    fn get_text_transform_vec(&self, id: &str) -> Result<Vec<String>, TextForgeError> {
        self.transforms
            .get(id)
            .ok_or_else(|| {
                TextForgeError::new(
                    TextForgeErrorCode::TokenArrayNotFound("Transform not found".into()),
                    "get_transform_vec",
                    id.to_string(),
                )
            })?
            .clone()
            .iter()
            .map(|t| t.to_text_line_unresolved().map(|s| s.to_string()))
            .collect::<Result<Vec<String>, TextForgeError>>()
    }

    // -- Processing ----------------------------------------------------

    fn run_transform<'a>(
        &self,
        id: &str,
        input: Cow<'a, str>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut result = input;

        let tokens = self
            .transforms
            .get(id)
            .ok_or_else(token_array_not_found(id))?;
        let mut context = GlobalExecutionContext::new();

        // ErrorManager local, descartável — não é o self.errors compartilhado
        let mut local_errors = ErrorManager::default();

        for token in tokens.iter() {
            if context.is_halted {
                break;
            }
            result = apply_transform(token, result, &mut local_errors, &mut context)?;
        }

        Ok(result)
    }

    #[cfg(feature = "buffered")]
    fn process_all(&mut self, id: &str, input: &str) -> Result<String, TextForgeError> {
        self.process_all_via_buffer(id, input)
    }

    #[cfg(not(feature = "buffered"))]
    fn process_all(&mut self, id: &str, input: &str) -> Result<String, TextForgeError> {
        match self.run_transform(id, Cow::from(input)) {
            Ok(result) => Ok(result.into_owned()),
            Err(e) => {
                self.errors.add_error(e.clone());
                Err(e)
            }
        }
    }

    fn process_single(
        &mut self,
        token: TokenWrapper,
        input: &str,
    ) -> Result<String, TextForgeError> {
        let mut context = GlobalExecutionContext::new();
        match token.apply_token(Cow::from(input), &mut context) {
            Ok(x) => Ok(x.to_string()),
            Err(e) => {
                self.errors.add_error(e.clone());
                Err(e)
            }
        }
    }

    #[cfg(feature = "buffered")]
    fn process_all_with_debug(&mut self, id: &str, input: &str) -> Result<String, TextForgeError> {
        self.process_debug_via_buffer(id, input)
    }

    #[cfg(not(feature = "buffered"))]
    fn process_all_with_debug(&mut self, id: &str, input: &str) -> Result<String, TextForgeError> {
        let mut result = Cow::from(input);
        let dashes = 10;

        let tokens = match self
            .transforms
            .get(id)
            .ok_or_else(token_array_not_found(id))
        {
            Ok(x) => x,
            Err(e) => {
                self.errors.add_error(e.clone());
                return Err(e);
            }
        };

        let mut log = String::new();
        log.push_str("PROCESSING STEP BY STEP:\n");
        log.push_str(&"-".repeat(dashes));
        log.push_str("\n\n");

        let mut context = GlobalExecutionContext::new();

        for (counter, token) in (0_i64..).zip(tokens.iter()) {
            if context.is_halted {
                break;
            }
            let bound = token
                .bind_for_debug(&mut context)
                .inspect_err(|error| self.errors.add_error(error.clone()))?;
            let line = bound.to_textforge_line();
            let red_result = result.red();
            let temp = bound
                .transform(result, Some(&mut context))
                .inspect_err(|error| self.errors.add_error(error.clone()))?;

            if token.get_string_repr() == "blk" {
                let v = match bound.get_params().first() {
                    Some(crate::parser::params::TextForgeParamTypes::String(name)) => name,
                    _ => {
                        return Err(TextForgeError::new(
                            TextForgeErrorCode::InvalidParameters(
                                "Invalid block debug metadata".into(),
                            ),
                            "process_all_with_debug",
                            "block name",
                        ));
                    }
                };

                log.push_str(
                    &format!(
                        "Step: [{}] => [{}]\n{}\n\tBlock Instruction: {}\t\tBlock Name: {}\n\t\t\tCurrent instructions Associated to this Block:\n{}",
                        counter.to_string().blue(),
                        (counter + 1).to_string().blue(),
                        "Block Declaration: ".to_string().green(),
                        line.yellow(),
                        v.to_string().green(),
                        context.get_formatted_block_items(v)?
                    )
                );
            } else {
                // Note: format! aloca, mas agora você faz 1 print no final.

                log.push_str(&format!(
                    "Step: [{}] => [{}]\nInstruction: {}\nBefore: {}\nAfter: {}\n\n",
                    counter.to_string().blue(),
                    (counter + 1).to_string().blue(),
                    line.yellow(),
                    red_result,
                    temp.green()
                ));
            }

            if (counter as usize) + 1 < tokens.len() {
                log.push_str(&"-".repeat(dashes));
                log.push_str("\n\n");
            }

            result = temp;
        }

        print!("{log}"); // 1 única saída
        Ok(result.to_string())
    }

    fn process_single_with_debug(
        &mut self,
        token: TokenWrapper,
        input: &str,
    ) -> Result<String, TextForgeError> {
        let mut ctx = GlobalExecutionContext::new();
        let output = match token.apply_token(Cow::from(input), &mut ctx) {
            Ok(x) => x,
            Err(e) => {
                self.errors.add_error(e.clone());
                return Err(e);
            }
        };
        println!(
            "Step: [{}] => [{}]\nInstruction: {}\nBefore: {}\nAfter: {}\n",
            (0).to_string().blue(),
            (1).to_string().blue(),
            token.to_textforge_line().yellow(),
            input.red(),
            output.green()
        );

        Ok(output.to_string())
    }

    fn process_file(
        &mut self,
        pipeline_id: &str,
        file_path: &Path,
        output_path: &Path,
    ) -> Result<(), TextForgeError> {
        use std::fs::File;

        let output_path_str = output_path.to_string_lossy().into_owned();
        let input = file_io::read_utf8_file(file_path, "processor.process_file")?;

        // Transfer ownership to the whole-document engine instead of cloning input.
        #[cfg(not(feature = "buffered"))]
        let result = self
            .run_transform(pipeline_id, Cow::Owned(input))
            .map(Cow::into_owned)
            .inspect_err(|error| self.errors.add_error(error.clone()))?;
        #[cfg(feature = "buffered")]
        let result = self.process_all(pipeline_id, &input)?;

        // Garante que o diretório de destino exista antes de criar o arquivo de saída,
        // no mesmo espírito do que process_batch já faz para cada `target`.
        if let Some(parent) = output_path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                TextForgeError::new(
                    TextForgeErrorCode::FileWritingError(Cow::from(e.to_string())),
                    Cow::from("processor.process_file"),
                    output_path_str.clone(),
                )
            })?;
        }

        let mut out = File::create(output_path).map_err(|e| {
            TextForgeError::new(
                TextForgeErrorCode::FileOpeningError(Cow::from(e.to_string())),
                Cow::from("processor.process_file"),
                output_path_str.clone(),
            )
        })?;

        out.write_all(result.as_bytes()).map_err(|e| {
            TextForgeError::new(
                TextForgeErrorCode::FileWritingError(Cow::from(e.to_string())),
                Cow::from("processor.process_file"),
                output_path_str.clone(),
            )
        })?;

        Ok(())
    }

    #[cfg(feature = "buffered")]
    fn process_batch(&self, tasks: Vec<(&Path, &str, &Path)>) -> Vec<Result<(), TextForgeError>> {
        self.process_batch_via_buffer(tasks)
    }

    #[cfg(not(feature = "buffered"))]
    fn process_batch(&self, tasks: Vec<(&Path, &str, &Path)>) -> Vec<Result<(), TextForgeError>> {
        tasks
            .into_par_iter()
            .map(
                |(origin, pipeline_id, target)| -> Result<(), TextForgeError> {
                    if !origin.exists() {
                        return Err(TextForgeError::new(
                            TextForgeErrorCode::FileNotFound("Origin file does not exist".into()),
                            Cow::Borrowed("process_batch"),
                            format!("{:?}", origin),
                        ));
                    }

                    if !origin.is_file() {
                        return Err(TextForgeError::new(
                            TextForgeErrorCode::ValidationError("Origin path is not a file".into()),
                            Cow::Borrowed("process_batch"),
                            format!("{:?}", origin),
                        ));
                    }

                    let input = file_io::read_utf8_file(origin, "process_batch")?;
                    let output = self.run_transform(pipeline_id, Cow::Owned(input))?;

                    if let Some(parent) = target.parent()
                        && !parent.as_os_str().is_empty()
                        && !parent.exists()
                    {
                        std::fs::create_dir_all(parent).map_err(|e| {
                            TextForgeError::new(
                                TextForgeErrorCode::FileWritingError(
                                    "Failed to create target directory".into(),
                                ),
                                Cow::Borrowed("process_batch"),
                                format!("{:?} - {}", parent, e),
                            )
                        })?;
                    }

                    std::fs::write(target, output.as_bytes()).map_err(|e| {
                        TextForgeError::new(
                            TextForgeErrorCode::FileWritingError(
                                "Failed to write target file".into(),
                            ),
                            Cow::Borrowed("process_batch"),
                            format!("{:?} - {}", target, e),
                        )
                    })?;

                    Ok(())
                },
            )
            .collect()
    }
    #[cfg(all(feature = "watchers", feature = "buffered"))]
    fn process_all_with_watchers(
        &mut self,
        id: &str,
        input: &str,
        watcher_list: &mut WatcherList,
        report_path: &Path,
    ) -> Result<String, TextForgeError> {
        self.process_watchers_via_buffer(id, input, watcher_list, report_path)
    }

    #[cfg(all(feature = "watchers", not(feature = "buffered")))]
    fn process_all_with_watchers(
        &mut self,
        id: &str,
        input: &str,
        watcher_list: &mut WatcherList,
        report_path: &Path,
    ) -> Result<String, TextForgeError> {
        let tokens = match self
            .transforms
            .get(id)
            .ok_or_else(token_array_not_found(id))
        {
            Ok(x) => x,
            Err(e) => {
                self.errors.add_error(e.clone());
                return Err(e);
            }
        };

        let mut context = GlobalExecutionContext::new();
        let mut local_errors = ErrorManager::default();

        // Estado do pipeline como Arc<str>, não String: reusar `current_arc` de
        // uma iteração como `before` da próxima é um Arc::clone (refcount bump),
        // não uma cópia de texto. apply_transform ainda recebe &str normalmente
        // (Arc<str> faz Deref<Target = str>, então &current_arc já serve).
        let mut current_arc: Arc<str> = Arc::from(input);

        // Guarda (before, current, instruction) do passo anterior até sabermos
        // o "after" dele — que só existe depois de rodar o passo seguinte.
        let mut pending: Option<(Arc<str>, Arc<str>, String)> = None;

        for token in tokens.iter() {
            let before_arc = Arc::clone(&current_arc); // O(1)

            let after = apply_transform(
                token,
                Cow::Borrowed(current_arc.as_ref()),
                &mut local_errors,
                &mut context,
            )?;

            // Única cópia de texto inteiro por passo: a saída do transform
            // (String nova, alocada pelo apply_transform em si — isso é
            // inerante ao trabalho da instrução) também precisa existir como
            // Arc<str> pra virar "current" deste passo / "before" do próximo.
            current_arc = Arc::from(after.as_ref());

            if let Some((prev_before, prev_current, prev_instruction)) = pending.take() {
                let watcher_ctx = WatcherContext::new(
                    prev_current,
                    prev_before,
                    Some(Arc::clone(&current_arc)), // O(1)
                    prev_instruction,
                );
                watcher_list.run_watchers(watcher_ctx)?;
            }

            pending = Some((
                before_arc,
                Arc::clone(&current_arc), // O(1)
                token.to_textforge_line().to_string(),
            ));
        }

        // Última instrução: não existe próximo passo — after = None, explícito.
        if let Some((before, current, instruction)) = pending.take() {
            let watcher_ctx = WatcherContext::new(current, before, None::<Arc<str>>, instruction);
            watcher_list.run_watchers(watcher_ctx)?;
        }

        watcher_list.to_json(report_path)?;
        watcher_list.reset();

        Ok(current_arc.to_string())
    }

    // -- Bytecode persistence (feature = "bytecode") --------------------
    //
    // Only read/write `.textforgebc`; never process input directly.

    #[cfg(feature = "bytecode")]
    fn write_to_bytecode_file(&mut self, id: &str, path: &Path) -> Result<(), TextForgeError> {
        let tokens = match self
            .transforms
            .get(id)
            .ok_or_else(token_array_not_found(id))
        {
            Ok(x) => x,
            Err(e) => {
                self.errors.add_error(e.clone());
                return Err(e);
            }
        };

        write_bytecode_to_file(path, tokens.to_vec())
    }

    #[cfg(feature = "bytecode")]
    fn read_from_bytecode_file(&mut self, path: &Path) -> Result<String, TextForgeError> {
        let tokens = match read_bytecode_from_file(path) {
            Ok(x) => x,
            Err(e) => {
                self.errors.add_error(e.clone());
                return Err(e);
            }
        };

        let identifier = self.add_transform(tokens.to_vec());

        Ok(identifier)
    }
}

//! Owned construction and execution boundary for language adapters.
//!
//! This Rust API is not a C ABI. It reuses the existing instruction registry,
//! construction tree, structural policies, and engine. Adapters convert foreign
//! values into these concrete types and translate [`BindingError`] into their
//! language's errors. [`PortableProcessor`] exposes the complete processor surface;
//! optional watchers and streams use runtime-neutral synchronous callback hooks.
//! No `test_access` feature is required.
//!
//! ```
//! use textforge::api::portable::{Argument, InstructionSpec, PipelinePlan};
//! let mut body = PipelinePlan::new();
//! body.add(InstructionSpec::new("atb".into(), vec![Argument::Text("🍌".into())]))?;
//! let mut plan = PipelinePlan::new();
//! plan.block("dynamic name".to_string(), body);
//! plan.call_block("dynamic name".to_string())?;
//! let pipeline = plan.compile()?;
//! assert_eq!(pipeline.process("é")?, "🍌é");
//! # Ok::<(), textforge::api::portable::BindingError>(())
//! ```
#[cfg(feature = "buffered")]
mod buffered;
mod error;
mod instruction;
mod path;
mod processor;
#[cfg(feature = "buffered")]
pub use buffered::{BufferOptions, BufferTask, PortableBuffer, StreamReader, StreamWriter};
#[cfg(feature = "watchers")]
mod watchers;
pub use path::FilePath;
pub use processor::{BatchPool, Features, FileTask, PortableProcessor, features};
#[cfg(feature = "watchers")]
pub use watchers::{Observation, Observer, Observers};

pub use error::{BindingError, ErrorStage};
pub use instruction::{Argument, InstructionSpec};

use crate::{
    api::{
        builder::tree::{Node, validate_and_lower},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    parser::params::TextForgeParamTypes,
    tokens::instructions::{blk::Blk, ifdc::Ifdc},
};
use std::{borrow::Cow, fmt};

/// An owned construction plan independent of any processor or foreign runtime.
///
/// Groups take ownership of complete subplans instead of retaining callbacks or
/// borrowed builders. Compilation validates the entire tree and can be repeated;
/// errors do not change the plan or produce a partially registered pipeline.
/// See [`Self::compile`], [`InstructionSpec`], and [`CompiledPipeline`].
///
/// ```
/// use textforge::api::portable::{InstructionSpec, PipelinePlan};
/// let mut plan = PipelinePlan::new();
/// plan.add(InstructionSpec::new("tbs".into(), vec![])).unwrap();
/// let first = plan.compile().unwrap();
/// let second = plan.compile().unwrap();
/// drop(plan);
/// assert_eq!(first.process(" é ").unwrap(), "é");
/// assert_eq!(second.process(" x ").unwrap(), "x");
/// ```
#[derive(Default)]
pub struct PipelinePlan {
    nodes: Vec<Node>,
}

impl PipelinePlan {
    /// Create an empty identity plan without creating or borrowing a processor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert and append one description atomically. Unknown names, excessive
    /// description depth, and numeric overflow return `Conversion` errors.
    /// Structural arity/type/ancestor checks are deferred to [`Self::compile`].
    pub fn add(&mut self, instruction: InstructionSpec) -> Result<(), BindingError> {
        let token = instruction::convert(&instruction)
            .map_err(|error| BindingError::from_core(ErrorStage::Conversion, error))?;
        self.nodes.push(Node::Instruction(token));
        Ok(())
    }

    /// Append a complete map definition atomically, preserving rule order.
    /// Foreign closures are evaluated by adapters; only owned arguments cross here.
    ///
    /// ```
    /// use textforge::api::portable::{Argument, PipelinePlan, InstructionSpec};
    /// let mut plan = PipelinePlan::new();
    /// plan.define_replace_map(Argument::Text("m".into()), vec![
    ///     (Argument::Text("cat".into()), Argument::Text("dog".into())),
    /// ])?;
    /// plan.add(InstructionSpec::new("rmape".into(),vec![Argument::Text("m".into())]))?;
    /// assert_eq!(plan.compile()?.process("cat")?,"dog");
    /// # Ok::<(), textforge::api::portable::BindingError>(())
    /// ```
    pub fn define_replace_map(
        &mut self,
        name: Argument,
        rules: Vec<(Argument, Argument)>,
    ) -> Result<(), BindingError> {
        super::replace_map_builder::validate_rule_count(rules.len())
            .map_err(|error| BindingError::from_core(ErrorStage::Validation, error))?;
        let mut converted = Vec::with_capacity(rules.len());
        for (pattern, replacement) in rules {
            let spec =
                InstructionSpec::new("rmapd".into(), vec![name.clone(), pattern, replacement]);
            converted.push(
                instruction::convert(&spec)
                    .map_err(|error| BindingError::from_core(ErrorStage::Conversion, error))?,
            );
        }
        self.nodes
            .extend(converted.into_iter().map(Node::Instruction));
        Ok(())
    }

    /// Move a complete block body into this plan using a dynamically owned name.
    /// Block-ancestor restrictions are checked by the shared rules at compile time.
    pub fn block(&mut self, name: String, mut body: PipelinePlan) {
        self.nodes.push(Node::Group {
            token: Box::new(Blk::default()),
            params: vec![TextForgeParamTypes::String(name)],
            children: std::mem::take(&mut body.nodes),
        });
    }

    /// Move a conditional body into this plan. The condition is re-evaluated for
    /// every child, preserving `if_do_contains_each` semantics of the Rust API.
    pub fn contains_each(&mut self, pattern: String, mut body: PipelinePlan) {
        self.nodes.push(Node::Group {
            token: Box::new(Ifdc::default()),
            params: vec![TextForgeParamTypes::String(pattern)],
            children: std::mem::take(&mut body.nodes),
        });
    }

    /// Append a block call with a dynamically owned name.
    pub fn call_block(&mut self, name: String) -> Result<(), BindingError> {
        self.add(InstructionSpec::new(
            "cblk".into(),
            vec![Argument::Text(name)],
        ))
    }

    /// Validate and lower the shared tree, then create an independent executable.
    /// No processor is mutated until the complete validation pass succeeds.
    /// Compilation does not resolve variables or guarantee input-dependent success.
    pub fn compile(&self) -> Result<CompiledPipeline, BindingError> {
        let tokens = validate_and_lower(&self.nodes)
            .map_err(|error| BindingError::from_core(ErrorStage::Validation, error))?;
        let mut processor = TextForgeProcessor::new();
        let id = processor.add_transform(tokens);
        Ok(CompiledPipeline { processor, id })
    }
}

impl fmt::Debug for PipelinePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipelinePlan")
            .field("root_nodes", &self.nodes.len())
            .finish_non_exhaustive()
    }
}

/// An owned executable, independent of its source plan and reusable across inputs.
///
/// [`Self::process`] calls [`TextForgeProcessorMethods::run_transform`], creating
/// fresh execution context per call without retaining a shared error log. Rust
/// callers can share this immutable executable across threads; each language
/// adapter still manages its runtime's threading and object-lifetime rules.
/// Input text is borrowed only for the call; returned text/errors are owned.
/// This focused object exposes immutable memory execution. Use [`PortableProcessor`]
/// for registration, files, debug, batches and optional watcher/buffer/bytecode APIs.
///
/// ```
/// use textforge::api::portable::{Argument, InstructionSpec, PipelinePlan};
/// let mut plan = PipelinePlan::new();
/// plan.add(InstructionSpec::new("ate".into(), vec![Argument::Text("!".into())])).unwrap();
/// let pipeline = plan.compile().unwrap();
/// drop(plan);
/// assert_eq!(pipeline.process("é").unwrap(), "é!");
/// assert_eq!(pipeline.process("").unwrap(), "!");
/// ```
pub struct CompiledPipeline {
    processor: TextForgeProcessor,
    id: String,
}

impl CompiledPipeline {
    /// Execute the existing in-memory engine with an isolated context. Runtime
    /// variable, expression, regex, and input-dependent errors have `Execution` stage.
    pub fn process(&self, input: &str) -> Result<String, BindingError> {
        self.processor
            .run_transform(&self.id, Cow::Borrowed(input))
            .map(Cow::into_owned)
            .map_err(|error| BindingError::from_core(ErrorStage::Execution, error))
    }
}

impl fmt::Debug for CompiledPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledPipeline").finish_non_exhaustive()
    }
}

// Plans may contain invalid deep hierarchies until compile(). Releasing a failed
// plan must remain iterative, just like validation and conversion.
impl Drop for PipelinePlan {
    fn drop(&mut self) {
        let mut pending = std::mem::take(&mut self.nodes);
        while let Some(node) = pending.pop() {
            if let Node::Group { children, .. } = node {
                pending.extend(children);
            }
        }
    }
}

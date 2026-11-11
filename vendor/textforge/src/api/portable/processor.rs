use super::{BindingError, ErrorStage, FilePath, InstructionSpec, PipelinePlan, instruction};
use crate::{
    api::{
        builder::tree::validate_and_lower,
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    utils::errors::{TextForgeError, TextForgeErrorCode},
};
use std::{borrow::Cow, collections::BTreeSet};

/// Build-time capabilities of this copy of TextForge.
/// Optional methods are compiled with the same features as the core processor.
///
/// ```
/// use textforge::api::portable::features;
/// assert_eq!(features().bytecode, cfg!(feature = "bytecode"));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Features {
    pub bytecode: bool,
    pub watchers: bool,
    pub buffered: bool,
}

pub fn features() -> Features {
    Features {
        bytecode: cfg!(feature = "bytecode"),
        watchers: cfg!(feature = "watchers"),
        buffered: cfg!(feature = "buffered"),
    }
}

/// An owned file-processing request; tasks may reference different pipelines.
///
/// ```
/// use textforge::api::portable::{FileTask, FilePath};
/// let task = FileTask { input: FilePath::from_text("in.txt".into()), pipeline_id: "id".into(), output: FilePath::from_text("out.txt".into()) };
/// assert_eq!(task.pipeline_id, "id");
/// ```
#[derive(Clone, Debug)]
pub struct FileTask {
    pub input: FilePath,
    pub pipeline_id: String,
    pub output: FilePath,
}

/// Reusable caller-owned batch worker pool, without exposing Rayon to adapters.
///
/// ```
/// use textforge::api::portable::BatchPool;
/// let pool = BatchPool::new(2).unwrap();
/// drop(pool);
/// ```
#[derive(Debug)]
pub struct BatchPool(pub(super) rayon::ThreadPool);

impl BatchPool {
    /// Zero selects Rayon's automatic worker count; nonzero requests that count.
    pub fn new(threads: u64) -> Result<Self, BindingError> {
        let threads = native_size(threads, "BatchPool")?;
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map(Self)
            .map_err(|error| {
                execution(TextForgeError::new(
                    TextForgeErrorCode::GenericError(error.to_string().into()),
                    "BatchPool",
                    "worker initialization",
                ))
            })
    }
}

/// Full processor surface with owned IDs, plans, paths and instruction snapshots.
/// Methods delegate to the existing processor rather than reimplement execution.
/// Use [`super::features`] to inspect optional capabilities.
///
/// ```
/// use textforge::api::portable::{PortableProcessor, InstructionSpec};
/// let mut processor = PortableProcessor::new();
/// let mut plan = processor.create_pipeline();
/// plan.add(InstructionSpec::new("tbs".into(), vec![])).unwrap();
/// let id = processor.register(&plan).unwrap();
/// assert_eq!(processor.process_all(&id, " é ").unwrap(), "é");
/// assert!(processor.transform_exists(&id));
/// processor.remove_transform(&id).unwrap();
/// ```
#[derive(Default)]
pub struct PortableProcessor {
    pub(super) core: TextForgeProcessor,
    ids: BTreeSet<String>,
}

impl PortableProcessor {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn create_pipeline(&self) -> PipelinePlan {
        PipelinePlan::new()
    }
    pub fn register(&mut self, plan: &PipelinePlan) -> Result<String, BindingError> {
        let tokens = validate_and_lower(&plan.nodes).map_err(validation)?;
        let id = self.core.add_transform(tokens);
        self.ids.insert(id.clone());
        Ok(id)
    }
    /// Owned-description equivalent of add_transform, validated atomically.
    pub fn add_transform(
        &mut self,
        instructions: Vec<InstructionSpec>,
    ) -> Result<String, BindingError> {
        let mut plan = PipelinePlan::new();
        for instruction in instructions {
            plan.add(instruction)?;
        }
        self.register(&plan)
    }
    pub fn remove_transform(&mut self, id: &str) -> Result<(), BindingError> {
        self.core.remove_transform(id).map_err(execution)?;
        self.ids.remove(id);
        Ok(())
    }
    pub fn transform_exists(&self, id: &str) -> bool {
        self.core.transform_exists(id)
    }
    pub fn list_transforms(&self) -> Vec<String> {
        self.ids.iter().cloned().collect()
    }
    pub fn show_transforms(&self) {
        self.core.show_transforms();
    }
    /// Return independent editable descriptions rather than private token objects.
    pub fn get_transform_vec(&self, id: &str) -> Result<Vec<InstructionSpec>, BindingError> {
        self.core
            .get_transform_vec(id)
            .map_err(execution)?
            .iter()
            .map(|token| instruction::describe(token).map_err(conversion))
            .collect()
    }
    pub fn get_text_transform_vec(&self, id: &str) -> Result<Vec<String>, BindingError> {
        self.core.get_text_transform_vec(id).map_err(execution)
    }
    pub fn clone_pipeline(&mut self, id: &str) -> Result<String, BindingError> {
        let tokens = self.core.get_transform_vec(id).map_err(execution)?;
        let id = self.core.add_transform(tokens);
        self.ids.insert(id.clone());
        Ok(id)
    }
    pub fn get_pipeline_from_file(&mut self, path: &FilePath) -> Result<String, BindingError> {
        let id = self
            .core
            .get_pipeline_from_file(path.as_path())
            .map_err(persistence)?;
        self.ids.insert(id.clone());
        Ok(id)
    }
    pub fn write_pipeline_to_file(
        &mut self,
        id: &str,
        path: &FilePath,
    ) -> Result<(), BindingError> {
        self.core
            .write_pipeline_to_file(id, path.as_path())
            .map_err(persistence)
    }
    #[cfg(feature = "bytecode")]
    pub fn read_from_bytecode_file(&mut self, path: &FilePath) -> Result<String, BindingError> {
        let id = self
            .core
            .read_from_bytecode_file(path.as_path())
            .map_err(persistence)?;
        self.ids.insert(id.clone());
        Ok(id)
    }
    #[cfg(feature = "bytecode")]
    pub fn write_to_bytecode_file(
        &mut self,
        id: &str,
        path: &FilePath,
    ) -> Result<(), BindingError> {
        self.core
            .write_to_bytecode_file(id, path.as_path())
            .map_err(persistence)
    }
    pub fn run_transform(&self, id: &str, input: &str) -> Result<String, BindingError> {
        self.core
            .run_transform(id, Cow::Borrowed(input))
            .map(Cow::into_owned)
            .map_err(execution)
    }
    pub fn process_all(&mut self, id: &str, input: &str) -> Result<String, BindingError> {
        self.core.process_all(id, input).map_err(execution)
    }
    pub fn process_single(
        &mut self,
        instruction: InstructionSpec,
        input: &str,
    ) -> Result<String, BindingError> {
        let token = instruction::convert(&instruction).map_err(conversion)?;
        self.core.process_single(token, input).map_err(execution)
    }
    pub fn process_all_with_debug(
        &mut self,
        id: &str,
        input: &str,
    ) -> Result<String, BindingError> {
        self.core
            .process_all_with_debug(id, input)
            .map_err(execution)
    }
    pub fn process_single_with_debug(
        &mut self,
        instruction: InstructionSpec,
        input: &str,
    ) -> Result<String, BindingError> {
        let token = instruction::convert(&instruction).map_err(conversion)?;
        self.core
            .process_single_with_debug(token, input)
            .map_err(execution)
    }
    pub fn process_file(
        &mut self,
        id: &str,
        input: &FilePath,
        output: &FilePath,
    ) -> Result<(), BindingError> {
        self.core
            .process_file(id, input.as_path(), output.as_path())
            .map_err(execution)
    }
    pub fn process_batch(&self, tasks: Vec<FileTask>) -> Vec<Result<(), BindingError>> {
        self.core
            .process_batch(
                tasks
                    .iter()
                    .map(|task| {
                        (
                            task.input.as_path(),
                            task.pipeline_id.as_str(),
                            task.output.as_path(),
                        )
                    })
                    .collect(),
            )
            .into_iter()
            .map(|result| result.map_err(execution))
            .collect()
    }
    pub fn process_batch_in_pool(
        &self,
        pool: &BatchPool,
        tasks: Vec<FileTask>,
    ) -> Vec<Result<(), BindingError>> {
        self.core
            .process_batch_in_pool(
                &pool.0,
                tasks
                    .iter()
                    .map(|task| {
                        (
                            task.input.as_path(),
                            task.pipeline_id.as_str(),
                            task.output.as_path(),
                        )
                    })
                    .collect(),
            )
            .into_iter()
            .map(|result| result.map_err(execution))
            .collect()
    }
}

pub(super) fn conversion(error: TextForgeError) -> BindingError {
    BindingError::from_core(ErrorStage::Conversion, error)
}
pub(super) fn validation(error: TextForgeError) -> BindingError {
    BindingError::from_core(ErrorStage::Validation, error)
}
pub(super) fn execution(error: TextForgeError) -> BindingError {
    BindingError::from_core(ErrorStage::Execution, error)
}
pub(super) fn persistence(error: TextForgeError) -> BindingError {
    BindingError::from_core(ErrorStage::Persistence, error)
}
pub(super) fn native_size(value: u64, location: &'static str) -> Result<usize, BindingError> {
    usize::try_from(value).map_err(|error| {
        conversion(TextForgeError::new(
            TextForgeErrorCode::TryIntoFailError(error.to_string().into()),
            location,
            value.to_string(),
        ))
    })
}

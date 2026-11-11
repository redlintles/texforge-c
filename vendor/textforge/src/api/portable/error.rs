use crate::utils::errors::TextForgeError;
use std::{error::Error, fmt};

/// The operation that failed at the portable API boundary.
///
/// Conversion checks names/numeric representation, validation checks the complete
/// hierarchy, and execution resolves variables and transforms input. See
/// [`super::PipelinePlan::compile`] and [`super::CompiledPipeline::process`].
///
/// ```
/// use textforge::api::portable::{ErrorStage, InstructionSpec, PipelinePlan};
/// let mut plan = PipelinePlan::new();
/// let error = plan.add(InstructionSpec::new("unknown".into(), vec![])).unwrap_err();
/// assert_eq!(error.stage, ErrorStage::Conversion);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorStage {
    /// Input could not be represented as an instruction or native-size number.
    Conversion,
    /// The complete construction hierarchy violates a structural rule.
    Validation,
    /// A validated pipeline failed while processing a particular input.
    Execution,
    /// Pipeline import/export or report persistence failed.
    Persistence,
    /// A user-provided observer returned an error.
    Callback,
}

/// Owned, uncolored error data suitable for exceptions or foreign result objects.
///
/// `code` preserves the engine's numeric error code; `message`, `instruction`,
/// and `input` preserve its raw context. Adapters should read fields rather than
/// parse [`fmt::Display`]. No borrowed engine object escapes this boundary.
/// See [`ErrorStage`] and [`super::CompiledPipeline::process`].
///
/// ```
/// use textforge::api::portable::{ErrorStage, InstructionSpec, PipelinePlan};
/// let mut plan = PipelinePlan::new();
/// let error = plan.add(InstructionSpec::new("unknown".into(), vec![])).unwrap_err();
/// assert_eq!(error.code, 2);
/// assert_eq!(error.stage, ErrorStage::Conversion);
/// assert!(!error.message.contains('\u{1b}'));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingError {
    /// Boundary that returned this error, including persistence and callbacks.
    pub stage: ErrorStage,
    /// Existing engine error code, without remapping into language-specific codes.
    pub code: u16,
    /// Raw message; no terminal color formatting or presentation labels are added.
    pub message: String,
    /// Engine instruction context, when available.
    pub instruction: String,
    /// Engine input/diagnostic context, when available.
    pub input: String,
}

impl BindingError {
    pub(super) fn from_core(stage: ErrorStage, error: TextForgeError) -> Self {
        Self {
            stage,
            code: error.error_code.get_error_code(),
            message: error.error_code.message().to_string(),
            instruction: error.instruction.into_owned(),
            input: error.input.into_owned(),
        }
    }
}

impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}: {} (code {}, instruction: {})",
            self.stage, self.message, self.code, self.instruction
        )
    }
}

impl Error for BindingError {}

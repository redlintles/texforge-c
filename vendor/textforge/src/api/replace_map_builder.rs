//! Scoped construction of named replacement maps, without executing rules.
use crate::{
    parser::resolve_var::{TokenWrapper, ValType},
    tokens::instructions::rmapd::Rmapd,
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

/// Collect rules for one map; `end()` closes configuration, not the pipeline.
/// Rules keep insertion order and normal execution-context interpolation.
/// The callback runs once during construction and is not retained.
///
/// ```
/// use textforge::api::{TextForgeBuilderMethods, processor::{TextForgeProcessor, TextForgeProcessorMethods}};
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().define_replace_map("animals", |map| {
///     map.add_replace("cat", "wolf")?.add_replace("wolf", "dog")?.end()
/// })?.execute_replace_map("animals")?.add_to_end("!")?.build()?;
/// assert_eq!(processor.process_all(&id, "cat wolf")?, "wolf dog!");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
pub struct ReplaceMapBuilder {
    name: ValType,
    rules: Vec<TokenWrapper>,
    ended: bool,
}

impl ReplaceMapBuilder {
    /// Create a scoped definition; normally supplied by `define_replace_map`.
    pub(crate) fn new(name: impl Into<ValType>) -> Self {
        Self {
            name: name.into(),
            rules: Vec::new(),
            ended: false,
        }
    }

    /// Append one regex/replacement pair. Regex compilation stays in the engine.
    pub fn add_replace(
        &mut self,
        pattern: impl Into<ValType>,
        replacement: impl Into<ValType>,
    ) -> Result<&mut Self, TextForgeError> {
        if self.ended {
            return Err(map_error("Replacement map configuration has already ended"));
        }
        self.rules.push(TokenWrapper::new(
            Box::new(Rmapd::default()),
            Some(vec![self.name.clone(), pattern.into(), replacement.into()]),
        ));
        Ok(self)
    }

    /// Finish configuration. An empty definition or a second call is an error.
    /// This method never emits the DSL `end` instruction or executes the map.
    pub fn end(&mut self) -> Result<(), TextForgeError> {
        if self.ended {
            return Err(map_error("Replacement map configuration has already ended"));
        }
        validate_rule_count(self.rules.len())?;
        self.ended = true;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<Vec<TokenWrapper>, TextForgeError> {
        if !self.ended {
            return Err(map_error(
                "Replacement map configuration must finish with end()",
            ));
        }
        Ok(self.rules)
    }
}

pub(crate) fn validate_rule_count(count: usize) -> Result<(), TextForgeError> {
    if count == 0 {
        return Err(map_error("Replacement maps require at least one rule"));
    }
    Ok(())
}

fn map_error(message: &str) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::ValidationError(message.to_owned().into()),
        "define_replace_map",
        "map configuration",
    )
}

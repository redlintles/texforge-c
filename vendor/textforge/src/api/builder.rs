use crate::{
    api::{TextForgeBlockMethods, TextForgeBuilderMethods, TextForgeConditionalMethods},
    parser::resolve_var::TokenWrapper,
    utils::errors::TextForgeError,
};

pub(crate) mod tree;
use super::{block_builder::BlockBuilder, conditional_builder::ConditionalBuilderEach};
use crate::{parser::params::TextForgeParamTypes, tokens::instructions::ifdc::Ifdc};
use tree::{Node, validate_and_lower};

use super::processor::{TextForgeProcessor, TextForgeProcessorMethods};

/// Collects an instruction tree, validates it, then registers a linear pipeline.
/// Context-dependent variable values are checked during execution.
///
/// ```
/// use textforge::api::{processor::TextForgeProcessor, TextForgeBuilderMethods};
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().trim_both_sides().unwrap().build().unwrap();
/// assert!(!id.is_empty());
/// ```
pub struct TextForgeBuilder<'ap> {
    nodes: Vec<Node>,
    processor: &'ap mut TextForgeProcessor,
}

impl<'ap> TextForgeBuilder<'ap> {
    pub fn new(processor: &'ap mut TextForgeProcessor) -> TextForgeBuilder<'ap> {
        TextForgeBuilder {
            nodes: Vec::new(),
            processor,
        }
    }

    /// Validates the complete tree before registering any instructions.
    /// On failure, the processor and the construction tree remain unchanged.
    /// Successful builds retain the tree and may be repeated.
    pub fn build(&mut self) -> Result<String, TextForgeError> {
        let tokens = validate_and_lower(&self.nodes)?;
        Ok(self.processor.add_transform(tokens))
    }
}

impl<'ap> TextForgeBuilderMethods for TextForgeBuilder<'ap> {
    fn push_token(&mut self, t: impl Into<TokenWrapper>) -> Result<(), TextForgeError> {
        self.nodes.push(Node::Instruction(t.into()));
        Ok(())
    }
}

impl TextForgeConditionalMethods for TextForgeBuilder<'_> {
    fn if_do_contains_each<F>(&mut self, value: &str, f: F) -> Result<&mut Self, TextForgeError>
    where
        F: FnOnce(&mut ConditionalBuilderEach) -> Result<(), TextForgeError>,
    {
        let mut child = ConditionalBuilderEach::new(
            Box::new(Ifdc::default()),
            vec![TextForgeParamTypes::String(value.to_string())],
        );
        f(&mut child)?;
        self.nodes.push(child.into_node());
        Ok(self)
    }
}
impl TextForgeBlockMethods for TextForgeBuilder<'_> {
    fn block_assoc<F>(
        &mut self,
        block_name: &'static str,
        f: F,
    ) -> Result<&mut Self, TextForgeError>
    where
        F: FnOnce(&mut BlockBuilder) -> Result<(), TextForgeError>,
    {
        let mut child = BlockBuilder::new(block_name);
        f(&mut child)?;
        self.nodes.push(child.into_node());
        Ok(self)
    }
}

#[cfg(test)]
mod test;

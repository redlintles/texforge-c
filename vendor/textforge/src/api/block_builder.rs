use super::{builder::tree::Node, conditional_builder::ConditionalBuilderEach};
use crate::tokens::instructions::ifdc::Ifdc;
use crate::{
    api::{TextForgeBuilderMethods, TextForgeConditionalMethods},
    parser::params::TextForgeParamTypes,
    parser::resolve_var::TokenWrapper,
    tokens::{InstructionMethods, instructions::blk::Blk},
    utils::errors::TextForgeError,
};

/// Collects a block body and lowers each child to one `blk` instruction.
/// Nested block declarations are forbidden, including beneath conditional tokens.
///
/// ```compile_fail
/// use textforge::api::{block_builder::BlockBuilder, TextForgeBlockMethods};
/// let mut body = BlockBuilder::new("outer");
/// body.block_assoc("inner", |_| Ok(())).unwrap();
/// ```
pub struct BlockBuilder {
    block_name: &'static str,
    children: Vec<Node>,
}

impl BlockBuilder {
    pub fn new(block_name: &'static str) -> Self {
        BlockBuilder {
            block_name,
            children: Vec::new(),
        }
    }

    pub(crate) fn into_node(self) -> Node {
        Node::Group {
            token: Box::new(Blk::default()),
            params: vec![TextForgeParamTypes::String(self.block_name.to_string())],
            children: self.children,
        }
    }

    /// Validate and lower this standalone block, propagating structural errors.
    ///
    /// ```
    /// use textforge::api::{block_builder::BlockBuilder, TextForgeBuilderMethods};
    /// let mut block = BlockBuilder::new("cleanup");
    /// block.trim_both_sides().unwrap();
    /// assert_eq!(block.build().unwrap().len(), 1);
    /// ```
    pub fn build(self) -> Result<Vec<Box<dyn InstructionMethods>>, TextForgeError> {
        super::builder::tree::build_group(self.into_node())
    }
}

impl TextForgeBuilderMethods for BlockBuilder {
    fn push_token(&mut self, t: impl Into<TokenWrapper>) -> Result<(), TextForgeError> {
        self.children.push(Node::Instruction(t.into()));
        Ok(())
    }
}

impl TextForgeConditionalMethods for BlockBuilder {
    fn if_do_contains_each<F>(&mut self, value: &str, f: F) -> Result<&mut Self, TextForgeError>
    where
        F: FnOnce(&mut ConditionalBuilderEach) -> Result<(), TextForgeError>,
    {
        let mut child = ConditionalBuilderEach::new(
            Box::new(Ifdc::default()),
            vec![TextForgeParamTypes::String(value.to_string())],
        );
        f(&mut child)?;
        self.children.push(child.into_node());
        Ok(self)
    }
}

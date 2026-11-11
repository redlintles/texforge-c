use super::builder::tree::Node;
use crate::{
    api::TextForgeBuilderMethods, parser::params::TextForgeParamTypes,
    parser::resolve_var::TokenWrapper, tokens::InstructionMethods, utils::errors::TextForgeError,
};

/// Wraps each child separately; the condition is re-evaluated for every child.
/// This is not a single condition guarding an entire body.
///
/// ```compile_fail
/// use textforge::api::{conditional_builder::ConditionalBuilderEach, TextForgeBlockMethods};
/// fn forbidden(body: &mut ConditionalBuilderEach) {
///     body.block_assoc("inner", |_| Ok(())).unwrap();
/// }
/// ```
pub struct ConditionalBuilderEach {
    token: Box<dyn InstructionMethods>,
    params: Vec<TextForgeParamTypes>,
    children: Vec<Node>,
}

impl ConditionalBuilderEach {
    pub fn new(token: Box<dyn InstructionMethods>, params: Vec<TextForgeParamTypes>) -> Self {
        ConditionalBuilderEach {
            token,
            params,
            children: Vec::new(),
        }
    }

    pub(crate) fn into_node(self) -> Node {
        Node::Group {
            token: self.token,
            params: self.params,
            children: self.children,
        }
    }

    /// Validate and lower a standalone conditional group.
    ///
    /// ```
    /// use textforge::api::{conditional_builder::ConditionalBuilderEach, TextForgeBuilderMethods};
    /// use textforge::tokens::instructions::ifdc::Ifdc;
    /// let mut conditional = ConditionalBuilderEach::new(Box::new(Ifdc::default()), vec!["x".to_string().into()]);
    /// conditional.trim_both_sides().unwrap();
    /// assert_eq!(conditional.build().unwrap().len(), 1);
    /// ```
    pub fn build(self) -> Result<Vec<Box<dyn InstructionMethods>>, TextForgeError> {
        super::builder::tree::build_group(self.into_node())
    }
}

impl TextForgeBuilderMethods for ConditionalBuilderEach {
    fn push_token(&mut self, t: impl Into<TokenWrapper>) -> Result<(), TextForgeError> {
        self.children.push(Node::Instruction(t.into()));
        Ok(())
    }
}

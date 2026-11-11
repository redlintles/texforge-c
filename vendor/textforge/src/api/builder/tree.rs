//! Temporary construction hierarchy. Only lowered instruction vectors escape to
//! the processor; neither the parser nor the execution engine stores this tree.
use crate::{
    parser::{
        params::TextForgeParamTypes,
        resolve_var::{TokenWrapper, ValType},
        structure,
    },
    tokens::InstructionMethods,
    utils::errors::TextForgeError,
};

/// A construction leaf or a group that wraps each child independently.
///
/// ```
/// use textforge::api::{processor::TextForgeProcessor, TextForgeBlockMethods,
///     TextForgeBuilderMethods};
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().block_assoc("example", |body| {
///     body.trim_both_sides()?;
///     Ok(())
/// }).unwrap().build().unwrap();
/// assert!(!id.is_empty());
/// ```
pub(crate) enum Node {
    Instruction(TokenWrapper),
    Group {
        token: Box<dyn InstructionMethods>,
        params: Vec<TextForgeParamTypes>,
        children: Vec<Node>,
    },
}

/// Validate the entire hierarchy before cloning leaves or creating output.
pub(crate) fn validate_and_lower(nodes: &[Node]) -> Result<Vec<TokenWrapper>, TextForgeError> {
    let mut pending: Vec<_> = nodes.iter().map(|node| (node, 0, false)).collect();
    while let Some((node, depth, inside_block)) = pending.pop() {
        match node {
            Node::Instruction(token) => structure::validate_at(token, depth, inside_block)?,
            Node::Group {
                token,
                params,
                children,
            } => {
                // A group header exposes an instruction slot, not a fake child.
                // Every real child is validated independently with inherited context.
                structure::rules::check(
                    &structure::rules::InstructionView::group(token.get_string_repr(), params),
                    structure::rules::ValidationContext {
                        depth,
                        inside_block,
                        inside_iteration: false,
                    },
                )?;
                let inside_block = inside_block || token.get_string_repr() == "blk";
                pending.extend(
                    children
                        .iter()
                        .map(|child| (child, depth + 1, inside_block)),
                );
            }
        }
    }

    // Ancestors are bounded by validation. Process children from left to right,
    // wrapping from the innermost group out to preserve declaration semantics.
    let mut output = Vec::new();
    let mut pending: Vec<(&Node, Vec<&Node>)> =
        nodes.iter().rev().map(|node| (node, Vec::new())).collect();
    while let Some((node, ancestors)) = pending.pop() {
        match node {
            Node::Group { children, .. } => {
                let mut ancestors = ancestors;
                ancestors.push(node);
                pending.extend(
                    children
                        .iter()
                        .rev()
                        .map(|child| (child, ancestors.clone())),
                );
            }
            Node::Instruction(token) => {
                let mut token = token.clone();
                for ancestor in ancestors.iter().rev() {
                    if let Node::Group {
                        token: template,
                        params,
                        ..
                    } = ancestor
                    {
                        let mut params = params
                            .iter()
                            .cloned()
                            .map(ValType::Literal)
                            .collect::<Vec<_>>();
                        params.push(ValType::Literal(token.into()));
                        token = TokenWrapper::new(template.clone(), Some(params));
                    }
                }
                output.push(token);
            }
        }
    }
    Ok(output)
}

/// Materialize only the outer instruction; nested wrappers retain runtime refs.
pub(crate) fn build_group(node: Node) -> Result<Vec<Box<dyn InstructionMethods>>, TextForgeError> {
    validate_and_lower(&[node])?
        .into_iter()
        .map(|wrapper| {
            let params = wrapper
                .get_params()
                .iter()
                .map(|value| match value {
                    ValType::Literal(value) => value.clone(),
                    ValType::VarRef(name) => TextForgeParamTypes::VarRef(name.clone()),
                })
                .collect::<Vec<_>>();
            let mut token = wrapper.get_default_token();
            token.set_params(&params)?;
            Ok(token)
        })
        .collect()
}

//! Iterative traversal shared by fluent builders, parameter binding, and export.
//! Policy lives in [`rules`]; traversal forwards inherited context to each real
//! instruction. See the extension guide in `docs/validation-rules.md`.
pub(crate) mod rules;

use crate::{
    parser::{
        params::TextForgeParamTypes,
        resolve_var::{TokenWrapper, ValType},
    },
    utils::errors::TextForgeError,
};
use rules::{BUILTIN_RULES, InstructionView, ValidationContext, ValidationRule, check_with_rules};

pub(crate) fn validate(token: &TokenWrapper) -> Result<(), TextForgeError> {
    validate_at(token, 0, false)
}

pub(crate) fn validate_block_child(token: &TokenWrapper) -> Result<(), TextForgeError> {
    validate_at(token, 1, true)
}

pub(crate) fn validate_at(
    token: &TokenWrapper,
    depth: usize,
    inside_block: bool,
) -> Result<(), TextForgeError> {
    walk(
        vec![(
            token,
            ValidationContext {
                depth,
                inside_block,
                inside_iteration: false,
            },
        )],
        BUILTIN_RULES,
    )
}

fn walk(
    mut pending: Vec<(&TokenWrapper, ValidationContext)>,
    rules: &[&dyn ValidationRule],
) -> Result<(), TextForgeError> {
    while let Some((token, context)) = pending.pop() {
        check_with_rules(&InstructionView::instruction(token), context, rules)?;
        let child_context = context.child(token.get_string_repr());
        for parameter in token.get_params() {
            if let ValType::Literal(TextForgeParamTypes::Token(child)) = parameter {
                pending.push((child, child_context));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod test;

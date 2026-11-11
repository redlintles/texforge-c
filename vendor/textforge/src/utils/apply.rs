use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::resolve_var::TokenWrapper,
    utils::errors::{ErrorManager, TextForgeError},
};

pub fn apply_transform<'a>(
    token: &TokenWrapper,
    input: Cow<'a, str>,
    error_manager: &mut ErrorManager,
    context: &mut GlobalExecutionContext,
) -> Result<Cow<'a, str>, TextForgeError> {
    match token.apply_token(input, &mut *context) {
        Ok(x) => Ok(x),
        Err(e) => {
            error_manager.add_error(e.clone());
            Err(e)
        }
    }
}

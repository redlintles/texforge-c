use crate::{
    context::execution_context::GlobalExecutionContext,
    parse_args,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_vec_len,
    },
};
use std::borrow::Cow;

/// RMAPE — apply a named replacement map without cascading.
///
/// ```
/// use textforge::tokens::transforms::rmape::Rmape;
/// let _token = Rmape::default();
/// ```
#[derive(Clone, Default)]
pub struct Rmape {
    params: Vec<TextForgeParamTypes>,
}
impl InstructionMethods for Rmape {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "rmape"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        let values: Vec<_> = self
            .params
            .iter()
            .filter_map(|p| match p {
                TextForgeParamTypes::String(s) => Some(shell_words::quote(s).into_owned()),
                _ => None,
            })
            .collect();
        format!(
            "rmape {};\n",
            values.first().map(String::as_str).unwrap_or("x")
        )
        .into()
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 1, "rmape", "Invalid parameter count")?;
        let _name = parse_args!(params, 0, String, "Map name must be text");
        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_vec_len(&self.params, 1, "rmape", "Unbound map execution")?;
        let context = context.ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::RequiredContextError(
                    "Replacement maps require a context".into(),
                ),
                "rmape",
                "",
            )
        })?;
        let name = match &self.params[0] {
            TextForgeParamTypes::String(name) => name.as_str(),
            _ => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters("Map name must be text".into()),
                    "",
                    "",
                ));
            }
        };
        context.execute_replace_map(name, input)
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x40
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;

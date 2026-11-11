#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::tokens::InstructionMethods;

use crate::parser::params::TextForgeParamTypes;

use crate::utils::errors::{TextForgeError, TextForgeErrorCode};
use crate::utils::validations::check_vec_len;

/// Jsone - Json Escape
///
/// Escapes JSON Special Characters in `input` with serde_json::to_string
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::jsone::Jsone};
///
///
/// let token = Jsone::default();
/// let expected_output = "\"{banana: '10'}\"".to_string();
///
/// assert_eq!(token.transform("{banana: '10'}".into(),None).unwrap().to_string(), expected_output);
/// ```

#[derive(Clone, Default)]
pub struct Jsone {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Jsone {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "jsone"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        "jsone;\n".into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let result = serde_json::to_string(input.as_ref()).map_err(|_| {
            TextForgeError::new(
                TextForgeErrorCode::TextParsingError("Failed to serialize to JSON".into()),
                "serde_json::to_string".to_string(),
                input.to_string(),
            )
        })?;

        Ok(result.into())
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "jcmc", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x26
    }

    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

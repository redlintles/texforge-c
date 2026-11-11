#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::tokens::InstructionMethods;

use crate::parser::params::TextForgeParamTypes;

use crate::utils::errors::{TextForgeError, TextForgeErrorCode};
use crate::utils::validations::check_vec_len;

/// Jsonu - Json Unescape
///
/// Unescapes JSON Special Characters in `input` with serde_json::from_str
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::jsonu::Jsonu};
///
///
/// let token = Jsonu::default();
///
/// let expected_output = "{banana: '10'}".to_string();
///
/// assert_eq!(token.transform("\"{banana: '10'}\"".into(),None).unwrap().to_string(), expected_output);
/// ```

#[derive(Clone, Default)]
pub struct Jsonu {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Jsonu {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "jsonu"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        "jsonu;\n".into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if !input.contains('\\') {
            let result = serde_json::from_str::<&str>(&input).map_err(|_| {
                TextForgeError::new(
                    TextForgeErrorCode::TextParsingError("Failed to deserialize to JSON".into()),
                    "serde_json::from_str",
                    input.to_string(),
                )
            })?;
            let start = result.as_ptr() as usize - input.as_ptr() as usize;
            let end = start + result.len();
            return Ok(crate::utils::allocation::keep(input, start..end));
        }
        let result = serde_json::from_str::<String>(input.as_ref()).map_err(|_| {
            TextForgeError::new(
                TextForgeErrorCode::TextParsingError("Failed to deserialize to JSON".into()),
                "serde_json::from_str",
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
        0x27
    }

    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

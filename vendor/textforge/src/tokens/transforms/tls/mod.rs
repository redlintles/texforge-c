#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::tokens::InstructionMethods;

use crate::utils::errors::TextForgeError;

use crate::parser::params::TextForgeParamTypes;
use crate::utils::validations::check_vec_len;

/// TLS - Trim left sides
///
/// Trim the left Side of `input`, removing all spaces from the beginning
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::tls::Tls};
///
/// let token = Tls::default();
///
/// assert_eq!(token.transform("   banana   ".into(),None).unwrap().to_string(), "banana   ");
/// ```
///
#[derive(Clone, Default)]
pub struct Tls {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Tls {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        "tls;\n".into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let trimmed = input.trim_start();
        let start = trimmed.as_ptr() as usize - input.as_ptr() as usize;
        let end = start + trimmed.len();
        Ok(crate::utils::allocation::keep(input, start..end))
    }

    fn get_string_repr(&self) -> &'static str {
        "tls"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "tls", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x06
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

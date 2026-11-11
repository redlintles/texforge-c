#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;
/// DLF - Delete First
///
/// Deletes the first character of `input`
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::dlf::Dlf};
///
/// let token = Dlf::default();
///
/// assert_eq!(token.transform("banana".into(),None).unwrap().to_string(), "anana");
/// ```
///
#[derive(Clone, Default)]
pub struct Dlf {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Dlf {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        "dlf;\n".into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let cut = input.chars().next().map_or(0, char::len_utf8);
        let end = input.len();
        Ok(crate::utils::allocation::keep(input, cut..end))
    }

    fn get_string_repr(&self) -> &'static str {
        "dlf"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "dlf", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x03
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

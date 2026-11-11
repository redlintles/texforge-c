#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;
/// TLA - To Lowercase All
///
/// Lowercases every character from `input`
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::tla::Tla};
///
/// let token = Tla::default();
///
/// assert_eq!(token.transform("BANANA".into(),None).unwrap().to_string(), "banana");
/// ```
///
#[derive(Clone, Default)]
pub struct Tla {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Tla {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "tla"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        "tla;\n".into()
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if input.is_empty() {
            return Ok(input);
        }
        if input.is_ascii() {
            if !input.bytes().any(|c| c.is_ascii_uppercase()) {
                return Ok(input);
            }
            let mut output = input.into_owned();
            output.make_ascii_lowercase();
            return Ok(output.into());
        }
        Ok(input.to_lowercase().into())
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "tla", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x13
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

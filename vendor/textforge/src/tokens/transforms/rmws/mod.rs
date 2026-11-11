#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;

/// RMWS - Remove Whitespace
///
/// Removes all whitespaces in `input`
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::rmws::Rmws};
///
/// let token = Rmws::default();
///
/// assert_eq!(token.transform("banana laranja cheia de canja".into(),None).unwrap().to_string(), "bananalaranjacheiadecanja");
/// ```
///
#[derive(Clone, Default)]
pub struct Rmws {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Rmws {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "rmws"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        "rmws;\n".into()
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if !input.chars().any(char::is_whitespace) {
            return Ok(input);
        }
        let mut output = input.into_owned();
        output.retain(|c| !c.is_whitespace());
        Ok(output.into())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "rmws", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x31
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;

/// JPSC - Join to PascalCase
///
/// If `input` is a string whose words are separated by spaces, join `input` as a PascalCase string
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::jpsc::Jpsc};
///
/// let token = Jpsc::default();
///
/// assert_eq!(token.transform("banana laranja cheia de canja".into(),None).unwrap().to_string(), "BananaLaranjaCheiaDeCanja");
/// ```
///
#[derive(Clone, Default)]
pub struct Jpsc {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Jpsc {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "jpsc"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        "jpsc;\n".into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut result = String::with_capacity(input.len());
        for word in input.split_whitespace() {
            crate::utils::allocation::capitalize_into(&mut result, word);
        }
        Ok(result.into())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "jpsc", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x2e
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;

/// URLE - URL Encode
///
/// Encodes `input` to the URL Encoding Format
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::urle::Urle};
///
/// let token = Urle::default();
///
/// assert_eq!(token.transform("banana laranja".into(),None).unwrap().to_string(), "banana%20laranja");
/// ```
///
#[derive(Clone, Default)]
pub struct Urle {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Urle {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "urle"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        "urle;\n".into()
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let result = urlencoding::encode(input.as_ref());
        match result {
            Cow::Borrowed(_) => Ok(input),
            Cow::Owned(v) => Ok(v.into()),
        }
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "urle", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x20
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

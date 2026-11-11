#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;

/// JCMC - Join to Camel Case
///
/// If `input` is a string whose words are separated by spaces, join `input` as a camelCase string
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::jcmc::Jcmc};
///
/// let token = Jcmc::default();
///
/// assert_eq!(token.transform("banana laranja cheia de canja".into(),None).unwrap().to_string(), "bananaLaranjaCheiaDeCanja");
/// ```
///
#[derive(Clone, Default)]
pub struct Jcmc {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Jcmc {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "jcmc"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        "jcmc;\n".into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut result = String::with_capacity(input.len());
        for (i, word) in input.split_whitespace().enumerate() {
            if i == 0 {
                result.push_str(word);
            } else {
                crate::utils::allocation::capitalize_into(&mut result, word);
            }
        }
        Ok(result.into())
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x2d
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "jcmc", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

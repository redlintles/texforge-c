#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::tokens::InstructionMethods;

use crate::parser::params::TextForgeParamTypes;
use crate::utils::errors::TextForgeError;
use crate::utils::validations::check_vec_len;

/// SPLC - Split Characters
///
/// Split `input` characters in a result whose chars are separed by spaces
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::splc::Splc};
///
/// let token = Splc::default();
///
/// assert_eq!(token.transform("banana".into(), None).unwrap().to_string(), "b a n a n a");
/// ```
///
#[derive(Clone, Default)]
pub struct Splc {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Splc {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "splc"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        "splc;\n".into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut output = String::with_capacity(input.len());
        for (i, ch) in input.chars().enumerate() {
            if i != 0 {
                output.push(' ');
            }
            output.push(ch);
        }
        Ok(output.into())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "rmws", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x23
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

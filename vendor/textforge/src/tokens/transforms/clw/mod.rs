#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;
/// Token `Clw` — Capitalize Last Word
///
/// Capitalizes the last word of `input`
///
/// This is achieved by splitting the input by whitespace, reversing the resulting vector,
/// capitalizing the first word, reversing it back, and rejoining into a single string.
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::clw::Clw};
///
/// let token = Clw::default();
/// assert_eq!(token.transform("foo bar".into(),None).unwrap().to_string(), "foo Bar");
/// ```
#[derive(Clone, Default)]
pub struct Clw {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Clw {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "clw"
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let start = input.rfind(' ').map_or(0, |i| i + 1);
        let mut chars = input[start..].chars();
        let Some(first) = chars.next() else {
            return Ok(input);
        };
        let replacement: String = first.to_uppercase().collect();
        Ok(crate::utils::allocation::replace(
            input,
            start..start + first.len_utf8(),
            &replacement,
        ))
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        "clw;\n".into()
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parser::params::TextForgeParamTypesJoin;

        check_vec_len(params, 0, "clw", params.join(""))?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x19
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

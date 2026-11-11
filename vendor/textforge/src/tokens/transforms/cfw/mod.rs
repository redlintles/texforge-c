#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;
/// Token `Cfw` — Capitalize First Word
///
/// Capitalizes the first word of `input`
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::cfw::Cfw};
///
/// let token = Cfw::default();
/// assert_eq!(token.transform("foo bar".into(),None).unwrap().to_string(), "Foo bar");
/// ```
#[derive(Clone, Default)]
pub struct Cfw {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Cfw {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "cfw"
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let Some(first) = input.chars().next() else {
            return Ok(input);
        };
        let replacement: String = first.to_uppercase().collect();
        Ok(crate::utils::allocation::replace(
            input,
            0..first.len_utf8(),
            &replacement,
        ))
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        "cfw;\n".into()
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parser::params::TextForgeParamTypesJoin;

        check_vec_len(params, 0, "cfw", params.join(""))?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x18
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

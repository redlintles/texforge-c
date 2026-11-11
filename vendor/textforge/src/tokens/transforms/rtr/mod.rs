#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;
/// RTR - Rotate Right
///
/// Rotates `input` to the right `n` times
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::rtr::Rtr};
///
/// let token = Rtr::new(2);
///
/// assert_eq!(token.transform("banana".into(),None).unwrap().to_string(), "nabana");
///
/// ```
#[derive(Clone, Default)]
pub struct Rtr {
    pub times: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Rtr {
    pub fn new(times: usize) -> Rtr {
        Rtr {
            times,
            params: vec![times.into()],
        }
    }
}

impl InstructionMethods for Rtr {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let len = input.chars().count();
        if len == 0 {
            return Ok(input);
        }
        let times = self.times % len;
        if times == 0 {
            return Ok(input);
        }
        let split = input
            .char_indices()
            .nth(len - times)
            .map_or(input.len(), |(i, _)| i);
        let mut output = String::with_capacity(input.len());
        output.push_str(&input[split..]);
        output.push_str(&input[..split]);
        Ok(output.into())
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("rtr {};\n", self.times).into()
    }
    fn get_string_repr(&self) -> &'static str {
        "rtr"
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 1, "rtr", "")?;

        self.times = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.params = vec![self.times.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x0f
    }

    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> =
            to_bytecode!(self.get_opcode(), [TextForgeParamTypes::Usize(self.times),]);
        Ok(result)
    }
}

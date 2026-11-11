#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::parser::params::TextForgeParamTypes;
use crate::tokens::InstructionMethods;
use crate::utils::validations::check_vec_len;

use crate::utils::errors::TextForgeError;

/// RTL - Rotate Left
///
/// Rotates `input` to the left `n` times
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::rtl::Rtl};
///
/// let token = Rtl::new(3);
///
/// assert_eq!(token.transform("banana".into(),None).unwrap().to_string(), "anaban");
///
/// ```
#[derive(Clone, Default)]
pub struct Rtl {
    pub times: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Rtl {
    pub fn new(times: usize) -> Rtl {
        Rtl {
            times,
            params: vec![times.into()],
        }
    }
}

impl InstructionMethods for Rtl {
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
            .nth(times)
            .map_or(input.len(), |(i, _)| i);
        let mut output = String::with_capacity(input.len());
        output.push_str(&input[split..]);
        output.push_str(&input[..split]);
        Ok(output.into())
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("rtl {};\n", self.times).into()
    }

    fn get_string_repr(&self) -> &'static str {
        "rtl"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 1, "rtl", "")?;

        self.times = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.params = vec![self.times.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x0e
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> =
            to_bytecode!(self.get_opcode(), [TextForgeParamTypes::Usize(self.times),]);
        Ok(result)
    }
}

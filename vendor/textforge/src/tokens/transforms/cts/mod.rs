#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::validations::{check_index_against_input, check_vec_len},
};

use crate::utils::errors::TextForgeError;

use crate::parser::params::TextForgeParamTypes;

/// Token `Cts` — Capitalize Single
///
/// Capitalizes a single word at the given index `i` within the input string.
///
/// Words are defined as sequences of characters separated by whitespace,
/// following the behavior of `input.split_whitespace()`.
///
/// If `i` is out of bounds for the number of words in the input, an `TextForgeError` is returned.
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::cts::Cts};
/// let token = Cts::new(1);
/// assert_eq!(token.transform("foo bar".into(),None).unwrap().to_string(), "foo Bar");
/// ```

#[derive(Clone, Default)]
pub struct Cts {
    pub index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Cts {
    pub fn new(index: usize) -> Self {
        Cts {
            index,
            params: vec![index.into()],
        }
    }
}

impl InstructionMethods for Cts {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "cts"
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_index_against_input(self.index, &input)?;
        let mut result = String::with_capacity(input.len());
        for (i, word) in input.split_whitespace().enumerate() {
            if i != 0 {
                result.push(' ');
            }
            if i == self.index {
                crate::utils::allocation::capitalize_into(&mut result, word);
            } else {
                result.push_str(word);
            }
        }
        Ok(result.into())
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("cts {};\n", self.index).into()
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 1, "cts", "")?;

        self.index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.params = vec![self.index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x1d
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> =
            to_bytecode!(self.get_opcode(), [TextForgeParamTypes::Usize(self.index),]);
        Ok(result)
    }
}

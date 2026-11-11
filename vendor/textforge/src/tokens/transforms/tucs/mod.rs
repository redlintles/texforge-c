#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{
        errors::TextForgeError,
        validations::{check_index_against_input, check_vec_len},
    },
};

use crate::parser::params::TextForgeParamTypes;

/// TUCS - To Uppercase Single
///
/// Uppercases a single character in `input` identified by `index`
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::tucs::Tucs};
///
/// let token = Tucs::new(1);
///
/// assert_eq!(token.transform("banana".into(),None).unwrap().to_string(), "bAnana");
///
/// ```

#[derive(Clone, Default)]
pub struct Tucs {
    index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Tucs {
    pub fn new(index: usize) -> Self {
        Tucs {
            index,
            params: vec![index.into()],
        }
    }
}

impl InstructionMethods for Tucs {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "tucs"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("tucs {};\n", self.index).into()
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_index_against_input(self.index, input.as_ref())?;
        let mut result = String::with_capacity(input.len());
        for (i, c) in input.char_indices() {
            if i == self.index {
                result.extend(c.to_uppercase());
            } else {
                result.push(c);
            }
        }
        Ok(result.into())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 1, "tucs", "")?;

        self.index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.params = vec![self.index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x14
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> =
            to_bytecode!(self.get_opcode(), [TextForgeParamTypes::Usize(self.index),]);
        Ok(result)
    }
}

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

/// TLCS - To Lowercase Single
///
/// Lowercases a single character in `input` identified by `index`
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::tlcs::Tlcs};
///
/// let token = Tlcs::new(1);
///
/// assert_eq!(token.transform("BANANA".into(),None).unwrap().to_string(), "BaNANA");
///
/// ```

#[derive(Clone, Default)]
pub struct Tlcs {
    index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Tlcs {
    pub fn new(index: usize) -> Self {
        Tlcs {
            index,
            params: vec![index.into()],
        }
    }
}

impl InstructionMethods for Tlcs {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "tlcs"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("tlcs {};\n", self.index).into()
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_index_against_input(self.index, input.as_ref())?;

        let mut result = String::with_capacity(input.len());
        for (i, c) in input.chars().enumerate() {
            if i == self.index {
                result.extend(c.to_lowercase());
            } else {
                result.push(c);
            }
        }
        Ok(result.into())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 1, "tlcs", "")?;

        self.index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.params = vec![self.index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x15
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> =
            to_bytecode!(self.get_opcode(), [TextForgeParamTypes::Usize(self.index),]);
        Ok(result)
    }
}

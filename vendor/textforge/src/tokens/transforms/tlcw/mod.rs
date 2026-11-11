#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::parser::params::TextForgeParamTypes;
use crate::utils::validations::check_vec_len;
use crate::{
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_index_against_words},
};

/// TLCW - To Lowercase Word
///
/// Lowercase a single word of string
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::tlcw::Tlcw};
///
/// let token = Tlcw::new(1);
///
/// assert_eq!(token.transform("BANANA LARANJA CHEIA DE CANJA".into(),None).unwrap().to_string(), "BANANA laranja CHEIA DE CANJA");
///
/// ```
#[derive(Clone, Default)]
pub struct Tlcw {
    index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Tlcw {
    pub fn new(index: usize) -> Self {
        Tlcw {
            index,
            params: vec![index.into()],
        }
    }
}
impl InstructionMethods for Tlcw {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "tlcw"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("tlcw {};\n", self.index).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_index_against_words(self.index, input.as_ref())?;
        let mut result = String::with_capacity(input.len());
        for (i, word) in input.split_whitespace().enumerate() {
            if i != 0 {
                result.push(' ');
            }
            if i == self.index {
                result.push_str(&word.to_lowercase());
            } else {
                result.push_str(word);
            }
        }
        Ok(result.into())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 1, "tlcw", "")?;

        self.index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.params = vec![self.index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x29
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> =
            to_bytecode!(self.get_opcode(), [TextForgeParamTypes::Usize(self.index),]);
        Ok(result)
    }
}

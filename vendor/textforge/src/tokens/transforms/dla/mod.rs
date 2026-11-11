#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::parser::params::TextForgeParamTypes;
use crate::tokens::InstructionMethods;
use crate::utils::validations::{check_index_against_input, check_vec_len};

use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

/// Dla - Delete After
/// Delete all characters after `index` in the specified `input`
///
/// It will throw an `TextForgeError` if index does not exists in the current `input`
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::dla::Dla};
///
/// let token = Dla::new(3);
///
/// assert_eq!(token.transform("banana laranja vermelha azul".into(),None).unwrap().to_string(), "bana")
///
/// ```
#[derive(Clone, Default)]
pub struct Dla {
    pub index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Dla {
    pub fn new(index: usize) -> Self {
        Dla {
            index,
            params: vec![index.into()],
        }
    }
}

impl InstructionMethods for Dla {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("dla {};\n", self.index).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_index_against_input(self.index, &input)?;

        if let Some(byte_index) = input.char_indices().nth(self.index + 1).map(|(i, _)| i) {
            return Ok(crate::utils::allocation::keep(input, 0..byte_index));
        }

        Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange(
                "Index is out of range for the desired string".into(),
            ),
            self.to_textforge_line(),
            input.as_ref().to_string(),
        ))
    }

    fn get_string_repr(&self) -> &'static str {
        "dla"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 1, "dla", "")?;
        self.index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.params = vec![self.index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x09
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> =
            to_bytecode!(self.get_opcode(), [TextForgeParamTypes::Usize(self.index),]);
        Ok(result)
    }
}

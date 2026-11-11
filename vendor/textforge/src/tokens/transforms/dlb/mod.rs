#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

use crate::parser::params::TextForgeParamTypes;
use crate::tokens::InstructionMethods;
use crate::utils::validations::{check_index_against_input, check_vec_len};

/// Dlb - Delete Before
/// Delete all characters before `index` in the specified `input`
///
/// It will throw an `TextForgeError` if index does not exists in the current `input`
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::dlb::Dlb};
///
/// let token = Dlb::new(3);
///
/// assert_eq!(token.transform("banana laranja vermelha azul".into(),None).unwrap().to_string(), "ana laranja vermelha azul")
///
/// ```
#[derive(Clone, Default)]
pub struct Dlb {
    pub index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Dlb {
    pub fn new(index: usize) -> Self {
        Dlb {
            index,
            params: vec![index.into()],
        }
    }
}

impl InstructionMethods for Dlb {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("dlb {};\n", self.index).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_index_against_input(self.index, input.as_ref())?;

        if let Some(byte_index) = input.char_indices().nth(self.index).map(|(i, _)| i) {
            let end = input.len();
            return Ok(crate::utils::allocation::keep(input, byte_index..end));
        }

        Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange(
                format!(
                    "Supported indexes 0-{}, entered index {}",
                    input.chars().count().saturating_sub(1),
                    self.index
                )
                .into(),
            ),
            self.to_textforge_line(),
            input.to_string(),
        ))
    }
    fn get_string_repr(&self) -> &'static str {
        "dlb"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 1, "dlb", "")?;

        self.index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.params = vec![self.index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x0a
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> =
            to_bytecode!(self.get_opcode(), [TextForgeParamTypes::Usize(self.index),]);
        Ok(result)
    }
}

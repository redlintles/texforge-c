#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::parser::params::TextForgeParamTypes;
use crate::utils::validations::check_vec_len;
use crate::{tokens::InstructionMethods, utils::validations::check_chunk_bound_indexes};

use crate::utils::errors::TextForgeError;

/// Slt - Select
///
/// Selects a subslice of `input` delimited by `start_index` and `end_index`(inclusive) discarding the rest in the process
/// If end_index is bigger than the length of the string, the subslice will include up to the last character of `input`
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods,transforms::slt::Slt};
///
/// let token = Slt::new(1,9999).unwrap();
///
/// assert_eq!(token.transform("banàna".into(),None).unwrap().to_string(), "anàna");
///
///
/// ```
#[derive(Clone, Default)]
pub struct Slt {
    pub start_index: usize,
    pub end_index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Slt {
    pub fn new(start_index: usize, end_index: usize) -> Result<Self, TextForgeError> {
        check_chunk_bound_indexes(start_index, end_index, None)?;
        Ok(Slt {
            start_index,
            end_index,
            params: vec![start_index.into(), end_index.into()],
        })
    }
}

impl InstructionMethods for Slt {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "slt"
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let len = input.chars().count();
        let mut end = self.end_index;

        if end > len {
            end = len - 1;
        }

        check_chunk_bound_indexes(self.start_index, end, Some(input.as_ref()))?;

        let start_byte = input
            .char_indices()
            .nth(self.start_index)
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Fim EXCLUSIVO: byte do (end_index + 1)º char, ou input.len() se passar do fim
        let end_byte_exclusive = input
            .char_indices()
            .nth(end.saturating_add(1))
            .map(|(i, _)| i)
            .unwrap_or(input.len());

        Ok(crate::utils::allocation::keep(
            input,
            start_byte..end_byte_exclusive,
        ))
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("slt {} {};\n", self.start_index, self.end_index).into()
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x11
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "slt", "")?;

        self.start_index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.end_index = parse_args!(params, 1, Usize, "Index should be of usize type");
        self.params = vec![self.start_index.into(), self.end_index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::Usize(self.start_index),
                TextForgeParamTypes::Usize(self.end_index),
            ]
        );
        Ok(result)
    }
}

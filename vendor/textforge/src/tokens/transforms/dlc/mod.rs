#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::utils::validations::check_vec_len;
use crate::{tokens::InstructionMethods, utils::validations::check_chunk_bound_indexes};

use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

use crate::parser::params::TextForgeParamTypes;
/// Dlc - Delete Chunk
///
/// Deletes an specific subslice of `input` delimited by `start_index` and `end_index`(inclusive)
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::dlc::Dlc};
///
/// let token = Dlc::new(1,5).unwrap();
///
/// assert_eq!(token.transform("bananalaranjacheiadecanja".into(),None).unwrap().to_string(), "blaranjacheiadecanja")
///
/// ```
#[derive(Clone, Default)]
pub struct Dlc {
    pub start_index: usize,
    pub end_index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Dlc {
    pub fn new(start_index: usize, end_index: usize) -> Result<Self, TextForgeError> {
        check_chunk_bound_indexes(start_index, end_index, None)?;
        Ok(Dlc {
            start_index,
            end_index,
            params: vec![start_index.into(), end_index.into()],
        })
    }
}

impl InstructionMethods for Dlc {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("dlc {} {};\n", self.start_index, self.end_index).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let len = input.chars().count();

        // opcional: se string vazia, deletar "tudo" vira vazio
        if len == 0 {
            return Ok("".into());
        }

        let mut end = self.end_index;

        // ✅ clamp correto (inclusive range)
        if end >= len {
            end = len - 1;
        }

        check_chunk_bound_indexes(self.start_index, end, Some(input.as_ref()))?;

        let start_index = input
            .char_indices()
            .nth(self.start_index)
            .map(|(i, _)| i)
            .ok_or_else(|| {
                TextForgeError::new(
                    TextForgeErrorCode::IndexOutOfRange(
                        format!(
                            "Invalid Index for this specific input, supported indexes 0-{}, entered index {}",
                            input.chars().count().saturating_sub(1),
                            self.start_index
                        ).into()
                    ),
                    self.to_textforge_line(),
                    input.to_string()
                )
            })?;

        // ✅ agora end é garantidamente <= last_index, então end+1 pode ser == len
        let end_index = input
            .char_indices()
            .nth(end + 1)
            .map(|(i, _)| i)
            .unwrap_or_else(|| input.len()); // se end é o último char, after começa no fim

        Ok(crate::utils::allocation::replace(
            input,
            start_index..end_index,
            "",
        ))
    }

    fn get_string_repr(&self) -> &'static str {
        "dlc"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "dlc", "")?;

        self.start_index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.end_index = parse_args!(params, 1, Usize, "Index should be of usize type");
        self.params = vec![self.start_index.into(), self.end_index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x08
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

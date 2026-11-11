#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{
        errors::TextForgeError,
        validations::{check_chunk_bound_indexes, check_vec_len},
    },
};

use crate::parser::params::TextForgeParamTypes;

/// TLCC - To Lowercase Chunk
///
/// Lowercases every character from a chunk delimited by `start_index` and `end_index`(inclusive) in `input`
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::tlcc::Tlcc};
///
/// let token = Tlcc::new(1,4).unwrap();
///
/// assert_eq!(token.transform("BANANA".into(),None).unwrap().to_string(), "BananA");
/// ```
///
#[derive(Clone, Default)]
pub struct Tlcc {
    start_index: usize,
    end_index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Tlcc {
    pub fn new(start_index: usize, end_index: usize) -> Result<Self, TextForgeError> {
        check_chunk_bound_indexes(start_index, end_index, None)?;
        Ok(Tlcc {
            start_index,
            end_index,
            params: vec![start_index.into(), end_index.into()],
        })
    }
}

impl InstructionMethods for Tlcc {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "tlcc"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("tlcc {} {};\n", self.start_index, self.end_index).into()
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_chunk_bound_indexes(self.start_index, self.end_index, Some(input.as_ref()))?;

        let total_chars = input.chars().count();
        let last_char_index = total_chars.saturating_sub(1);

        let end = if self.end_index > last_char_index {
            last_char_index
        } else {
            self.end_index
        };

        let mut result = String::with_capacity(input.len());
        for (i, c) in input.chars().enumerate() {
            if i >= self.start_index && i <= end {
                result.extend(c.to_lowercase());
            } else {
                result.push(c);
            }
        }
        Ok(result.into())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "tlcc", "")?;

        self.start_index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.end_index = parse_args!(params, 1, Usize, "Index should be of usize type");
        self.params = vec![self.start_index.into(), self.end_index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x17
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

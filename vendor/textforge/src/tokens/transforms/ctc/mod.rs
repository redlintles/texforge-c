#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::utils::errors::TextForgeError;
use crate::utils::validations::check_vec_len;
use crate::{tokens::InstructionMethods, utils::validations::check_chunk_bound_indexes};

use crate::parser::params::TextForgeParamTypes;
/// Token `Ctc` — Capitalize Chunk
///
/// Capitalizes every word in a character slice of the input, defined by `start_index` and `end_index` (inclusive).
///
/// The range is applied directly to the character indices of the original string. The extracted chunk is then split
/// into words (using `split_whitespace()`), capitalized individually, and finally reinserted into the original string.
///
/// - If `start_index` is out of bounds for the number of characters in the input, an `TextForgeError` is returned.
/// - If `end_index` exceeds the input's length, it will be clamped to the input's character count.
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::ctc::Ctc};
///
/// let token = Ctc::new(1, 5).unwrap();
/// assert_eq!(token.transform("bananabananosa".into(),None).unwrap().to_string(), "bAnanabananosa");
/// ```
#[derive(Clone, Default)]
pub struct Ctc {
    pub start_index: usize,
    pub end_index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Ctc {
    pub fn new(start_index: usize, end_index: usize) -> Result<Self, TextForgeError> {
        check_chunk_bound_indexes(start_index, end_index, None)?;
        Ok(Ctc {
            start_index,
            end_index,
            params: vec![start_index.into(), end_index.into()],
        })
    }
}

impl InstructionMethods for Ctc {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "ctc"
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

        check_chunk_bound_indexes(self.start_index, end, Some(&input))?;

        // Convert char indices to byte indices
        let start_byte = input
            .char_indices()
            .nth(self.start_index)
            .map(|(byte_idx, _)| byte_idx)
            .unwrap(); // safe: start_index < total_chars

        let end_byte = if end + 1 >= len {
            input.len()
        } else {
            input
                .char_indices()
                .nth(end + 1)
                .map(|(byte_idx, _)| byte_idx)
                .unwrap()
        };

        let mut result = String::with_capacity(input.len());
        result.push_str(&input[..start_byte]);
        let mut first = true;
        for ch in input[start_byte..end_byte].chars() {
            if ch.is_whitespace() {
                result.push(ch);
                first = true;
            } else {
                if first {
                    result.extend(ch.to_uppercase());
                } else {
                    result.push(ch);
                }
                first = false;
            }
        }
        result.push_str(&input[end_byte..]);
        Ok(result.into())
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("ctc {} {};\n", self.start_index, self.end_index).into()
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;
        use crate::parser::params::TextForgeParamTypesJoin;

        check_vec_len(params, 2, "ctc", params.join(""))?;

        self.start_index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.end_index = parse_args!(params, 1, Usize, "Index should be of usize type");
        self.params = vec![self.start_index.into(), self.end_index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x1b
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::Usize(self.start_index),
                TextForgeParamTypes::Usize(self.end_index),
            ]
        );
        Ok(result)
    }
}

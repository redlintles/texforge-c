#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::utils::errors::TextForgeError;

use crate::parser::params::TextForgeParamTypes;
use crate::utils::validations::check_vec_len;
use crate::{tokens::InstructionMethods, utils::validations::check_chunk_bound_indexes};

/// Token `Ctr` — Capitalize Range
///
/// Capitalizes a range of words delimited by `start_index` and `end_index`(inclusive)
///
/// Words are defined as sequences of characters separated by whitespace,
/// following the behavior of `input.split_whitespace()`.
///
/// If `start_index` is out of bounds for the number of words in the `input``, an `TextForgeError` is returned.
/// If `end_index` is out of bound for the number of words in the input, it's clamped up to the number of words in `input`
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods,transforms::ctr::Ctr};
/// let token = Ctr::new(1,5).unwrap();
/// assert_eq!(token.transform("foo bar mar".into(),None).unwrap().to_string(), "foo Bar Mar");
/// ```
///
#[derive(Clone, Default)]
pub struct Ctr {
    pub start_index: usize,
    pub end_index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Ctr {
    pub fn new(start_index: usize, end_index: usize) -> Result<Self, TextForgeError> {
        check_chunk_bound_indexes(start_index, end_index, None)?;
        Ok(Ctr {
            start_index,
            end_index,
            params: vec![start_index.into(), end_index.into()],
        })
    }
}

impl InstructionMethods for Ctr {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "ctr"
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if input.trim().is_empty() {
            return Ok(input);
        }

        // Since the user will probably not know the length of the string in the middle of the processing
        // Better simply adjust end_index to input.len() if its bigger. instead of throwing an "hard to debug" error

        let mut end = self.end_index;
        let total = input.split_whitespace().count();
        if end > total {
            end = total;
        }
        check_chunk_bound_indexes(self.start_index, end, Some(&input))?;

        let mut result = String::with_capacity(input.len());
        for (i, word) in input.split_whitespace().enumerate() {
            if i != 0 {
                result.push(' ');
            }
            if (self.start_index..=end).contains(&i) {
                crate::utils::allocation::capitalize_into(&mut result, word);
            } else {
                result.push_str(word);
            }
        }
        Ok(result.into())
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("ctr {} {};\n", self.start_index, self.end_index).into()
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "ctr", "")?;

        self.start_index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.end_index = parse_args!(params, 1, Usize, "Index should be of usize type");
        self.params = vec![self.start_index.into(), self.end_index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x1c
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

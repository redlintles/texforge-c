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
/// TUCC - To uppercase Chunk
///
/// Lowercases every character from a chunk delimited by `start_index` and `end_index`(inclusive) in `input`
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::tucc::Tucc};
///
/// let token = Tucc::new(1,4).unwrap();
///
/// assert_eq!(token.transform("banana".into(),None).unwrap().to_string(), "bANANa");
/// ```
///
#[derive(Clone, Default)]
pub struct Tucc {
    start_index: usize,
    end_index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Tucc {
    pub fn new(start_index: usize, end_index: usize) -> Result<Self, TextForgeError> {
        check_chunk_bound_indexes(start_index, end_index, None)?;
        Ok(Tucc {
            start_index,
            end_index,
            params: vec![start_index.into(), end_index.into()],
        })
    }
}

impl InstructionMethods for Tucc {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "tucc"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("tucc {} {};\n", self.start_index, self.end_index).into()
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_chunk_bound_indexes(self.start_index, self.end_index, Some(input.as_ref()))?;

        // Since the user will probably not know the length of the string in the middle of the processing
        // Better simply adjust end_index to input.len() if its bigger. instead of throwing an "hard to debug" error
        let mut end = self.end_index;
        let total = input.chars().count();

        if end > total {
            end = input.len();
        }
        let mut result = String::with_capacity(input.len());
        for (i, c) in input.chars().enumerate() {
            if i >= self.start_index && i <= end {
                result.extend(c.to_uppercase());
            } else {
                result.push(c);
            }
        }
        Ok(result.into())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "tucc", "")?;

        self.start_index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.end_index = parse_args!(params, 1, Usize, "Index should be of usize type");
        self.params = vec![self.start_index.into(), self.end_index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x16
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

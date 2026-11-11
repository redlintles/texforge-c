#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::parser::params::TextForgeParamTypes;

use crate::utils::validations::check_vec_len;
use crate::{
    tokens::InstructionMethods,
    utils::errors::{TextForgeError, TextForgeErrorCode},
};
/// Ins - Insert
///
/// Inserts `text` after `index` position in `input`
///
/// If index does not exists in current string, `TextForgeError` is returned
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::ins::Ins};
///
/// let token = Ins::new(2,"laranja");
///
/// assert_eq!(token.transform("banana".into(),None).unwrap().to_string(), "banlaranjaana");
/// ```
#[derive(Clone, Default)]
pub struct Ins {
    index: usize,
    text_to_insert: String,
    params: Vec<TextForgeParamTypes>,
}

impl Ins {
    pub fn new(index: usize, text_to_insert: &str) -> Self {
        Ins {
            index,
            text_to_insert: text_to_insert.to_string(),
            params: vec![index.into(), text_to_insert.to_string().into()],
        }
    }
}
impl InstructionMethods for Ins {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "ins"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("ins {} {};\n", self.index, self.text_to_insert).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if self.index > input.chars().count() {
            return Err(
                TextForgeError::new(
                    TextForgeErrorCode::IndexOutOfRange(
                        format!(
                            "Index does not exist in current string, for the input {}, only indexes between 0-{} are allowed",
                            input,
                            input.chars().count() - 1
                        ).into()
                    ),
                    self.to_textforge_line(),
                    input.to_string()
                )
            );
        }
        let byte_index = input
            .char_indices()
            .nth(self.index + 1)
            .map(|(i, _)| i)
            .unwrap_or(input.len());

        Ok(crate::utils::allocation::replace(
            input,
            byte_index..byte_index,
            &self.text_to_insert,
        ))
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "ins", "")?;

        self.index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.text_to_insert =
            parse_args!(params, 1, String, "Text_to_insert should be of String type");

        self.params = vec![self.index.into(), self.text_to_insert.to_string().into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x28
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::Usize(self.index),
                TextForgeParamTypes::String(self.text_to_insert.clone()),
            ]
        );
        Ok(result)
    }
}

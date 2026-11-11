#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{
        errors::TextForgeError,
        validations::{check_index_against_input, check_vec_len},
    },
};

use crate::parser::params::TextForgeParamTypes;

/// DLS - Delete Single
///
/// Delete's a single character specified by `index` in `input`
///
/// It will throw an `TextForgeError` if index does not exists in `input`
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::dls::Dls};
///
/// let token = Dls::new(3);
///
/// assert_eq!(token.transform("banana".into(),None).unwrap().to_string(), "banna");
/// ```
#[derive(Clone, Default)]
pub struct Dls {
    pub index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Dls {
    pub fn new(index: usize) -> Self {
        Dls {
            index,
            params: vec![index.into()],
        }
    }
}

impl InstructionMethods for Dls {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "dls"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("dls {};\n", self.index).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_index_against_input(self.index, &input)?;
        let (start, ch) = input.char_indices().nth(self.index).ok_or_else(|| {
            TextForgeError::new(
                crate::utils::errors::TextForgeErrorCode::IndexOutOfRange(
                    "Invalid character index".into(),
                ),
                "dls",
                input.to_string(),
            )
        })?;
        Ok(crate::utils::allocation::replace(
            input,
            start..start + ch.len_utf8(),
            "",
        ))
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 1, "dls", "")?;

        self.index = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.params = vec![self.index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x32
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> =
            to_bytecode!(self.get_opcode(), [TextForgeParamTypes::Usize(self.index),]);
        Ok(result)
    }
}

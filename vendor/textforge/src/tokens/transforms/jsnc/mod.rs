#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;

/// JSNC - Join to Snake Case
///
/// If `input` is a string whose words are separated by spaces, join `input` as a snake_case lowercased string
/// For conversion between the screaming and normal versions of this case style, use the tokens TLA(To Lowercase All) and TUA(To Uppercase All) together with this one.
///
/// See Also:
///
/// - [`Tua` - To Uppercase All](crate::tokens::transforms::tua)
/// - [`Tla` - To Lowercase All](crate::tokens::transforms::tla)
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::jsnc::Jsnc};
///
/// let token = Jsnc::default();
///
/// assert_eq!(token.transform("banana laranja cheia de canja".into(),None).unwrap().to_string(), "banana_laranja_cheia_de_canja");
/// ```
///
#[derive(Clone, Default)]
pub struct Jsnc {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Jsnc {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "jsnc"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        "jsnc;\n".into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut result = String::with_capacity(input.len());
        for (i, word) in input.split_whitespace().enumerate() {
            if i != 0 {
                result.push('_');
            }
            result.push_str(word);
        }
        if result.is_ascii() {
            result.make_ascii_lowercase();
        } else {
            result = result.to_lowercase();
        }
        Ok(result.into())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "jsnc", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x2c
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_vec_len,
    },
};

use crate::parser::params::TextForgeParamTypes;
/// URLD - URL Decode
///
/// Decodes `input` from the URL Encoding Format
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::urld::Urld};
///
/// let token = Urld::default();
///
/// assert_eq!(token.transform("banana%20laranja".into(),None).unwrap().to_string(), "banana laranja");
/// ```
///

#[derive(Clone, Default)]
pub struct Urld {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Urld {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "urld"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        "urld;\n".into()
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        // Validação de percent encoding
        let bytes = input.as_bytes();
        let len = bytes.len();

        let mut i = 0;
        while i < len {
            if bytes[i] == b'%' {
                if i + 2 >= len
                    || !bytes[i + 1].is_ascii_hexdigit()
                    || !bytes[i + 2].is_ascii_hexdigit()
                {
                    return Err(TextForgeError::new(
                        TextForgeErrorCode::TextParsingError("Failed parsing URL string".into()),
                        "urld",
                        input.to_string(),
                    ));
                }
                i += 3;
                continue;
            }
            i += 1;
        }

        let result = urlencoding::decode(input.as_ref()).map_err(|_| {
            TextForgeError::new(
                TextForgeErrorCode::TextParsingError("Failed parsing URL string".into()),
                "urld",
                input.to_string(),
            )
        })?;

        match result {
            Cow::Borrowed(_) => Ok(input),
            Cow::Owned(v) => Ok(v.into()),
        }
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x21
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "urld", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

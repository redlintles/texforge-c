#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;

/// Token `Atb` — Add to Beginning
///
/// Adds `text` to the beginning of `input`
///
/// - See Also
///
/// - [ATE - Add to End](crate::tokens::transforms::ate)
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::atb::Atb};
///
/// let token = Atb::new("foo");
/// assert_eq!(token.transform(" bar".into(), None).unwrap().to_string(), "foo bar");
/// ```
#[derive(Clone, Default)]
pub struct Atb {
    pub text: String,
    params: Vec<TextForgeParamTypes>,
}

impl Atb {
    pub fn new(text: &str) -> Self {
        Atb {
            text: text.to_string(),
            params: vec![text.to_string().into()],
        }
    }
}

impl InstructionMethods for Atb {
    fn supports_preparation(&self) -> bool {
        true
    }

    #[cfg(feature = "buffered")]
    fn transform_buffered(
        &self,
        input: crate::api::TextBuffer,
        _context: Option<&mut GlobalExecutionContext>,
    ) -> Result<crate::api::TextBuffer, TextForgeError> {
        input.prepend(&self.text)
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("atb {};\n", self.text).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        Ok(crate::utils::allocation::replace(input, 0..0, &self.text))
    }

    fn get_string_repr(&self) -> &'static str {
        "atb"
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;
        use crate::parser::params::TextForgeParamTypesJoin;

        check_vec_len(params, 1, "atb", params.join(""))?;

        self.text = parse_args!(params, 0, String, "Text should be of string type");

        self.params = vec![self.text.to_string().into()];

        Ok(())
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x01
    }

    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [TextForgeParamTypes::String(self.text.clone()),]
        );
        Ok(result)
    }
}

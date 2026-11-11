#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use html_escape::decode_html_entities;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;

/// HTMLU - HTML Unescape
///
/// Unescapes Special HTML Entities in `input` to their corresponding characters
/// Used when some HTML text is gonna be processed as a normal string
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::htmlu::Htmlu};
///
/// let token = Htmlu::default();
///
/// assert_eq!(token.transform("&lt;div&gt;banana&lt;/div&gt;".into(),None).unwrap().to_string(), "<div>banana</div>");
/// ```
#[derive(Clone, Default)]
pub struct Htmlu {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Htmlu {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "htmlu"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        "htmlu;\n".into()
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        match input {
            Cow::Borrowed(s) => Ok(decode_html_entities(s)),

            Cow::Owned(s) => match decode_html_entities(&s) {
                Cow::Borrowed(_) => Ok(Cow::Owned(s)),
                Cow::Owned(result) => Ok(Cow::Owned(result)),
            },
        }
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "dlf", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x25
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

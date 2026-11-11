#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

#[cfg(feature = "bytecode")]
use crate::to_bytecode;

use crate::{
    context::execution_context::GlobalExecutionContext, parser::resolve_var::TokenWrapper,
    tokens::InstructionMethods,
};

use crate::utils::errors::TextForgeError;

use crate::parser::params::TextForgeParamTypes;

/// Ifdc - If Do Contains
///
/// if `input` contains `text`, the `inner` token will be executed, otherwise `input` is returned with no changes
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, instructions::ifdc::Ifdc, transforms::atb::Atb};
///
/// let token = Ifdc::new(
///     "xy",
///     (Box::new(Atb::new("laranja")) as Box<dyn InstructionMethods>).into()
/// );
/// assert_eq!(token.transform("larryxy".into(), None).unwrap().to_string(), "laranjalarryxy"); // Adds laranja to the beginning
/// assert_eq!(token.transform("banana".into(), None).unwrap().to_string(), "banana"); // Does nothing
///
/// ```
#[derive(Clone, Default)]
pub struct Ifdc {
    text: String,
    inner: TokenWrapper,
    params: Vec<TextForgeParamTypes>,
}

impl Ifdc {
    pub fn new(text: &str, inner: TokenWrapper) -> Self {
        Ifdc {
            text: text.to_string(),
            params: vec![
                TextForgeParamTypes::String(text.to_string()),
                TextForgeParamTypes::Token(inner.clone()),
            ],
            inner,
        }
    }
}

impl InstructionMethods for Ifdc {
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("ifdc {} do {}", self.text, self.inner.to_textforge_line()).into()
    }

    fn get_string_repr(&self) -> &'static str {
        "ifdc"
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if input.contains(&self.text) {
            return match context {
                Some(context) => self.inner.apply_token(input, context),
                None => self
                    .inner
                    .apply_token(input, &mut GlobalExecutionContext::new()),
            };
        }

        Ok(input)
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x33
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::{parse_args, utils::validations::check_vec_len};

        use crate::parser::params::TextForgeParamTypesJoin;

        check_vec_len(params, 2, "ifdc", params.join(""))?;

        self.text = parse_args!(params, 0, String, "");

        self.inner = parse_args!(params, 1, Token, "");

        self.params = vec![
            TextForgeParamTypes::String(parse_args!(params, 0, String, "")),
            TextForgeParamTypes::Token(parse_args!(params, 1, Token, "")),
        ];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        let result = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::String(self.text.clone()),
                TextForgeParamTypes::Token(self.inner.clone()),
            ]
        );

        Ok(result)
    }
}

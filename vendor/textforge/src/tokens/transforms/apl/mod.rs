use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, lines::invalid, validations::check_vec_len},
};
use std::borrow::Cow;

/// APL — Append one logical line, preserving the final-terminator convention.
///
/// Uses one-based indices and shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::apl::Apl};
/// let token = Apl::new("b").unwrap();
/// assert_eq!(token.transform("a".into(), None).unwrap(), "a\nb");
/// ```
#[derive(Clone)]
pub struct Apl {
    text: String,
    params: Vec<TextForgeParamTypes>,
}
impl Apl {
    /// Construct `apl`. Line indices are one-based.
    pub fn new(text: &str) -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[text.to_owned().into()])?;
        Ok(token)
    }
}
impl Default for Apl {
    fn default() -> Self {
        Self {
            text: String::new(),
            params: vec![String::new().into()],
        }
    }
}
impl InstructionMethods for Apl {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "apl"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        let mut parts = vec![self.get_string_repr().to_owned()];
        for parameter in &self.params {
            parts.push(match parameter {
                TextForgeParamTypes::String(value) => shell_words::quote(value).into_owned(),
                TextForgeParamTypes::Usize(value) => value.to_string(),
                _ => String::new(),
            });
        }
        format!("{};\n", parts.join(" ")).into()
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 1, "apl", "Invalid line instruction arity")?;
        let text = crate::parse_args!(params, 0, String, "Expected String");
        if text.chars().any(crate::utils::lines::is_break) {
            return Err(invalid("apl", "Expected a single line", &text));
        }
        self.text = text;

        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        Ok(crate::utils::allocation::append_line(input, &self.text))
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x42
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;

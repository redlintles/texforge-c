use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};
use std::borrow::Cow;

/// DLE — Delete empty lines; lines containing whitespace are retained.
///
/// Uses one-based indices and shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::dle::Dle};
/// let token = Dle::new().unwrap();
/// assert_eq!(token.transform("a\n\nb".into(), None).unwrap(), "a\nb");
/// ```
#[derive(Clone, Default)]
pub struct Dle {
    params: Vec<TextForgeParamTypes>,
}
impl Dle {
    /// Construct `dle`. Line indices are one-based.
    pub fn new() -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[])?;
        Ok(token)
    }
}

impl InstructionMethods for Dle {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "dle"
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
        check_vec_len(params, 0, "dle", "Invalid line instruction arity")?;

        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        Ok(crate::utils::allocation::filter_lines(input, |text| {
            !text.is_empty()
        }))
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x4c
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;

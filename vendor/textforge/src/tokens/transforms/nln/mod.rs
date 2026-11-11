use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};
use std::borrow::Cow;

/// NLN — Remove all recognized line terminators without inserting spaces.
///
/// Uses one-based indices and shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::nln::Nln};
/// let token = Nln::new().unwrap();
/// assert_eq!(token.transform("a\nb".into(), None).unwrap(), "ab");
/// ```
#[derive(Clone, Default)]
pub struct Nln {
    params: Vec<TextForgeParamTypes>,
}
impl Nln {
    /// Construct `nln`. Line indices are one-based.
    pub fn new() -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[])?;
        Ok(token)
    }
}

impl InstructionMethods for Nln {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "nln"
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
        check_vec_len(params, 0, "nln", "Invalid line instruction arity")?;

        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if !input.chars().any(crate::utils::lines::is_break) {
            return Ok(input);
        }
        let mut output = input.into_owned();
        output.retain(|c| !crate::utils::lines::is_break(c));
        Ok(output.into())
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x49
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;

use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, lines::lines, validations::check_vec_len},
};
use std::borrow::Cow;

/// REVLN — Reverse line contents while preserving terminators at their original positions.
///
/// Uses one-based indices and shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::revln::Revln};
/// let token = Revln::new().unwrap();
/// assert_eq!(token.transform("a\nb".into(), None).unwrap(), "b\na");
/// ```
#[derive(Clone, Default)]
pub struct Revln {
    params: Vec<TextForgeParamTypes>,
}
impl Revln {
    /// Construct `revln`. Line indices are one-based.
    pub fn new() -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[])?;
        Ok(token)
    }
}

impl InstructionMethods for Revln {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "revln"
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
        check_vec_len(params, 0, "revln", "Invalid line instruction arity")?;

        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if lines(&input).nth(1).is_none() {
            return Ok(input);
        }
        let parts: Vec<_> = lines(&input).collect();
        let mut output = String::with_capacity(input.len());
        for ((text, _), (_, ending)) in parts.iter().rev().zip(parts.iter()) {
            output.push_str(text);
            output.push_str(ending);
        }
        if parts.first().is_some_and(|(t, _)| t.is_empty())
            && parts.last().is_some_and(|(_, e)| e.is_empty())
        {
            let ending = parts
                .iter()
                .find_map(|(_, e)| (!e.is_empty()).then_some(*e))
                .unwrap_or("\n");
            output.push_str(ending);
        }
        Ok(output.into())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x4f
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;

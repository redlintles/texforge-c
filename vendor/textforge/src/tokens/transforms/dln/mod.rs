use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::TextForgeError,
        lines::{check_line, lines},
        validations::check_vec_len,
    },
};
use std::borrow::Cow;

/// DLN — Delete one line, including its terminator.
///
/// Uses one-based indices and shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::dln::Dln};
/// let token = Dln::new(1).unwrap();
/// assert_eq!(token.transform("a\nb".into(), None).unwrap(), "b");
/// ```
#[derive(Clone)]
pub struct Dln {
    line: usize,
    params: Vec<TextForgeParamTypes>,
}
impl Dln {
    /// Construct `dln`. Line indices are one-based.
    pub fn new(line: usize) -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[line.into()])?;
        Ok(token)
    }
}
impl Default for Dln {
    fn default() -> Self {
        Self {
            line: 1,
            params: vec![1usize.into()],
        }
    }
}
impl InstructionMethods for Dln {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "dln"
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
        check_vec_len(params, 1, "dln", "Invalid line instruction arity")?;
        let line = crate::parse_args!(params, 0, Usize, "Expected Usize");
        if line == 0 {
            return Err(crate::utils::lines::invalid(
                "dln",
                "Line numbers start at one",
                "0",
            ));
        }
        self.line = line;

        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let count = lines(&input).count();
        check_line(self.line, count, "dln", &input)?;
        let start: usize = lines(&input)
            .take(self.line - 1)
            .map(|(t, e)| t.len() + e.len())
            .sum();
        let length: usize = lines(&input)
            .skip(self.line - 1)
            .take(1)
            .map(|(t, e)| t.len() + e.len())
            .sum();
        Ok(crate::utils::allocation::replace(
            input,
            start..start + length,
            "",
        ))
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x43
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;

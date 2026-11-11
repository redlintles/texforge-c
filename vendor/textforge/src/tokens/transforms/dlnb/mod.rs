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

/// DLNB — Delete lines before the given line, retaining that line.
///
/// Uses one-based indices and shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::dlnb::Dlnb};
/// let token = Dlnb::new(2).unwrap();
/// assert_eq!(token.transform("a\nb".into(), None).unwrap(), "b");
/// ```
#[derive(Clone)]
pub struct Dlnb {
    line: usize,
    params: Vec<TextForgeParamTypes>,
}
impl Dlnb {
    /// Construct `dlnb`. Line indices are one-based.
    pub fn new(line: usize) -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[line.into()])?;
        Ok(token)
    }
}
impl Default for Dlnb {
    fn default() -> Self {
        Self {
            line: 1,
            params: vec![1usize.into()],
        }
    }
}
impl InstructionMethods for Dlnb {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "dlnb"
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
        check_vec_len(params, 1, "dlnb", "Invalid line instruction arity")?;
        let line = crate::parse_args!(params, 0, Usize, "Expected Usize");
        if line == 0 {
            return Err(crate::utils::lines::invalid(
                "dlnb",
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
        check_line(self.line, lines(&input).count(), "dlnb", &input)?;
        let offset: usize = lines(&input)
            .take(self.line - 1)
            .map(|(t, e)| t.len() + e.len())
            .sum();
        let end = input.len();
        Ok(crate::utils::allocation::keep(input, offset..end))
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x45
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;

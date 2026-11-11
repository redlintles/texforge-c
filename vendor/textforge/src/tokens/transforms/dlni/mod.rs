use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::TextForgeError,
        lines::{check_range, lines},
        validations::check_vec_len,
    },
};
use std::borrow::Cow;

/// DLNI — Delete an inclusive interval of lines and retain the rest.
///
/// Uses one-based indices and shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::dlni::Dlni};
/// let token = Dlni::new(2, 2).unwrap();
/// assert_eq!(token.transform("a\nb\nc".into(), None).unwrap(), "a\nc");
/// ```
#[derive(Clone)]
pub struct Dlni {
    start_line: usize,
    end_line: usize,
    params: Vec<TextForgeParamTypes>,
}
impl Dlni {
    /// Construct `dlni`. Line indices are one-based.
    pub fn new(start_line: usize, end_line: usize) -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[start_line.into(), end_line.into()])?;
        Ok(token)
    }
}
impl Default for Dlni {
    fn default() -> Self {
        Self {
            start_line: 1,
            end_line: 1,
            params: vec![1usize.into(), 1usize.into()],
        }
    }
}
impl InstructionMethods for Dlni {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "dlni"
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
        check_vec_len(params, 2, "dlni", "Invalid line instruction arity")?;
        let start_line = crate::parse_args!(params, 0, Usize, "Expected Usize");
        if start_line == 0 {
            return Err(crate::utils::lines::invalid(
                "dlni",
                "Line numbers start at one",
                "0",
            ));
        }
        let end_line = crate::parse_args!(params, 1, Usize, "Expected Usize");
        if end_line == 0 {
            return Err(crate::utils::lines::invalid(
                "dlni",
                "Line numbers start at one",
                "0",
            ));
        }
        self.start_line = start_line;
        self.end_line = end_line;

        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_range(
            self.start_line,
            self.end_line,
            lines(&input).count(),
            "dlni",
            &input,
        )?;
        let start: usize = lines(&input)
            .take(self.start_line - 1)
            .map(|(t, e)| t.len() + e.len())
            .sum();
        let length: usize = lines(&input)
            .skip(self.start_line - 1)
            .take(self.end_line - self.start_line + 1)
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
        0x47
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;

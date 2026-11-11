use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, lines::invalid, validations::check_vec_len},
};
use std::borrow::Cow;

/// DLM — Delete lines whose content matches a regular expression.
///
/// Uses one-based indices and shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::dlm::Dlm};
/// let token = Dlm::new("^b$").unwrap();
/// assert_eq!(token.transform("a\nb".into(), None).unwrap(), "a\n");
/// ```
#[derive(Clone)]
pub struct Dlm {
    pattern: String,
    regex: Option<regex::Regex>,
    params: Vec<TextForgeParamTypes>,
}
impl Dlm {
    /// Construct `dlm`. Line indices are one-based.
    pub fn new(pattern: &str) -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[pattern.to_owned().into()])?;
        Ok(token)
    }
}
impl Default for Dlm {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            regex: regex::Regex::new("").ok(),
            params: vec![String::new().into()],
        }
    }
}
impl InstructionMethods for Dlm {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "dlm"
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
        check_vec_len(params, 1, "dlm", "Invalid line instruction arity")?;
        let pattern = crate::parse_args!(params, 0, String, "Expected String");
        // A valid regex may contain interpolation-like text, such as POSIX
        // character classes. Compile it before treating failed patterns as
        // unresolved pipeline parameters.
        let regex = match regex::Regex::new(&pattern) {
            Ok(regex) => Some(regex),
            Err(_) if pattern.contains("{{") || pattern.contains("[[") => None,
            Err(error) => {
                return Err(TextForgeError::new(
                    crate::utils::errors::TextForgeErrorCode::TextParsingError(
                        error.to_string().into(),
                    ),
                    "dlm",
                    pattern.clone(),
                ));
            }
        };
        self.pattern = pattern;
        self.regex = regex;

        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let pattern = self
            .regex
            .as_ref()
            .ok_or_else(|| invalid("dlm", "Unresolved regex", &self.pattern))?;
        Ok(crate::utils::allocation::filter_lines(input, |text| {
            !pattern.is_match(text)
        }))
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x4e
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;

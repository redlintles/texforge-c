use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, lines::lines, validations::check_vec_len},
};
use std::borrow::Cow;

/// JOINLN — Join line contents with a separator, discarding the final terminator.
///
/// Uses one-based indices and shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::joinln::Joinln};
/// let token = Joinln::new(" ").unwrap();
/// assert_eq!(token.transform("a\nb".into(), None).unwrap(), "a b");
/// ```
#[derive(Clone)]
pub struct Joinln {
    separator: String,
    params: Vec<TextForgeParamTypes>,
}
impl Joinln {
    /// Construct `joinln`. Line indices are one-based.
    pub fn new(separator: &str) -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[separator.to_owned().into()])?;
        Ok(token)
    }
}
impl Default for Joinln {
    fn default() -> Self {
        Self {
            separator: String::new(),
            params: vec![String::new().into()],
        }
    }
}
impl InstructionMethods for Joinln {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "joinln"
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
        check_vec_len(params, 1, "joinln", "Invalid line instruction arity")?;
        let separator = crate::parse_args!(params, 0, String, "Expected String");
        self.separator = separator;

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
        let mut output = String::with_capacity(input.len());
        for (i, (text, _)) in lines(&input).enumerate() {
            if i != 0 {
                output.push_str(&self.separator);
            }
            output.push_str(text);
        }
        Ok(output.into())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x51
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;

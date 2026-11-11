use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, lines::lines, validations::check_vec_len},
};
use std::borrow::Cow;

/// UNIQLN — Keep the first occurrence of each exact line content, in original order.
///
/// Uses one-based indices and shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::uniqln::Uniqln};
/// let token = Uniqln::new().unwrap();
/// assert_eq!(token.transform("a\na\nb".into(), None).unwrap(), "a\nb");
/// ```
#[derive(Clone, Default)]
pub struct Uniqln {
    params: Vec<TextForgeParamTypes>,
}
impl Uniqln {
    /// Construct `uniqln`. Line indices are one-based.
    pub fn new() -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[])?;
        Ok(token)
    }
}

impl InstructionMethods for Uniqln {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "uniqln"
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
        check_vec_len(params, 0, "uniqln", "Invalid line instruction arity")?;

        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut seen = std::collections::HashSet::new();
        // Keep borrowed keys while scanning; the output borrows only from input.
        let mut output: Option<String> = None;
        let mut offset = 0;
        for (text, ending) in lines(&input) {
            if seen.insert(text) {
                if let Some(output) = output.as_mut() {
                    output.push_str(text);
                    output.push_str(ending);
                }
            } else if output.is_none() {
                let mut result = String::with_capacity(input.len());
                result.push_str(&input[..offset]);
                output = Some(result);
            }
            offset += text.len() + ending.len();
        }
        drop(seen);
        Ok(output.map_or(input, Cow::Owned))
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x50
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;

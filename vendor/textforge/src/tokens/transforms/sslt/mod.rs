#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use regex::Regex;

use crate::context::execution_context::GlobalExecutionContext;
use crate::parser::params::TextForgeParamTypes;
use crate::tokens::InstructionMethods;
use crate::utils::validations::check_vec_len;

use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

/// SSLT - Split Select
///
/// Splits `input` by `pattern and return `index` of the resulting vec,
/// *discarding* the rest of the text in the process.
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::sslt::Sslt};
///
/// let token = Sslt::new("_", 1).unwrap();
///
/// assert_eq!(token.transform("foobar_foo_bar_bar_foo_barfoo".into(),None).unwrap().to_string(), "foo");
///
/// ```
#[derive(Clone)]
pub struct Sslt {
    pub pattern: Regex,
    pub index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Sslt {
    pub fn new(pattern: &str, index: usize) -> Result<Self, TextForgeError> {
        let pattern = Regex::new(pattern).map_err(|e| {
            TextForgeError::new(
                TextForgeErrorCode::TextParsingError(e.to_string().into()),
                "",
                "",
            )
        })?;
        Ok(Sslt {
            index,
            params: vec![pattern.to_string().into(), index.into()],
            pattern,
        })
    }
}

impl Default for Sslt {
    fn default() -> Self {
        Sslt {
            pattern: Regex::new("").unwrap(),
            index: 0,
            params: vec!["".to_string().into(), (0).into()],
        }
    }
}

impl InstructionMethods for Sslt {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "sslt"
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let result = self
            .pattern
            .split(input.as_ref())
            .nth(self.index)
            .ok_or_else(|| {
                TextForgeError::new(
                    TextForgeErrorCode::IndexOutOfRange(
                        "Index does not exist in the splitted vec".into(),
                    ),
                    self.to_textforge_line(),
                    input.to_string(),
                )
            })?;
        let start = result.as_ptr() as usize - input.as_ptr() as usize;
        let end = start + result.len();
        Ok(crate::utils::allocation::keep(input, start..end))
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("sslt {} {};\n", self.pattern, self.index).into()
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "sslt", "")?;

        let pattern_payload = parse_args!(params, 0, String, "Pattern should be of string type");

        self.pattern = Regex::new(&pattern_payload).map_err(|_| {
            TextForgeError::new(
                TextForgeErrorCode::TextParsingError("Failed to create regex".into()),
                "sslt",
                pattern_payload.clone(),
            )
        })?;

        self.index = parse_args!(params, 1, Usize, "Index should be of type Usize");

        self.params = vec![self.pattern.to_string().into(), self.index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x1a
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::String(self.pattern.to_string()),
                TextForgeParamTypes::Usize(self.index),
            ]
        );
        Ok(result)
    }
}

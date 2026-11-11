use std::borrow::Cow;

use regex::Regex;

use crate::{
    parse_args,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_vec_len,
    },
};

#[cfg(feature = "test_access")]
pub mod test;

/// EMJ - Extract Matches Joined
///
/// Finds every non-overlapping match of `pattern` in `input`, discards everything
/// that did not match, and joins the surviving matches (in the order they appear)
/// with `separator`.
///
/// If `pattern` matches nothing, the result is an empty string — the whole input
/// is discarded. If `pattern` can match an empty string (e.g. `"a*"`), every match
/// position still counts, so the output may contain a lot of `separator`s.
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::emj::Emj};
///
/// let token = Emj::new("laranja", ",").unwrap();
///
/// assert_eq!(
///     token.transform("banana laranja banana laranja".into(),None).unwrap().to_string(),
///     "laranja,laranja");
/// ```
#[derive(Clone, Debug)]
pub struct Emj {
    pub pattern: Regex,
    pub separator: String,
    params: Vec<TextForgeParamTypes>,
}

impl Emj {
    /// Builds a new `Emj` from a regex pattern and a separator.
    ///
    /// Returns `Err` (the underlying regex crate's error message, as a `String`)
    /// if `pattern` is not a valid regular expression.
    pub fn new(pattern: &str, separator: &str) -> Result<Self, String> {
        let pattern = Regex::new(pattern).map_err(|x| x.to_string())?;
        Ok(Emj {
            params: vec![pattern.to_string().into(), separator.to_string().into()],
            pattern,
            separator: separator.to_string(),
        })
    }
}

impl Default for Emj {
    fn default() -> Self {
        Emj {
            pattern: Regex::new("").unwrap(),
            separator: "".to_string(),
            params: vec!["".to_string().into(), "".to_string().into()],
        }
    }
}

impl InstructionMethods for Emj {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 2, "emj", "")?;
        let pattern_payload = parse_args!(params, 0, String, "Pattern should be of string type");

        self.pattern = Regex::new(&pattern_payload).map_err(|_| {
            TextForgeError::new(
                TextForgeErrorCode::TextParsingError("Failed to create regex".into()),
                "emj",
                pattern_payload.clone(),
            )
        })?;

        self.separator = parse_args!(params, 1, String, "separator should be of type String");
        self.params = vec![
            self.pattern.to_string().into(),
            self.separator.clone().into(),
        ];
        Ok(())
    }

    fn get_string_repr(&self) -> &'static str {
        "emj"
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }

    fn to_textforge_line(&self) -> std::borrow::Cow<'static, str> {
        format!("emj {} {};\n", self.pattern, self.separator).into()
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x37
    }

    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::String(self.pattern.to_string()),
                TextForgeParamTypes::String(self.separator.clone()),
            ]
        );
        Ok(result)
    }

    /// Collects every non-overlapping match of `self.pattern` in `input`, in order,
    /// and joins them with `self.separator`. Everything that did not match is dropped.
    /// Does not use the execution context.
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut crate::context::execution_context::GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut output = String::new();
        for (i, m) in self.pattern.find_iter(&input).enumerate() {
            if i != 0 {
                output.push_str(&self.separator);
            }
            output.push_str(m.as_str());
        }
        Ok(output.into())
    }
}

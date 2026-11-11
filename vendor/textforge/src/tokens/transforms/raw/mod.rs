#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use regex::Regex;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_vec_len,
    },
};

use crate::parser::params::TextForgeParamTypes;
/// RAW - Replace All With
///
/// Replace all ocurrences of `pattern` in `input` with `text_to_replace`
///
/// See Also:
///
/// - [`RCW` - Replace Count With](crate::tokens::transforms::rcw)
/// - [`RFW` - Replace First With](crate::tokens::transforms::rfw)
/// - [`RLW` - Replace Last With](crate::tokens::transforms::rlw)
/// - [`RNW` - Replace Nth With](crate::tokens::transforms::rnw)
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::raw::Raw};
///
/// let token = Raw::new(&"a", "b").unwrap();
///
/// assert_eq!(token.transform("aaaaa".into(),None).unwrap().to_string(), "bbbbb");
/// ```
///
#[derive(Clone, Debug)]
pub struct Raw {
    pub pattern: Regex,
    pub text_to_replace: String,
    params: Vec<TextForgeParamTypes>,
}

impl Raw {
    pub fn new(pattern: &str, text_to_replace: &str) -> Result<Self, String> {
        let pattern = Regex::new(pattern).map_err(|x| x.to_string())?;
        Ok(Raw {
            text_to_replace: text_to_replace.to_string(),
            params: vec![
                pattern.to_string().into(),
                text_to_replace.to_string().into(),
            ],
            pattern,
        })
    }
}

impl Default for Raw {
    fn default() -> Self {
        Raw {
            pattern: Regex::new("").unwrap(),
            text_to_replace: "".to_string(),
            params: vec!["".to_string().into(), "".to_string().into()],
        }
    }
}

impl InstructionMethods for Raw {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("raw {} {};\n", self.pattern, self.text_to_replace).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        match input {
            Cow::Borrowed(v) => Ok(self.pattern.replace_all(v, &self.text_to_replace)),

            Cow::Owned(v) => match self.pattern.replace_all(&v, &self.text_to_replace) {
                Cow::Borrowed(_) => Ok(Cow::Owned(v)),

                Cow::Owned(result) => Ok(Cow::Owned(result)),
            },
        }
    }

    fn get_string_repr(&self) -> &'static str {
        "raw"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "raw", "")?;

        let pattern_payload = parse_args!(params, 0, String, "Pattern should be of string type");

        self.pattern = Regex::new(&pattern_payload).map_err(|_| {
            TextForgeError::new(
                TextForgeErrorCode::TextParsingError("Failed to create regex".into()),
                "sslt",
                pattern_payload.clone(),
            )
        })?;

        self.text_to_replace = parse_args!(
            params,
            1,
            String,
            "Text_to_replace should be of type String"
        );

        self.params = vec![
            self.pattern.to_string().into(),
            self.text_to_replace.to_string().into(),
        ];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x0c
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::String(self.pattern.to_string()),
                TextForgeParamTypes::String(self.text_to_replace.clone()),
            ]
        );
        Ok(result)
    }
}

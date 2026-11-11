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
/// RLW - Replace Last With
///
/// Replace the last ocurrency of `pattern` in `input` with `text_to_replace`
///
/// See Also:
///
/// - [`RAW` - Replace All With](crate::tokens::transforms::raw)
/// - [`RCW` - Replace First With](crate::tokens::transforms::rcw)
/// - [`RFW` - Replace Last With](crate::tokens::transforms::rfw)
/// - [`RNW` - Replace Nth With](crate::tokens::transforms::rnw)
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::rlw::Rlw};
///
/// let token = Rlw::new(&"a", "b").unwrap();
///
/// assert_eq!(token.transform("aaaaa".into(),None).unwrap().to_string(), "aaaab");
/// ```
///
#[derive(Clone, Debug)]
pub struct Rlw {
    pub pattern: Regex,
    pub text_to_replace: String,
    params: Vec<TextForgeParamTypes>,
}

impl Rlw {
    pub fn new(pattern: &str, text_to_replace: &str) -> Result<Self, String> {
        let pattern = Regex::new(pattern).map_err(|x| x.to_string())?;
        Ok(Rlw {
            text_to_replace: text_to_replace.to_string(),
            params: vec![
                pattern.to_string().into(),
                text_to_replace.to_string().into(),
            ],
            pattern,
        })
    }
}

impl Default for Rlw {
    fn default() -> Self {
        Rlw {
            pattern: Regex::new("").unwrap(),
            text_to_replace: "".to_string(),
            params: vec!["".to_string().into(), "".to_string().into()],
        }
    }
}

impl InstructionMethods for Rlw {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("rlw {} {};\n", self.pattern, self.text_to_replace).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if let Some(m) = self.pattern.find_iter(input.as_ref()).last() {
            let (start, end) = (m.start(), m.end());

            return Ok(crate::utils::allocation::replace(
                input,
                start..end,
                &self.text_to_replace,
            ));
        }
        Ok(input)
    }

    fn get_string_repr(&self) -> &'static str {
        "rlw"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "rlw", "")?;

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
        0x1e
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

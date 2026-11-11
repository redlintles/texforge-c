#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext, parse_args,
    utils::validations::check_vec_len,
};

use regex::Regex;

use crate::{
    tokens::InstructionMethods,
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

use crate::parser::params::TextForgeParamTypes;
/// RLW - Replace Last With
///
/// Replace the `nth`` ocurrency of `pattern` in `input` with `text_to_replace`
///
/// See Also:
///
/// - [`RAW` - Replace All With](crate::tokens::transforms::raw)
/// - [`RCW` - Replace Count With](crate::tokens::transforms::rcw)
/// - [`RFW` - Replace First With](crate::tokens::transforms::rfw)
/// - [`RLW` - Replace Last With](crate::tokens::transforms::rlw)
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::rnw::Rnw};
///
/// let token = Rnw::new(&"a", "b", 2).unwrap();
///
/// assert_eq!(token.transform("aaaaa".into(),None).unwrap().to_string(), "aabaa");
/// ```
///
#[derive(Clone, Debug)]
pub struct Rnw {
    pub pattern: Regex,
    pub text_to_replace: String,
    pub index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Rnw {
    pub fn new(pattern: &str, text_to_replace: &str, index: usize) -> Result<Self, String> {
        let pattern = Regex::new(pattern).map_err(|x| x.to_string())?;
        Ok(Rnw {
            text_to_replace: text_to_replace.to_string(),
            params: vec![
                text_to_replace.to_string().into(),
                pattern.to_string().into(),
                index.into(),
            ],
            pattern,
            index,
        })
    }
}

impl Default for Rnw {
    fn default() -> Self {
        Rnw {
            pattern: Regex::new("").unwrap(),
            text_to_replace: "".to_string(),
            index: 0,
            params: vec!["".to_string().into(), "".to_string().into(), (0).into()],
        }
    }
}

impl InstructionMethods for Rnw {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!(
            "rnw {} {} {};\n",
            self.pattern, self.text_to_replace, self.index
        )
        .into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut idx = None;

        for (count, m) in self.pattern.find_iter(input.as_ref()).enumerate() {
            if count == self.index {
                idx = Some((m.start(), m.end()));
                break;
            }
        }

        if let Some((start, end)) = idx {
            return Ok(crate::utils::allocation::replace(
                input,
                start..end,
                &self.text_to_replace,
            ));
        }
        Ok(input)
    }
    fn get_string_repr(&self) -> &'static str {
        "rnw"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 3, "rnw", "")?;

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

        self.index = parse_args!(params, 2, Usize, "Index should be of type Usize");

        self.params = vec![
            self.pattern.to_string().into(),
            self.text_to_replace.to_string().into(),
            self.index.into(),
        ];
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x1f
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::String(self.pattern.to_string()),
                TextForgeParamTypes::String(self.text_to_replace.clone()),
                TextForgeParamTypes::Usize(self.index),
            ]
        );
        Ok(result)
    }
}

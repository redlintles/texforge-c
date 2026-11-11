use std::borrow::Cow;

use regex::Regex;

use crate::{
    context::execution_context::GlobalExecutionContext,
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

/// Rmp - Does nothing
#[derive(Clone, Debug)]
pub struct Rmp {
    pattern: Regex,
    params: Vec<TextForgeParamTypes>,
}

impl Rmp {
    pub fn new(pattern: &str) -> Result<Self, String> {
        let pattern = Regex::new(pattern).map_err(|x| x.to_string())?;
        Ok(Rmp {
            params: vec![pattern.to_string().into()],
            pattern,
        })
    }
}

impl Default for Rmp {
    fn default() -> Self {
        Rmp {
            pattern: Regex::new("").unwrap(),
            params: vec!["".to_string().into(), "".to_string().into()],
        }
    }
}

impl InstructionMethods for Rmp {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x3d
    }
    fn get_string_repr(&self) -> &'static str {
        "rmp"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        Cow::from(format!("rmp {};\n", self.pattern))
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        match input {
            Cow::Borrowed(v) => Ok(self.pattern.replace_all(v, "")),

            Cow::Owned(v) => match self.pattern.replace_all(&v, "") {
                Cow::Borrowed(_) => Ok(Cow::Owned(v)),

                Cow::Owned(result) => Ok(Cow::Owned(result)),
            },
        }
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 1, "rmp", "param parsing error, invalid vec len")?;
        let pattern_payload = parse_args!(params, 0, String, "Pattern should be of string type");

        self.pattern = Regex::new(&pattern_payload).map_err(|_| {
            TextForgeError::new(
                TextForgeErrorCode::TextParsingError("Failed to create regex".into()),
                "sslt",
                pattern_payload.clone(),
            )
        })?;

        self.params = vec![self.pattern.to_string().into()];
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

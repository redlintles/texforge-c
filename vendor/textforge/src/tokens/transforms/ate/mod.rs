#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;
/// Token `Ate` — Add to End
///
/// Appends `text` to the end of `input`
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::ate::Ate};
///
/// let token = Ate::new(" bar");
/// assert_eq!(token.transform("foo".into(),None).unwrap().to_string(), "foo bar");
/// ```

#[derive(Clone, Default)]
pub struct Ate {
    pub text: String,
    params: Vec<TextForgeParamTypes>,
}

impl Ate {
    pub fn new(text: &str) -> Self {
        Ate {
            text: text.to_string(),
            params: vec![text.to_string().into()],
        }
    }
}

impl InstructionMethods for Ate {
    fn supports_preparation(&self) -> bool {
        true
    }

    #[cfg(feature = "buffered")]
    fn transform_buffered(
        &self,
        input: crate::api::TextBuffer,
        _context: Option<&mut GlobalExecutionContext>,
    ) -> Result<crate::api::TextBuffer, TextForgeError> {
        {
            let mut input = input;
            input.append_text(&self.text)?;
            Ok(input)
        }
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("ate {};\n", self.text).into()
    }

    fn transform<'a>(
        &self,
        mut input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if !self.text.is_empty() {
            input.to_mut().push_str(&self.text);
        }
        Ok(input)
    }

    fn get_string_repr(&self) -> &'static str {
        "ate"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;
        use crate::parser::params::TextForgeParamTypesJoin;

        check_vec_len(params, 1, "ate", params.join(""))?;

        self.text = parse_args!(params, 0, String, "Text should be of string type");

        self.params = vec![self.text.to_string().into()];

        Ok(())
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x02
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [TextForgeParamTypes::String(self.text.clone()),]
        );
        Ok(result)
    }
}

#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;
/// PADR - Pad Right
///
/// Repeats `text` characters until `max_len` is reached, and then insert the result at the end of `input`
///
/// See Also:
///
/// - [`Padr` - Pad Left](crate::tokens::transforms::padr)
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::padr::Padr};
///
/// let token = Padr::new("xy", 7);
///
/// assert_eq!(token.transform("banana".into(),None).unwrap().to_string(), "bananax");
/// ```
#[derive(Clone, Default)]
pub struct Padr {
    pub text: String,
    pub max_len: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Padr {
    pub fn new(text: &str, max_len: usize) -> Self {
        Padr {
            text: text.to_string(),
            max_len,
            params: vec![text.to_string().into(), max_len.into()],
        }
    }
}

impl InstructionMethods for Padr {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "padr"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("padr {} {};\n", self.text, self.max_len).into()
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let character_count = input.chars().count();

        if character_count >= self.max_len {
            return Ok(input);
        }
        let ml = self.max_len - character_count;
        if self.text.is_empty() {
            return Ok(input);
        }
        let mut output = input.into_owned();
        output.extend(self.text.chars().cycle().take(ml));
        Ok(output.into())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "padr", "")?;

        self.text = parse_args!(params, 0, String, "Text_to_insert should be of String type");
        self.max_len = parse_args!(params, 1, Usize, "Index should be of usize type");
        self.params = vec![self.text.to_string().into(), self.max_len.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x30
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::String(self.text.clone()),
                TextForgeParamTypes::Usize(self.max_len),
            ]
        );
        Ok(result)
    }
}

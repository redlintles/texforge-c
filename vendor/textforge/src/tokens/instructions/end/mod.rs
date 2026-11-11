use std::borrow::Cow;

use crate::{
    context::execution_context::{GlobalContextMethods, GlobalExecutionContext},
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

#[cfg(feature = "test_access")]
pub mod test;

/// End - Does nothing
#[derive(Clone, Default)]
pub struct End {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for End {
    fn supports_preparation(&self) -> bool {
        true
    }

    #[cfg(feature = "buffered")]
    fn transform_buffered(
        &self,
        input: crate::api::TextBuffer,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<crate::api::TextBuffer, TextForgeError> {
        {
            if let Some(context) = context {
                context.halt();
                Ok(input)
            } else {
                let options = input.options().clone();
                let result = self.transform(input.into_string()?.into(), None)?;
                crate::api::TextBuffer::from_text(result.as_ref(), options)
            }
        }
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x3e
    }
    fn get_string_repr(&self) -> &'static str {
        "end"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        Cow::from("end;\n".to_string())
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        context
            .ok_or_else(|| {
                TextForgeError::new(
                    crate::utils::errors::TextForgeErrorCode::RequiredContextError(
                        "Context required for end instruction".into(),
                    ),
                    "end",
                    input.to_string(),
                )
            })?
            .halt();

        Ok(input)
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "End", "param parsing error, invalid vec len")?;

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

use std::borrow::Cow;

use crate::{
    context::execution_context::{GlobalContextMethods, GlobalExecutionContext},
    parse_args,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode::RequiredContextError},
        validations::check_vec_len,
    },
};

#[cfg(feature = "test_access")]
pub mod test;
#[derive(Clone)]
pub struct Cblk {
    block_name: String,
    params: Vec<TextForgeParamTypes>,
}

impl Default for Cblk {
    fn default() -> Self {
        Cblk {
            block_name: "x".to_string(),
            params: vec!["x".to_string().into()],
        }
    }
}

impl InstructionMethods for Cblk {
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x35
    }
    fn get_string_repr(&self) -> &'static str {
        "cblk"
    }

    fn to_textforge_line(&self) -> std::borrow::Cow<'static, str> {
        format!("cblk {};\n", self.block_name).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let context = context.ok_or_else(|| {
            TextForgeError::new(
                RequiredContextError("Context required for proper working!".into()),
                std::borrow::Cow::Borrowed("val"),
                std::borrow::Cow::Borrowed(""),
            )
        })?;
        let mut result = input;
        let tokens = context.take_block(&self.block_name)?;

        for token in tokens.iter() {
            result = token.apply_token(result, context)?;
        }

        context.put_block(&self.block_name, tokens);
        Ok(result)
    }

    fn set_params(
        &mut self,
        params: &[crate::parser::params::TextForgeParamTypes],
    ) -> Result<(), crate::utils::errors::TextForgeError> {
        check_vec_len(
            params,
            1,
            "call block",
            "param parsing error, invalid vec len",
        )?;

        self.block_name = parse_args!(params, 0, String, "Block name should be of string type");
        self.params = vec![self.block_name.to_string().into()];
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result = to_bytecode!(
            self.get_opcode(),
            [TextForgeParamTypes::String(self.block_name.to_string()),]
        );

        Ok(result)
    }
}

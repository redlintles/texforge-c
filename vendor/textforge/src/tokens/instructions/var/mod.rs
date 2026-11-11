use std::borrow::Cow;

use crate::{
    context::execution_context::{
        GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues,
    },
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
pub struct Var {
    var_name: String,
    var_value: TextForgeParamTypes,
    params: Vec<TextForgeParamTypes>,
}

impl Default for Var {
    fn default() -> Self {
        Var {
            var_name: "x".to_string(),
            var_value: TextForgeParamTypes::String("".to_string()),
            params: vec![
                TextForgeParamTypes::String("x".to_string()),
                TextForgeParamTypes::String("".to_string()),
            ],
        }
    }
}

impl InstructionMethods for Var {
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x39
    }
    fn get_string_repr(&self) -> &'static str {
        "var"
    }

    fn to_textforge_line(&self) -> std::borrow::Cow<'static, str> {
        format!("var {} = {};\n", self.var_name, self.var_value).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let context = context.ok_or_else(|| {
            TextForgeError::new(
                RequiredContextError("Context required for proper working!".into()),
                std::borrow::Cow::Borrowed("var"),
                std::borrow::Cow::Borrowed(""),
            )
        })?;
        let value = match &self.var_value {
            TextForgeParamTypes::VarRef(name) => context.get_var(name)?.value.clone(),
            other => VarValues::try_from(other.clone())?,
        };

        context.add_var(
            &self.var_name,
            VarEntry {
                value,
                mutable: true,
            },
        )?;

        Ok(input)
    }

    fn set_params(
        &mut self,
        params: &[crate::parser::params::TextForgeParamTypes],
    ) -> Result<(), crate::utils::errors::TextForgeError> {
        check_vec_len(params, 2, "var", "param parsing error, invalid vec len")?;

        self.var_name = parse_args!(params, 0, String, "Val name should be of string type");
        self.var_value = params[1].clone();

        self.params = vec![self.var_name.to_string().into(), self.var_value.clone()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;

        let result = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::String(self.var_name.clone()),
                self.var_value.clone(),
            ]
        );
        Ok(result)
    }
}

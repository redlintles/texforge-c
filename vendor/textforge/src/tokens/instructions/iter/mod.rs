use std::borrow::Cow;

use crate::{
    context::execution_context::{
        GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues,
    },
    parse_args,
    parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

#[cfg(feature = "test_access")]
pub mod test;

/// Iter - Does nothing
#[derive(Clone, Default)]
pub struct Iter {
    times: usize,
    inner: TokenWrapper,
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Iter {
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x3c
    }
    fn get_string_repr(&self) -> &'static str {
        "iter"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        Cow::from(format!(
            "iter {} times {}",
            self.times,
            self.inner.to_textforge_line()
        ))
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let context = context.unwrap();

        let mut result = input;

        context.add_var(
            "__COUNTER__",
            VarEntry {
                mutable: true,
                value: VarValues::Usize(0),
            },
        )?;
        for i in 0..self.times {
            result = self.inner.apply_token(result, context)?;
            let var_mut = context.get_mut_var("__COUNTER__")?;
            var_mut.value = VarValues::Usize(i);
        }

        context.rm_var("__COUNTER__")?;

        Ok(result)
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 2, "iter", "param parsing error, invalid vec len")?;
        self.times = parse_args!(params, 0, Usize, "First argument should be of type usize");
        self.inner = parse_args!(
            params,
            1,
            Token,
            "Second argument should be of type instruction"
        );
        self.params = vec![self.times.into(), self.inner.clone().into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

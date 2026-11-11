use std::borrow::Cow;

use evalexpr::eval_with_context;

use crate::{
    context::execution_context::{
        GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues,
    },
    parse_args,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::{
            TextForgeError,
            TextForgeErrorCode::{InvalidExprError, RequiredContextError},
        },
        expr::{build_eval_context, value_to_plain_string},
        validations::check_vec_len,
    },
};

#[cfg(feature = "test_access")]
pub mod test;

/// Eval - Parses logical and
#[derive(Clone, Default)]
pub struct Eval {
    expr: String,
    target: String,
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Eval {
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x3b
    }
    fn get_string_repr(&self) -> &'static str {
        "eval"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        Cow::from(format!("eval {} in {};\n", self.expr, self.target))
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

        // Immutable borrow — dropped as soon as eval_ctx is built, since
        // HashMapContext owns its own converted values rather than
        // referencing context's HashMap.
        let eval_ctx = build_eval_context(context.get_all_vars())?;

        let result = eval_with_context(&self.expr, &eval_ctx).map_err(|e| {
            TextForgeError::new(
                InvalidExprError(Cow::from(e.to_string())),
                Cow::from("eval.transform"),
                Cow::from(self.expr.to_string()),
            )
        })?;

        match context.get_mut_var(&self.target) {
            Ok(var_mut) => {
                if var_mut.mutable {
                    var_mut.value = VarValues::String(value_to_plain_string(&result));
                }
            }
            Err(e) => {
                if e.error_code.get_error_code() == 9u16 {
                    context.add_var(
                        &self.target,
                        VarEntry {
                            value: VarValues::String(value_to_plain_string(&result)),
                            mutable: true,
                        },
                    )?;
                } else {
                    return Err(e);
                }
            }
        }

        Ok(input)
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 2, "eval", "param parsing error, invalid vec len")?;

        self.expr = parse_args!(params, 0, String, "Expr should be of type string");
        self.target = parse_args!(params, 1, String, " should be of type string");

        self.params = vec![self.expr.to_string().into(), self.target.to_string().into()];
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}

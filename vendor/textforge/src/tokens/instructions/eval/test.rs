// src/tokens/instructions/eval/test.rs

#[cfg(feature = "test_access")]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use evalexpr::{Context, Value};

    use crate::context::execution_context::{
        GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues,
    };
    use crate::parser::params::TextForgeParamTypes;
    use crate::parser::resolve_var::TokenWrapper;
    use crate::tokens::InstructionMethods;
    use crate::tokens::instructions::eval::{Eval, build_eval_context, value_to_plain_string};
    use crate::utils::errors::TextForgeErrorCode; // ajuste pro path real (parser::params, no seu caso)

    // ============================
    // Contrato básico da instrução
    // ============================

    #[test]
    fn get_string_repr_is_eval() {
        let t = Eval::default();
        assert_eq!(t.get_string_repr(), "eval");
    }

    #[test]
    fn set_params_sets_expr_and_target() {
        let mut t = Eval::default();
        let params = vec![
            TextForgeParamTypes::String("2 + 2".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ];

        assert!(t.set_params(&params).is_ok());

        let stored = t.get_params();
        assert!(matches!(stored.first(), Some(TextForgeParamTypes::String(s)) if s == "2 + 2"));
        assert!(matches!(stored.get(1), Some(TextForgeParamTypes::String(s)) if s == "result"));
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Eval::default();
        let params = vec![TextForgeParamTypes::String("2 + 2".to_string())];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_non_string_expr() {
        let mut t = Eval::default();
        let params = vec![
            TextForgeParamTypes::Usize(1),
            TextForgeParamTypes::String("result".to_string()),
        ];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidParameters(_)
        ));
    }

    #[test]
    fn to_textforge_line_matches_eval_syntax() {
        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("2 + 2".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        assert_eq!(t.to_textforge_line().as_ref(), "eval 2 + 2 in result;\n");
    }

    // ============================
    // build_eval_context(): conversão VarValues -> evalexpr::Value
    // ============================

    #[test]
    fn build_eval_context_converts_usize_var_to_int() {
        let mut vars = HashMap::new();
        vars.insert(
            "n".to_string(),
            VarEntry {
                value: VarValues::Usize(7),
                mutable: true,
            },
        );

        let ctx = build_eval_context(&vars).unwrap();

        assert_eq!(ctx.get_value("n"), Some(&Value::from_int(7)));
    }

    #[test]
    fn build_eval_context_parses_numeric_string_as_int() {
        let mut vars = HashMap::new();
        vars.insert(
            "n".to_string(),
            VarEntry {
                value: VarValues::String("42".to_string()),
                mutable: false,
            },
        );

        let ctx = build_eval_context(&vars).unwrap();

        assert_eq!(ctx.get_value("n"), Some(&Value::from_int(42)));
    }

    #[test]
    fn build_eval_context_keeps_non_numeric_string_as_string() {
        let mut vars = HashMap::new();
        vars.insert(
            "name".to_string(),
            VarEntry {
                value: VarValues::String("ola".to_string()),
                mutable: false,
            },
        );

        let ctx = build_eval_context(&vars).unwrap();

        assert_eq!(ctx.get_value("name"), Some(&Value::from("ola".to_string())));
    }

    #[test]
    fn build_eval_context_keeps_float_like_string_as_string() {
        // Decisão de projeto: float não é tentado na conversão, então uma
        // string como "5.5" continua sendo Value::String, não Value::Float.
        let mut vars = HashMap::new();
        vars.insert(
            "n".to_string(),
            VarEntry {
                value: VarValues::String("5.5".to_string()),
                mutable: false,
            },
        );

        let ctx = build_eval_context(&vars).unwrap();

        assert_eq!(ctx.get_value("n"), Some(&Value::from("5.5".to_string())));
    }

    #[test]
    fn build_eval_context_skips_token_vars() {
        let mut vars = HashMap::new();
        vars.insert(
            "tok".to_string(),
            VarEntry {
                value: VarValues::Token(TokenWrapper::default()),
                mutable: false,
            },
        );

        let ctx = build_eval_context(&vars).unwrap();

        assert_eq!(ctx.get_value("tok"), None);
    }

    // ============================
    // value_to_plain_string(): sem os artefatos do Display do evalexpr
    // ============================

    #[test]
    fn value_to_plain_string_strips_quotes_from_string_value() {
        assert_eq!(
            value_to_plain_string(&Value::from("hello".to_string())),
            "hello"
        );
    }

    #[test]
    fn value_to_plain_string_formats_int_and_float() {
        assert_eq!(value_to_plain_string(&Value::from_int(6)), "6");
        assert_eq!(value_to_plain_string(&Value::from_float(1.5)), "1.5");
    }

    #[test]
    fn value_to_plain_string_formats_boolean() {
        assert_eq!(value_to_plain_string(&Value::from(true)), "true");
    }

    #[test]
    fn value_to_plain_string_formats_tuple_without_inner_quotes() {
        let tuple = Value::Tuple(vec![Value::from_int(1), Value::from("a".to_string())]);
        assert_eq!(value_to_plain_string(&tuple), "(1, a)");
    }

    #[test]
    fn value_to_plain_string_formats_empty() {
        assert_eq!(value_to_plain_string(&Value::Empty), "");
    }

    // ============================
    // transform(): contrato geral
    // ============================

    #[test]
    fn transform_fails_without_context() {
        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("1 + 1".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        let err = t.transform("input".into(), None).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::RequiredContextError(_)
        ));
    }

    #[test]
    fn transform_fails_when_target_var_is_not_mutable() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "result",
            VarEntry {
                value: VarValues::String("".to_string()),
                mutable: false,
            },
        )
        .unwrap();

        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("1 + 1".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        let err = t.transform("input".into(), Some(&mut ctx)).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::NonMutableVariableError(_)
        ));
    }

    #[test]
    fn transform_preserves_input_unchanged() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "result",
            VarEntry {
                value: VarValues::String("".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("1 + 1".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        let result = t.transform("input inalterado".into(), Some(&mut ctx));

        assert_eq!(result.unwrap().to_string(), "input inalterado".to_string());
    }

    #[test]
    fn transform_computes_simple_arithmetic() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "result",
            VarEntry {
                value: VarValues::String("".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("2 + 3".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        t.transform("input".into(), Some(&mut ctx)).unwrap();

        let var = ctx.get_var("result").unwrap();
        assert!(matches!(&var.value, VarValues::String(s) if s == "5"));
    }

    #[test]
    fn transform_uses_context_variable_in_expression() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "x",
            VarEntry {
                value: VarValues::String("5".to_string()),
                mutable: false,
            },
        )
        .unwrap();
        ctx.add_var(
            "result",
            VarEntry {
                value: VarValues::String("".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("x + 10".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        t.transform("input".into(), Some(&mut ctx)).unwrap();

        let var = ctx.get_var("result").unwrap();
        assert!(matches!(&var.value, VarValues::String(s) if s == "15"));
    }

    #[test]
    fn transform_concatenates_non_numeric_string_vars_without_literal_quotes() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "greeting",
            VarEntry {
                value: VarValues::String("ola".to_string()),
                mutable: false,
            },
        )
        .unwrap();
        ctx.add_var(
            "result",
            VarEntry {
                value: VarValues::String("".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("greeting + \" mundo\"".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        t.transform("input".into(), Some(&mut ctx)).unwrap();

        let var = ctx.get_var("result").unwrap();
        // Sem essa checagem, o Display do evalexpr guardaria "\"ola mundo\""
        // (com aspas literais) em vez de "ola mundo".
        assert!(matches!(&var.value, VarValues::String(s) if s == "ola mundo"));
    }

    #[test]
    fn transform_string_literal_result_has_no_literal_quotes() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "result",
            VarEntry {
                value: VarValues::String("".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("\"hello\"".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        t.transform("input".into(), Some(&mut ctx)).unwrap();

        let var = ctx.get_var("result").unwrap();
        assert!(matches!(&var.value, VarValues::String(s) if s == "hello"));
    }

    #[test]
    fn transform_skips_token_variable_without_erroring() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "tok",
            VarEntry {
                value: VarValues::Token(TokenWrapper::default()),
                mutable: false,
            },
        )
        .unwrap();
        ctx.add_var(
            "result",
            VarEntry {
                value: VarValues::String("".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("1 + 1".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        t.transform("input".into(), Some(&mut ctx)).unwrap();

        let var = ctx.get_var("result").unwrap();
        assert!(matches!(&var.value, VarValues::String(s) if s == "2"));
    }

    #[test]
    fn transform_fails_on_invalid_expression_syntax() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "result",
            VarEntry {
                value: VarValues::String("".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("2 +".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        let err = t.transform("input".into(), Some(&mut ctx)).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidExprError(_)
        ));
    }

    #[test]
    fn transform_fails_when_expression_references_unknown_variable() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "result",
            VarEntry {
                value: VarValues::String("".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("nao_existe + 1".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        let err = t.transform("input".into(), Some(&mut ctx)).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidExprError(_)
        ));
    }

    #[test]
    fn transform_fails_when_mixing_float_like_string_var_with_arithmetic() {
        // "5.5" não é convertido pra Value::Float (decisão de projeto), então
        // continua Value::String — somar com um Int não é uma combinação
        // válida pro operador `+` do evalexpr.
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::String("5.5".to_string()),
                mutable: false,
            },
        )
        .unwrap();
        ctx.add_var(
            "result",
            VarEntry {
                value: VarValues::String("".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Eval::default();
        t.set_params(&[
            TextForgeParamTypes::String("n + 1".to_string()),
            TextForgeParamTypes::String("result".to_string()),
        ])
        .unwrap();

        let err = t.transform("input".into(), Some(&mut ctx)).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidExprError(_)
        ));
    }
}

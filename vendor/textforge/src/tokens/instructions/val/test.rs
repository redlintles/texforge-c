#[cfg(feature = "test_access")]
#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::context::execution_context::{
        GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues,
    };
    use crate::parser::params::TextForgeParamTypes;
    use crate::parser::resolve_var::{TokenWrapper, ValType};
    use crate::tokens::InstructionMethods;
    use crate::tokens::instructions::val::Val;
    use crate::tokens::transforms::{rfw::Rfw, rpt::Rpt};
    use crate::utils::errors::TextForgeErrorCode;

    // ============================
    // Contrato básico da instrução
    // ============================

    #[test]
    fn get_string_repr_is_val() {
        let t = Val::default();
        assert_eq!(t.get_string_repr(), "val");
    }

    #[test]
    fn set_params_sets_name_and_string_value() {
        let mut t = Val::default();
        let params = vec![
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("5".to_string()),
        ];

        assert!(t.set_params(&params).is_ok());

        let stored = t.get_params();
        assert!(matches!(stored.first(), Some(TextForgeParamTypes::String(s)) if s == "n"));
        assert!(matches!(stored.get(1), Some(TextForgeParamTypes::String(s)) if s == "5"));
    }

    #[test]
    fn set_params_accepts_varref_as_value() {
        let mut t = Val::default();
        let params = vec![
            TextForgeParamTypes::String("y".to_string()),
            TextForgeParamTypes::VarRef("x".to_string()),
        ];

        assert!(t.set_params(&params).is_ok());
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Val::default();
        let params = vec![TextForgeParamTypes::String("n".to_string())];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_non_string_name() {
        let mut t = Val::default();
        let params = vec![
            TextForgeParamTypes::Usize(1),
            TextForgeParamTypes::String("5".to_string()),
        ];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidParameters(_)
        ));
    }

    // ============================
    // Bugs encontrados durante a escrita da suíte (não relacionados à inferência,
    // mas pegos ao conferir Val::default() contra a gramática real em table.rs)
    // ============================

    #[test]
    fn default_params_should_match_val_syntax_but_currently_dont() {
        // A sintaxe de `val` em table.rs declara os dois slots como
        // SyntaxToken::String. `Val::default()`, porém, põe um `Token` no slot 1
        // (em vez de String), inconsistente com o próprio `val_value` (String(""))
        // e com o que `resolve_variables` exige. Se um `Val::default()` for
        // envolvido num TokenWrapper sem passar por `set_params` antes
        // (ex: `TokenWrapper::new(Box::new(Val::default()), None)`), isso quebra
        // em runtime com IncompatibleTypeError. Documenta o formato correto
        // esperado — falha contra a implementação atual.
        let t = Val::default();
        let params = t.get_params();

        assert!(
            matches!(params.get(1), Some(TextForgeParamTypes::String(_))),
            "esperado slot[1] = String (bate com val_value/gramática), veio {:?}",
            params.get(1)
        );
    }

    #[test]
    fn to_textforge_line_should_be_reparseable_but_is_missing_the_terminator() {
        // Toda outra instrução termina to_textforge_line() com ";\n" (ver rpt:
        // "rpt {};\n", rfw: "rfw {} {};\n") porque read_from_text espera um ';'
        // no fim da linha (strip_suffix(";") em text/reader.rs). Val::to_textforge_line()
        // não adiciona esse sufixo — serializar um pipeline com `val` e tentar reler
        // via read_from_text vai falhar. Documenta o formato esperado; falha hoje.
        let mut t = Val::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("5".to_string()),
        ])
        .unwrap();

        assert_eq!(t.to_textforge_line().as_ref(), "val n = 5;\n");
    }

    // ============================
    // transform(): declaração de variável no contexto
    // ============================

    #[test]
    fn transform_declares_immutable_string_variable() {
        let mut t = Val::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("5".to_string()),
        ])
        .unwrap();

        let mut ctx = GlobalExecutionContext::new();
        let result = t
            .transform("input inalterado".into(), Some(&mut ctx))
            .unwrap();

        assert_eq!(result.to_string(), "input inalterado");

        let var = ctx
            .get_var("n")
            .expect("variável 'n' deveria existir no contexto");
        assert!(matches!(&var.value, VarValues::String(s) if s == "5"));
        assert!(!var.mutable, "val deve sempre declarar variável imutável");
    }

    #[test]
    fn transform_aliases_existing_variable_via_varref() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "x",
            VarEntry {
                value: VarValues::String("hi".to_string()),
                mutable: false,
            },
        )
        .unwrap();

        let mut t = Val::default();
        t.set_params(&[
            TextForgeParamTypes::String("y".to_string()),
            TextForgeParamTypes::VarRef("x".to_string()),
        ])
        .unwrap();

        t.transform("qualquer coisa".into(), Some(&mut ctx))
            .unwrap();

        let y = ctx
            .get_var("y")
            .expect("'y' deveria ter sido criada a partir de 'x'");
        assert!(matches!(&y.value, VarValues::String(s) if s == "hi"));
    }

    #[test]
    fn transform_fails_without_context() {
        let mut t = Val::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("5".to_string()),
        ])
        .unwrap();

        let err = t.transform("input".into(), None).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::RequiredContextError(_)
        ));
    }

    #[test]
    fn transform_fails_when_varref_source_is_missing() {
        let mut ctx = GlobalExecutionContext::new();
        let mut t = Val::default();
        t.set_params(&[
            TextForgeParamTypes::String("y".to_string()),
            TextForgeParamTypes::VarRef("inexistente".to_string()),
        ])
        .unwrap();

        let err = t.transform("input".into(), Some(&mut ctx)).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::VariableNotFound(_)
        ));
    }

    // ============================
    // resolve_variables(): coerção de tipo entre variável e slot consumidor.
    // Os dois testes pedidos (String->Usize com sucesso e com falha) mais os
    // casos irmãos (Usize->String, e o caso sem conversão possível: Token).
    //
    // Estes só passam depois que o coerce_var_value discutido anteriormente for
    // aplicado em globals/var.rs — hoje "resolve_variables_coerces_*" vão falhar
    // contra a implementação antiga (que só aceita match exato de tipo).
    // Nota: exercitam resolve_variables, não Val diretamente — plausível candidato
    // a migrar pra um módulo de teste próprio de globals::var no futuro.
    // ============================

    #[test]
    fn resolve_variables_coerces_string_var_into_usize_slot() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::String("5".to_string()),
                mutable: false,
            },
        )
        .unwrap();

        let wrapper = TokenWrapper::new(
            Box::new(Rpt::default()),
            Some(vec![ValType::VarRef("n".to_string())]),
        );

        let result = wrapper.apply_token(Cow::from("ab"), &mut ctx).unwrap();

        assert_eq!(result.to_string(), "ababababab");
    }

    #[test]
    fn resolve_variables_errors_when_string_var_is_not_numeric() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::String("abc".to_string()),
                mutable: false,
            },
        )
        .unwrap();

        let wrapper = TokenWrapper::new(
            Box::new(Rpt::default()),
            Some(vec![ValType::VarRef("n".to_string())]),
        );

        let err = wrapper.apply_token(Cow::from("ab"), &mut ctx).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::IncompatibleTypeError(_)
        ));
    }

    #[test]
    fn resolve_variables_coerces_usize_var_into_string_slot() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::Usize(7),
                mutable: false,
            },
        )
        .unwrap();

        let wrapper = TokenWrapper::new(
            Box::new(Rfw::default()),
            Some(vec![
                ValType::VarRef("n".to_string()),
                ValType::Literal(TextForgeParamTypes::String("X".to_string())),
            ]),
        );

        let result = wrapper
            .apply_token(Cow::from("say 7 twice 7"), &mut ctx)
            .unwrap();

        assert_eq!(result.to_string(), "say X twice 7".to_string());
    }

    #[test]
    fn resolve_variables_rejects_token_var_in_string_slot() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "tok",
            VarEntry {
                value: VarValues::Token(TokenWrapper::default()),
                mutable: false,
            },
        )
        .unwrap();

        let wrapper = TokenWrapper::new(
            Box::new(Rfw::default()),
            Some(vec![
                ValType::VarRef("tok".to_string()),
                ValType::Literal(TextForgeParamTypes::String("X".to_string())),
            ]),
        );

        let err = wrapper
            .apply_token(Cow::from("input"), &mut ctx)
            .unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::IncompatibleTypeError(_)
        ));
    }
}

#[cfg(all(test, feature = "bytecode"))]
mod opcode_regression {
    use crate::{
        globals::table::{QuerySource, QueryTarget, TOKEN_TABLE, TargetValue},
        parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
        tokens::{InstructionMethods, instructions::val::Val},
    };

    #[test]
    fn val_opcode_matches_registry_and_survives_parameter_roundtrip() {
        let mut token = Val::default();
        token
            .set_params(&["prefix".to_string().into(), "🍌".to_string().into()])
            .unwrap();
        let opcode = match TOKEN_TABLE
            .find((QuerySource::Identifier("val".into()), QueryTarget::Bytecode))
            .unwrap()
        {
            TargetValue::Bytecode(opcode) => opcode,
            _ => panic!("expected opcode"),
        };
        assert_eq!(token.get_opcode(), opcode);
        assert_eq!(opcode, 0x36);
        let parameter = TextForgeParamTypes::Token(TokenWrapper::new(Box::new(token), None));
        let (_, bytes) = parameter
            .param_to_bytecode(
                &mut crate::context::execution_context::GlobalExecutionContext::new(),
            )
            .unwrap();
        match TextForgeParamTypes::from_bytecode(bytes).unwrap() {
            TextForgeParamTypes::Token(wrapper) => assert_eq!(wrapper.get_string_repr(), "val"),
            _ => panic!("expected val token"),
        }
    }
}

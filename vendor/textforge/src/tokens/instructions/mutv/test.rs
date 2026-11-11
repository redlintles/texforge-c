#[cfg(feature = "test_access")]
#[cfg(test)]
mod tests {
    use crate::context::execution_context::{
        GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues,
    };
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::instructions::mutv::Mutv;
    use crate::tokens::instructions::var::Var;
    use crate::utils::errors::TextForgeErrorCode;

    // ============================
    // Contrato básico da instrução
    // ============================

    #[test]
    fn get_string_repr_is_mutv() {
        let t = Mutv::default();
        assert_eq!(t.get_string_repr(), "mutv");
    }

    #[test]
    fn set_params_sets_name_and_string_value() {
        let mut t = Mutv::default();
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
        let mut t = Mutv::default();
        let params = vec![
            TextForgeParamTypes::String("y".to_string()),
            TextForgeParamTypes::VarRef("x".to_string()),
        ];

        assert!(t.set_params(&params).is_ok());
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Mutv::default();
        let params = vec![TextForgeParamTypes::String("n".to_string())];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_non_string_name() {
        let mut t = Mutv::default();
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

    #[test]
    fn default_params_match_mutv_syntax() {
        let t = Mutv::default();
        let params = t.get_params();

        assert!(matches!(
            params.get(1),
            Some(TextForgeParamTypes::String(_))
        ));
    }

    #[test]
    fn to_textforge_line_is_reparseable() {
        let mut t = Mutv::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("5".to_string()),
        ])
        .unwrap();

        assert_eq!(t.to_textforge_line().as_ref(), "mutv n = 5;\n");
    }

    // ============================
    // transform(): alteração de variável existente
    // ============================

    #[test]
    fn transform_updates_existing_mutable_variable() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::String("old".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Mutv::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("new".to_string()),
        ])
        .unwrap();

        let result = t
            .transform("input inalterado".into(), Some(&mut ctx))
            .unwrap();
        assert_eq!(result.to_string(), "input inalterado");

        let n = ctx.get_var("n").unwrap();
        assert!(matches!(&n.value, VarValues::String(s) if s == "new"));
        // continua mutável após a alteração
        assert!(n.mutable);
    }

    #[test]
    fn transform_allows_changing_the_stored_type() {
        // get_mut_var só valida mutabilidade, não tipo: nada em `mutv` impede
        // trocar VarValues::String por VarValues::Usize na mesma entrada.
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::String("old".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Mutv::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::Usize(42),
        ])
        .unwrap();

        t.transform("input".into(), Some(&mut ctx)).unwrap();

        let n = ctx.get_var("n").unwrap();
        assert!(matches!(&n.value, VarValues::Usize(42)));
    }

    #[test]
    fn transform_aliases_new_value_from_existing_variable_via_varref() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "source",
            VarEntry {
                value: VarValues::String("hi".to_string()),
                mutable: false,
            },
        )
        .unwrap();
        ctx.add_var(
            "target",
            VarEntry {
                value: VarValues::String("old".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Mutv::default();
        t.set_params(&[
            TextForgeParamTypes::String("target".to_string()),
            TextForgeParamTypes::VarRef("source".to_string()),
        ])
        .unwrap();

        t.transform("qualquer coisa".into(), Some(&mut ctx))
            .unwrap();

        let target = ctx.get_var("target").unwrap();
        assert!(matches!(&target.value, VarValues::String(s) if s == "hi"));
    }

    #[test]
    fn transform_fails_without_context() {
        let mut t = Mutv::default();
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
    fn transform_fails_when_variable_does_not_exist() {
        let mut ctx = GlobalExecutionContext::new();
        let mut t = Mutv::default();
        t.set_params(&[
            TextForgeParamTypes::String("nao_existe".to_string()),
            TextForgeParamTypes::String("5".to_string()),
        ])
        .unwrap();

        let err = t.transform("input".into(), Some(&mut ctx)).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::VariableNotFound(_)
        ));
    }

    #[test]
    fn transform_fails_when_variable_is_not_mutable() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::String("old".to_string()),
                mutable: false,
            },
        )
        .unwrap();

        let mut t = Mutv::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("new".to_string()),
        ])
        .unwrap();

        let err = t.transform("input".into(), Some(&mut ctx)).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::NonMutableVariableError(_)
        ));

        // e o valor original não deve ter sido tocado
        let n = ctx.get_var("n").unwrap();
        assert!(matches!(&n.value, VarValues::String(s) if s == "old"));
    }

    #[test]
    fn transform_fails_when_varref_source_is_missing() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::String("old".to_string()),
                mutable: true,
            },
        )
        .unwrap();

        let mut t = Mutv::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
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
    // Integração: var declara, mutv altera
    // ============================

    #[test]
    fn var_then_mutv_updates_the_declared_variable() {
        let mut ctx = GlobalExecutionContext::new();

        let mut declare = Var::default();
        declare
            .set_params(&[
                TextForgeParamTypes::String("counter".to_string()),
                TextForgeParamTypes::Usize(0),
            ])
            .unwrap();
        declare.transform("input".into(), Some(&mut ctx)).unwrap();

        let mut mutate = Mutv::default();
        mutate
            .set_params(&[
                TextForgeParamTypes::String("counter".to_string()),
                TextForgeParamTypes::Usize(1),
            ])
            .unwrap();
        mutate.transform("input".into(), Some(&mut ctx)).unwrap();

        let counter = ctx.get_var("counter").unwrap();
        assert!(matches!(&counter.value, VarValues::Usize(1)));
    }

    #[test]
    fn mutv_cannot_alter_a_variable_declared_by_val() {
        // val declara sempre mutable: false, então mutv sobre um nome criado
        // por val deve falhar com NonMutableVariableError.
        use crate::tokens::instructions::val::Val;

        let mut ctx = GlobalExecutionContext::new();

        let mut declare = Val::default();
        declare
            .set_params(&[
                TextForgeParamTypes::String("n".to_string()),
                TextForgeParamTypes::String("5".to_string()),
            ])
            .unwrap();
        declare.transform("input".into(), Some(&mut ctx)).unwrap();

        let mut mutate = Mutv::default();
        mutate
            .set_params(&[
                TextForgeParamTypes::String("n".to_string()),
                TextForgeParamTypes::String("6".to_string()),
            ])
            .unwrap();

        let err = mutate
            .transform("input".into(), Some(&mut ctx))
            .unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::NonMutableVariableError(_)
        ));
    }
}

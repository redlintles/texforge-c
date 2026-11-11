#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::resolve_var::TokenWrapper;
    use crate::tokens::InstructionMethods;
    use crate::tokens::instructions::ifdc::Ifdc;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn to_textforge_line_ok() {
        let token = Ifdc::new("xy", TokenWrapper::default());
        let s = token.to_textforge_line();
        assert!(s.contains("ifdc xy do"), "ifdc header ok");
    }

    #[test]
    fn transform_executes_inner_if_contains() {
        // Se Dlf faz "prefixo laranja" ou algo diferente, troque esse teste.
        // Aqui eu só testo o fluxo: contém => chama inner, não contém => retorna input
        let mut ctx = GlobalExecutionContext::new();
        let token = Ifdc::new("xy", TokenWrapper::default());

        let a = token.transform("abcxydef".into(), Some(&mut ctx));
        assert!(
            a.is_ok(),
            "contains -> inner executed (at least does not fail)"
        );

        let b = token.transform("banana".into(), Some(&mut ctx)).unwrap();
        assert_eq!(b, "banana".to_string(), "does nothing when not contains");
    }

    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;
        use crate::parser::params::TextForgeParamTypes;

        #[test]
        fn opcode_ok() {
            let t = Ifdc::default();
            assert_eq!(t.get_opcode(), 0x33);
        }

        #[test]
        fn set_params_rejects_wrong_len() {
            let mut t = Ifdc::default();
            let params: Vec<TextForgeParamTypes> =
                vec![TextForgeParamTypes::String("xy".to_string())];

            let err = t.set_params(&params).unwrap_err();

            assert!(matches!(
                err.error_code,
                TextForgeErrorCode::InvalidArgumentNumber(_)
            ));
        }

        #[test]
        fn set_params_accepts_string_as_first_param() {
            let mut t = Ifdc::default();
            let params: Vec<TextForgeParamTypes> = vec![
                TextForgeParamTypes::String("xy".to_string()),
                // depende de como você representa tokens no bytecode:
                TextForgeParamTypes::Token(TokenWrapper::default()),
            ];

            assert_eq!(t.set_params(&params), Ok(()));
        }
    }
}

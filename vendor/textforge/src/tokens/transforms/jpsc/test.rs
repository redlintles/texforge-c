// src/tokens/transforms/jpsc/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::jpsc::Jpsc;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_jpsc() {
        let t = Jpsc::default();
        assert_eq!(t.get_string_repr(), "jpsc");
    }

    #[test]
    fn to_textforge_line_is_constant() {
        let t = Jpsc::default();
        assert_eq!(t.to_textforge_line().as_ref(), "jpsc;\n");
    }

    #[test]
    fn transform_matches_doc_example() {
        let t = Jpsc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana laranja cheia de canja".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "BananaLaranjaCheiaDeCanja"
        );
    }

    #[test]
    fn transform_single_word_capitalizes() {
        let t = Jpsc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "Banana"
        );
    }

    #[test]
    fn transform_collapses_whitespace() {
        // split_whitespace() colapsa espaços/tabs/newlines
        let t = Jpsc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform(
                "  banana   laranja \n cheia\tde   canja  ".into(),
                Some(&mut ctx)
            )
            .unwrap()
            .to_string(),
            "BananaLaranjaCheiaDeCanja"
        );
    }

    #[test]
    fn transform_empty_string_returns_empty() {
        let t = Jpsc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn transform_unicode_preserved() {
        let t = Jpsc::default();
        let mut ctx = GlobalExecutionContext::new();

        // depende do capitalize() do seu projeto; esperado típico:
        assert_eq!(
            t.transform("maçã com canela".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "MaçãComCanela"
        );
    }

    #[test]
    fn set_params_accepts_empty_param_list() {
        let mut t = Jpsc::default();
        let params: Vec<TextForgeParamTypes> = vec![];

        assert_eq!(t.set_params(&params), Ok(()));
    }

    #[test]
    fn set_params_rejects_any_params() {
        let mut t = Jpsc::default();
        let params = vec![TextForgeParamTypes::Usize(1)];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    // ============================
    // Bytecode-only tests (separados)
    // ============================
    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;

        #[test]
        fn get_opcode_is_2e() {
            let t = Jpsc::default();
            assert_eq!(t.get_opcode(), 0x2e);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_no_params() {
            let t = Jpsc::default();
            let bc = t.to_bytecode().unwrap();

            // header mínimo: 8 + 4 + 1 = 13
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x2e);

            let param_count = bc[i] as usize;
            assert_eq!(param_count, 0);
        }
    }
}

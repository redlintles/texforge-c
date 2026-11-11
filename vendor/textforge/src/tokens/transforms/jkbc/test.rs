// src/tokens/transforms/jkbc/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::jkbc::Jkbc;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_jkbc() {
        let t = Jkbc::default();
        assert_eq!(t.get_string_repr(), "jkbc");
    }

    #[test]
    fn to_textforge_line_is_constant() {
        let t = Jkbc::default();
        assert_eq!(t.to_textforge_line().as_ref(), "jkbc;\n");
    }

    #[test]
    fn transform_matches_doc_example() {
        let t = Jkbc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana laranja cheia de canja".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banana-laranja-cheia-de-canja"
        );
    }

    #[test]
    fn transform_single_word_lowercases() {
        let t = Jkbc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("BaNaNa".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banana"
        );
    }

    #[test]
    fn transform_collapses_whitespace_and_lowercases() {
        let t = Jkbc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform(
                "  Banana   LARANJA \n Cheia\tDe   Canja  ".into(),
                Some(&mut ctx)
            )
            .unwrap()
            .to_string(),
            "banana-laranja-cheia-de-canja"
        );
    }

    #[test]
    fn transform_empty_string_returns_empty() {
        let t = Jkbc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn transform_unicode_lowercase_behavior() {
        let t = Jkbc::default();
        let mut ctx = GlobalExecutionContext::new();

        // unicode + lowercasing (Rust faz lowercase unicode-aware)
        assert_eq!(
            t.transform("MAÇÃ COM CANELA".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "maçã-com-canela"
        );
    }

    #[test]
    fn set_params_accepts_empty_param_list() {
        let mut t = Jkbc::default();
        let params: Vec<TextForgeParamTypes> = vec![];

        assert_eq!(t.set_params(&params), Ok(()));
    }

    #[test]
    fn set_params_rejects_any_params() {
        let mut t = Jkbc::default();
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
        fn get_opcode_is_2b() {
            let t = Jkbc::default();
            assert_eq!(t.get_opcode(), 0x2b);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_no_params() {
            let t = Jkbc::default();
            let bc = t.to_bytecode().unwrap();

            // header mínimo: 8 + 4 + 1 = 13
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;

            // total_size = tamanho do "body" (opcode+count+params...)
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x2b);

            let param_count = bc[i] as usize;
            assert_eq!(param_count, 0);
        }
    }
}

#[test]
fn optimized_conversion_preserves_contextual_greek_sigma() {
    use super::Jkbc;
    use crate::tokens::InstructionMethods;
    let token = Jkbc::default();
    assert_eq!(token.transform("ΟΣ ΑΣ".into(), None).unwrap(), "ος-ας");
}

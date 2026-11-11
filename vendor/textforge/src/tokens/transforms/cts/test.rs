// src/tokens/transforms/cts/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::cts::Cts;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn params_sets_index() {
        let t = Cts::new(3);
        assert_eq!(t.index, 3);
    }

    #[test]
    fn get_string_repr_is_cts() {
        let t = Cts::default();
        assert_eq!(t.get_string_repr(), "cts");
    }

    #[test]
    fn to_textforge_line_formats_correctly() {
        let t = Cts::new(7);
        assert_eq!(t.to_textforge_line().as_ref(), "cts 7;\n");
    }

    #[test]
    fn transform_capitalizes_word_at_index() {
        let t = Cts::new(1);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("foo bar".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "foo Bar"
        );
    }

    #[test]
    fn transform_capitalizes_first_word() {
        let t = Cts::new(0);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("foo bar".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "Foo bar"
        );
    }

    #[test]
    fn transform_capitalizes_last_word() {
        let t = Cts::new(2);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("a b c".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "a b C"
        );
    }

    #[test]
    fn transform_collapses_whitespace_due_to_split_whitespace() {
        // split_whitespace normaliza espaços/tabs/newlines
        let t = Cts::new(1);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("foo   bar".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "foo Bar"
        );
    }

    #[test]
    fn transform_errors_when_index_out_of_bounds() {
        let t = Cts::new(7);
        let mut ctx = GlobalExecutionContext::new();

        let got = t.transform("one two".into(), Some(&mut ctx));
        assert!(got.is_err());
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Cts::default();
        let params = vec![TextForgeParamTypes::Usize(1), TextForgeParamTypes::Usize(2)];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_accepts_single_usize_param() {
        let mut t = Cts::default();
        let params = vec![TextForgeParamTypes::Usize(7)];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.index, 7);
    }

    #[test]
    fn set_params_rejects_wrong_param_type() {
        let mut t = Cts::default();
        let params = vec![TextForgeParamTypes::String("x".to_string())];

        let got = t.set_params(&params);

        let expected = Err(crate::utils::errors::TextForgeError::new(
            TextForgeErrorCode::InvalidParameters("Index should be of usize type".into()),
            "",
            "",
        ));

        assert_eq!(got, expected);
    }

    // ============================
    // Bytecode-only tests (separados)
    // ============================
    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;
        use crate::parser::params::TextForgeParamTypes;

        #[test]
        fn get_opcode_is_1d() {
            let t = Cts::default();
            assert_eq!(t.get_opcode(), 0x1d);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_decodes_one_param() {
            let t = Cts::new(7);
            let bc = t.to_bytecode().unwrap();

            // header mínimo: 8 + 4 + 1 = 13
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x1d);

            let param_count = bc[i] as usize;
            i += 1;
            assert_eq!(param_count, 1);

            // param 1
            let p1_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap()) as usize;
            i += 8;
            let p1_start = i;
            let p1_end = p1_start + (p1_total - 8);
            let p1_payload = bc[p1_start..p1_end].to_vec();

            let decoded = TextForgeParamTypes::from_bytecode(p1_payload).unwrap();
            match decoded {
                TextForgeParamTypes::Usize(n) => assert_eq!(n, 7),
                _ => panic!("Expected Usize param"),
            }
        }
    }
}

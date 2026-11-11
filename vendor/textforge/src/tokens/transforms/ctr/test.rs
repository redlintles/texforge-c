// src/tokens/transforms/ctr/test.rs

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::ctr::Ctr;
    use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

    #[test]
    fn params_accepts_valid_range() {
        let t = Ctr::new(0, 1).unwrap();
        assert_eq!(t.start_index, 0);
        assert_eq!(t.end_index, 1);
    }

    #[test]
    fn get_string_repr_is_ctr() {
        let t = Ctr::default();
        assert_eq!(t.get_string_repr(), "ctr");
    }

    #[test]
    fn to_textforge_line_formats_correctly() {
        let t = Ctr::new(2, 7).unwrap();
        assert_eq!(t.to_textforge_line().as_ref(), "ctr 2 7;\n");
    }

    #[test]
    fn transform_capitalizes_range_basic_case() {
        let t = Ctr::new(1, 5).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("foo bar mar".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "foo Bar Mar"
        );
    }

    #[test]
    fn transform_capitalizes_only_inside_range() {
        // indices por split_whitespace():
        // 0: "aa", 1:"bb", 2:"cc", 3:"dd"
        let t = Ctr::new(1, 2).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("aa bb cc dd".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "aa Bb Cc dd"
        );
    }

    #[test]
    fn transform_end_index_is_clamped_when_too_big() {
        // total words = 3, end_index = 999 => clamp para total (3)
        // range vira 1..=3, então capitaliza índices 1 e 2 (3 não existe)
        let t = Ctr::new(1, 999).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("foo bar baz".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "foo Bar Baz"
        );
    }

    #[test]
    fn transform_empty_input_stays_empty() {
        let t = Ctr::new(0, 1).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn transform_errors_when_start_out_of_bounds_for_input_words() {
        let t = Ctr::new(5, 6).unwrap(); // relação ok, mas input tem poucas palavras
        let mut ctx = GlobalExecutionContext::new();

        let got: Result<Cow<'_, str>, TextForgeError> =
            t.transform("one two".into(), Some(&mut ctx));
        assert!(got.is_err());
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Ctr::default();
        let params = vec![TextForgeParamTypes::Usize(1)];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_accepts_two_usize_params() {
        let mut t = Ctr::default();
        let params = vec![TextForgeParamTypes::Usize(2), TextForgeParamTypes::Usize(7)];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.start_index, 2);
        assert_eq!(t.end_index, 7);
    }

    #[test]
    fn set_params_rejects_wrong_param_type() {
        let mut t = Ctr::default();
        let params = vec![
            TextForgeParamTypes::String("x".to_string()),
            TextForgeParamTypes::Usize(7),
        ];

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
        fn get_opcode_is_1c() {
            let t = Ctr::default();
            assert_eq!(t.get_opcode(), 0x1c);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_decodes_two_params() {
            let t = Ctr::new(2, 7).unwrap();
            let bc = t.to_bytecode().unwrap();

            // header mínimo: 8 + 4 + 1 = 13
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x1c);

            let param_count = bc[i] as usize;
            i += 1;
            assert_eq!(param_count, 2);

            // param 1
            let p1_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap()) as usize;
            i += 8;
            let p1_start = i;
            let p1_end = p1_start + (p1_total - 8);
            let p1_payload = bc[p1_start..p1_end].to_vec();
            i = p1_end;

            let decoded1 = TextForgeParamTypes::from_bytecode(p1_payload).unwrap();
            match decoded1 {
                TextForgeParamTypes::Usize(n) => assert_eq!(n, 2),
                _ => panic!("Expected Usize param #1"),
            }

            // param 2
            let p2_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap()) as usize;
            i += 8;
            let p2_start = i;
            let p2_end = p2_start + (p2_total - 8);
            let p2_payload = bc[p2_start..p2_end].to_vec();

            let decoded2 = TextForgeParamTypes::from_bytecode(p2_payload).unwrap();
            match decoded2 {
                TextForgeParamTypes::Usize(n) => assert_eq!(n, 7),
                _ => panic!("Expected Usize param #2"),
            }
        }
    }
}

// src/tokens/transforms/ctc/test.rs

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::ctc::Ctc;
    use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

    #[test]
    fn params_accepts_valid_range() {
        let t = Ctc::new(0, 1).unwrap();
        assert_eq!(t.start_index, 0);
        assert_eq!(t.end_index, 1);
    }

    #[test]
    fn get_string_repr_is_ctc() {
        let t = Ctc::default();
        assert_eq!(t.get_string_repr(), "ctc");
    }

    #[test]
    fn to_textforge_line_formats_correctly() {
        let t = Ctc::new(2, 7).unwrap();
        assert_eq!(t.to_textforge_line().as_ref(), "ctc 2 7;\n");
    }

    #[test]
    fn transform_capitalizes_chunk_simple_letters() {
        // exemplo do doc: 1..5 em "bananabananosa" => "bAnanabananosa"
        let mut ctx = GlobalExecutionContext::new();

        let t = Ctc::new(1, 5).unwrap();
        assert_eq!(
            t.transform("bananabananosa".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bAnanabananosa"
        );
    }

    #[test]
    fn transform_capitalizes_words_inside_chunk_and_reinserts() {
        // chunk pega "foo bar" e vira "Foo Bar"
        let input = "xx foo bar yy";
        // índices por caractere:
        // "xx " = 3 chars => start 3
        // "xx foo bar" (sem o espaço antes de yy) termina no índice 10? vamos computar:
        // string: x(0) x(1) ' '(2) f(3) o(4) o(5) ' '(6) b(7) a(8) r(9) ' '(10) y(11) y(12)
        // chunk [3..10) => chars 3..10 => "foo bar" (até antes do espaço)
        let mut ctx = GlobalExecutionContext::new();

        let t = Ctc::new(3, 10).unwrap();
        assert_eq!(
            t.transform(input.into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "xx Foo Bar yy"
        );
    }

    #[test]
    fn transform_end_index_clamps_to_len() {
        let input = "hello";
        let t = Ctc::new(0, 999).unwrap(); // end enorme, deve clamp
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform(input.into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "Hello"
        );
    }

    #[test]
    fn transform_on_unicode_char_indices() {
        // garante que char_indices/byte slicing está correto
        // "á" é 1 char (2 bytes), mas índice por char deve funcionar
        let input = "ábc def";
        // capitalizar chunk "ábc" => "Ábc"
        let t = Ctc::new(0, 3).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform(input.into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "Ábc def"
        );
    }

    #[test]
    fn transform_errors_on_out_of_bounds_start() {
        let input = "abc";
        let t = Ctc::new(5, 6).unwrap(); // params() só valida relação, não input
        let mut ctx = GlobalExecutionContext::new();

        let got: Result<Cow<'_, str>, TextForgeError> = t.transform(input.into(), Some(&mut ctx));
        assert!(got.is_err());
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Ctc::default();
        let params = vec![TextForgeParamTypes::Usize(1)];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_accepts_two_usize_params() {
        let mut t = Ctc::default();
        let params = vec![TextForgeParamTypes::Usize(1), TextForgeParamTypes::Usize(5)];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.start_index, 1);
        assert_eq!(t.end_index, 5);
    }

    #[test]
    fn set_params_rejects_wrong_param_type() {
        let mut t = Ctc::default();
        let params = vec![
            TextForgeParamTypes::String("x".to_string()),
            TextForgeParamTypes::Usize(5),
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
        fn get_opcode_is_1b() {
            let t = Ctc::default();
            assert_eq!(t.get_opcode(), 0x1b);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_decodes_two_params() {
            let t = Ctc::new(2, 7).unwrap();
            let bc = t.to_bytecode().unwrap();

            // header mínimo: 8 + 4 + 1 = 13
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x1b);

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

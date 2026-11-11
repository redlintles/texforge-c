// src/tokens/transforms/dlb/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::dlb::Dlb;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn params_sets_index() {
        let t = Dlb::new(3);
        assert_eq!(t.index, 3);
    }

    #[test]
    fn get_string_repr_is_dlb() {
        let t = Dlb::default();
        assert_eq!(t.get_string_repr(), "dlb");
    }

    #[test]
    fn to_textforge_line_formats_correctly() {
        let t = Dlb::new(7);
        assert_eq!(t.to_textforge_line().as_ref(), "dlb 7;\n");
    }

    #[test]
    fn transform_deletes_before_index_example_like_doc() {
        // index 3 em "banana ..." remove "ban" => "ana ..."
        let t = Dlb::new(3);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana laranja vermelha azul".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "ana laranja vermelha azul"
        );
    }

    #[test]
    fn transform_index_zero_keeps_string_intact() {
        // drain(0..0) não remove nada
        let t = Dlb::new(0);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("abcdef".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "abcdef"
        );
    }

    #[test]
    fn transform_unicode_safe_char_indexing() {
        // "á" é 1 char, mas múltiplos bytes: index por char deve funcionar
        // index 1 remove só o primeiro char => "bcdef"
        let t = Dlb::new(1);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("ábcdef".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bcdef"
        );
    }

    #[test]
    fn transform_errors_when_index_out_of_bounds() {
        // check_index_against_input deve falhar antes do drain
        let t = Dlb::new(999);
        let mut ctx = GlobalExecutionContext::new();

        let got = t.transform("abc".into(), Some(&mut ctx));
        assert!(got.is_err());
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Dlb::default();
        let params = vec![TextForgeParamTypes::Usize(1), TextForgeParamTypes::Usize(2)];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_accepts_single_usize_param() {
        let mut t = Dlb::default();
        let params = vec![TextForgeParamTypes::Usize(7)];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.index, 7);
    }

    #[test]
    fn set_params_rejects_wrong_param_type() {
        let mut t = Dlb::default();
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
        fn get_opcode_is_0a() {
            let t = Dlb::default();
            assert_eq!(t.get_opcode(), 0x0a);
        }
        #[test]
        fn to_bytecode_has_expected_header_and_decodes_one_param() {
            let t = Dlb::new(7);
            let bc = t.to_bytecode().unwrap();

            // header mínimo: 8 + 4 + 1 = 13
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x0a);

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

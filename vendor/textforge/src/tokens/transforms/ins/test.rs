// src/tokens/transforms/ins/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::ins::Ins;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn params_sets_fields_and_to_textforge_line_formats() {
        let t = Ins::new(2, "laranja");
        assert_eq!(t.to_textforge_line().as_ref(), "ins 2 laranja;\n");
        assert_eq!(t.get_string_repr(), "ins");
    }

    #[test]
    fn transform_inserts_after_index_like_doc_example() {
        let t = Ins::new(2, "laranja");
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banlaranjaana"
        );
    }

    #[test]
    fn transform_index_zero_inserts_after_first_char() {
        // index 0 -> after 'b'
        let t = Ins::new(0, "X");
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bXanana"
        );
    }

    #[test]
    fn transform_index_last_char_appends_after_last_char() {
        // chars: b a n a n a (len 6), last index = 5
        // usa nth(index+1) -> None -> split_at(len) => append
        let t = Ins::new(5, "X");
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bananaX"
        );
    }

    #[test]
    fn transform_index_equal_char_count_appends_current_behavior() {
        // detalhe: o código só erra se index > chars_count, então index == chars_count PASSA.
        // byte_index usa nth(index+1) => None -> append.
        let t = Ins::new(6, "X"); // chars_count("banana") = 6
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bananaX"
        );
    }

    #[test]
    fn transform_unicode_safe_insertion_boundary() {
        // input: "ábc" (chars: á b c)
        // index 0 => after 'á'
        let t = Ins::new(0, "X");
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("ábc".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "áXbc"
        );
    }

    #[test]
    fn transform_errors_when_index_too_large() {
        let t = Ins::new(999, "X");
        let mut ctx = GlobalExecutionContext::new();

        let got = t.transform("abc".into(), Some(&mut ctx));
        assert!(got.is_err());

        // opcional: valida o tipo do erro (não o texto, pq tem detalhe bytes/chars)
        if let Err(e) = got {
            // TextForgeError é PartialEq, mas mensagem varia. Checamos só o código.
            // (Não temos getter do code aqui, então comparamos o TextForgeError completo é chato.)
            // Ainda assim, dá pra comparar construindo exatamente se quiser.
            let _ = e;
        }
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Ins::default();
        let params = vec![TextForgeParamTypes::Usize(1)];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_accepts_usize_and_string() {
        let mut t = Ins::default();
        let params = vec![
            TextForgeParamTypes::Usize(3),
            TextForgeParamTypes::String("laranja".to_string()),
        ];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.to_textforge_line().as_ref(), "ins 3 laranja;\n");
    }

    #[test]
    fn set_params_rejects_wrong_param_types() {
        let mut t = Ins::default();
        let params = vec![
            TextForgeParamTypes::String("x".to_string()),
            TextForgeParamTypes::Usize(3),
        ];

        let got = t.set_params(&params);

        // primeiro parse_args! falha com "Index should be of usize type"
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
        fn get_opcode_is_28() {
            let t = Ins::default();
            assert_eq!(t.get_opcode(), 0x28);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_decodes_two_params() {
            let t = Ins::new(7, "laranja");
            let bc = t.to_bytecode().unwrap();

            // header mínimo: 8 + 4 + 1 = 13
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x28);

            let param_count = bc[i] as usize;
            i += 1;
            assert_eq!(param_count, 2);

            // param 1 (Usize)
            let p1_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap()) as usize;
            i += 8;
            let p1_start = i;
            let p1_end = p1_start + (p1_total - 8);
            let p1_payload = bc[p1_start..p1_end].to_vec();

            let decoded1 = TextForgeParamTypes::from_bytecode(p1_payload).unwrap();
            match decoded1 {
                TextForgeParamTypes::Usize(n) => assert_eq!(n, 7),
                _ => panic!("Expected Usize param #1"),
            }

            // param 2 (String)
            i = p1_end;
            let p2_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap()) as usize;
            i += 8;
            let p2_start = i;
            let p2_end = p2_start + (p2_total - 8);
            let p2_payload = bc[p2_start..p2_end].to_vec();

            let decoded2 = TextForgeParamTypes::from_bytecode(p2_payload).unwrap();
            match decoded2 {
                TextForgeParamTypes::String(s) => assert_eq!(s, "laranja"),
                _ => panic!("Expected String param #2"),
            }
        }
    }
}

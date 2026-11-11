// src/tokens/transforms/rlw/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::rlw::Rlw;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_rlw() {
        let t = Rlw::default();
        assert_eq!(t.get_string_repr(), "rlw");
    }

    #[test]
    fn params_creates_valid_regex_and_fields() {
        let t = Rlw::new("a+", "b").unwrap();
        assert_eq!(t.pattern.as_str(), "a+");
        assert_eq!(t.text_to_replace, "b".to_string());
    }

    #[test]
    fn params_rejects_invalid_regex() {
        let err = Rlw::new("(", "b").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn to_textforge_line_contains_pattern_and_replacement() {
        let t = Rlw::new("a+", "b").unwrap();
        let line = t.to_textforge_line();
        assert_eq!(line.as_ref(), "rlw a+ b;\n");
    }

    #[test]
    fn transform_replaces_last_occurrence_doc_example() {
        let t = Rlw::new("a", "b").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("aaaaa".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "aaaab"
        );
    }

    #[test]
    fn transform_when_no_match_returns_original() {
        let t = Rlw::new("z", "b").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("aaaaa".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "aaaaa"
        );
    }

    #[test]
    fn transform_replaces_last_match_when_regex_groups() {
        // aqui "a+" casa o bloco inteiro como UM match, então trocar "último match" == trocar o único match.
        let t = Rlw::new("a+", "b").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("aaaaa".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "b"
        );
    }

    #[test]
    fn transform_replaces_last_match_only() {
        let t = Rlw::new(r"\d+", "X").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("a1 b22 c333".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "a1 b22 cX"
        );
    }

    #[test]
    fn transform_handles_utf8_safely() {
        // se o regex encontra "ã", a substituição precisa manter UTF-8 correto
        let t = Rlw::new("ã", "A").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("maçã maçã".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "maçã maçA"
        );
    }

    // ============================
    // Bytecode-only tests
    // ============================
    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;
        use crate::parser::params::TextForgeParamTypes;

        #[test]
        fn get_opcode_is_0x1e() {
            let t = Rlw::default();
            assert_eq!(t.get_opcode(), 0x1e);
        }

        #[test]
        fn set_params_parses_two_params() {
            let mut t = Rlw::default();

            let params = vec![
                TextForgeParamTypes::String("a+".to_string()),
                TextForgeParamTypes::String("b".to_string()),
            ];

            assert_eq!(t.set_params(&params), Ok(()));
            assert_eq!(t.pattern.as_str(), "a+");
            assert_eq!(t.text_to_replace, "b".to_string());
        }

        #[test]
        fn set_params_rejects_wrong_param_count() {
            let mut t = Rlw::default();

            let params = vec![TextForgeParamTypes::String("a+".to_string())];

            let err = t.set_params(&params).unwrap_err();

            assert!(matches!(
                err.error_code,
                TextForgeErrorCode::InvalidArgumentNumber(_)
            ));
        }

        #[test]
        fn set_params_rejects_wrong_types() {
            let mut t = Rlw::default();

            let params = vec![
                TextForgeParamTypes::Usize(7), // deveria ser String (pattern)
                TextForgeParamTypes::String("b".to_string()),
            ];

            let got = t.set_params(&params);

            let expected = Err(crate::utils::errors::TextForgeError::new(
                TextForgeErrorCode::InvalidParameters("Pattern should be of string type".into()),
                "",
                "",
            ));

            assert_eq!(got, expected);
        }

        #[test]
        fn set_params_rejects_invalid_regex_payload() {
            let mut t = Rlw::default();

            let params = vec![
                TextForgeParamTypes::String("(".to_string()),
                TextForgeParamTypes::String("b".to_string()),
            ];

            let got = t.set_params(&params);

            let expected = Err(crate::utils::errors::TextForgeError::new(
                TextForgeErrorCode::TextParsingError("Failed to create regex".into()),
                "sslt",
                "(".to_string(),
            ));

            assert_eq!(got, expected);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_two_params() {
            let t = Rlw::new("a+", "b").unwrap();
            let bc = t.to_bytecode().unwrap();

            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x1e);

            let param_count = bc[i] as usize;
            i += 1;
            assert_eq!(param_count, 2);

            // Param 1: String("a+")
            let _p1_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            let p1_type = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            let p1_payload_size = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap()) as usize;
            i += 4;
            assert_eq!(p1_type, 0x01);
            let p1_payload = &bc[i..i + p1_payload_size];
            i += p1_payload_size;
            assert_eq!(std::str::from_utf8(p1_payload).unwrap(), "a+");

            // Param 2: String("b")
            let _p2_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            let p2_type = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            let p2_payload_size = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap()) as usize;
            i += 4;
            assert_eq!(p2_type, 0x01);
            let p2_payload = &bc[i..i + p2_payload_size];
            i += p2_payload_size;
            assert_eq!(std::str::from_utf8(p2_payload).unwrap(), "b");
            assert_eq!(i, i); // Anti unused var warning, fix this for testing purposes on the bytecode size
        }
    }
}

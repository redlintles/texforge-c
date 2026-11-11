// src/tokens/transforms/emj/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::emj::Emj;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_emj() {
        let t = Emj::default();
        assert_eq!(t.get_string_repr(), "emj");
    }

    #[test]
    fn constructor_creates_valid_regex_and_stores_separator() {
        let t = Emj::new(r"\d+", ",").unwrap();
        assert_eq!(t.separator, ",".to_string());
        assert_eq!(t.pattern.as_str(), r"\d+");
    }

    #[test]
    fn constructor_rejects_invalid_regex() {
        let err = Emj::new("(", ",").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn to_textforge_line_contains_pattern_and_separator() {
        let t = Emj::new("laranja", ",").unwrap();
        let line = t.to_textforge_line();
        assert_eq!(line.as_ref(), "emj laranja ,;\n");
    }

    #[test]
    fn transform_collects_and_joins_matches_doc_example() {
        let t = Emj::new("laranja", ",").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana laranja banana laranja".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "laranja,laranja"
        );
    }

    #[test]
    fn transform_with_regex_pattern() {
        let t = Emj::new(r"\d+", "-").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("a1 b22 c333".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "1-22-333"
        );
    }

    #[test]
    fn transform_no_matches_returns_empty_string() {
        let t = Emj::new("zzz", "_").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        // No match -> the whole input is discarded, not returned unchanged.
        assert_eq!(
            t.transform("aaaa*".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            ""
        );
    }

    #[test]
    fn transform_discards_non_matching_text() {
        let t = Emj::new("[A-Z]+", "").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("aXXbYYc".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "XXYY"
        );
    }

    #[test]
    fn transform_preserves_order_of_appearance() {
        // Guards against an implementation that accidentally sorts matches
        // instead of keeping them in the order they were found.
        let t = Emj::new(r"\d+", ",").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("b22 a1".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "22,1"
        );
    }

    #[test]
    fn transform_empty_separator_concatenates_matches() {
        let t = Emj::new(r"\d+", "").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("a1 b22 c333".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "122333"
        );
    }

    #[test]
    fn set_params_parses_pattern_and_separator_and_builds_regex() {
        let mut t = Emj::default();

        let params = vec![
            TextForgeParamTypes::String(r"\d+".to_string()),
            TextForgeParamTypes::String("-".to_string()),
        ];
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.pattern.as_str(), r"\d+");
        assert_eq!(t.separator, "-".to_string());
        assert_eq!(
            t.transform("a1b2c3".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "1-2-3"
        );
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Emj::default();

        let params = vec![TextForgeParamTypes::String(r"\d+".to_string())];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_wrong_param_types() {
        let mut t = Emj::default();

        // first param should be String(pattern)
        let params = vec![
            TextForgeParamTypes::Usize(7),
            TextForgeParamTypes::String(",".to_string()),
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
        let mut t = Emj::default();

        let params = vec![
            TextForgeParamTypes::String("(".to_string()),
            TextForgeParamTypes::String(",".to_string()),
        ];

        let got = t.set_params(&params);

        let expected = Err(crate::utils::errors::TextForgeError::new(
            TextForgeErrorCode::TextParsingError("Failed to create regex".into()),
            "emj",
            "(".to_string(),
        ));

        assert_eq!(got, expected);
    }

    // ============================
    // Bytecode-only tests (separados)
    // ============================
    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;

        #[test]
        fn get_opcode_is_37() {
            let t = Emj::default();
            assert_eq!(t.get_opcode(), 0x37);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_two_string_params() {
            let t = Emj::new("laranja", ",").unwrap();
            let bc = t.to_bytecode().unwrap();

            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x37);

            let param_count = bc[i] as usize;
            i += 1;
            assert_eq!(param_count, 2);

            // Param 1: String("laranja") -- the pattern
            let _p1_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            let p1_type = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            let p1_payload_size = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap()) as usize;
            i += 4;
            assert_eq!(p1_type, 0x01);
            let p1_payload = &bc[i..i + p1_payload_size];
            i += p1_payload_size;
            assert_eq!(std::str::from_utf8(p1_payload).unwrap(), "laranja");

            // Param 2: String(",") -- the separator
            let _p2_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            let p2_type = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            let p2_payload_size = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap()) as usize;
            i += 4;
            assert_eq!(p2_type, 0x01);
            let p2_payload = &bc[i..i + p2_payload_size];
            i += p2_payload_size;
            assert_eq!(std::str::from_utf8(p2_payload).unwrap(), ",");

            assert_eq!(i, bc.len());
        }
    }
}

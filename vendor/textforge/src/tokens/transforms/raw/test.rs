// src/tokens/transforms/raw/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::raw::Raw;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_raw() {
        let t = Raw::default();
        assert_eq!(t.get_string_repr(), "raw");
    }

    #[test]
    fn params_creates_valid_regex() {
        let t = Raw::new("a+", "b").unwrap();
        assert_eq!(t.text_to_replace, "b".to_string());
        assert_eq!(t.pattern.as_str(), "a+");
    }

    #[test]
    fn params_rejects_invalid_regex() {
        let err = Raw::new("(", "b").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn to_textforge_line_contains_pattern_and_replacement() {
        let t = Raw::new("a+", "b").unwrap();
        let line = t.to_textforge_line();
        assert_eq!(line.as_ref(), "raw a+ b;\n");
    }

    #[test]
    fn transform_replaces_all_occurrences_doc_example() {
        let t = Raw::new("a", "b").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("aaaaa".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bbbbb"
        );
    }

    #[test]
    fn transform_with_regex_pattern() {
        let t = Raw::new(r"\d+", "X").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("a1 b22 c333".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "aX bX cX"
        );
    }

    #[test]
    fn transform_no_matches_returns_same_string() {
        let t = Raw::new("zzz", "_").unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banana".to_string()
        );
    }

    #[test]
    fn set_params_parses_two_strings_and_builds_regex() {
        let mut t = Raw::default();

        let params = vec![
            TextForgeParamTypes::String("a+".to_string()),
            TextForgeParamTypes::String("b".to_string()),
        ];
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.pattern.as_str(), "a+");
        assert_eq!(t.text_to_replace, "b".to_string());
        assert_eq!(
            t.transform("aaaa".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "b".to_string()
        );
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Raw::default();

        let params = vec![TextForgeParamTypes::String("a+".to_string())];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_wrong_param_types() {
        let mut t = Raw::default();

        // primeiro deveria ser String(pattern)
        let params = vec![
            TextForgeParamTypes::Usize(7),
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
        let mut t = Raw::default();

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

    // ============================
    // Bytecode-only tests (separados)
    // ============================
    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;

        #[test]
        fn get_opcode_is_0b() {
            let t = Raw::default();
            assert_eq!(t.get_opcode(), 0x0b);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_two_string_params() {
            let t = Raw::new("a+", "b").unwrap();
            let bc = t.to_bytecode().unwrap();

            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x0b);

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

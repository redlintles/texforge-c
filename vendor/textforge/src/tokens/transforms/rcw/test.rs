// src/tokens/transforms/rcw/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::rcw::Rcw;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_rcw() {
        let t = Rcw::default();
        assert_eq!(t.get_string_repr(), "rcw");
    }

    #[test]
    fn params_creates_valid_regex_and_fields() {
        let t = Rcw::new("a+", "b", 3).unwrap();
        assert_eq!(t.pattern.as_str(), "a+");
        assert_eq!(t.text_to_replace, "b".to_string());
        assert_eq!(t.count, 3);
    }

    #[test]
    fn params_rejects_invalid_regex() {
        let err = Rcw::new("(", "b", 1).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn to_textforge_line_contains_pattern_replacement_and_count() {
        let t = Rcw::new("a+", "b", 3).unwrap();
        let line = t.to_textforge_line();
        assert_eq!(line.as_ref(), "rcw a+ b 3;\n");
    }

    #[test]
    fn transform_replaces_count_occurrences_doc_example() {
        let t = Rcw::new("a", "b", 3).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("aaaaa".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bbbaa"
        );
    }

    #[test]
    fn transform_count_zero_returns_same_string() {
        let t = Rcw::new("a", "b", 0).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("aaaaa".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "aaaaa"
        );
    }

    #[test]
    fn transform_count_larger_than_matches_replaces_all_matches() {
        let t = Rcw::new("a", "b", 99).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("aa".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bb"
        );
    }

    #[test]
    fn transform_with_regex_pattern_replaces_n_times() {
        let t = Rcw::new(r"\d+", "X", 2).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("a1 b22 c333 d4444".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "aX bX c333 d4444"
        );
    }

    // ============================
    // Bytecode-only tests (separados)
    // ============================
    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;
        use crate::parser::params::TextForgeParamTypes;

        #[test]
        fn get_opcode_is_0x10() {
            let t = Rcw::default();
            assert_eq!(t.get_opcode(), 0x10);
        }

        #[test]
        fn set_params_parses_three_params() {
            let mut t = Rcw::default();

            let params = vec![
                TextForgeParamTypes::String("a+".to_string()),
                TextForgeParamTypes::String("b".to_string()),
                TextForgeParamTypes::Usize(3),
            ];

            assert_eq!(t.set_params(&params), Ok(()));
            assert_eq!(t.pattern.as_str(), "a+");
            assert_eq!(t.text_to_replace, "b".to_string());
            assert_eq!(t.count, 3);
        }

        #[test]
        fn set_params_rejects_wrong_param_count() {
            let mut t = Rcw::default();

            let params = vec![
                TextForgeParamTypes::String("a+".to_string()),
                TextForgeParamTypes::String("b".to_string()),
            ];

            let err = t.set_params(&params).unwrap_err();

            assert!(matches!(
                err.error_code,
                TextForgeErrorCode::InvalidArgumentNumber(_)
            ));
        }

        #[test]
        fn set_params_rejects_wrong_types() {
            let mut t = Rcw::default();

            let params = vec![
                TextForgeParamTypes::Usize(7), // deveria ser String (pattern)
                TextForgeParamTypes::String("b".to_string()),
                TextForgeParamTypes::Usize(3),
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
            let mut t = Rcw::default();

            let params = vec![
                TextForgeParamTypes::String("(".to_string()),
                TextForgeParamTypes::String("b".to_string()),
                TextForgeParamTypes::Usize(3),
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
        fn to_bytecode_has_expected_header_and_three_params() {
            let t = Rcw::new("a+", "b", 3).unwrap();
            let bc = t.to_bytecode().unwrap();

            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x10);

            let param_count = bc[i] as usize;
            i += 1;
            assert_eq!(param_count, 3);

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

            // Param 3: Usize(3)
            let _p3_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            let p3_type = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            let p3_payload_size = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap()) as usize;
            i += 4;
            assert_eq!(p3_type, 0x02);
            assert_eq!(p3_payload_size, 8);
            let p3_payload = &bc[i..i + p3_payload_size];
            i += p3_payload_size;
            let n = usize::from_be_bytes(p3_payload.try_into().unwrap());
            assert_eq!(n, 3);
            assert_eq!(i, i); // Anti unused var warning, fix this for testing purposes on the bytecode size
        }
    }
}

#[test]
fn allocation_regression_preserves_storage() {
    use super::Rcw;
    use crate::tokens::InstructionMethods;
    use std::borrow::Cow;
    let token = Rcw::new("NEVER_MATCH", "x", 2).unwrap();
    let result = token.transform(Cow::Borrowed("é🍌"), None).unwrap();
    assert_eq!(result, "é🍌");
    assert!(matches!(result, Cow::Borrowed(_)));
    let mut input = String::with_capacity(128);
    input.push_str("é🍌");
    let pointer = input.as_ptr();
    let result = token.transform(Cow::Owned(input), None).unwrap();
    assert_eq!(result, "é🍌");
    assert_eq!(result.as_ptr(), pointer);
}

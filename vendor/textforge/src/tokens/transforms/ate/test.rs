// src/tokens/transforms/ate/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::ate::Ate;
    use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

    #[test]
    fn params_sets_text() {
        let t = Ate::new(" bar");
        assert_eq!(t.text, " bar".to_string());
    }

    #[test]
    fn get_string_repr_is_ate() {
        let t = Ate::default();
        assert_eq!(t.get_string_repr(), "ate");
    }

    #[test]
    fn to_textforge_line_formats_correctly() {
        let t = Ate::new("xyz");
        assert_eq!(t.to_textforge_line().as_ref(), "ate xyz;\n");
    }

    #[test]
    fn transform_appends_text() {
        let t = Ate::new(" bar");
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("foo".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "foo bar"
        );
    }

    #[test]
    fn transform_with_empty_text_keeps_input() {
        let t = Ate::new("");
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("foo".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "foo"
        );
    }

    #[test]
    fn transform_with_empty_input_returns_only_text() {
        let t = Ate::new("bar");
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            "bar".to_string()
        );
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Ate::default();
        let params = vec![
            TextForgeParamTypes::String("a".to_string()),
            TextForgeParamTypes::String("b".to_string()),
        ];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_accepts_single_string_param() {
        let mut t = Ate::default();
        let params = vec![TextForgeParamTypes::String(" bar".to_string())];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.text, " bar".to_string());
    }

    #[test]
    fn set_params_rejects_wrong_param_type() {
        let mut t = Ate::default();
        let params = vec![TextForgeParamTypes::Usize(123)];

        let got = t.set_params(&params);

        let expected = Err(TextForgeError::new(
            TextForgeErrorCode::InvalidParameters("Text should be of string type".into()),
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
        fn get_opcode_is_02() {
            let t = Ate::default();
            assert_eq!(t.get_opcode(), 0x02);
        }

        #[test]
        fn to_bytecode_has_expected_layout_and_decodes_param() {
            // Confere layout gerado por to_bytecode! + write_as_instruction_param:
            // [total_size u64][opcode u32][param_count u8][param...]
            // param = [param_total_size u64][param_type u32][payload_size u32][payload...]
            let t = Ate::new(" bar");
            let bc = t.to_bytecode().unwrap();

            // Header mínimo: 8 + 4 + 1 = 13 bytes
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;

            // total_size = tamanho do "body" (opcode+count+params...)
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x02);

            let param_count = bc[i] as usize;
            i += 1;
            assert_eq!(param_count, 1);

            // param_total_size inclui os 8 bytes dele mesmo
            let param_total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap()) as usize;
            i += 8;

            // O resto do param tem (param_total_size - 8) bytes
            let param_start = i;
            let param_end = param_start + (param_total_size - 8);

            let param_payload_for_decoder = bc[param_start..param_end].to_vec();

            // Agora dá pra usar seu decoder de param:
            let decoded = TextForgeParamTypes::from_bytecode(param_payload_for_decoder).unwrap();

            match decoded {
                TextForgeParamTypes::String(s) => assert_eq!(s, " bar".to_string()),
                _ => panic!("Expected TextForgeParamTypes::String"),
            }
        }
    }
}

#[test]
fn allocation_regression_preserves_storage() {
    use super::Ate;
    use crate::tokens::InstructionMethods;
    use std::borrow::Cow;
    let token = Ate::new("");
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

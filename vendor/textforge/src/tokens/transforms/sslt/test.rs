#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::{InstructionMethods, transforms::sslt::Sslt};
    use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

    #[test]
    fn get_string_repr_is_sslt() {
        let t = Sslt::default();
        assert_eq!(t.get_string_repr(), "sslt");
    }

    #[test]
    fn to_textforge_line_is_correctish() {
        let t = Sslt::new("_", 1).unwrap();
        // Regex Display imprime o pattern, então isso deve bater.
        assert_eq!(t.to_textforge_line().as_ref(), "sslt _ 1;\n");
    }

    #[test]
    fn transform_selects_expected_piece() {
        let t = Sslt::new("_", 1).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("foobar_foo_bar_bar_foo_barfoo".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "foo"
        );
    }

    #[test]
    fn transform_supports_empty_segments() {
        // "a__b" split "_" => ["a", "", "b"]
        let t = Sslt::new("_", 1).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("a__b".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            ""
        );
    }

    #[test]
    fn transform_errors_on_out_of_range() {
        let t = Sslt::new("_", 99).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        let got = t.transform("a_b".into(), Some(&mut ctx));

        let expected = Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange("Index does not exist in the splitted vec".into()),
            t.to_textforge_line(),
            "a_b".to_string(),
        ));

        assert_eq!(got, expected);
    }
    #[test]
    fn set_params_accepts_two_params() {
        let mut t = Sslt::default();
        let params = vec![
            TextForgeParamTypes::String("_".to_string()),
            TextForgeParamTypes::Usize(1),
        ];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.index, 1);
        assert_eq!(t.pattern.to_string(), "_".to_string());
    }

    #[test]
    fn set_params_rejects_wrong_len() {
        let mut t = Sslt::default();
        let params = vec![TextForgeParamTypes::Usize(1)];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    // ============================
    // Bytecode tests
    // ============================
    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;

        #[test]
        fn get_opcode_is_0x1a() {
            let t = Sslt::default();
            assert_eq!(t.get_opcode(), 0x1a);
        }

        #[test]
        fn to_bytecode_has_opcode_and_two_params() {
            let t = Sslt::new("_", 1).unwrap();
            let bc = t.to_bytecode().unwrap();

            // Formato: [u64 total_size_be][u32 opcode_be][u8 param_count]...
            assert!(bc.len() >= 13);

            let total_size = u64::from_be_bytes(bc[0..8].try_into().unwrap()) as usize;
            assert_eq!(total_size, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[8..12].try_into().unwrap());
            assert_eq!(opcode, 0x1a);

            let param_count = bc[12] as usize;
            assert_eq!(param_count, 2);
        }
    }
}

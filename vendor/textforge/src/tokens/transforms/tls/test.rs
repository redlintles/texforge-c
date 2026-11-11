#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::{InstructionMethods, transforms::tls::Tls};
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_tls() {
        let t = Tls::default();
        assert_eq!(t.get_string_repr(), "tls");
    }

    #[test]
    fn to_textforge_line_is_correct() {
        let t = Tls::default();
        assert_eq!(t.to_textforge_line().as_ref(), "tls;\n");
    }

    #[test]
    fn transform_trims_left_spaces_only() {
        let t = Tls::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("   banana   ".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banana   "
        );
    }

    #[test]
    fn transform_trims_left_whitespace_tabs_newlines_too() {
        let t = Tls::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("\t\n\r   banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banana"
        );
    }

    #[test]
    fn transform_no_left_whitespace_unchanged() {
        let t = Tls::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banana"
        );
    }

    #[test]
    fn transform_only_whitespace_becomes_empty() {
        let t = Tls::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("     ".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            ""
        );
    }

    #[test]
    fn transform_empty_is_empty() {
        let t = Tls::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn set_params_accepts_empty() {
        let mut t = Tls::default();
        let params: Vec<TextForgeParamTypes> = vec![];
        assert_eq!(t.set_params(&params), Ok(()));
    }

    #[test]
    fn set_params_rejects_any_params() {
        let mut t = Tls::default();
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
        fn get_opcode_is_0x06() {
            let t = Tls::default();
            assert_eq!(t.get_opcode(), 0x06);
        }

        #[test]
        fn to_bytecode_contains_opcode_and_zero_params() {
            let t = Tls::default();
            let bc = t.to_bytecode().unwrap();

            // Formato esperado: [u64 total_size_be][u32 opcode_be][u8 param_count]...
            assert!(bc.len() >= 13);

            let total_size = u64::from_be_bytes(bc[0..8].try_into().unwrap()) as usize;
            assert_eq!(total_size, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[8..12].try_into().unwrap());
            assert_eq!(opcode, 0x06);

            let param_count = bc[12] as usize;
            assert_eq!(param_count, 0);
        }
    }
}

#[test]
fn allocation_regression_preserves_storage() {
    use super::Tls;
    use crate::tokens::InstructionMethods;
    use std::borrow::Cow;
    let token = Tls::default();
    let result = token.transform(Cow::Borrowed("  é🍌"), None).unwrap();
    assert_eq!(result, "é🍌");
    assert!(matches!(result, Cow::Borrowed(_)));
    let mut input = String::with_capacity(128);
    input.push_str("  é🍌");
    let pointer = input.as_ptr();
    let result = token.transform(Cow::Owned(input), None).unwrap();
    assert_eq!(result, "é🍌");
    assert_eq!(result.as_ptr(), pointer);
}

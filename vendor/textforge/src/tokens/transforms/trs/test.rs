#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::{InstructionMethods, transforms::trs::Trs};
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_trs() {
        let t = Trs::default();
        assert_eq!(t.get_string_repr(), "trs");
    }

    #[test]
    fn to_textforge_line_is_correct() {
        let t = Trs::default();
        assert_eq!(t.to_textforge_line().as_ref(), "trs;\n");
    }

    #[test]
    fn transform_trims_end_spaces_only() {
        let t = Trs::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("   banana   ".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "   banana"
        );
    }

    #[test]
    fn transform_does_not_trim_start() {
        let t = Trs::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("   banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "   banana"
        );
    }

    #[test]
    fn transform_no_trailing_whitespace_unchanged() {
        let t = Trs::default();
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
        let t = Trs::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("      ".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            ""
        );
    }

    #[test]
    fn transform_trims_tabs_and_newlines_at_end() {
        let t = Trs::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana \t\n\r".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banana"
        );
    }

    #[test]
    fn transform_empty_is_empty() {
        let t = Trs::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn set_params_accepts_empty() {
        let mut t = Trs::default();
        let params: Vec<TextForgeParamTypes> = vec![];
        assert_eq!(t.set_params(&params), Ok(()));
    }

    #[test]
    fn set_params_rejects_any_params() {
        let mut t = Trs::default();
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
        fn get_opcode_is_0x07() {
            let t = Trs::default();
            assert_eq!(t.get_opcode(), 0x07);
        }

        #[test]
        fn to_bytecode_contains_opcode_and_zero_params() {
            let t = Trs::default();
            let bc = t.to_bytecode().unwrap();

            assert!(!bc.is_empty());
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x07);

            let param_count = bc[i] as usize;
            assert_eq!(param_count, 0);
        }
    }
}

#[test]
fn allocation_regression_preserves_storage() {
    use super::Trs;
    use crate::tokens::InstructionMethods;
    use std::borrow::Cow;
    let token = Trs::default();
    let result = token.transform(Cow::Borrowed("é🍌  "), None).unwrap();
    assert_eq!(result, "é🍌");
    assert!(matches!(result, Cow::Borrowed(_)));
    let mut input = String::with_capacity(128);
    input.push_str("é🍌  ");
    let pointer = input.as_ptr();
    let result = token.transform(Cow::Owned(input), None).unwrap();
    assert_eq!(result, "é🍌");
    assert_eq!(result.as_ptr(), pointer);
}

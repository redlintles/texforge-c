#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::{InstructionMethods, transforms::rev::Rev};
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_rev() {
        let t = Rev::default();
        assert_eq!(t.get_string_repr(), "rev");
    }

    #[test]
    fn to_textforge_line_is_correct() {
        let t = Rev::default();
        assert_eq!(t.to_textforge_line().as_ref(), "rev;\n");
    }

    #[test]
    fn transform_reverses_ascii() {
        let t = Rev::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("foobar".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "raboof"
        );
    }

    #[test]
    fn transform_empty_is_empty() {
        let t = Rev::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn transform_single_char_is_same() {
        let t = Rev::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("x".into(), Some(&mut ctx)).unwrap().to_string(),
            "x"
        );
    }

    #[test]
    fn transform_unicode_safe() {
        // chars() => reversão por scalar values (não por byte)
        let t = Rev::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("áβç".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "çβá"
        );
    }

    #[test]
    fn set_params_accepts_empty() {
        let mut t = Rev::default();
        let params: Vec<TextForgeParamTypes> = vec![];
        assert_eq!(t.set_params(&params), Ok(()));
    }

    #[test]
    fn set_params_rejects_any_params() {
        let mut t = Rev::default();
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
        fn get_opcode_is_0x22() {
            let t = Rev::default();
            assert_eq!(t.get_opcode(), 0x22);
        }

        #[test]
        fn to_bytecode_contains_opcode_and_zero_params() {
            let t = Rev::default();
            let bc = t.to_bytecode().unwrap();

            assert!(!bc.is_empty());
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x22);

            let param_count = bc[i] as usize;
            assert_eq!(param_count, 0);
        }
    }
}

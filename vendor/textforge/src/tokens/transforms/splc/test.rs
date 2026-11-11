#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::{InstructionMethods, transforms::splc::Splc};
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_splc() {
        let t = Splc::default();
        assert_eq!(t.get_string_repr(), "splc");
    }

    #[test]
    fn to_textforge_line_is_correct() {
        let t = Splc::default();
        assert_eq!(t.to_textforge_line().as_ref(), "splc;\n");
    }

    #[test]
    fn transform_splits_ascii_chars() {
        let t = Splc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "b a n a n a"
        );
    }

    #[test]
    fn transform_keeps_existing_spaces_as_chars() {
        let t = Splc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("a b".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "a   b"
        );
        // explicação: chars = ['a',' ','b'] => "a" + " " + " " + " " + "b" => "a␠␠␠b"
    }

    #[test]
    fn transform_unicode_chars_ok() {
        let t = Splc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("áβ🍌".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "á β 🍌"
        );
    }

    #[test]
    fn transform_empty_is_empty() {
        let t = Splc::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn set_params_accepts_empty() {
        let mut t = Splc::default();
        let params: Vec<TextForgeParamTypes> = vec![];
        assert_eq!(t.set_params(&params), Ok(()));
    }

    #[test]
    fn set_params_rejects_any_params() {
        let mut t = Splc::default();
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
        fn get_opcode_is_0x23() {
            let t = Splc::default();
            assert_eq!(t.get_opcode(), 0x23);
        }

        #[test]
        fn to_bytecode_contains_opcode_and_zero_params() {
            let t = Splc::default();
            let bc = t.to_bytecode().unwrap();

            // Formato esperado: [u64 total_size_be][u32 opcode_be][u8 param_count]...
            assert!(bc.len() >= 13);

            let total_size = u64::from_be_bytes(bc[0..8].try_into().unwrap()) as usize;
            assert_eq!(total_size, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[8..12].try_into().unwrap());
            assert_eq!(opcode, 0x23);

            let param_count = bc[12] as usize;
            assert_eq!(param_count, 0);
        }
    }
}

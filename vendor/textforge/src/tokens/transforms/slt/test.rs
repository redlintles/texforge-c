#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::{InstructionMethods, transforms::slt::Slt};
    use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

    #[test]
    fn get_string_repr_is_slt() {
        let t = Slt::default();
        assert_eq!(t.get_string_repr(), "slt");
    }

    #[test]
    fn to_textforge_line_is_correct() -> Result<(), TextForgeError> {
        let t = Slt::new(1, 3).unwrap();
        assert_eq!(t.to_textforge_line().as_ref(), "slt 1 3;\n");
        Ok(())
    }

    #[test]
    fn transform_selects_basic_slice() {
        let t = Slt::new(1, 3).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "ana"
        );
    }

    #[test]
    fn transform_supports_unicode_safely() {
        // banàna => b a n à n a
        // indices: 0 b, 1 a, 2 n, 3 à, 4 n, 5 a
        let t = Slt::new(1, 4).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banàna".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "anàn"
        );
    }

    #[test]
    fn transform_end_index_beyond_len_selects_until_end() {
        let t = Slt::new(1, 9999).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "anana"
        );
    }

    #[test]
    fn transform_rejects_invalid_bounds() {
        // start > end deve falhar (quem define isso é seu check_chunk_bound_indexes)
        let t = Slt::new(5, 1);

        assert!(t.is_err());
    }

    #[test]
    fn set_params_accepts_two_params() {
        let mut t = Slt::default();
        let params = vec![TextForgeParamTypes::Usize(1), TextForgeParamTypes::Usize(3)];
        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.start_index, 1);
        assert_eq!(t.end_index, 3);
    }

    #[test]
    fn set_params_rejects_wrong_len() {
        let mut t = Slt::default();
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
        fn get_opcode_is_0x11() {
            let t = Slt::default();
            assert_eq!(t.get_opcode(), 0x11);
        }

        #[test]
        fn to_bytecode_contains_opcode_and_two_params() {
            let t = Slt::new(1, 3).unwrap();
            let bc = t.to_bytecode().unwrap();

            // Formato: [u64 total_size_be][u32 opcode_be][u8 param_count]...
            assert!(bc.len() >= 13);

            let total_size = u64::from_be_bytes(bc[0..8].try_into().unwrap()) as usize;
            assert_eq!(total_size, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[8..12].try_into().unwrap());
            assert_eq!(opcode, 0x11);

            let param_count = bc[12] as usize;
            assert_eq!(param_count, 2);
        }
    }
}

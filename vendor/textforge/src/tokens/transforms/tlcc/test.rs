#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::{InstructionMethods, transforms::tlcc::Tlcc};
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_tlcc() {
        let t = Tlcc::default();
        assert_eq!(t.get_string_repr(), "tlcc");
    }

    #[test]
    fn to_textforge_line_is_correct() {
        let t = Tlcc::new(1, 4).unwrap();
        assert_eq!(t.to_textforge_line().as_ref(), "tlcc 1 4;\n");
    }

    #[test]
    fn transform_lowercases_only_the_chunk() {
        let t = Tlcc::new(1, 4).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        // BANANA => BananA (1..4 inclusive)
        assert_eq!(
            t.transform("BANANA".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "BananA"
        );
    }

    #[test]
    fn transform_supports_unicode_safely() {
        // "ÁBÇDÊ" lowercasing chunk should not break UTF-8
        let t = Tlcc::new(1, 3).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        // indexes: 0 Á, 1 B, 2 Ç, 3 D, 4 Ê
        assert_eq!(
            t.transform("ÁBÇDÊ".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "ÁbçdÊ"
        );
    }

    #[test]
    fn transform_errors_on_invalid_bounds() {
        // start > end (depende de como seu validator define)
        let got = Tlcc::new(4, 1);
        assert!(got.is_err());
    }

    #[test]
    fn set_params_accepts_two_usizes() {
        let mut t = Tlcc::default();
        let params = vec![TextForgeParamTypes::Usize(1), TextForgeParamTypes::Usize(4)];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.to_textforge_line().as_ref(), "tlcc 1 4;\n");
    }

    #[test]
    fn set_params_rejects_wrong_len() {
        let mut t = Tlcc::default();
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
        fn get_opcode_is_0x17() {
            let t = Tlcc::default();
            assert_eq!(t.get_opcode(), 0x17);
        }

        #[test]
        fn to_bytecode_has_opcode_and_two_params() {
            let t = Tlcc::new(1, 4).unwrap();
            let bc = t.to_bytecode().unwrap();

            assert!(bc.len() >= 13);

            let total_size = u64::from_be_bytes(bc[0..8].try_into().unwrap()) as usize;
            assert_eq!(total_size, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[8..12].try_into().unwrap());
            assert_eq!(opcode, 0x17);

            let param_count = bc[12] as usize;
            assert_eq!(param_count, 2);
        }
    }
}

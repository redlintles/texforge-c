#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::{InstructionMethods, transforms::tucc::Tucc};
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_tucc() {
        let t = Tucc::default();
        assert_eq!(t.get_string_repr(), "tucc");
    }

    #[test]
    fn to_textforge_line_is_correct() {
        let t = Tucc::new(1, 4).unwrap();
        assert_eq!(t.to_textforge_line().as_ref(), "tucc 1 4;\n");
    }

    #[test]
    fn transform_uppercases_chunk_inclusive() {
        // 1..=4 em "banana" => "a n a n" vira "A N A N"
        let t = Tucc::new(1, 4).unwrap();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bANANa"
        );
    }

    #[test]
    fn set_params_accepts_two_usize() {
        let mut t = Tucc::default();
        let params = vec![TextForgeParamTypes::Usize(1), TextForgeParamTypes::Usize(4)];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.to_textforge_line().as_ref(), "tucc 1 4;\n");
    }

    #[test]
    fn set_params_rejects_wrong_len() {
        let mut t = Tucc::default();
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
        fn get_opcode_is_0x16() {
            let t = Tucc::default();
            assert_eq!(t.get_opcode(), 0x16);
        }
        #[test]
        fn to_bytecode_has_opcode_and_two_params() {
            let t = Tucc::new(2, 5).unwrap();
            let bc = t.to_bytecode().unwrap();

            assert!(bc.len() >= 13);

            let total_size = u64::from_be_bytes(bc[0..8].try_into().unwrap()) as usize;
            assert_eq!(total_size, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[8..12].try_into().unwrap());
            assert_eq!(opcode, 0x16);

            let param_count = bc[12] as usize;
            assert_eq!(param_count, 2);
        }
    }
}

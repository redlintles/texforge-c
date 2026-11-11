#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::{InstructionMethods, transforms::tucw::Tucw};
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_tucw() {
        let t = Tucw::default();
        assert_eq!(t.get_string_repr(), "tucw");
    }

    #[test]
    fn to_textforge_line_is_correct() {
        let t = Tucw::new(1);
        assert_eq!(t.to_textforge_line().as_ref(), "tucw 1;\n");
    }

    #[test]
    fn transform_uppercases_one_word_by_index() {
        let t = Tucw::new(1);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana laranja cheia de canja".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banana LARANJA cheia de canja"
        );
    }

    #[test]
    fn set_params_accepts_one_usize() {
        let mut t = Tucw::default();
        let params = vec![TextForgeParamTypes::Usize(2)];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.to_textforge_line().as_ref(), "tucw 2;\n");
    }

    #[test]
    fn set_params_rejects_wrong_len() {
        let mut t = Tucw::default();
        let params: Vec<TextForgeParamTypes> = vec![];

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
        fn get_opcode_is_0x2a() {
            let t = Tucw::default();
            assert_eq!(t.get_opcode(), 0x2a);
        }

        #[test]
        fn to_bytecode_has_opcode_and_one_param() {
            let t = Tucw::new(5);
            let bc = t.to_bytecode().unwrap();

            assert!(bc.len() >= 13);

            let total_size = u64::from_be_bytes(bc[0..8].try_into().unwrap()) as usize;
            assert_eq!(total_size, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[8..12].try_into().unwrap());
            assert_eq!(opcode, 0x2a);

            let param_count = bc[12] as usize;
            assert_eq!(param_count, 1);
        }
    }
}

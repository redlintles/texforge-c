#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::{InstructionMethods, transforms::tlcw::Tlcw};
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_tlcw() {
        let t = Tlcw::default();
        assert_eq!(t.get_string_repr(), "tlcw");
    }

    #[test]
    fn to_textforge_line_is_correct() {
        let t = Tlcw::new(2);
        assert_eq!(t.to_textforge_line().as_ref(), "tlcw 2;\n");
    }

    #[test]
    fn transform_lowercases_one_word_by_index() {
        let t = Tlcw::new(1);
        let input = "BANANA LARANJA CHEIA DE CANJA";
        let expected = "BANANA laranja CHEIA DE CANJA".to_string();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform(input.into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            expected
        );
    }

    #[test]
    fn transform_index_zero_lowercases_first_word() {
        let t = Tlcw::new(0);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("BANANA LARANJA".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banana LARANJA"
        );
    }

    #[test]
    fn set_params_accepts_one_usize() {
        let mut t = Tlcw::default();
        let params = vec![TextForgeParamTypes::Usize(1)];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.to_textforge_line().as_ref(), "tlcw 1;\n");
    }

    #[test]
    fn set_params_rejects_wrong_len() {
        let mut t = Tlcw::default();
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
        fn get_opcode_is_0x29() {
            let t = Tlcw::default();
            assert_eq!(t.get_opcode(), 0x29);
        }
        #[test]
        fn to_bytecode_has_opcode_and_one_param() {
            let t = Tlcw::new(7);
            let bc = t.to_bytecode().unwrap();

            assert!(bc.len() >= 13);

            let total_size = u64::from_be_bytes(bc[0..8].try_into().unwrap()) as usize;
            assert_eq!(total_size, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[8..12].try_into().unwrap());
            assert_eq!(opcode, 0x29);

            let param_count = bc[12] as usize;
            assert_eq!(param_count, 1);
        }
    }
}

#[test]
fn optimized_conversion_preserves_contextual_greek_sigma() {
    use super::Tlcw;
    use crate::tokens::InstructionMethods;
    let token = Tlcw::new(0);
    assert_eq!(token.transform("ΟΣ ΑΣ".into(), None).unwrap(), "ος ΑΣ");
}

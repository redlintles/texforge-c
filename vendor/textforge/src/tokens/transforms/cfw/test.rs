// src/tokens/transforms/cfw/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::cfw::Cfw;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_cfw() {
        let t = Cfw::default();
        assert_eq!(t.get_string_repr(), "cfw");
    }

    #[test]
    fn to_textforge_line_is_constant() {
        let t = Cfw::default();
        assert_eq!(t.to_textforge_line().as_ref(), "cfw;\n");
    }

    #[test]
    fn transform_capitalizes_first_word() {
        let t = Cfw::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("foo bar".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "Foo bar"
        );
    }

    #[test]
    fn transform_empty_input_stays_empty() {
        let t = Cfw::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn transform_single_word() {
        let t = Cfw::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("hello".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "Hello"
        );
    }

    // Se sua função capitalize() lida com espaços/pontuação de um jeito específico,
    // dá pra ir refinando esses testes depois. Por enquanto é um “smoke test”.
    #[test]
    fn transform_preserves_rest_of_string_basic_case() {
        let t = Cfw::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("foo bar baz".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "Foo bar baz"
        );
    }

    #[test]
    fn set_params_accepts_empty_param_list() {
        let mut t = Cfw::default();
        let params: Vec<TextForgeParamTypes> = vec![];

        assert_eq!(t.set_params(&params), Ok(()));
    }

    #[test]
    fn set_params_rejects_any_params() {
        let mut t = Cfw::default();
        let params = vec![TextForgeParamTypes::String("x".to_string())];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    // ============================
    // Bytecode-only tests (separados)
    // ============================
    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;

        #[test]
        fn get_opcode_is_18() {
            let t = Cfw::default();
            assert_eq!(t.get_opcode(), 0x18);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_no_params() {
            let t = Cfw::default();
            let bc = t.to_bytecode().unwrap();

            // Header mínimo: 8 + 4 + 1 = 13 bytes
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;

            // total_size = tamanho do "body" (opcode+count+params...)
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x18);

            let param_count = bc[i] as usize;
            assert_eq!(param_count, 0);
        }
    }
}

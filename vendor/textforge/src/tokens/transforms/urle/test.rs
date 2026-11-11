// src/tokens/transforms/urle/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::urle::Urle;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_urle() {
        let t = Urle::default();
        assert_eq!(t.get_string_repr(), "urle");
    }

    #[test]
    fn to_textforge_line_is_constant() {
        let t = Urle::default();
        assert_eq!(t.to_textforge_line().as_ref(), "urle;\n");
    }

    #[test]
    fn transform_matches_doc_example() {
        let t = Urle::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana laranja".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banana%20laranja"
        );
    }

    #[test]
    fn transform_encodes_reserved_characters() {
        let t = Urle::default();

        // reservado: ?, =, &, /, :
        let input = "a?b=c&d/e:f";
        let expected = "a%3Fb%3Dc%26d%2Fe%3Af".to_string();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform(input.into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            expected
        );
    }

    #[test]
    fn transform_encodes_plus_as_percent2b() {
        // importante: urlencoding::encode NÃO usa '+' pra espaço; ele usa %20,
        // e '+' vira %2B
        let t = Urle::default();

        let input = "a+b c";
        let expected = "a%2Bb%20c".to_string();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform(input.into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            expected
        );
    }

    #[test]
    fn transform_empty_string_returns_empty() {
        let t = Urle::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn transform_unicode_is_percent_encoded_utf8() {
        let t = Urle::default();

        // "maçã 🍎" em UTF-8 percent-encoded
        let input = "maçã 🍎";
        let expected = "ma%C3%A7%C3%A3%20%F0%9F%8D%8E".to_string();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform(input.into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            expected
        );
    }

    #[test]
    fn set_params_accepts_empty_param_list() {
        let mut t = Urle::default();
        let params: Vec<TextForgeParamTypes> = vec![];

        assert_eq!(t.set_params(&params), Ok(()));
    }

    #[test]
    fn set_params_rejects_any_params() {
        let mut t = Urle::default();
        let params = vec![TextForgeParamTypes::Usize(1)];

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
        fn get_opcode_is_20() {
            let t = Urle::default();
            assert_eq!(t.get_opcode(), 0x20);
        }
        #[test]
        fn to_bytecode_has_expected_header_and_no_params() {
            let t = Urle::default();
            let bc = t.to_bytecode().unwrap();

            // header mínimo: 8 + 4 + 1 = 13
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x20);

            let param_count = bc[i] as usize;
            assert_eq!(param_count, 0);
        }
    }
}

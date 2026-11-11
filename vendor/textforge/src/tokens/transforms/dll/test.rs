// src/tokens/transforms/dll/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::dll::Dll;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_dll() {
        let t = Dll::default();
        assert_eq!(t.get_string_repr(), "dll");
    }

    #[test]
    fn to_textforge_line_is_constant() {
        let t = Dll::default();
        assert_eq!(t.to_textforge_line().as_ref(), "dll;\n");
    }

    #[test]
    fn transform_deletes_last_char_basic() {
        let t = Dll::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banan"
        );
    }

    #[test]
    fn transform_empty_string_stays_empty() {
        let t = Dll::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn transform_single_char_becomes_empty() {
        let t = Dll::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("a".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn transform_unicode_last_char_removed_safely_accented() {
        let t = Dll::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("abá".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "ab"
        );
    }

    #[test]
    fn transform_unicode_last_char_removed_safely_emoji() {
        let t = Dll::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("boom💥".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "boom"
        );
    }

    #[test]
    fn set_params_accepts_empty_param_list() {
        let mut t = Dll::default();
        let params: Vec<TextForgeParamTypes> = vec![];

        assert_eq!(t.set_params(&params), Ok(()));
    }

    #[test]
    fn set_params_rejects_any_params() {
        let mut t = Dll::default();
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
        fn get_opcode_is_04() {
            let t = Dll::default();
            assert_eq!(t.get_opcode(), 0x04);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_no_params() {
            let t = Dll::default();
            let bc = t.to_bytecode().unwrap();

            // header mínimo: 8 + 4 + 1 = 13
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;

            // total_size = tamanho do "body" (opcode+count+params...)
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x04);

            let param_count = bc[i] as usize;
            assert_eq!(param_count, 0);
        }
    }
}

#[test]
fn allocation_regression_preserves_storage() {
    use super::Dll;
    use crate::tokens::InstructionMethods;
    use std::borrow::Cow;
    let token = Dll::default();
    let result = token.transform(Cow::Borrowed("é🍌"), None).unwrap();
    assert_eq!(result, "é");
    assert!(matches!(result, Cow::Borrowed(_)));
    let mut input = String::with_capacity(128);
    input.push_str("é🍌");
    let pointer = input.as_ptr();
    let result = token.transform(Cow::Owned(input), None).unwrap();
    assert_eq!(result, "é");
    assert_eq!(result.as_ptr(), pointer);
}

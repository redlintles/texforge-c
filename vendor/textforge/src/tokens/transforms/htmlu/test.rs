// src/tokens/transforms/htmlu/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::htmlu::Htmlu;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn get_string_repr_is_htmlu() {
        let t = Htmlu::default();
        assert_eq!(t.get_string_repr(), "htmlu");
    }

    #[test]
    fn to_textforge_line_is_constant() {
        let t = Htmlu::default();
        assert_eq!(t.to_textforge_line().as_ref(), "htmlu;\n");
    }

    #[test]
    fn transform_unescapes_html_like_doc_example() {
        let t = Htmlu::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("&lt;div&gt;banana&lt;/div&gt;".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "<div>banana</div>"
        );
    }

    #[test]
    fn transform_unescapes_quotes_and_ampersand() {
        let t = Htmlu::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("&lt;a href=&quot;x&amp;y&quot;&gt;".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            r#"<a href="x&y">"#
        );
    }

    #[test]
    fn transform_no_entities_is_identity() {
        let t = Htmlu::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banana"
        );
    }

    #[test]
    fn transform_preserves_unicode_text() {
        let t = Htmlu::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("maçã &amp; pão".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "maçã & pão"
        );
    }

    #[test]
    fn transform_roundtrip_with_htmle_string_expectation() {
        // Sem depender diretamente do token HTMLE: apenas valida decodificação correta
        let t = Htmlu::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("a&lt;b&amp;c&gt;d".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "a<b&c>d"
        );
    }

    #[test]
    fn set_params_accepts_empty_param_list() {
        let mut t = Htmlu::default();
        let params: Vec<TextForgeParamTypes> = vec![];

        assert_eq!(t.set_params(&params), Ok(()));
    }

    #[test]
    fn set_params_rejects_any_params() {
        let mut t = Htmlu::default();
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
        fn get_opcode_is_25() {
            let t = Htmlu::default();
            assert_eq!(t.get_opcode(), 0x25);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_no_params() {
            let t = Htmlu::default();
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
            assert_eq!(opcode, 0x25);

            let param_count = bc[i] as usize;
            assert_eq!(param_count, 0);
        }
    }
}

#[test]
fn allocation_regression_preserves_storage() {
    use super::Htmlu;
    use crate::tokens::InstructionMethods;
    use std::borrow::Cow;
    let token = Htmlu::default();
    let result = token.transform(Cow::Borrowed("banana"), None).unwrap();
    assert_eq!(result, "banana");
    assert!(matches!(result, Cow::Borrowed(_)));
    let mut input = String::with_capacity(128);
    input.push_str("banana");
    let pointer = input.as_ptr();
    let result = token.transform(Cow::Owned(input), None).unwrap();
    assert_eq!(result, "banana");
    assert_eq!(result.as_ptr(), pointer);
}

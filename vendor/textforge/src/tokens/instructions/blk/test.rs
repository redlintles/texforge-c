//! Tests for token `Blk`.
//!
//! Parent module has: `#[cfg(feature = "test_access")] pub mod test;`

#[cfg(test)]
mod common {
    use crate::{
        context::execution_context::{GlobalContextMethods, GlobalExecutionContext},
        parser::params::TextForgeParamTypes,
        parser::resolve_var::TokenWrapper,
        tokens::{
            InstructionMethods,
            instructions::blk::Blk,
            transforms::{atb::Atb, dlf::Dlf},
        },
        utils::errors::TextForgeErrorCode,
    };

    // Small helper: builds the exact `Vec<TextForgeParamTypes>` a `blk name assoc <token>;`
    // line resolves to, so tests don't repeat the same boilerplate.
    fn blk_params(name: &str, inner: Box<dyn InstructionMethods>) -> Vec<TextForgeParamTypes> {
        vec![
            TextForgeParamTypes::String(name.to_string()),
            TextForgeParamTypes::Token(TokenWrapper::new(inner, None)),
        ]
    }

    #[test]
    fn default_has_expected_shape() {
        let t = Blk::default();

        assert_eq!(t.get_string_repr(), "blk");
        // Default inner token is `Null::default()` -> "null;\n"
        assert_eq!(t.to_textforge_line().as_ref(), "blk x assoc null;\n");
        assert_eq!(t.get_params().len(), 2);
    }

    #[test]
    fn set_params_sets_block_name_and_inner() {
        let mut t = Blk::default();
        let params = blk_params("greet", Box::new(Atb::new("foo")));

        assert!(t.set_params(&params).is_ok());
        assert_eq!(t.to_textforge_line().as_ref(), "blk greet assoc atb foo;\n");
    }

    #[test]
    fn set_params_rejects_wrong_param_count_too_few() {
        let mut t = Blk::default();
        let params = vec![TextForgeParamTypes::String("only_one".to_string())];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_wrong_param_count_too_many() {
        let mut t = Blk::default();
        let params = vec![
            TextForgeParamTypes::String("a".to_string()),
            TextForgeParamTypes::Token(TokenWrapper::default()),
            TextForgeParamTypes::String("extra".to_string()),
        ];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_wrong_type_for_block_name() {
        let mut t = Blk::default();
        let params = vec![
            TextForgeParamTypes::Usize(1),
            TextForgeParamTypes::Token(TokenWrapper::default()),
        ];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidParameters(_)
        ));
    }

    #[test]
    fn set_params_rejects_wrong_type_for_inner() {
        let mut t = Blk::default();
        let params = vec![
            TextForgeParamTypes::String("name".to_string()),
            TextForgeParamTypes::String("not a token".to_string()),
        ];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidParameters(_)
        ));
    }

    #[test]
    fn get_params_reflects_last_set_params_call() {
        let mut t = Blk::default();
        let params = blk_params("qux", Box::new(Atb::new("baz")));

        t.set_params(&params).unwrap();

        let stored = t.get_params();
        assert_eq!(stored.len(), 2);

        match &stored[0] {
            TextForgeParamTypes::String(s) => assert_eq!(s, "qux"),
            _ => panic!("Expected first param to be String"),
        }
        match &stored[1] {
            TextForgeParamTypes::Token(tok) => {
                assert_eq!(tok.to_textforge_line().as_ref(), "atb baz;\n")
            }
            _ => panic!("Expected second param to be Token"),
        }
    }

    #[test]
    fn transform_requires_context() {
        let t = Blk::default();
        let err = t.transform("input".into(), None).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::RequiredContextError(_)
        ));
    }

    #[test]
    fn transform_returns_input_unchanged() {
        let mut ctx = GlobalExecutionContext::new();
        let mut t = Blk::default();
        t.set_params(&blk_params("greet", Box::new(Atb::new("foo"))))
            .unwrap();

        assert_eq!(
            t.transform("hello".into(), Some(&mut ctx)).unwrap(),
            "hello".to_string()
        );
        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap(),
            "".to_string()
        );
    }

    #[test]
    fn transform_creates_block_with_single_instruction() {
        let mut ctx = GlobalExecutionContext::new();
        let mut t = Blk::default();
        t.set_params(&blk_params("greet", Box::new(Atb::new("foo"))))
            .unwrap();

        t.transform("hello".into(), Some(&mut ctx)).unwrap();

        let block = ctx.take_block("greet").unwrap();
        assert_eq!(block.len(), 1);
        assert_eq!(block[0].to_textforge_line().as_ref(), "atb foo;\n");
    }

    #[test]
    fn transform_appends_to_existing_block_instead_of_overwriting() {
        let mut ctx = GlobalExecutionContext::new();

        let mut first = Blk::default();
        first
            .set_params(&blk_params("greet", Box::new(Atb::new("foo"))))
            .unwrap();
        first.transform("x".into(), Some(&mut ctx)).unwrap();

        let mut second = Blk::default();
        second
            .set_params(&blk_params("greet", Box::new(Dlf::default())))
            .unwrap();
        second.transform("x".into(), Some(&mut ctx)).unwrap();

        let block = ctx.take_block("greet").unwrap();
        assert_eq!(block.len(), 2);
        assert_eq!(block[0].to_textforge_line().as_ref(), "atb foo;\n");
        assert_eq!(block[1].to_textforge_line().as_ref(), "dlf;\n");
    }

    #[test]
    fn transform_keeps_different_block_names_independent() {
        let mut ctx = GlobalExecutionContext::new();

        let mut a = Blk::default();
        a.set_params(&blk_params("a", Box::new(Atb::new("1"))))
            .unwrap();
        a.transform("x".into(), Some(&mut ctx)).unwrap();

        let mut b = Blk::default();
        b.set_params(&blk_params("b", Box::new(Atb::new("2"))))
            .unwrap();
        b.transform("x".into(), Some(&mut ctx)).unwrap();

        let block_a = ctx.take_block("a").unwrap();
        let block_b = ctx.take_block("b").unwrap();

        assert_eq!(block_a.len(), 1);
        assert_eq!(block_b.len(), 1);
        assert_eq!(block_a[0].to_textforge_line().as_ref(), "atb 1;\n");
        assert_eq!(block_b[0].to_textforge_line().as_ref(), "atb 2;\n");
    }

    #[test]
    fn transform_on_unknown_block_after_take_recreates_it() {
        // take_block removes the block; a later blk on the same name should
        // start a fresh vec rather than erroring out.
        let mut ctx = GlobalExecutionContext::new();

        let mut t = Blk::default();
        t.set_params(&blk_params("greet", Box::new(Atb::new("foo"))))
            .unwrap();
        t.transform("x".into(), Some(&mut ctx)).unwrap();

        let taken = ctx.take_block("greet").unwrap();
        assert_eq!(taken.len(), 1);

        // "greet" no longer exists in the context now.
        let mut t2 = Blk::default();
        t2.set_params(&blk_params("greet", Box::new(Dlf::default())))
            .unwrap();
        t2.transform("x".into(), Some(&mut ctx)).unwrap();

        let block = ctx.take_block("greet").unwrap();
        assert_eq!(block.len(), 1);
        assert_eq!(block[0].to_textforge_line().as_ref(), "dlf;\n");
    }
}

#[cfg(all(test, feature = "bytecode"))]
mod bytecode {
    use crate::{
        parser::params::TextForgeParamTypes,
        parser::resolve_var::TokenWrapper,
        tokens::{InstructionMethods, instructions::blk::Blk, transforms::atb::Atb},
    };

    #[test]
    fn opcode_is_expected() {
        let t = Blk::default();
        assert_eq!(t.get_opcode(), 0x34);
    }

    #[test]
    fn to_bytecode_has_expected_header_and_param_layout() {
        let mut t = Blk::default();

        let params = vec![
            TextForgeParamTypes::String("greet".to_string()),
            TextForgeParamTypes::Token(TokenWrapper::new(Box::new(Atb::new("foo")), None)),
        ];
        t.set_params(&params).unwrap();

        let bytes = t.to_bytecode().unwrap();

        // Header: [u64 instruction_total_size][u32 opcode][u8 param_count]
        assert!(bytes.len() > 13);

        let total_size = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        assert_eq!(total_size as usize, bytes.len() - 8);

        let opcode = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
        assert_eq!(opcode, 0x34);

        let param_count = bytes[12];
        assert_eq!(param_count, 2);

        // First param: [u64 total][u32 type=String(0x01)][u32 payload_size][payload]
        let mut idx = 13;
        let param1_total = u64::from_be_bytes(bytes[idx..idx + 8].try_into().unwrap()) as usize;
        idx += 8;
        let param1_type = u32::from_be_bytes(bytes[idx..idx + 4].try_into().unwrap());
        assert_eq!(param1_type, 0x01);
        idx += 4;
        let param1_payload_size =
            u32::from_be_bytes(bytes[idx..idx + 4].try_into().unwrap()) as usize;
        idx += 4;
        let param1_payload = &bytes[idx..idx + param1_payload_size];
        assert_eq!(param1_payload, b"greet");
        idx += param1_payload_size;

        // sanity: consumed exactly what the declared total size said we would
        assert_eq!(idx, 13 + param1_total);

        // Second param: [u64 total][u32 type=Token(0x03)][u32 payload_size][payload...]
        let param2_type = u32::from_be_bytes(bytes[idx + 8..idx + 12].try_into().unwrap());
        assert_eq!(param2_type, 0x03);

        let param2_payload_size =
            u32::from_be_bytes(bytes[idx + 12..idx + 16].try_into().unwrap()) as usize;
        // The nested `atb foo;` instruction has its own non-empty bytecode body.
        assert!(param2_payload_size > 0);

        // The nested payload starts with the inner instruction's own bytecode
        // header: [u64 total][u32 opcode]. `Atb`'s opcode is 0x01.
        let nested_opcode =
            u32::from_be_bytes(bytes[idx + 16 + 8..idx + 16 + 12].try_into().unwrap());
        assert_eq!(nested_opcode, 0x01);
    }

    #[test]
    fn to_bytecode_reflects_block_name_changes() {
        let mut t1 = Blk::default();
        t1.set_params(&[
            TextForgeParamTypes::String("a".to_string()),
            TextForgeParamTypes::Token(TokenWrapper::new(Box::new(Atb::new("x")), None)),
        ])
        .unwrap();

        let mut t2 = Blk::default();
        t2.set_params(&[
            TextForgeParamTypes::String("bbbb".to_string()),
            TextForgeParamTypes::Token(TokenWrapper::new(Box::new(Atb::new("x")), None)),
        ])
        .unwrap();

        let bytes1 = t1.to_bytecode().unwrap();
        let bytes2 = t2.to_bytecode().unwrap();

        // Different block name lengths should produce different total sizes.
        assert_ne!(bytes1.len(), bytes2.len());
    }
}

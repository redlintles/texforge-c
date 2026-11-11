//! Tests for token `Cblk` (call block).
//!
//! Parent module has: `#[cfg(feature = "test_access")] pub mod test;`

#[cfg(test)]
mod common {
    use crate::{
        context::execution_context::{GlobalContextMethods, GlobalExecutionContext},
        parser::{
            params::TextForgeParamTypes,
            resolve_var::{TokenWrapper, ValType},
        },
        tokens::{InstructionMethods, instructions::cblk::Cblk, transforms::atb::Atb},
        utils::errors::TextForgeErrorCode,
    };

    #[test]
    fn default_has_expected_shape() {
        let t = Cblk::default();

        assert_eq!(t.get_string_repr(), "cblk");
        // Note: unlike most other tokens, `to_textforge_line` does not append "\n" here.
        assert_eq!(t.to_textforge_line().as_ref(), "cblk x;\n");
        assert_eq!(t.get_params().len(), 1);
    }

    #[test]
    fn set_params_sets_block_name() {
        let mut t = Cblk::default();
        let params = vec![TextForgeParamTypes::String("greet".to_string())];

        assert!(t.set_params(&params).is_ok());
        assert_eq!(t.to_textforge_line().as_ref(), "cblk greet;\n");
    }

    #[test]
    fn set_params_rejects_wrong_param_count_too_few() {
        let mut t = Cblk::default();
        let params: Vec<TextForgeParamTypes> = vec![];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_wrong_param_count_too_many() {
        let mut t = Cblk::default();
        let params = vec![
            TextForgeParamTypes::String("a".to_string()),
            TextForgeParamTypes::String("b".to_string()),
        ];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_wrong_type_for_block_name() {
        let mut t = Cblk::default();
        let params = vec![TextForgeParamTypes::Usize(1)];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidParameters(_)
        ));
    }

    #[test]
    fn get_params_reflects_last_set_params_call() {
        let mut t = Cblk::default();
        t.set_params(&[TextForgeParamTypes::String("qux".to_string())])
            .unwrap();

        let stored = t.get_params();
        assert_eq!(stored.len(), 1);
        match &stored[0] {
            TextForgeParamTypes::String(s) => assert_eq!(s, "qux"),
            _ => panic!("Expected param to be String"),
        }
    }

    #[test]
    fn transform_requires_context() {
        let t = Cblk::default();
        let err = t.transform("input".into(), None).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::RequiredContextError(_)
        ));
    }

    #[test]
    fn transform_errors_when_block_not_found() {
        let mut ctx = GlobalExecutionContext::new();
        let mut t = Cblk::default();
        t.set_params(&[TextForgeParamTypes::String("missing".to_string())])
            .unwrap();

        let err = t.transform("x".into(), Some(&mut ctx)).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::BlockNotFound(_)
        ));
    }

    #[test]
    fn transform_returns_input_unchanged_for_empty_block() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.put_block("empty", vec![]);

        let mut t = Cblk::default();
        t.set_params(&[TextForgeParamTypes::String("empty".to_string())])
            .unwrap();

        assert_eq!(
            t.transform("hello".into(), Some(&mut ctx)).unwrap(),
            "hello".to_string()
        );
    }

    #[test]
    fn transform_applies_single_instruction_in_block() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.put_block(
            "greet",
            vec![TokenWrapper::new(Box::new(Atb::new("A")), None)],
        );

        let mut t = Cblk::default();
        t.set_params(&[TextForgeParamTypes::String("greet".to_string())])
            .unwrap();

        assert_eq!(
            t.transform("hello".into(), Some(&mut ctx)).unwrap(),
            "Ahello".to_string()
        );
    }

    #[test]
    fn transform_applies_multiple_instructions_in_order() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.put_block(
            "greet",
            vec![
                TokenWrapper::new(Box::new(Atb::new("A")), None),
                TokenWrapper::new(Box::new(Atb::new("B")), None),
            ],
        );

        let mut t = Cblk::default();
        t.set_params(&[TextForgeParamTypes::String("greet".to_string())])
            .unwrap();

        // "hello" -> Atb("A") -> "Ahello" -> Atb("B") -> "BAhello"
        assert_eq!(
            t.transform("hello".into(), Some(&mut ctx)).unwrap(),
            "BAhello".to_string()
        );
    }

    #[test]
    fn transform_puts_block_back_so_it_can_run_again() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.put_block(
            "greet",
            vec![TokenWrapper::new(Box::new(Atb::new("A")), None)],
        );

        let mut t = Cblk::default();
        t.set_params(&[TextForgeParamTypes::String("greet".to_string())])
            .unwrap();

        assert_eq!(
            t.transform("x".into(), Some(&mut ctx)).unwrap(),
            "Ax".to_string()
        );
        assert_eq!(
            t.transform("y".into(), Some(&mut ctx)).unwrap(),
            "Ay".to_string()
        );

        // The block should still be there afterwards (not consumed by execution).
        let block = ctx.take_block("greet").unwrap();
        assert_eq!(block.len(), 1);
    }

    #[test]
    fn transform_supports_nested_call_block() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.put_block(
            "inner",
            vec![TokenWrapper::new(Box::new(Atb::new("Z")), None)],
        );

        let mut inner_cblk = Cblk::default();
        inner_cblk
            .set_params(&[TextForgeParamTypes::String("inner".to_string())])
            .unwrap();
        ctx.put_block("outer", vec![TokenWrapper::new(Box::new(inner_cblk), None)]);

        let mut outer = Cblk::default();
        outer
            .set_params(&[TextForgeParamTypes::String("outer".to_string())])
            .unwrap();

        assert_eq!(
            outer.transform("y".into(), Some(&mut ctx)).unwrap(),
            "Zy".to_string()
        );
    }

    #[test]
    fn transform_propagates_error_from_inner_instruction() {
        // Force a resolve_variables failure: Atb expects a String param, we hand it
        // a Usize via TokenWrapper's raw constructor (bypasses set_params typing).
        let mut ctx = GlobalExecutionContext::new();
        let bad_token = TokenWrapper::new(
            Box::new(Atb::default()),
            Some(vec![ValType::Literal(TextForgeParamTypes::Usize(5))]),
        );
        ctx.put_block("broken", vec![bad_token]);

        let mut t = Cblk::default();
        t.set_params(&[TextForgeParamTypes::String("broken".to_string())])
            .unwrap();

        let err = t.transform("x".into(), Some(&mut ctx)).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::IncompatibleTypeError(_)
        ));

        // Documents current behavior: `transform` takes the block out of the
        // context up front and only puts it back after a *successful* full
        // pass. If an instruction fails mid-loop, the early `?` return skips
        // `put_block`, so the block is gone from the context afterwards.
        let missing = ctx.take_block("broken");
        assert!(missing.is_err());
    }
}

#[cfg(all(test, feature = "bytecode"))]
mod bytecode {
    use crate::{
        parser::params::TextForgeParamTypes,
        tokens::{InstructionMethods, instructions::cblk::Cblk},
    };

    #[test]
    fn opcode_is_expected() {
        let t = Cblk::default();
        assert_eq!(t.get_opcode(), 0x35);
    }

    #[test]
    fn to_bytecode_has_expected_header_and_param_layout() {
        let mut t = Cblk::default();
        t.set_params(&[TextForgeParamTypes::String("greet".to_string())])
            .unwrap();

        let bytes = t.to_bytecode().unwrap();

        // Header: [u64 instruction_total_size][u32 opcode][u8 param_count]
        assert!(bytes.len() > 13);

        let total_size = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        assert_eq!(total_size as usize, bytes.len() - 8);

        let opcode = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
        assert_eq!(opcode, 0x35);

        let param_count = bytes[12];
        assert_eq!(param_count, 1);

        // Param: [u64 total][u32 type=String(0x01)][u32 payload_size][payload]
        let mut idx = 13;
        let param_total = u64::from_be_bytes(bytes[idx..idx + 8].try_into().unwrap()) as usize;
        idx += 8;
        let param_type = u32::from_be_bytes(bytes[idx..idx + 4].try_into().unwrap());
        assert_eq!(param_type, 0x01);
        idx += 4;
        let payload_size = u32::from_be_bytes(bytes[idx..idx + 4].try_into().unwrap()) as usize;
        idx += 4;
        let payload = &bytes[idx..idx + payload_size];
        assert_eq!(payload, b"greet");
        idx += payload_size;

        assert_eq!(idx, 13 + param_total);
        assert_eq!(idx, bytes.len());
    }

    #[test]
    fn to_bytecode_can_be_parsed_into_params_and_fed_back_into_set_params() {
        // Same technique as the `atb` bytecode test: skip the leading u64
        // param-total header, then feed the rest to `TextForgeParamTypes::from_bytecode`.
        let mut t = Cblk::default();
        t.set_params(&[TextForgeParamTypes::String("greet".to_string())])
            .unwrap();

        let bytes = t.to_bytecode().unwrap();
        let idx = 13 + 8;
        let param_slice = bytes[idx..].to_vec();

        let parsed_param = TextForgeParamTypes::from_bytecode(param_slice).unwrap();

        let mut rebuilt = Cblk::default();
        rebuilt.set_params(&[parsed_param]).unwrap();

        assert_eq!(rebuilt.to_textforge_line().as_ref(), "cblk greet;\n");
    }

    #[test]
    fn to_bytecode_reflects_block_name_length() {
        let mut short = Cblk::default();
        short
            .set_params(&[TextForgeParamTypes::String("a".to_string())])
            .unwrap();

        let mut long = Cblk::default();
        long.set_params(&[TextForgeParamTypes::String(
            "a_much_longer_name".to_string(),
        )])
        .unwrap();

        assert!(long.to_bytecode().unwrap().len() > short.to_bytecode().unwrap().len());
    }
}

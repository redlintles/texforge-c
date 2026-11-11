use crate::{
    api::{
        TextForgeBlockMethods, TextForgeBuilderMethods, TextForgeConditionalMethods,
        block_builder::BlockBuilder,
        conditional_builder::ConditionalBuilderEach,
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    parser::resolve_var::{TokenWrapper, ValType},
    tokens::{
        InstructionMethods,
        instructions::{blk::Blk, ifdc::Ifdc},
        transforms::atb::Atb,
    },
    utils::errors::TextForgeErrorCode,
};

fn leaf() -> TokenWrapper {
    TokenWrapper::new(Box::new(Atb::new("!")), None)
}

fn conditional(child: TokenWrapper) -> TokenWrapper {
    TokenWrapper::new(
        Box::new(Ifdc::default()),
        Some(vec![ValType::from("x"), ValType::Literal(child.into())]),
    )
}

fn block(child: TokenWrapper) -> TokenWrapper {
    TokenWrapper::new(
        Box::new(Blk::default()),
        Some(vec![
            ValType::from("nested"),
            ValType::Literal(child.into()),
        ]),
    )
}

#[test]
fn rejects_blocks_at_every_descendant_depth_at_finalization() {
    for depth in 0..4 {
        let mut child = block(leaf());
        for _ in 0..depth {
            child = conditional(child);
        }
        let mut builder = BlockBuilder::new("outer");
        builder.add_to_beginning("ok").unwrap();
        builder.push_token(child).unwrap();
        let error = builder.build().err().unwrap();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::NestedBlocksNotAllowedError(_)
        ));
    }
}

#[test]
fn root_push_cannot_bypass_block_rules() {
    let mut processor = TextForgeProcessor::new();
    let mut builder = processor.create_pipeline();
    builder
        .push_token(block(conditional(block(leaf()))))
        .unwrap();
    assert!(builder.build().is_err());
    assert!(builder.build().is_err());
}

#[test]
fn failed_block_binding_keeps_previous_state() {
    let mut token = Blk::default();
    token
        .set_params(&["valid".to_string().into(), leaf().into()])
        .unwrap();
    let before = token.to_textforge_line();
    let error = token
        .set_params(&[
            "invalid".to_string().into(),
            conditional(block(leaf())).into(),
        ])
        .unwrap_err();
    assert!(matches!(
        error.error_code,
        TextForgeErrorCode::NestedBlocksNotAllowedError(_)
    ));
    assert_eq!(token.to_textforge_line(), before);
}

#[test]
fn rejects_bad_shapes_and_excess_depth_at_build() {
    for invalid in [
        TokenWrapper::new(Box::new(Atb::default()), Some(vec![])),
        TokenWrapper::new(Box::new(Atb::default()), Some(vec![ValType::from(1usize)])),
        conditional(conditional(leaf())),
    ] {
        let mut processor = TextForgeProcessor::new();
        let mut builder = processor.create_pipeline();
        builder.push_token(invalid).unwrap();
        assert!(builder.build().is_err());
    }
    let mut block_builder = BlockBuilder::new("outer");
    assert!(
        block_builder
            .push_token(conditional(conditional(leaf())))
            .is_ok()
    );
    assert_eq!(block_builder.build().unwrap().len(), 1);
    let mut block_builder = BlockBuilder::new("outer");
    block_builder
        .push_token(conditional(conditional(conditional(leaf()))))
        .unwrap();
    assert!(block_builder.build().is_err());
}

#[test]
fn conditional_resolves_builder_parameters_and_variables() {
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .val("prefix", "🍌")
        .unwrap()
        .if_do_contains_each("x", |b| {
            b.add_to_beginning("{{prefix}}")?.add_to_end("é")?;
            Ok(())
        })
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all(&id, "x").unwrap(), "🍌xé");
    assert_eq!(processor.process_all(&id, "").unwrap(), "");
}

#[test]
fn block_conditionals_flatten_in_order_and_execute_each_condition_again() {
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .block_assoc("outer", |b| {
            b.if_do_contains_each("x", |c| {
                c.add_to_beginning("🍌")?.add_to_end("é")?;
                Ok(())
            })?;
            Ok(())
        })
        .unwrap()
        .call_block("outer")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.get_transform_vec(&id).unwrap().len(), 3);
    assert_eq!(processor.process_all(&id, "x").unwrap(), "🍌xé");
    assert_eq!(processor.process_all(&id, "").unwrap(), "");
}

#[test]
fn closure_error_does_not_append_partial_children() {
    let mut processor = TextForgeProcessor::new();
    let mut builder = processor.create_pipeline();
    let result = builder.block_assoc("outer", |b| {
        b.add_to_beginning("discard")?;
        Err(crate::utils::errors::TextForgeError::new(
            TextForgeErrorCode::ValidationError("closure failed".into()),
            "test",
            "",
        ))
    });
    assert!(result.is_err());
    let id = builder.build().unwrap();
    assert!(processor.get_transform_vec(&id).unwrap().is_empty());
}

#[test]
fn conditional_constructor_preserves_runtime_binding() {
    let mut builder =
        ConditionalBuilderEach::new(Box::new(Ifdc::default()), vec!["x".to_string().into()]);
    builder.add_to_beginning("🍌").unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(
        builder
            .build()
            .unwrap()
            .into_iter()
            .map(TokenWrapper::from)
            .collect(),
    );
    assert_eq!(processor.process_all(&id, "x").unwrap(), "🍌x");
}

#[test]
fn each_condition_is_rechecked_after_previous_child_changes_text() {
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .if_do_contains_each("x", |b| {
            b.remove_pattern("x")?.add_to_end("must not run")?;
            Ok(())
        })
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all(&id, "x").unwrap(), "");
}

#[cfg(feature = "test_access")]
#[test]
fn flattened_pipeline_survives_text_and_bytecode_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .val("prefix", "🍌")
        .unwrap()
        .block_assoc("outer", |b| {
            b.if_do_contains_each("x", |c| {
                c.add_to_beginning("{{prefix}}")?.add_to_end("é")?;
                Ok(())
            })?;
            Ok(())
        })
        .unwrap()
        .call_block("outer")
        .unwrap()
        .build()
        .unwrap();
    let text = dir.path().join("pipeline.textforge");
    let bytecode = dir.path().join("pipeline.textforgebc");
    std::fs::write(&text, "").unwrap();
    std::fs::write(&bytecode, "").unwrap();
    processor.write_pipeline_to_file(&id, &text).unwrap();
    processor.write_to_bytecode_file(&id, &bytecode).unwrap();
    let text_id = processor.get_pipeline_from_file(&text).unwrap();
    let bytecode_id = processor.read_from_bytecode_file(&bytecode).unwrap();
    for loaded in [text_id, bytecode_id] {
        assert_eq!(processor.get_transform_vec(&loaded).unwrap().len(), 4);
        assert_eq!(processor.process_all(&loaded, "x").unwrap(), "🍌xé");
        assert_eq!(processor.process_all(&loaded, "").unwrap(), "");
    }
}

#[test]
fn preserves_groups_until_build_and_allows_repeatable_builds() {
    use super::tree::Node;
    let mut processor = TextForgeProcessor::new();
    let mut builder = processor.create_pipeline();
    builder
        .block_assoc("outer", |body| {
            body.add_to_beginning("🍌")?;
            body.if_do_contains_each("x", |conditional| {
                conditional.add_to_end("é")?.trim_both_sides()?;
                Ok(())
            })?;
            Ok(())
        })
        .unwrap()
        .call_block("outer")
        .unwrap();
    assert_eq!(builder.nodes.len(), 2);
    match &builder.nodes[0] {
        Node::Group { children, .. } => {
            assert_eq!(children.len(), 2);
            match &children[1] {
                Node::Group { children, .. } => assert_eq!(children.len(), 2),
                _ => panic!("conditional hierarchy was lost"),
            }
        }
        _ => panic!("block hierarchy was lost"),
    }
    let first = builder.build().unwrap();
    let second = builder.build().unwrap();
    assert_ne!(first, second);
    for id in [first, second] {
        assert_eq!(processor.get_transform_vec(&id).unwrap().len(), 4);
        assert_eq!(processor.process_all(&id, "x").unwrap(), "🍌xé");
    }
}

#[test]
fn failed_conditional_group_does_not_partially_mutate_block() {
    let mut builder = BlockBuilder::new("outer");
    builder.add_to_beginning("keep").unwrap();
    let error = builder.if_do_contains_each("x", |body| {
        body.add_to_beginning("discard")?;
        Err(crate::utils::errors::TextForgeError::new(
            TextForgeErrorCode::ValidationError("closure failed".into()),
            "test",
            "",
        ))
    });
    assert!(error.is_err());
    assert_eq!(builder.build().unwrap().len(), 1);
}

#[test]
fn root_group_closure_failure_preserves_previous_tree() {
    let mut processor = TextForgeProcessor::new();
    let mut builder = processor.create_pipeline();
    builder.add_to_beginning("keep").unwrap();
    let result = builder.if_do_contains_each("x", |body| {
        body.add_to_beginning("discard")?;
        Err(crate::utils::errors::TextForgeError::new(
            TextForgeErrorCode::ValidationError("closure failed".into()),
            "test",
            "",
        ))
    });
    assert!(result.is_err());
    let id = builder.build().unwrap();
    assert_eq!(processor.process_all(&id, "x").unwrap(), "keepx");
}

#[test]
fn final_tree_validation_rejects_nested_block_groups_even_when_empty() {
    use super::tree::Node;
    for empty in [false, true] {
        let inner = Node::Group {
            token: Box::new(Blk::default()),
            params: vec![crate::parser::params::TextForgeParamTypes::String(
                "inner".to_string(),
            )],
            children: if empty {
                vec![]
            } else {
                vec![Node::Instruction(leaf())]
            },
        };
        let condition = Node::Group {
            token: Box::new(Ifdc::default()),
            params: vec![crate::parser::params::TextForgeParamTypes::String(
                "x".to_string(),
            )],
            children: vec![inner],
        };
        let mut processor = TextForgeProcessor::new();
        let mut builder = processor.create_pipeline();
        builder.nodes.push(Node::Group {
            token: Box::new(Blk::default()),
            params: vec![crate::parser::params::TextForgeParamTypes::String(
                "outer".to_string(),
            )],
            children: vec![condition],
        });
        let error = builder.build().unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::NestedBlocksNotAllowedError(_)
        ));
    }
}

#[cfg(feature = "bytecode")]
#[test]
fn context_aware_parameter_export_still_resolves_nested_values() {
    use crate::{
        context::execution_context::GlobalExecutionContext, parser::params::TextForgeParamTypes,
        tokens::instructions::val::Val,
    };
    let mut context = GlobalExecutionContext::new();
    let mut variable = Val::default();
    variable
        .set_params(&["prefix".to_string().into(), "🍌".to_string().into()])
        .unwrap();
    variable.transform("".into(), Some(&mut context)).unwrap();
    let parameter =
        TextForgeParamTypes::Token(TokenWrapper::new(Box::new(Atb::new("{{prefix}}")), None));
    let (_, bytes) = parameter.param_to_bytecode(&mut context).unwrap();
    match TextForgeParamTypes::from_bytecode(bytes).unwrap() {
        TextForgeParamTypes::Token(token) => {
            assert!(
                matches!(&token.get_params()[0], ValType::Literal(TextForgeParamTypes::String(value)) if value == "🍌")
            );
        }
        _ => panic!("expected exported token"),
    }
}

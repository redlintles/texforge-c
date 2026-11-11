use super::rules::{NestingRule, Parameter, SignatureRule};
use super::*;
use crate::{
    tokens::{
        instructions::{blk::Blk, ifdc::Ifdc},
        transforms::atb::Atb,
    },
    utils::errors::TextForgeErrorCode,
};

/// Example policy used to verify extension without changing traversal/lowering.
/// The companion tests demonstrate literal access and inherited ancestor context.
struct RejectBang;

impl ValidationRule for RejectBang {
    fn check(
        &self,
        instruction: &InstructionView<'_>,
        context: ValidationContext,
    ) -> Result<(), TextForgeError> {
        if instruction.name == "atb" {
            assert_eq!(context.depth, 2);
            assert!(context.inside_block);
            if matches!(instruction.parameters.first(), Some(Parameter::Literal(TextForgeParamTypes::String(value))) if value == "!")
            {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::ValidationError("Reserved prefix".into()),
                    instruction.name,
                    "extension policy",
                ));
            }
        }
        Ok(())
    }
}

fn block_condition(prefix: &str) -> TokenWrapper {
    let leaf = TokenWrapper::new(Box::new(Atb::new(prefix)), None);
    let conditional = TokenWrapper::new(
        Box::new(Ifdc::default()),
        Some(vec![ValType::from("x"), ValType::Literal(leaf.into())]),
    );
    TokenWrapper::new(
        Box::new(Blk::default()),
        Some(vec![
            ValType::from("example"),
            ValType::Literal(conditional.into()),
        ]),
    )
}

#[test]
fn additional_policy_receives_real_descendants_and_inherited_context() {
    let context = ValidationContext {
        depth: 0,
        inside_block: false,
        inside_iteration: false,
    };
    let rules: &[&dyn ValidationRule] = &[&NestingRule, &SignatureRule, &RejectBang];
    walk(vec![(&block_condition("🍌"), context)], rules).unwrap();
    let error = walk(vec![(&block_condition("!"), context)], rules).unwrap_err();
    assert!(matches!(
        error.error_code,
        TextForgeErrorCode::ValidationError(_)
    ));
    assert_eq!(error.input, "extension policy");
}

#[test]
fn group_views_have_real_prefix_and_instruction_slot_without_a_dummy_leaf() {
    let prefix = [TextForgeParamTypes::String("x".to_string())];
    let view = InstructionView::group("ifdc", &prefix);
    assert_eq!(view.parameters.len(), 2);
    assert!(
        matches!(view.parameters[0], Parameter::Literal(TextForgeParamTypes::String(value)) if value == "x")
    );
    assert!(matches!(view.parameters[1], Parameter::Instruction));
    rules::check(
        &view,
        ValidationContext {
            depth: 0,
            inside_block: false,
            inside_iteration: false,
        },
    )
    .unwrap();
}

#[test]
fn signature_rules_defer_variable_resolution_to_execution() {
    let token = TokenWrapper::new(
        Box::new(Atb::default()),
        Some(vec![ValType::VarRef("not_defined_yet".to_string())]),
    );
    validate(&token).unwrap();
}

//! Side-effect-free validation rules shared by tree groups and instruction leaves.
//! Register a rule in `BUILTIN_RULES`; traversal and lowering require no changes.
use crate::{
    globals::table::{QuerySource, QueryTarget, SyntaxToken, TOKEN_TABLE, TargetValue},
    parser::{
        params::{MAX_DEPTH_ASSOC_PAYLOAD, MAX_DEPTH_NORMAL, TextForgeParamTypes},
        resolve_var::{TokenWrapper, ValType},
    },
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

/// An inherited position in the construction hierarchy, never engine state.
/// Depth counts instruction ancestors; `inside_block` remains true for all block
/// descendants. See [`super::validate_at`] and [builder traversal](crate::api::builder).
///
/// ```
/// use textforge::api::{processor::TextForgeProcessor, TextForgeBlockMethods, TextForgeBuilderMethods};
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().block_assoc("cleanup", |body| {
///     body.trim_both_sides()?;
///     Ok(())
/// }).unwrap().build().unwrap();
/// assert!(!id.is_empty());
/// ```
#[derive(Clone, Copy)]
pub(crate) struct ValidationContext {
    pub depth: usize,
    pub inside_block: bool,
    pub inside_iteration: bool,
}

impl ValidationContext {
    pub fn child(self, parent_name: &str) -> Self {
        Self {
            depth: self.depth + 1,
            inside_block: self.inside_block || parent_name == "blk",
            inside_iteration: self.inside_iteration || parent_name == "iter",
        }
    }
}

/// A borrowed scalar, a runtime reference, or a nested-instruction slot.
/// A group supplies an instruction slot without inventing a dummy child. Actual
/// children are visited separately. See [`InstructionView`] and [`ValidationRule`].
///
/// ```
/// use textforge::api::{processor::TextForgeProcessor, TextForgeBuilderMethods};
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().var("prefix", "🍌").unwrap()
///     .add_to_beginning("{{prefix}}").unwrap().build().unwrap();
/// assert!(!id.is_empty());
/// ```
pub(crate) enum Parameter<'a> {
    Literal(&'a TextForgeParamTypes),
    Reference,
    Instruction,
}

impl<'a> Parameter<'a> {
    fn literal(value: &'a TextForgeParamTypes) -> Self {
        match value {
            TextForgeParamTypes::Token(_) => Self::Instruction,
            TextForgeParamTypes::VarRef(_) => Self::Reference,
            other => Self::Literal(other),
        }
    }
}

/// Read-only instruction data presented uniformly to every registered rule.
/// The signature includes effective parameters only, excluding DSL literals such
/// as `assoc`. The view owns no instruction values and cannot mutate the builder.
/// See [`SignatureRule`] and [`crate::tokens::InstructionMethods::set_params`].
///
/// ```
/// use textforge::api::{processor::TextForgeProcessor, TextForgeBuilderMethods};
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().add_to_beginning("🍌").unwrap().build().unwrap();
/// assert!(!id.is_empty());
/// ```
pub(crate) struct InstructionView<'a> {
    pub name: &'static str,
    pub parameters: Vec<Parameter<'a>>,
}

impl<'a> InstructionView<'a> {
    pub fn instruction(token: &'a TokenWrapper) -> Self {
        Self {
            name: token.get_string_repr(),
            parameters: token
                .get_params()
                .iter()
                .map(|value| match value {
                    ValType::Literal(value) => Parameter::literal(value),
                    ValType::VarRef(_) => Parameter::Reference,
                })
                .collect(),
        }
    }

    pub fn group(name: &'static str, prefix: &'a [TextForgeParamTypes]) -> Self {
        let mut parameters = prefix.iter().map(Parameter::literal).collect::<Vec<_>>();
        parameters.push(Parameter::Instruction);
        Self { name, parameters }
    }
}

/// One independently testable policy, applied once per real instruction/group.
/// Add implementations to [`BUILTIN_RULES`]; keep traversal in [`super::validate_at`]
/// and lowering in [`crate::api::builder::tree`] independent of policy.
/// Rules must not resolve variables or execute user instructions.
///
/// ```
/// use textforge::api::{processor::TextForgeProcessor, TextForgeBuilderMethods};
/// let mut processor = TextForgeProcessor::new();
/// // Finalization applies the shared rules before registration.
/// assert!(processor.create_pipeline().trim_both_sides().unwrap().build().is_ok());
/// ```
pub(crate) trait ValidationRule: Sync {
    fn check(
        &self,
        instruction: &InstructionView<'_>,
        context: ValidationContext,
    ) -> Result<(), TextForgeError>;
}

/// Enforces ancestor-sensitive block exclusion and the parsers' depth limits.
/// Limits come from the same constants as text/bytecode parsing. See
/// [`crate::parser::params`] and [`ValidationContext`].
///
/// ```compile_fail
/// use textforge::api::{block_builder::BlockBuilder, TextForgeBlockMethods};
/// let mut outer = BlockBuilder::new("outer");
/// outer.block_assoc("forbidden", |_| Ok(())).unwrap();
/// ```
pub(super) struct NestingRule;

impl ValidationRule for NestingRule {
    fn check(
        &self,
        instruction: &InstructionView<'_>,
        context: ValidationContext,
    ) -> Result<(), TextForgeError> {
        if context.inside_block && instruction.name == "blk" {
            return Err(TextForgeError::new(
                TextForgeErrorCode::NestedBlocksNotAllowedError(
                    "A block cannot contain another block at any descendant depth".into(),
                ),
                instruction.name,
                format!("depth={}", context.depth),
            ));
        }
        if (context.inside_block || context.inside_iteration) && instruction.name == "rmapd" {
            return Err(TextForgeError::new(
                TextForgeErrorCode::ValidationError(
                    "Replacement maps may only be defined globally or inside conditionals".into(),
                ),
                instruction.name,
                format!("depth={}", context.depth),
            ));
        }
        let maximum = usize::from(if context.inside_block {
            MAX_DEPTH_ASSOC_PAYLOAD
        } else {
            MAX_DEPTH_NORMAL
        });
        if context.depth > maximum {
            return Err(TextForgeError::new(
                TextForgeErrorCode::ValidationError("Nested token depth exceeded".into()),
                instruction.name,
                format!("depth={}, max={maximum}", context.depth),
            ));
        }
        Ok(())
    }
}

/// Checks effective arity and literal parameter types against the global registry.
/// Runtime references are deferred to the execution context. See
/// [`crate::globals::table`] and [`Parameter`].
///
/// ```
/// use textforge::api::{processor::TextForgeProcessor, TextForgeBuilderMethods};
/// let mut processor = TextForgeProcessor::new();
/// // A string literal is invalid where repeat expects a number.
/// let mut builder = processor.create_pipeline();
/// builder.repeat("invalid number").unwrap();
/// assert!(builder.build().is_err());
/// ```
pub(super) struct SignatureRule;

impl ValidationRule for SignatureRule {
    fn check(
        &self,
        instruction: &InstructionView<'_>,
        _context: ValidationContext,
    ) -> Result<(), TextForgeError> {
        if let TargetValue::Syntax(syntax) = TOKEN_TABLE.find((
            QuerySource::Identifier(instruction.name.into()),
            QueryTarget::Syntax,
        ))? {
            let expected = syntax
                .iter()
                .filter_map(|part| match part.token {
                    SyntaxToken::Literal(_) => None,
                    other => Some(other),
                })
                .collect::<Vec<_>>();
            if expected.len() != instruction.parameters.len() {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters("Parameter count mismatch".into()),
                    instruction.name,
                    format!(
                        "expected={}, got={}",
                        expected.len(),
                        instruction.parameters.len()
                    ),
                ));
            }
            for (index, (parameter, expected)) in
                instruction.parameters.iter().zip(expected).enumerate()
            {
                let compatible = matches!(
                    (parameter, expected),
                    (
                        Parameter::Literal(TextForgeParamTypes::String(_)),
                        SyntaxToken::String
                    ) | (
                        Parameter::Literal(TextForgeParamTypes::Usize(_)),
                        SyntaxToken::Usize
                    ) | (Parameter::Instruction, SyntaxToken::Token)
                        | (Parameter::Reference, _)
                );
                if !compatible {
                    return Err(TextForgeError::new(
                        TextForgeErrorCode::IncompatibleTypeError(
                            "Literal parameter type mismatch".into(),
                        ),
                        instruction.name,
                        format!("parameter={index}, expected={expected:?}"),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Ordered policy registry. Add a new rule here after documenting and testing it.
pub(super) const BUILTIN_RULES: &[&dyn ValidationRule] = &[&NestingRule, &SignatureRule];

pub(super) fn check_with_rules(
    instruction: &InstructionView<'_>,
    context: ValidationContext,
    rules: &[&dyn ValidationRule],
) -> Result<(), TextForgeError> {
    for rule in rules {
        rule.check(instruction, context)?;
    }
    Ok(())
}

pub(crate) fn check(
    instruction: &InstructionView<'_>,
    context: ValidationContext,
) -> Result<(), TextForgeError> {
    check_with_rules(instruction, context, BUILTIN_RULES)
}

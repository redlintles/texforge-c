use crate::{
    globals::table::{QuerySource, QueryTarget, TOKEN_TABLE, TargetValue},
    parser::{
        params::{MAX_DEPTH_ASSOC_PAYLOAD, MAX_DEPTH_NORMAL, TextForgeParamTypes},
        resolve_var::{TokenWrapper, ValType},
    },
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

/// An owned, language-neutral instruction argument.
///
/// Text retains the engine's interpolation semantics; it is not a verbatim-text
/// escape. References are explicit and are resolved during execution. Numbers
/// are checked when converted from `u64` to the engine's `usize`. Nested
/// instructions are data, never foreign callbacks. See [`InstructionSpec`].
///
/// ```
/// use textforge::api::portable::{Argument, InstructionSpec, PipelinePlan};
/// let mut plan = PipelinePlan::new();
/// plan.add(InstructionSpec::new("atb".into(), vec![Argument::Text("🍌".into())])).unwrap();
/// assert_eq!(plan.compile().unwrap().process("x").unwrap(), "🍌x");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Argument {
    /// String parameter with normal TextForge interpolation semantics.
    Text(String),
    /// Nonnegative integer, checked against the current target's `usize` range.
    Number(u64),
    /// Exact execution-context variable name; no surrounding braces are needed.
    Variable(String),
    /// One nested instruction, such as the body parameter of `iter`.
    Instruction(Box<InstructionSpec>),
}

/// An owned instruction description using the registry's canonical DSL mnemonic.
///
/// Arguments exclude syntax literals such as `assoc`, `do`, and `times`. Names
/// and numeric representation are checked during [`super::PipelinePlan::add`];
/// arity, parameter types, and ancestry are checked during compilation. No engine
/// token, trait object, or parser type appears in this public description.
///
/// ```
/// use textforge::api::portable::{InstructionSpec, PipelinePlan};
/// let trim = InstructionSpec::new("tbs".into(), vec![]);
/// let mut plan = PipelinePlan::new();
/// plan.add(trim).unwrap();
/// assert_eq!(plan.compile().unwrap().process("  é  ").unwrap(), "é");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstructionSpec {
    /// Canonical mnemonic, for example `atb`, `tbs`, or `iter`.
    pub name: String,
    /// Effective parameters in the same order as the instruction registry.
    pub arguments: Vec<Argument>,
}

impl InstructionSpec {
    /// Create an owned description; no instruction is executed or registered.
    pub fn new(name: String, arguments: Vec<Argument>) -> Self {
        Self { name, arguments }
    }
}

/// Convert nested descriptions in post-order without recursive conversion calls.
/// Depth is bounded before creating/cloning engine token objects. The final tree
/// validation can impose a stricter limit once group ancestors are known.
pub(super) fn convert(specification: &InstructionSpec) -> Result<TokenWrapper, TextForgeError> {
    let maximum = usize::from(MAX_DEPTH_ASSOC_PAYLOAD.max(MAX_DEPTH_NORMAL));
    let mut pending = vec![(specification, 0, false)];
    let mut converted = Vec::new();
    while let Some((specification, depth, visited)) = pending.pop() {
        if !visited {
            if depth > maximum {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::ValidationError(
                        "Nested instruction description depth exceeded".into(),
                    ),
                    specification.name.clone(),
                    format!("depth={depth}, max={maximum}"),
                ));
            }
            pending.push((specification, depth, true));
            for argument in &specification.arguments {
                if let Argument::Instruction(child) = argument {
                    pending.push((child, depth + 1, false));
                }
            }
            continue;
        }
        let token = match TOKEN_TABLE
            .find((
                QuerySource::Identifier(specification.name.clone().into()),
                QueryTarget::Token,
            ))
            .map_err(|error| {
                TextForgeError::new(
                    error.error_code,
                    specification.name.clone(),
                    "portable instruction lookup",
                )
            })? {
            TargetValue::Token(token) => token.into_box(),
            _ => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::GenericError(
                        "Registry did not return an instruction".into(),
                    ),
                    specification.name.clone(),
                    "portable conversion",
                ));
            }
        };
        let mut parameters = Vec::with_capacity(specification.arguments.len());
        for (index, argument) in specification.arguments.iter().enumerate() {
            parameters.push(match argument {
                Argument::Text(value) => {
                    ValType::Literal(TextForgeParamTypes::String(value.clone()))
                }
                // Match the public builder's val/var/mut_var conversion exactly:
                // assignments store numeric literals as decimal text. Keep this
                // normalization here so every language adapter shares the fix.
                Argument::Number(value)
                    if index == 1 && matches!(specification.name.as_str(), "val" | "var" | "mutv") =>
                {
                    // Preserve the ordinary portable target-size check before
                    // adopting the builder's text representation.
                    let value = usize::try_from(*value).map_err(|_| {
                        TextForgeError::new(
                            TextForgeErrorCode::TryIntoFailError("Number exceeds the target's usize range".into()),
                            specification.name.clone(),
                            value.to_string(),
                        )
                    })?;
                    ValType::Literal(TextForgeParamTypes::String(value.to_string()))
                }
                Argument::Number(value) => ValType::Literal(TextForgeParamTypes::Usize(
                    usize::try_from(*value).map_err(|_| {
                        TextForgeError::new(
                            TextForgeErrorCode::TryIntoFailError(
                                "Number exceeds the target's usize range".into(),
                            ),
                            specification.name.clone(),
                            value.to_string(),
                        )
                    })?,
                )),
                Argument::Variable(name) => ValType::VarRef(name.clone()),
                Argument::Instruction(_) => ValType::Literal(TextForgeParamTypes::Token(
                    converted.pop().ok_or_else(|| {
                        TextForgeError::new(
                            TextForgeErrorCode::ValidationError("Missing converted child".into()),
                            specification.name.clone(),
                            "portable conversion",
                        )
                    })?,
                )),
            });
        }
        converted.push(TokenWrapper::new(token, Some(parameters)));
    }
    converted.pop().ok_or_else(|| {
        TextForgeError::new(
            TextForgeErrorCode::ValidationError("Missing converted instruction".into()),
            "portable conversion",
            "",
        )
    })
}

// Rejected foreign descriptions can be arbitrarily deep. Release their owned
// children iteratively too; rejecting depth must not recurse while dropping data.
impl Drop for InstructionSpec {
    fn drop(&mut self) {
        let mut pending = std::mem::take(&mut self.arguments);
        while let Some(argument) = pending.pop() {
            if let Argument::Instruction(mut child) = argument {
                pending.extend(std::mem::take(&mut child.arguments));
            }
        }
    }
}

/// Export independent descriptions without exposing private wrapper types.
pub(super) fn describe(root: &TokenWrapper) -> Result<InstructionSpec, TextForgeError> {
    let mut pending = vec![(root, false)];
    let mut output = Vec::new();
    while let Some((token, visited)) = pending.pop() {
        if !visited {
            pending.push((token, true));
            for value in token.get_params() {
                if let ValType::Literal(TextForgeParamTypes::Token(child)) = value {
                    pending.push((child, false));
                }
            }
            continue;
        }
        let mut arguments = Vec::new();
        for value in token.get_params() {
            arguments.push(match value {
                ValType::VarRef(name) | ValType::Literal(TextForgeParamTypes::VarRef(name)) => {
                    Argument::Variable(name.clone())
                }
                ValType::Literal(TextForgeParamTypes::String(text)) => Argument::Text(text.clone()),
                ValType::Literal(TextForgeParamTypes::Usize(number)) => {
                    Argument::Number(u64::try_from(*number).map_err(|error| {
                        TextForgeError::new(
                            TextForgeErrorCode::TryIntoFailError(error.to_string().into()),
                            token.get_string_repr(),
                            "numeric snapshot",
                        )
                    })?)
                }
                ValType::Literal(TextForgeParamTypes::Token(_)) => {
                    Argument::Instruction(Box::new(output.pop().ok_or_else(|| {
                        TextForgeError::new(
                            TextForgeErrorCode::ValidationError(
                                "Missing instruction snapshot".into(),
                            ),
                            token.get_string_repr(),
                            "portable snapshot",
                        )
                    })?))
                }
            });
        }
        output.push(InstructionSpec::new(
            token.get_string_repr().to_string(),
            arguments,
        ));
    }
    output.pop().ok_or_else(|| {
        TextForgeError::new(
            TextForgeErrorCode::ValidationError("Missing root snapshot".into()),
            "portable snapshot",
            "",
        )
    })
}

use std::{borrow::Cow, ops::Deref};

use crate::{
    context::execution_context::{GlobalContextMethods, GlobalExecutionContext},
    globals::table::{QuerySource, QueryTarget, SyntaxDef, SyntaxToken, TOKEN_TABLE, TargetValue},
    parser::params::TextForgeParamTypes,
    tokens::{InstructionMethods, instructions::null::Null},
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        expr::interpolate,
        regexes::VAR_REF_RE,
    },
};

#[derive(Clone, Debug)]
pub enum ValType {
    Literal(TextForgeParamTypes),
    VarRef(String),
}

impl From<String> for ValType {
    fn from(value: String) -> Self {
        ValType::from(value.as_str())
    }
}

impl From<&str> for ValType {
    fn from(value: &str) -> Self {
        match VAR_REF_RE.captures(value) {
            Some(caps) => Self::VarRef(caps[1].to_string()),
            None => Self::Literal(TextForgeParamTypes::String(value.to_string())),
        }
    }
}
impl From<usize> for ValType {
    fn from(value: usize) -> Self {
        Self::Literal(TextForgeParamTypes::Usize(value))
    }
}

impl From<ValType> for TextForgeParamTypes {
    fn from(value: ValType) -> Self {
        match value {
            ValType::Literal(v) => v,
            ValType::VarRef(name) => TextForgeParamTypes::VarRef(name),
        }
    }
}

pub struct TokenWrapper {
    params: Vec<ValType>,
    pub token: Box<dyn InstructionMethods>,
    prepared: Option<Box<dyn InstructionMethods>>,
}

// Clones may leave the processor through get_transform_vec and expose a mutable
// public token template. Only the private registered instance retains its cache.
impl Clone for TokenWrapper {
    fn clone(&self) -> Self {
        Self {
            params: self.params.clone(),
            token: self.token.clone(),
            prepared: None,
        }
    }
}

impl Default for TokenWrapper {
    fn default() -> Self {
        TokenWrapper {
            params: Vec::new(),
            token: Box::new(Null::default()),
            prepared: None,
        }
    }
}

impl Deref for TokenWrapper {
    type Target = Box<dyn InstructionMethods>;
    fn deref(&self) -> &Self::Target {
        &self.token
    }
}

impl From<Box<dyn InstructionMethods>> for TokenWrapper {
    fn from(value: Box<dyn InstructionMethods>) -> Self {
        let token_params = value
            .get_params()
            .clone()
            .into_iter()
            .map(ValType::Literal)
            .collect::<Vec<ValType>>();
        TokenWrapper {
            params: token_params,
            token: value,
            prepared: None,
        }
    }
}

impl From<TokenWrapper> for Box<dyn InstructionMethods> {
    fn from(value: TokenWrapper) -> Self {
        value.token
    }
}

impl TokenWrapper {
    // Only called after ownership moves into a processor's private pipeline map.
    // Externally mutable public token templates never acquire a stale cache.
    pub(crate) fn prepare_constant(&mut self) {
        if !self.token.supports_preparation()
            || !self.params.iter().all(|parameter| match parameter {
                ValType::Literal(TextForgeParamTypes::Usize(_)) => true,
                ValType::Literal(TextForgeParamTypes::String(text)) => !["{{", "}}", "[[", "]]"]
                    .iter()
                    .any(|marker| text.contains(marker)),
                _ => false,
            })
        {
            return;
        }
        let context = &mut GlobalExecutionContext::new();
        if let Ok(parameters) =
            ValType::resolve_variables(self.token.as_ref(), &self.params, context)
        {
            let mut token = self.token.clone();
            if token.set_params(&parameters).is_ok() {
                self.prepared = Some(token);
            }
        }
    }

    pub fn get_params(&self) -> &Vec<ValType> {
        &self.params
    }
    pub fn get_default_token(&self) -> Box<dyn InstructionMethods> {
        self.token.clone()
    }
    pub fn new(token: Box<dyn InstructionMethods>, params: Option<Vec<ValType>>) -> Self {
        match params {
            Some(param_vec) => TokenWrapper {
                params: param_vec,
                token,
                prepared: None,
            },
            None => {
                let token_params = token
                    .get_params()
                    .clone()
                    .into_iter()
                    .map(ValType::Literal)
                    .collect::<Vec<ValType>>();
                TokenWrapper {
                    params: token_params,
                    token,
                    prepared: None,
                }
            }
        }
    }
    /// Bind once for debug execution and metadata, so logging cannot consult a
    /// token template whose default parameters differ from the executed values.
    pub(crate) fn bind_for_debug(
        &self,
        context: &mut GlobalExecutionContext,
    ) -> Result<Box<dyn InstructionMethods>, TextForgeError> {
        let params = ValType::resolve_variables(self.token.as_ref(), &self.params, context)?;
        let mut token = self.token.clone();
        token.set_params(&params)?;
        Ok(token)
    }

    pub fn apply_token<'a>(
        &self,
        input: Cow<'a, str>,
        context: &mut GlobalExecutionContext,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if let Some(prepared) = &self.prepared {
            return prepared.transform(input, Some(context));
        }
        let parsed_params =
            ValType::resolve_variables(self.token.as_ref(), &self.params, &mut *context)?;
        let mut t = self.token.clone();
        t.set_params(&parsed_params)?;
        t.transform(input, Some(context))
    }

    #[cfg(feature = "buffered")]
    pub(crate) fn apply_buffered(
        &self,
        input: crate::api::TextBuffer,
        context: &mut GlobalExecutionContext,
    ) -> Result<crate::api::TextBuffer, TextForgeError> {
        if let Some(prepared) = &self.prepared {
            return prepared.transform_buffered(input, Some(context));
        }
        let params = ValType::resolve_variables(self.token.as_ref(), &self.params, context)?;
        let mut token = self.token.clone();
        token.set_params(&params)?;
        token.transform_buffered(input, Some(context))
    }

    pub fn to_text_line_resolved(
        &self,
        context: &mut GlobalExecutionContext,
    ) -> Result<String, TextForgeError> {
        let parsed_params =
            ValType::resolve_variables(self.token.as_ref(), &self.params, &mut *context)?;
        let mut t = self.token.clone();
        t.set_params(&parsed_params)?;

        Ok(t.to_textforge_line().into())
    }

    pub fn to_text_line_unresolved(&self) -> Result<String, TextForgeError> {
        crate::parser::structure::validate(self)?;
        // Serialize owned descriptions using the same registry as the parser.
        // Individual token Display methods are intended for diagnostics and do
        // not consistently quote empty text, whitespace, quotes or backslashes.
        let mut pending = vec![(self, false)];
        let mut output = Vec::<String>::new();
        while let Some((wrapper, visited)) = pending.pop() {
            if !visited {
                pending.push((wrapper, true));
                for parameter in &wrapper.params {
                    if let ValType::Literal(TextForgeParamTypes::Token(child)) = parameter {
                        pending.push((child, false));
                    }
                }
                continue;
            }
            let syntax = match TOKEN_TABLE.find((
                QuerySource::Identifier(wrapper.get_string_repr().into()),
                QueryTarget::Syntax,
            ))? {
                TargetValue::Syntax(syntax) => syntax,
                _ => {
                    return Err(TextForgeError::new(
                        TextForgeErrorCode::SerializationError("Missing instruction syntax".into()),
                        "to_text_line_unresolved",
                        wrapper.get_string_repr(),
                    ));
                }
            };
            let mut parameters = wrapper.params.iter();
            let mut fields = vec![wrapper.get_string_repr().to_owned()];
            for part in syntax.iter() {
                if let SyntaxToken::Literal(literal) = part.token {
                    fields.push(literal.to_owned());
                    continue;
                }
                let parameter = parameters.next().ok_or_else(|| {
                    TextForgeError::new(
                        TextForgeErrorCode::SerializationError(
                            "Missing instruction parameter".into(),
                        ),
                        "to_text_line_unresolved",
                        wrapper.get_string_repr(),
                    )
                })?;
                let value = match parameter {
                    ValType::Literal(TextForgeParamTypes::Token(_)) => {
                        output.pop().ok_or_else(|| {
                            TextForgeError::new(
                                TextForgeErrorCode::SerializationError(
                                    "Missing nested instruction".into(),
                                ),
                                "to_text_line_unresolved",
                                wrapper.get_string_repr(),
                            )
                        })?
                    }
                    ValType::Literal(TextForgeParamTypes::Usize(value)) => value.to_string(),
                    ValType::VarRef(name) | ValType::Literal(TextForgeParamTypes::VarRef(name)) => {
                        let reference = format!("{{{{{name}}}}}");
                        if !VAR_REF_RE.is_match(&reference) {
                            return Err(TextForgeError::new(
                                TextForgeErrorCode::SerializationError(
                                    "Variable name is not representable in text syntax; use bytecode export".into(),
                                ),
                                "to_text_line_unresolved", name.clone(),
                            ));
                        }
                        shell_words::quote(&reference).into_owned()
                    }
                    ValType::Literal(TextForgeParamTypes::String(value)) => {
                        if value.contains(['\n', '\r']) {
                            return Err(TextForgeError::new(
                                TextForgeErrorCode::SerializationError(
                                    "Text pipeline parameters cannot contain line breaks; use bytecode export for multiline values".into(),
                                ),
                                "to_text_line_unresolved", wrapper.get_string_repr(),
                            ));
                        }
                        shell_words::quote(value).into_owned()
                    }
                };
                fields.push(value);
            }
            output.push(fields.join(" "));
        }
        let text = output.pop().ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::SerializationError("Missing root instruction".into()),
                "to_text_line_unresolved",
                "",
            )
        })?;
        Ok(format!("{text};\n"))
    }
    #[cfg(feature = "bytecode")]
    pub fn to_bytecode_resolved(
        &self,
        context: &mut GlobalExecutionContext,
    ) -> Result<Vec<u8>, TextForgeError> {
        let parsed_params =
            ValType::resolve_variables(self.token.as_ref(), &self.params, &mut *context)?;
        let mut t = self.token.clone();
        t.set_params(&parsed_params)?;

        t.to_bytecode()
    }
    #[cfg(feature = "bytecode")]
    pub fn to_bytecode_unresolved(&self) -> Result<Vec<u8>, TextForgeError> {
        let mut unresolved_params: Vec<TextForgeParamTypes> = Vec::new();
        for val in self.params.iter() {
            match val {
                ValType::Literal(x) => unresolved_params.push(x.clone()),
                ValType::VarRef(name) => {
                    unresolved_params.push(TextForgeParamTypes::VarRef(name.to_string()));
                }
            }
        }

        // This persistence path must not resolve nested references. The public
        // context-aware parameter writer keeps its existing resolved semantics.
        let count = u8::try_from(unresolved_params.len()).map_err(|_| {
            TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError("Parameter count exceeds u8".into()),
                "to_bytecode_unresolved",
                "",
            )
        })?;
        let mut body = Vec::new();
        body.extend_from_slice(&self.get_opcode().to_be_bytes());
        body.push(count);
        for parameter in &unresolved_params {
            parameter.write_unresolved_instruction_param(&mut body)?;
        }
        let mut result = Vec::with_capacity(8 + body.len());
        result.extend_from_slice(&(body.len() as u64).to_be_bytes());
        result.extend_from_slice(&body);
        Ok(result)
    }
}

pub fn get_effective_param_types(expected: &[SyntaxDef]) -> Vec<SyntaxToken> {
    expected
        .iter()
        .filter_map(|ip| match ip.token {
            SyntaxToken::Literal(_) => None,
            other => Some(other),
        })
        .collect()
}

impl ValType {
    fn coerce_param(
        value: TextForgeParamTypes,
        expected: SyntaxToken,
        context_label: &str,
    ) -> Result<TextForgeParamTypes, TextForgeError> {
        match (value, expected) {
            (TextForgeParamTypes::String(v), SyntaxToken::String) => {
                Ok(TextForgeParamTypes::String(v))
            }
            (TextForgeParamTypes::Usize(v), SyntaxToken::Usize) => {
                Ok(TextForgeParamTypes::Usize(v))
            }
            (TextForgeParamTypes::Token(v), SyntaxToken::Token) => {
                Ok(TextForgeParamTypes::Token(v))
            }

            // Coerção elástica: string <-> número
            (TextForgeParamTypes::String(s), SyntaxToken::Usize) => s
                .parse::<usize>()
                .map(TextForgeParamTypes::Usize)
                .map_err(|_| {
                    TextForgeError::new(
                        TextForgeErrorCode::IncompatibleTypeError(
                            format!(
                                "'{}' contém \"{}\", que não é um número válido",
                                context_label, s
                            )
                            .into(),
                        ),
                        "resolve_variables()",
                        "",
                    )
                }),
            (TextForgeParamTypes::Usize(n), SyntaxToken::String) => {
                Ok(TextForgeParamTypes::String(n.to_string()))
            }

            // Token não tem conversão sensata a partir de String/Usize, e cobre
            // também o caso degenerado de um TextForgeParamTypes::VarRef não resolvido
            // chegando aqui (não deveria acontecer, mas é um catch-all seguro).
            (_, expected) => Err(TextForgeError::new(
                TextForgeErrorCode::IncompatibleTypeError(
                    format!(
                        "'{}' não pode ser usado como {:?} aqui",
                        context_label, expected
                    )
                    .into(),
                ),
                "resolve_variables()",
                "",
            )),
        }
    }
    fn resolve_variables(
        t: &dyn InstructionMethods,
        values: &[ValType],
        context: &mut GlobalExecutionContext,
    ) -> Result<Vec<TextForgeParamTypes>, TextForgeError> {
        let mut result = Vec::new();

        let query_result = TOKEN_TABLE.find((
            QuerySource::Identifier(t.get_string_repr().into()),
            QueryTarget::Syntax,
        ))?;

        let expected_params = get_effective_param_types(
            &(match query_result {
                TargetValue::Syntax(x) => x,
                _ => unreachable!("Unreachable Code"),
            }),
        );

        if values.len() != expected_params.len() {
            return Err(TextForgeError::new(
                TextForgeErrorCode::InvalidParameters("Param count mismatch".into()),
                "resolve_variables",
                format!(
                    "token={}, expected={}, got={}",
                    t.get_string_repr(),
                    expected_params.len(),
                    values.len()
                ),
            ));
        }

        for (i, value) in values.iter().enumerate() {
            match value {
                ValType::Literal(literal) => match (literal, expected_params[i]) {
                    // Único braço que muda: uma string literal passa pela
                    // interpolação antes de virar o parâmetro final. Isso
                    // cobre tanto o caso comum (nenhum {{}}/[[]] presente —
                    // interpolate() devolve a string inalterada pelo
                    // fast-path) quanto strings com N marcadores misturados.
                    (TextForgeParamTypes::String(s), SyntaxToken::String) => {
                        result.push(TextForgeParamTypes::String(interpolate(s, context)?));
                    }

                    (TextForgeParamTypes::Usize(_), SyntaxToken::Usize)
                    | (TextForgeParamTypes::Token(_), SyntaxToken::Token) => {
                        result.push(literal.clone());
                    }

                    _ => {
                        return Err(TextForgeError::new(
                            TextForgeErrorCode::IncompatibleTypeError(
                                "Literal type and required param type are different".into(),
                            ),
                            "resolve_variables()",
                            "",
                        ));
                    }
                },

                ValType::VarRef(name) => {
                    let variable = context.get_var(name)?;

                    let coerced = Self::coerce_param(
                        variable.value.clone().into(),
                        expected_params[i],
                        name,
                    )?;

                    result.push(coerced);
                }
            }
        }

        Ok(result)
    }
}

#[cfg(all(test, feature = "test_access"))]
mod preparation_test;

use std::{borrow::Cow, collections::HashMap};

use evalexpr::{ContextWithMutableVariables, HashMapContext, Value};

use crate::{
    context::execution_context::{
        GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues,
    },
    parser::params::TextForgeParamTypes,
    utils::{
        errors::{
            TextForgeError,
            TextForgeErrorCode::{self, InvalidExprError},
        },
        regexes::INTERP_RE,
    },
};

/// Builds a snapshot evalexpr context from the current variable table, so
/// expressions can reference stored variables by name.
///
/// Numeric conversion is a resolution-time concern only: everything in
/// `GlobalExecutionContext` stays `VarValues::String`/`Usize` before and
/// after `eval` runs — this function never mutates the context, it only
/// reads from it to build a disposable `HashMapContext`. Each variable is
/// tried as an `i64`; if it doesn't parse, it's passed to evalexpr as a
/// plain string. Floats aren't attempted on purpose: ATP's smallest
/// logical unit is a character, always represented as an integer, so
/// float support isn't worth the extra ambiguity.
///
/// Token variables are skipped: they aren't representable as evalexpr
/// values. Referencing one in an expression will surface as evalexpr's own
/// "identifier not found" error, which is an acceptable failure mode here.
pub fn build_eval_context(
    vars: &HashMap<String, VarEntry>,
) -> Result<HashMapContext, TextForgeError> {
    let mut ctx = HashMapContext::new();

    for (name, entry) in vars.iter() {
        let value = match &entry.value {
            VarValues::Usize(n) => Value::from_int(*n as i64),
            VarValues::String(s) => match s.parse::<i64>() {
                Ok(i) => Value::from_int(i),
                Err(_) => Value::from(s.clone()),
            },
            VarValues::Token(_) => {
                continue;
            }
        };

        ctx.set_value(name.clone(), value).map_err(|e| {
            TextForgeError::new(
                InvalidExprError(Cow::from(e.to_string())),
                Cow::from("eval.build_eval_context"),
                Cow::from(name.clone()),
            )
        })?;
    }

    Ok(ctx)
}

/// Converts an evalexpr result back into a plain string, without the
/// formatting evalexpr's own `Display` impl adds (e.g. wrapping strings in
/// literal quotes, since that impl is meant to print back valid expression
/// syntax, not a clean value).
pub fn value_to_plain_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Tuple(t) => {
            let parts: Vec<String> = t.iter().map(value_to_plain_string).collect();
            format!("({})", parts.join(", "))
        }
        Value::Empty => String::new(),
    }
}

/// Expande `{{var}}` e `[[expr]]` embutidos em `input`.
///
/// - `{{nome}}` → valor da variável `nome` no contexto.
/// - `[[expr]]` → resultado do evalexpr via `expr()`.
/// - Um delimitador precedido de `\` (`\{{`, `\}}`, `\[[`, `\]]`) imprime o
///   delimitador literalmente e consome o `\` — não dispara interpolação.
///   O `\` só tem esse efeito colado imediatamente num desses quatro pares;
///   em qualquer outro lugar (`\n`, um `\` avulso, `\{` de chave única) ele
///   não é tratado como escape e passa direto, sem significado especial.
/// - Qualquer `{{`, `}}`, `[[` ou `]]` que não feche um marcador válido
///   (nome começando com dígito, sem fechamento, etc.) é removido da saída
///   — a menos que tenha sido escapado, caso em que aparece literalmente.
///   Pra imprimir `{{nome}}` por completo, escape as DUAS pontas:
///   `\{{nome\}}`. Escapar só uma ponta deixa a outra "solta" — e ela
///   também é removida, por não formar marcador completo sozinha.
/// - Nada é re-escaneado: o valor resolvido de uma variável ou expressão
///   nunca volta a ser interpolado, mesmo que contenha `{{`/`[[` por
///   coincidência — evita loop em variáveis que se referenciam entre si.
pub fn interpolate(
    input: &str,
    context: &GlobalExecutionContext,
) -> Result<String, TextForgeError> {
    // Fast path: nenhum dos quatro delimitadores aparece de forma nenhuma
    // (completa, solta ou escapada) — nada a fazer.
    if !input.contains("{{")
        && !input.contains("}}")
        && !input.contains("[[")
        && !input.contains("]]")
    {
        return Ok(input.to_string());
    }

    let mut out = String::with_capacity(input.len());
    let mut last_end = 0;

    for caps in INTERP_RE.captures_iter(input) {
        let m = caps.get(0).unwrap();
        out.push_str(&input[last_end..m.start()]);

        if let Some(esc) = caps.name("esc") {
            // esc.as_str() é "\{{", "\}}", "\[[" ou "\]]" — descarta o \.
            out.push_str(&esc.as_str()[1..]);
        } else if let Some(var_name) = caps.name("var") {
            let variable = context.get_var(var_name.as_str())?;
            let value: TextForgeParamTypes = variable.value.clone().into();
            let as_string: String = value.try_into()?;
            out.push_str(&as_string);
        } else if let Some(expr_src) = caps.name("expr") {
            out.push_str(&expr(expr_src.as_str(), context)?);
        }
        // Nenhum grupo nomeado bateu: caiu no fallback de delimitador solto
        // (`{{`, `}}`, `[[` ou `]]` incompleto/inválido) — não empurra nada
        // pra `out`, ou seja, remove esse trecho.

        last_end = m.end();
    }

    out.push_str(&input[last_end..]);
    Ok(out)
}
/// Avalia uma expressão evalexpr contra as variáveis do contexto atual e
/// devolve o resultado como texto plano — sem as aspas literais que o
/// Display do evalexpr adiciona em Value::String. Usado pela instruction
/// `eval` e pela interpolação `[[expr]]` em strings do pipeline.
pub fn expr(expression: &str, context: &GlobalExecutionContext) -> Result<String, TextForgeError> {
    let eval_ctx = build_eval_context(context.get_all_vars())?;

    let result = evalexpr::eval_with_context(expression, &eval_ctx).map_err(|e| {
        TextForgeError::new(
            TextForgeErrorCode::InvalidExprError(Cow::from(e.to_string())),
            Cow::from("api::expr"),
            Cow::from(expression.to_string()),
        )
    })?;

    Ok(value_to_plain_string(&result))
}

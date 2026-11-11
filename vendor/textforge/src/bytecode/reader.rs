use std::{
    fs::OpenOptions,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use crate::{
    globals::table::{QuerySource, QueryTarget, SyntaxToken, TOKEN_TABLE, TargetValue},
    parser::{
        params::TextForgeParamTypes,
        resolve_var::{TokenWrapper, ValType},
    },
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_file_path,
    },
};

const PARAM_STRING: u32 = 0x01;
const PARAM_USIZE: u32 = 0x02;
const PARAM_TOKEN: u32 = 0x03;
const PARAM_VARREF: u32 = 0x04;

fn param_type_from_code(code: u32) -> Option<SyntaxToken> {
    match code {
        PARAM_STRING => Some(SyntaxToken::String),
        PARAM_USIZE => Some(SyntaxToken::Usize),
        PARAM_TOKEN => Some(SyntaxToken::Token),
        PARAM_VARREF => Some(SyntaxToken::String), // VarRef ocupa slot "string-like" no schema
        _ => None,
    }
}

/// Compatibilidade bytecode vs schema:
/// - VarRef (0x04) precisa ser compatível com o tipo esperado do parâmetro.
///   Como VarRef é "referência", ele deve ser aceito quando o schema espera String/Usize/Token.
///   A checagem fina acontece depois, em runtime, na resolve_variables().
fn bytecode_compatible(expected: &SyntaxToken, actual: &SyntaxToken, type_code: u32) -> bool {
    match (expected, actual, type_code) {
        // Tipos diretos
        (SyntaxToken::String, SyntaxToken::String, PARAM_STRING) => true,
        (SyntaxToken::Usize, SyntaxToken::Usize, PARAM_USIZE) => true,
        (SyntaxToken::Token, SyntaxToken::Token, PARAM_TOKEN) => true,

        // VarRef: pode aparecer onde o schema espera String/Usize/Token
        (SyntaxToken::String, SyntaxToken::String, PARAM_VARREF) => true,
        (SyntaxToken::Usize, SyntaxToken::String, PARAM_VARREF) => true,
        (SyntaxToken::Token, SyntaxToken::String, PARAM_VARREF) => true,

        _ => false,
    }
}

/// Lê exatamente N bytes do reader com erro bem formado.
fn read_exact<const N: usize>(
    reader: &mut BufReader<std::fs::File>,
    ctx: &'static str,
) -> Result<[u8; N], TextForgeError> {
    let mut buf = [0u8; N];
    reader.read_exact(&mut buf).map_err(|e| {
        TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError("Failed Reading Bytecode".into()),
            ctx,
            e.to_string(),
        )
    })?;
    Ok(buf)
}

/// Check the physical file bound before trusting a declared byte count.
fn ensure_available(
    reader: &mut BufReader<std::fs::File>,
    len: u64,
    ctx: &'static str,
) -> Result<(), TextForgeError> {
    let remaining = reader
        .get_ref()
        .metadata()
        .and_then(|metadata| {
            reader
                .stream_position()
                .map(|position| metadata.len().saturating_sub(position))
        })
        .map_err(|error| {
            TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError(
                    "Cannot determine remaining bytecode size".into(),
                ),
                ctx,
                error.to_string(),
            )
        })?;
    if len > remaining {
        return Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError(
                "Declared size exceeds remaining bytecode".into(),
            ),
            ctx,
            format!("declared={len}, remaining={remaining}"),
        ));
    }
    Ok(())
}

/// Reserve fallibly so malformed sizes cannot trigger an allocator abort.
fn byte_buffer(len: usize, ctx: &'static str) -> Result<Vec<u8>, TextForgeError> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(len).map_err(|error| {
        TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError("Cannot allocate bytecode buffer".into()),
            ctx,
            error.to_string(),
        )
    })?;
    Ok(buffer)
}

fn read_vec(
    reader: &mut BufReader<std::fs::File>,
    len: usize,
    ctx: &'static str,
) -> Result<Vec<u8>, TextForgeError> {
    ensure_available(reader, len as u64, ctx)?;
    let mut buf = byte_buffer(len, ctx)?;
    buf.resize(len, 0);
    reader.read_exact(&mut buf).map_err(|e| {
        TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError("Failed Reading Bytecode".into()),
            ctx,
            e.to_string(),
        )
    })?;
    Ok(buf)
}

/// Skip compatible instruction padding without allocating a padding-sized buffer.
fn skip_bytes(reader: &mut BufReader<std::fs::File>, len: u64) -> Result<(), TextForgeError> {
    const CONTEXT: &str = "read_bytecode_from_file(skip)";
    ensure_available(reader, len, CONTEXT)?;
    reader
        .stream_position()
        .and_then(|position| reader.seek(SeekFrom::Start(position + len)))
        .map_err(|error| {
            TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError("Failed skipping bytecode padding".into()),
                CONTEXT,
                error.to_string(),
            )
        })?;
    Ok(())
}

/// Converte bytes UTF-8 em String com erro padronizado.
fn utf8_string(bytes: &[u8], ctx: &'static str) -> Result<String, TextForgeError> {
    std::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .map_err(|e| {
            TextForgeError::new(
                TextForgeErrorCode::BytecodeParamParsingError(
                    "Failed parsing bytes to UTF8 string".into(),
                ),
                ctx,
                e.to_string(),
            )
        })
}

/// Decodifica um "param record" (já sem o u64 total_size externo),
/// retornando um ValType (Literal ou VarRef).
fn decode_param_record_to_valtype(param_record: &[u8]) -> Result<ValType, TextForgeError> {
    if param_record.len() < 8 {
        return Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError("Param record too small".into()),
            "decode_param_record_to_valtype",
            format!("len={}", param_record.len()),
        ));
    }

    let type_code = u32::from_be_bytes([
        param_record[0],
        param_record[1],
        param_record[2],
        param_record[3],
    ]);
    let payload_size = u32::from_be_bytes([
        param_record[4],
        param_record[5],
        param_record[6],
        param_record[7],
    ]) as usize;

    let remaining = param_record.len().saturating_sub(8);
    if payload_size > remaining {
        return Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError("Payload exceeds remaining".into()),
            "decode_param_record_to_valtype",
            format!("payload_size={}, remaining={}", payload_size, remaining),
        ));
    }

    let payload = &param_record[8..8 + payload_size];

    match type_code {
        PARAM_STRING => {
            let s = utf8_string(payload, "decode_param_record_to_valtype(PARAM_STRING)")?;
            Ok(ValType::Literal(TextForgeParamTypes::String(s)))
        }
        PARAM_USIZE => {
            if payload.len() != 8 {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParamParsingError(
                        "Invalid usize payload size".into(),
                    ),
                    "decode_param_record_to_valtype(PARAM_USIZE)",
                    format!("len={}", payload.len()),
                ));
            }
            let mut b = [0u8; 8];
            b.copy_from_slice(payload);
            Ok(ValType::Literal(TextForgeParamTypes::Usize(
                usize::from_be_bytes(b),
            )))
        }
        PARAM_VARREF => {
            let name = utf8_string(payload, "decode_param_record_to_valtype(PARAM_VARREF)")?;
            Ok(ValType::VarRef(name))
        }
        PARAM_TOKEN => {
            // Aqui payload é o "token payload":
            // opcode(u32) + param_count(u8) + params...
            // Seu TextForgeParamTypes::from_bytecode já sabe ler o layout do PARAM_TOKEN
            // (ele espera receber o param completo no formato param record ou no formato total_size+...,
            // então vamos reconstruir no formato "param completo": total(u64)+type+payload_size+payload.
            let total = 8u64 + 4 + 4 + (payload.len() as u64);
            let full_len = usize::try_from(total).map_err(|error| {
                TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "Token parameter size overflow".into(),
                    ),
                    "decode_param_record_to_valtype(PARAM_TOKEN)",
                    error.to_string(),
                )
            })?;
            let mut full_param =
                byte_buffer(full_len, "decode_param_record_to_valtype(PARAM_TOKEN)")?;
            full_param.extend_from_slice(&total.to_be_bytes());
            full_param.extend_from_slice(&PARAM_TOKEN.to_be_bytes());
            full_param.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            full_param.extend_from_slice(payload);

            let parsed = TextForgeParamTypes::from_bytecode(full_param)?;
            match parsed {
                TextForgeParamTypes::Token(tw) => {
                    Ok(ValType::Literal(TextForgeParamTypes::Token(tw)))
                }
                _ => Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "Expected Token wrapper from PARAM_TOKEN".into(),
                    ),
                    "decode_param_record_to_valtype(PARAM_TOKEN)",
                    "",
                )),
            }
        }
        _ => Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParamNotRecognized(
                format!("Param Bytecode Not Recognized 0x{:X}", type_code).into(),
            ),
            "decode_param_record_to_valtype",
            "",
        )),
    }
}

pub fn read_bytecode_from_file(path: &Path) -> Result<Vec<TokenWrapper>, TextForgeError> {
    check_file_path(path, Some("textforgebc"))?;

    let file = OpenOptions::new().read(true).open(path).map_err(|_| {
        TextForgeError::new(
            TextForgeErrorCode::FileOpeningError("Failed opening File".into()),
            "bytecode reader",
            format!("{:?}", path),
        )
    })?;

    let mut reader = BufReader::new(file);

    // --- header ---
    let expected_magic_number: [u8; 8] = [38, 235, 245, 8, 244, 137, 1, 179];
    let magic_number = read_exact::<8>(&mut reader, "read_bytecode_from_file(magic)")?;
    if magic_number != expected_magic_number {
        return Err(TextForgeError::new(
            TextForgeErrorCode::FileReadingError("Incompatible Magic Number with ATP".into()),
            "bytecode reader",
            "",
        ));
    }

    let protocol_version_bytes = read_exact::<8>(&mut reader, "read_bytecode_from_file(protocol)")?;
    let protocol_version = u64::from_be_bytes(protocol_version_bytes);
    if protocol_version != 1 {
        return Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError("Unsupported protocol version".into()),
            "read_bytecode_from_file",
            protocol_version.to_string(),
        ));
    }

    let instruction_count_bytes =
        read_exact::<4>(&mut reader, "read_bytecode_from_file(instr_count)")?;
    let instruction_count = u32::from_be_bytes(instruction_count_bytes);

    // Even a parameterless instruction needs size(8), opcode(4), count(1).
    // Validate plausibility without reserving memory from an untrusted count.
    ensure_available(
        &mut reader,
        u64::from(instruction_count) * 13,
        "read_bytecode_from_file(instr_count)",
    )?;
    let mut result: Vec<TokenWrapper> = Vec::new();

    // --- body ---
    for _ in 0..instruction_count {
        // instruction_total_size (u64)
        let instruction_total_size_bytes =
            read_exact::<8>(&mut reader, "read_bytecode_from_file(instr_total)")?;
        let instruction_total_size = u64::from_be_bytes(instruction_total_size_bytes);

        // mínimo interno = opcode(4) + param_count(1) = 5
        if instruction_total_size < 5 {
            return Err(TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError("Invalid instruction_total_size".into()),
                "read_bytecode_from_file",
                format!("instruction_total_size={}", instruction_total_size),
            ));
        }

        ensure_available(
            &mut reader,
            instruction_total_size,
            "read_bytecode_from_file(instr_total)",
        )?;

        // bytes consumidos dentro da instrução (sem contar os 8 do total_size)
        let mut consumed_in_instruction: u64 = 0;

        // opcode (u32)
        let opcode_bytes = read_exact::<4>(&mut reader, "read_bytecode_from_file(opcode)")?;
        let opcode = u32::from_be_bytes(opcode_bytes);
        consumed_in_instruction += 4;

        if opcode == 0 {
            return Err(TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError("Invalid instruction opcode (0)".into()),
                "read_bytecode_from_file",
                "opcode=0".to_string(),
            ));
        }

        // param_count (u8)
        let param_count_bytes =
            read_exact::<1>(&mut reader, "read_bytecode_from_file(param_count)")?;
        let param_count = param_count_bytes[0] as usize;
        consumed_in_instruction += 1;

        // schema
        let expected =
            match TOKEN_TABLE.find((QuerySource::Bytecode(opcode), QueryTarget::Syntax))? {
                TargetValue::Syntax(p) => p,
                _ => unreachable!("Invalid query result (Syntax)"),
            };

        let min_required = expected
            .iter()
            .filter(|p| !p.optional)
            .filter(|p| !matches!(p.token, SyntaxToken::Literal(_)))
            .count();
        let max_allowed = expected
            .iter()
            .filter(|p| !matches!(p.token, SyntaxToken::Literal(_)))
            .count();

        if param_count < min_required || param_count > max_allowed {
            return Err(TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError(
                    "Invalid param count for instruction".into(),
                ),
                "read_bytecode_from_file",
                format!(
                    "opcode=0x{:x} got={} expected=[min={}, max={}]",
                    opcode, param_count, min_required, max_allowed
                ),
            ));
        }

        // lê params como ValType e valida vs schema
        let mut params: Vec<ValType> = Vec::with_capacity(param_count);
        let mut expected_i = 0usize;

        for param_i in 0..param_count {
            // Param total size (u64)
            let param_total_size_bytes =
                read_exact::<8>(&mut reader, "read_bytecode_from_file(param_total)")?;
            let param_total_size = u64::from_be_bytes(param_total_size_bytes);
            consumed_in_instruction += 8;

            if param_total_size < 8 {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError("Invalid param_total_size".into()),
                    "read_bytecode_from_file",
                    format!("param_total_size={}", param_total_size),
                ));
            }

            // A parameter record must also fit inside its enclosing instruction.
            let record_size = param_total_size - 8;
            if record_size > instruction_total_size.saturating_sub(consumed_in_instruction) {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "Parameter exceeds instruction size".into(),
                    ),
                    "read_bytecode_from_file(param_record)",
                    format!("param_total_size={param_total_size}"),
                ));
            }
            let param_record_len = usize::try_from(record_size).map_err(|error| {
                TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError("Parameter size overflow".into()),
                    "read_bytecode_from_file(param_record)",
                    error.to_string(),
                )
            })?;
            let param_record = read_vec(
                &mut reader,
                param_record_len,
                "read_bytecode_from_file(param_record)",
            )?;
            consumed_in_instruction += param_record_len as u64;

            if param_record.len() < 4 {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError("Param record too short".into()),
                    "read_bytecode_from_file",
                    format!("opcode=0x{:x} param_index={}", opcode, param_i),
                ));
            }

            let type_code = u32::from_be_bytes([
                param_record[0],
                param_record[1],
                param_record[2],
                param_record[3],
            ]);
            let actual_pt = param_type_from_code(type_code).ok_or_else(|| {
                TextForgeError::new(
                    TextForgeErrorCode::BytecodeParamNotRecognized(
                        "Param Bytecode Not Recognized".into(),
                    ),
                    "read_bytecode_from_file",
                    format!("type_code=0x{:x}", type_code),
                )
            })?;

            // avança schema ignorando Literals e pulando optionals quando necessário
            while expected_i < expected.len() {
                let expected_pt = &expected[expected_i].token;

                if matches!(expected_pt, SyntaxToken::Literal(_)) {
                    expected_i += 1;
                    continue;
                }

                if bytecode_compatible(expected_pt, &actual_pt, type_code) {
                    break;
                }

                if expected[expected_i].optional {
                    expected_i += 1;
                    continue;
                }

                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError("Param type mismatch".into()),
                    "read_bytecode_from_file",
                    format!(
                        "opcode=0x{:x} param={} got={:?}(0x{:x}) expected={:?}",
                        opcode, param_i, actual_pt, type_code, expected_pt
                    ),
                ));
            }

            if expected_i >= expected.len() {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "Too many params (schema overflow)".into(),
                    ),
                    "read_bytecode_from_file",
                    format!("opcode=0x{:x} param_index={}", opcode, param_i),
                ));
            }

            let vt = decode_param_record_to_valtype(&param_record)?;
            params.push(vt);
            expected_i += 1;
        }

        // guardrail instruction_total_size
        if consumed_in_instruction != instruction_total_size {
            if consumed_in_instruction < instruction_total_size {
                skip_bytes(
                    &mut reader,
                    instruction_total_size - consumed_in_instruction,
                )?;
            } else {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "Instruction size mismatch (over-read)".into(),
                    ),
                    "read_bytecode_from_file",
                    format!(
                        "opcode=0x{:x} consumed={} expected={}",
                        opcode, consumed_in_instruction, instruction_total_size
                    ),
                ));
            }
        }

        // instancia o token "default" (sem params aplicados!)
        let token_ref =
            match TOKEN_TABLE.find((QuerySource::Bytecode(opcode), QueryTarget::Token))? {
                TargetValue::Token(t) => t,
                _ => unreachable!("Invalid query result (Token)"),
            };
        let token = token_ref.into_box();

        // Grow only for records actually decoded and report reservation failures.
        result.try_reserve(1).map_err(|error| {
            TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError("Cannot allocate instruction list".into()),
                "read_bytecode_from_file(instr_count)",
                error.to_string(),
            )
        })?;
        result.push(TokenWrapper::new(token, Some(params)));
    }

    Ok(result)
}

#[cfg(all(test, feature = "test_access"))]
mod tests {
    use super::*;

    fn header(count: u32) -> Vec<u8> {
        let mut bytes = vec![38, 235, 245, 8, 244, 137, 1, 179];
        bytes.extend_from_slice(&1u64.to_be_bytes());
        bytes.extend_from_slice(&count.to_be_bytes());
        bytes
    }

    fn parse(bytes: &[u8]) -> Result<Vec<TokenWrapper>, TextForgeError> {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pipeline.textforgebc");
        std::fs::write(&path, bytes).unwrap();
        read_bytecode_from_file(&path)
    }

    #[test]
    fn rejects_maximum_instruction_count_in_tiny_file() {
        let error = parse(&header(u32::MAX)).err().unwrap();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::BytecodeParsingError(_)
        ));
        assert_eq!(error.instruction, "read_bytecode_from_file(instr_count)");
    }

    #[test]
    fn rejects_count_exceeding_available_records() {
        let mut bytes = header(2);
        bytes.extend_from_slice(&5u64.to_be_bytes());
        bytes.extend_from_slice(&0x38u32.to_be_bytes());
        bytes.push(0);
        assert!(parse(&bytes).is_err());
    }

    #[test]
    fn empty_pipeline_remains_valid() {
        assert!(parse(&header(0)).unwrap().is_empty());
    }

    #[test]
    fn rejects_maximum_instruction_size_before_allocation() {
        let mut bytes = header(1);
        bytes.extend_from_slice(&u64::MAX.to_be_bytes());
        bytes.extend_from_slice(&0x38u32.to_be_bytes());
        bytes.push(0);
        assert!(matches!(
            parse(&bytes).err().unwrap().error_code,
            TextForgeErrorCode::BytecodeParsingError(_)
        ));
    }

    #[test]
    fn rejects_parameter_size_exceeding_instruction() {
        let mut bytes = header(1);
        bytes.extend_from_slice(&21u64.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&u64::MAX.to_be_bytes());
        bytes.extend_from_slice(&PARAM_STRING.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        assert!(matches!(
            parse(&bytes).err().unwrap().error_code,
            TextForgeErrorCode::BytecodeParsingError(_)
        ));
    }

    #[test]
    fn preserves_padding_and_next_instruction() {
        let mut bytes = header(2);
        bytes.extend_from_slice(&8u64.to_be_bytes());
        bytes.extend_from_slice(&0x38u32.to_be_bytes());
        bytes.extend_from_slice(&[0, 7, 8, 9]);
        bytes.extend_from_slice(&5u64.to_be_bytes());
        bytes.extend_from_slice(&0x38u32.to_be_bytes());
        bytes.push(0);
        assert_eq!(parse(&bytes).unwrap().len(), 2);
    }

    #[test]
    fn unicode_parameters_still_roundtrip() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pipeline.textforgebc");
        std::fs::write(&path, []).unwrap();
        let token = crate::text::reader::read_from_text("atb \"👩🏽‍💻 é\";").unwrap();
        crate::bytecode::writer::write_bytecode_to_file(&path, vec![token]).unwrap();
        let tokens = read_bytecode_from_file(&path).unwrap();
        let mut context = crate::context::execution_context::GlobalExecutionContext::new();
        assert_eq!(
            tokens[0].apply_token("x".into(), &mut context).unwrap(),
            "👩🏽‍💻 éx"
        );
    }
}

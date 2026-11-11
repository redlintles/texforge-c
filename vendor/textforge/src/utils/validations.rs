use std::{borrow::Cow, path::Path};

use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

pub fn check_file_path(path: &Path, ext: Option<&str>) -> Result<(), TextForgeError> {
    // canonicalize() exige que o path exista; isso é ok aqui porque queremos validar um arquivo real.
    let path = path.canonicalize().map_err(|e| {
        TextForgeError::new(
            TextForgeErrorCode::ValidationError("Path canonicalization failed".into()),
            Cow::Borrowed("canonicalize"),
            format!("{:?} - {}", path, e),
        )
    })?;

    check_pipeline_extension(&path, ext)?;

    // Diretório pai (redundante depois do canonicalize, mas mantém a intenção explícita)
    let parent = path.parent().ok_or_else(|| {
        TextForgeError::new(
            TextForgeErrorCode::ValidationError("Path has no parent directory".into()),
            Cow::Borrowed("check_file_path"),
            path.to_string_lossy().to_string(),
        )
    })?;

    if !parent.exists() {
        return Err(TextForgeError::new(
            TextForgeErrorCode::ValidationError("Parent directory does not exist".into()),
            Cow::Borrowed("check_file_path"),
            parent.to_string_lossy().to_string(),
        ));
    }

    if path.is_dir() {
        return Err(TextForgeError::new(
            TextForgeErrorCode::ValidationError("Path is a directory, not a file".into()),
            Cow::Borrowed("check_file_path"),
            path.to_string_lossy().to_string(),
        ));
    }

    Ok(())
}

/// Validate format extensions independently from whether the destination exists.
fn check_pipeline_extension(path: &Path, expected: Option<&str>) -> Result<(), TextForgeError> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::ValidationError("No file extension found".into()),
                "check_file_path",
                path.display().to_string(),
            )
        })?;
    let accepted = match expected {
        Some("textforge" | "tft") => matches!(extension, "textforge" | "tft"),
        Some("textforgebc" | "tfb") => matches!(extension, "textforgebc" | "tfb"),
        Some(expected) => extension == expected,
        None => matches!(extension, "textforge" | "tft" | "textforgebc" | "tfb"),
    };
    if !accepted {
        let supported = match expected {
            Some("textforge" | "tft") => ".textforge or .tft",
            Some("textforgebc" | "tfb") => ".textforgebc or .tfb",
            Some(extension) => extension,
            None => ".textforge, .tft, .textforgebc or .tfb",
        };
        return Err(TextForgeError::new(
            TextForgeErrorCode::ValidationError(
                format!("Unsupported pipeline extension '.{extension}'; expected {supported}")
                    .into(),
            ),
            "check_file_path",
            path.display().to_string(),
        ));
    }
    Ok(())
}

/// Prepare a write destination without requiring a pre-existing output file.
/// Parent directories are created only after callers have validated their data.
pub(crate) fn prepare_output_path(path: &Path, instruction: &str) -> Result<(), TextForgeError> {
    if path.file_name().is_none() || path.is_dir() {
        return Err(TextForgeError::new(
            TextForgeErrorCode::ValidationError("Output path must name a file".into()),
            instruction.to_owned(),
            path.display().to_string(),
        ));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            TextForgeError::new(
                TextForgeErrorCode::FileOpeningError(error.to_string().into()),
                instruction.to_owned(),
                path.display().to_string(),
            )
        })?;
    }
    Ok(())
}

/// Check a pipeline output's format before creating its parent directories.
pub(crate) fn check_pipeline_output_path(
    path: &Path,
    extension: Option<&str>,
) -> Result<(), TextForgeError> {
    check_pipeline_extension(path, extension)?;
    prepare_output_path(path, "pipeline output")
}

/// Valida limites de chunk com índices em *contagem de chars* (não bytes),
/// assumindo semântica inclusiva em tokens (start..=end).
///
/// Regras:
/// - Sempre exige `start_index < end_index`
/// - Se `check_against` for Some(text):
///     - exige `start_index` e `end_index` dentro de `0..text.chars().count()`
///
/// Obs: se você quiser permitir chunks de 1 caractere (start==end), mude a regra.
pub fn check_chunk_bound_indexes(
    start_index: usize,
    end_index: usize,
    check_against: Option<&str>,
) -> Result<(), TextForgeError> {
    // regra estrutural (independente do texto)
    if start_index >= end_index {
        let fmt_err = format!("check_chunk_bound_indexes {} {};", start_index, end_index);
        return Err(TextForgeError::new(
            TextForgeErrorCode::InvalidIndex("Start index must be smaller than end index".into()),
            Cow::Owned(fmt_err),
            format!("Start Index: {}, End Index: {}", start_index, end_index),
        ));
    }

    if let Some(text) = check_against {
        let total_chars = text.chars().count();

        // start precisa existir
        if !(0..total_chars).contains(&start_index) {
            return Err(TextForgeError::new(
                TextForgeErrorCode::IndexOutOfRange(
                    "Start index does not exist in current string!".into(),
                ),
                Cow::Borrowed("check_chunk_bound_indexes"),
                text.to_string(),
            ));
        }

        // end também precisa existir
        if !(0..total_chars).contains(&end_index) {
            return Err(TextForgeError::new(
                TextForgeErrorCode::IndexOutOfRange(
                    "End index does not exist in current string!".into(),
                ),
                Cow::Borrowed("check_chunk_bound_indexes"),
                text.to_string(),
            ));
        }
    }

    Ok(())
}

pub fn check_index_against_input(index: usize, input: &str) -> Result<(), TextForgeError> {
    let character_count = input.chars().count();
    if !(0..character_count).contains(&index) {
        return Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange(
                format!(
                    "Index {} does not exist for {}, only indexes between 0-{} are allowed!",
                    index,
                    input,
                    character_count.saturating_sub(1)
                )
                .into(),
            ),
            Cow::Borrowed("check_index_against_input"),
            input.to_string(),
        ));
    }

    Ok(())
}

pub fn check_index_against_words(index: usize, input: &str) -> Result<(), TextForgeError> {
    let word_count = input.split_whitespace().count();

    if word_count == 0 {
        return Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange("Input has no words".into()),
            Cow::Borrowed("check_index_against_words"),
            input.to_string(),
        ));
    }

    if !(0..word_count).contains(&index) {
        return Err(
            TextForgeError::new(
                TextForgeErrorCode::IndexOutOfRange(
                    format!(
                        "Word index {} does not exist for input, only indexes between 0-{} are allowed!",
                        index,
                        word_count.saturating_sub(1)
                    ).into()
                ),
                Cow::Borrowed("check_index_against_words"),
                input.to_string()
            )
        );
    }

    Ok(())
}

pub fn check_vec_len<T>(
    v: &[T],
    expected_len: usize,
    ctx: impl Into<Cow<'static, str>>,
    payload: impl Into<String>,
) -> Result<(), TextForgeError> {
    if v.len() == expected_len {
        Ok(())
    } else {
        Err(TextForgeError::new(
            TextForgeErrorCode::InvalidArgumentNumber(
                format!(
                    "Only {} arguments are allowed for this instruction, passed {}",
                    expected_len,
                    v.len()
                )
                .into(),
            ),
            ctx.into(),
            payload.into(),
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_tmp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut p = std::env::temp_dir();
        p.push(format!("{}_{}_{}", prefix, std::process::id(), nanos));
        p
    }

    #[cfg(test)]
    mod check_vec_len_tests {
        use super::check_vec_len;

        #[test]
        fn check_vec_len_ok_for_string_vec() {
            let v = vec!["a".to_string(), "b".to_string(), "c".to_string()];

            let result = check_vec_len(&v, 3, "check_vec_len", v.join(" "));

            assert!(result.is_ok());
        }

        #[test]
        fn check_vec_len_err_for_string_vec() {
            let v = vec!["a".to_string(), "b".to_string()];

            let result = check_vec_len(&v, 3, "check_vec_len", v.join(" "));

            assert!(result.is_err());

            // Se TextForgeError tiver Display, isso geralmente vira uma checagem bem útil
            let err = result.unwrap_err();
            let msg = format!("{err}");

            // Não assumo a estrutura interna do TextForgeError, então valido por mensagem
            assert!(
                msg.contains("Only 3 arguments are allowed") || msg.contains("Only 3 arguments")
            );
            assert!(msg.contains("passed 2") || msg.contains("passed 2"));
        }

        #[test]
        fn check_vec_len_is_generic_for_any_type() {
            let v: Vec<u32> = vec![10, 20, 30, 40];

            let ok = check_vec_len(v.as_slice(), 4, "check_vec_len(u32)", "payload-u32");
            assert!(ok.is_ok());

            let err = check_vec_len(v.as_slice(), 3, "check_vec_len(u32)", "payload-u32");
            assert!(err.is_err());
        }

        #[cfg(feature = "bytecode")]
        mod bytecode_tests {
            use super::check_vec_len;
            use crate::parser::params::TextForgeParamTypes;

            #[test]
            fn check_vec_len_ok_for_textforgeparamtypes_vec() {
                let instruction: Vec<TextForgeParamTypes> = vec![
                    TextForgeParamTypes::Usize(1),
                    TextForgeParamTypes::String("abc".to_string()),
                ];

                let result = check_vec_len(
                    instruction.as_slice(),
                    2,
                    "check_vec_len(bytecode)",
                    "<bytecode>",
                );

                assert!(result.is_ok());
            }

            #[test]
            fn check_vec_len_err_for_textforgeparamtypes_vec() {
                let instruction: Vec<TextForgeParamTypes> = vec![
                    TextForgeParamTypes::Usize(1),
                    TextForgeParamTypes::String("abc".to_string()),
                ];

                let result = check_vec_len(
                    instruction.as_slice(),
                    1,
                    "check_vec_len(bytecode)",
                    "<bytecode>",
                );

                assert!(result.is_err());

                let err = result.unwrap_err();
                let msg = format!("{err}");
                assert!(
                    msg.contains("Only 1 arguments are allowed") || msg.contains("Only 1 argument")
                );
                assert!(msg.contains("passed 2") || msg.contains("passed 2"));
            }
        }
    }

    mod check_file_path_tests {
        use super::*;

        #[test]
        fn ok_for_existing_file_with_default_ext_textforge() {
            let dir = unique_tmp_dir("textforge_check_file_path_ok");
            fs::create_dir_all(&dir).unwrap();

            let file = dir.join("input.textforge");
            fs::write(&file, "banana").unwrap();

            assert!(check_file_path(&file, None).is_ok());

            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn ok_for_existing_file_with_custom_ext() {
            let dir = unique_tmp_dir("textforge_check_file_path_custom_ext");
            fs::create_dir_all(&dir).unwrap();

            let file = dir.join("input.textforgebc");
            fs::write(&file, "banana").unwrap();

            assert!(check_file_path(&file, None).is_ok());

            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn err_for_wrong_extension() {
            let dir = unique_tmp_dir("textforge_check_file_path_wrong_ext");
            fs::create_dir_all(&dir).unwrap();

            let file = dir.join("input.txt");
            fs::write(&file, "banana").unwrap();

            let r = check_file_path(&file, None);
            assert!(r.is_err());

            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn err_for_missing_extension() {
            let dir = unique_tmp_dir("textforge_check_file_path_no_ext");
            fs::create_dir_all(&dir).unwrap();

            let file = dir.join("input");
            fs::write(&file, "banana").unwrap();

            let r = check_file_path(&file, None);
            assert!(r.is_err());

            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn err_for_directory_path() {
            let dir = unique_tmp_dir("textforge_check_file_path_is_dir");
            fs::create_dir_all(&dir).unwrap();

            // canonicalize vai funcionar, mas is_dir deve falhar
            let r = check_file_path(&dir, None);
            assert!(r.is_err());

            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn err_for_nonexistent_path_canonicalize_fails() {
            let dir = unique_tmp_dir("textforge_check_file_path_missing");
            // não cria diretório/arquivo
            let file = dir.join("missing.textforge");

            let r = check_file_path(&file, None);
            assert!(r.is_err());
        }
    }

    mod check_chunk_bound_indexes_tests {
        use super::*;

        #[test]
        fn ok_when_start_lt_end_and_no_text() {
            assert!(check_chunk_bound_indexes(1, 3, None).is_ok());
        }

        #[test]
        fn err_when_start_ge_end_no_text() {
            assert!(check_chunk_bound_indexes(3, 3, None).is_err());
            assert!(check_chunk_bound_indexes(4, 2, None).is_err());
        }

        #[test]
        fn ok_when_indices_in_range_with_text() {
            // "banàna" tem 6 chars
            let text = "banàna";
            assert!(check_chunk_bound_indexes(1, 4, Some(text)).is_ok());
        }

        #[test]
        fn err_when_start_out_of_range_with_text() {
            let text = "abc"; // 3 chars: idx 0..2
            assert!(check_chunk_bound_indexes(3, 4, Some(text)).is_err());
        }

        #[test]
        fn err_when_end_out_of_range_with_text() {
            let text = "abc"; // 3 chars: idx 0..2
            assert!(check_chunk_bound_indexes(1, 3, Some(text)).is_err());
        }

        #[test]
        fn err_when_start_ge_end_with_text() {
            let text = "abcdef";
            assert!(check_chunk_bound_indexes(2, 2, Some(text)).is_err());
            assert!(check_chunk_bound_indexes(3, 1, Some(text)).is_err());
        }
    }

    mod check_index_against_input_tests {
        use super::*;

        #[test]
        fn ok_when_index_in_range() {
            assert!(check_index_against_input(0, "banana").is_ok());
            assert!(check_index_against_input(5, "banana").is_ok());
        }

        #[test]
        fn err_when_index_out_of_range() {
            assert!(check_index_against_input(6, "banana").is_err());
            assert!(check_index_against_input(999, "banana").is_err());
        }

        #[test]
        fn err_when_input_empty() {
            assert!(check_index_against_input(0, "").is_err());
        }
    }

    mod check_index_against_words_tests {
        use super::*;

        #[test]
        fn ok_when_index_in_range() {
            assert!(check_index_against_words(0, "banana laranja").is_ok());
            assert!(check_index_against_words(1, "banana laranja").is_ok());
        }

        #[test]
        fn ok_with_multiple_spaces_and_newlines() {
            assert!(check_index_against_words(2, "banana   laranja\ncheia\tde  canja").is_ok());
        }

        #[test]
        fn err_when_index_out_of_range() {
            assert!(check_index_against_words(2, "banana laranja").is_err());
            assert!(check_index_against_words(999, "banana laranja").is_err());
        }

        #[test]
        fn err_when_input_has_no_words() {
            assert!(check_index_against_words(0, "").is_err());
            assert!(check_index_against_words(0, "     \n\t  ").is_err());
        }

        #[test]
        fn supports_unicode_words() {
            // split_whitespace funciona bem com unicode; aqui garante que o contador não quebra
            assert!(check_index_against_words(1, "banàna maçã").is_ok());
            assert!(check_index_against_words(2, "banàna maçã").is_err());
        }
    }
}

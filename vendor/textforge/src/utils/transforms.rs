use crate::utils::errors::TextForgeError;
use std::borrow::Cow;

pub fn string_to_usize(chunk: &str) -> Result<usize, TextForgeError> {
    let mut parsed_chunk = String::from(chunk);
    if chunk.ends_with(";") {
        parsed_chunk.pop();
    }

    match parsed_chunk.parse() {
        Ok(v) => Ok(v),
        Err(_) => {
            let str_chunk = chunk.to_string();
            Err(TextForgeError::new(
                super::errors::TextForgeErrorCode::TextParsingError(
                    "String to usize Parsing failed".into(),
                ),
                Cow::Owned(str_chunk),
                chunk.to_string(),
            ))
        }
    }
}

pub fn capitalize(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    crate::utils::allocation::capitalize_into(&mut output, input);
    output
}

pub fn extend_string(input: &str, max_len: usize) -> String {
    input.chars().cycle().take(max_len).collect()
}

pub fn get_safe_utf8_char_index(index: usize, input: &str) -> Result<usize, TextForgeError> {
    input
        .char_indices()
        .nth(index)
        .map(|(i, _)| i)
        .ok_or_else(|| {
            TextForgeError::new(
                super::errors::TextForgeErrorCode::IndexOutOfRange("".into()),
                Cow::Borrowed("Get safe utf-8 char index"),
                input.to_string(),
            )
        })
}

// tests for utils/string utils (or wherever these fns live)
//
// Observação: estes testes assumem que:
// - o feature flag "test_access" existe no seu Cargo.toml
// - as funções estão acessíveis via `crate::utils::...` (ajuste o `use` se o caminho real for outro)
// - TextForgeError expõe `code` (ou getter equivalente) para checar o tipo do erro.
//   Se o seu TextForgeError NÃO expõe o código publicamente, mantive os asserts de erro de forma que
//   você consiga facilmente trocar para `format!("{err:?}")` / `.to_string()` / etc.

#[cfg(feature = "test_access")]
mod test_access {
    // Ajuste este path se as funções estiverem em outro módulo.
    // Ex: use crate::utils::text::{string_to_usize, capitalize, extend_string, get_safe_utf8_char_index};

    #[cfg(test)]
    mod string_to_usize_tests {
        use crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        use crate::utils::transforms::string_to_usize;

        #[test]
        fn parses_plain_integer() -> Result<(), TextForgeError> {
            assert_eq!(string_to_usize("0")?, 0);
            assert_eq!(string_to_usize("42")?, 42);
            Ok(())
        }

        #[test]
        fn parses_integer_with_trailing_semicolon() -> Result<(), TextForgeError> {
            assert_eq!(string_to_usize("7;")?, 7);
            assert_eq!(string_to_usize("12345;")?, 12345);
            Ok(())
        }

        #[test]
        fn rejects_non_numeric() {
            let err = string_to_usize("abc").unwrap_err();

            // Se TextForgeError tiver `code` público:
            assert!(
                matches!(err.error_code, TextForgeErrorCode::TextParsingError(_)),
                "expected TextParsingError, got: {err:?}"
            );
        }

        #[test]
        fn rejects_semicolon_only() {
            let err = string_to_usize(";").unwrap_err();

            assert!(
                matches!(err.error_code, TextForgeErrorCode::TextParsingError(_)),
                "expected TextParsingError, got: {err:?}"
            );
        }

        #[test]
        fn rejects_negative_number_for_usize() {
            let err = string_to_usize("-1").unwrap_err();

            assert!(
                matches!(err.error_code, TextForgeErrorCode::TextParsingError(_)),
                "expected TextParsingError, got: {err:?}"
            );
        }

        #[test]
        fn rejects_with_whitespace_if_not_trimmed_by_caller() {
            // Sua função não faz trim; então isto deve falhar.
            let err = string_to_usize(" 10").unwrap_err();

            assert!(
                matches!(err.error_code, TextForgeErrorCode::TextParsingError(_)),
                "expected TextParsingError, got: {err:?}"
            );
        }

        #[test]
        fn rejects_overflow() {
            // usize::MAX + "1" de forma portável: usamos um número bem grande.
            // (Qualquer coisa absurdamente grande deve dar erro de parse por overflow.)
            let err = string_to_usize("999999999999999999999999999999999999999").unwrap_err();

            assert!(
                matches!(err.error_code, TextForgeErrorCode::TextParsingError(_)),
                "expected TextParsingError, got: {err:?}"
            );
        }
    }
    #[cfg(test)]
    mod capitalize_tests {
        use crate::utils::transforms::capitalize;

        #[test]
        fn capitalizes_first_char_ascii() {
            assert_eq!(capitalize("banana"), "Banana");
            assert_eq!(capitalize("a"), "A");
            assert_eq!(capitalize("Already"), "Already");
        }

        #[test]
        fn returns_empty_for_empty_input() {
            assert_eq!(capitalize(""), "");
        }

        #[test]
        fn preserves_rest_of_string_without_lowercasing() {
            // A função só uppercasa o primeiro char; o resto fica como está.
            assert_eq!(capitalize("bANANA"), "BANANA");
            assert_eq!(capitalize("bAnAnA"), "BAnAnA");
        }

        #[test]
        fn handles_utf8_first_char() {
            // Exercita unicode no primeiro char (não só ASCII).
            assert_eq!(capitalize("área"), "Área");
            assert_eq!(capitalize("çasa"), "Çasa");
        }

        #[test]
        fn handles_case_where_uppercase_expands_to_multiple_chars() {
            // Em Unicode, alguns caracteres podem virar mais de 1 char ao uppercasing.
            // Ex clássico: "ß" -> "SS" (depende do mapeamento Unicode do Rust, mas em geral é isso).
            let out = capitalize("ßeta");
            assert!(out.starts_with("SS") || out.starts_with("ẞ"), "got: {out}");
        }
    }
    #[cfg(test)]
    mod extend_string_tests {
        use crate::utils::transforms::extend_string;

        #[test]
        fn returns_empty_if_input_empty() {
            assert_eq!(extend_string("", 10), "");
        }

        #[test]
        fn returns_empty_if_max_len_zero() {
            assert_eq!(extend_string("abc", 0), "");
        }

        #[test]
        fn repeats_and_truncates_ascii() {
            assert_eq!(extend_string("ab", 1), "a");
            assert_eq!(extend_string("ab", 2), "ab");
            assert_eq!(extend_string("ab", 3), "aba");
            assert_eq!(extend_string("ab", 4), "abab");
            assert_eq!(extend_string("ab", 5), "ababa");
        }

        #[test]
        fn respects_char_count_not_byte_count_utf8() {
            // "á" é multibyte, mas conta como 1 char.
            assert_eq!(extend_string("á", 1), "á");
            assert_eq!(extend_string("á", 3), "ááá");

            // Emoji também é multibyte.
            assert_eq!(extend_string("🔥", 2), "🔥🔥");
        }

        #[test]
        fn works_with_multi_char_input_utf8() {
            assert_eq!(extend_string("áβ", 1), "á");
            assert_eq!(extend_string("áβ", 2), "áβ");
            assert_eq!(extend_string("áβ", 3), "áβá");
            assert_eq!(extend_string("áβ", 4), "áβáβ");
        }

        #[test]
        fn output_len_equals_max_len_in_chars() {
            let s = extend_string("abc", 10);
            assert_eq!(s.chars().count(), 10);

            let s = extend_string("🔥", 7);
            assert_eq!(s.chars().count(), 7);
        }
    }

    #[cfg(test)]
    mod get_safe_utf8_char_index_tests {
        use crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        use crate::utils::transforms::get_safe_utf8_char_index;

        #[test]
        fn returns_correct_byte_index_ascii() -> Result<(), TextForgeError> {
            let input = "banana";
            assert_eq!(get_safe_utf8_char_index(0, input)?, 0); // 'b'
            assert_eq!(get_safe_utf8_char_index(1, input)?, 1); // 'a'
            assert_eq!(get_safe_utf8_char_index(5, input)?, 5); // last 'a'
            Ok(())
        }

        #[test]
        fn returns_correct_byte_index_utf8() -> Result<(), TextForgeError> {
            // "á" ocupa 2 bytes em UTF-8.
            let input = "ábc";
            // indices de chars: 0:'á'(byte 0), 1:'b'(byte 2), 2:'c'(byte 3)
            assert_eq!(get_safe_utf8_char_index(0, input)?, 0);
            assert_eq!(get_safe_utf8_char_index(1, input)?, 2);
            assert_eq!(get_safe_utf8_char_index(2, input)?, 3);
            Ok(())
        }

        #[test]
        fn returns_correct_byte_index_with_emoji() -> Result<(), TextForgeError> {
            // "🔥" é 4 bytes.
            let input = "a🔥b";
            // 0:'a'(0), 1:'🔥'(1), 2:'b'(5)
            assert_eq!(get_safe_utf8_char_index(0, input)?, 0);
            assert_eq!(get_safe_utf8_char_index(1, input)?, 1);
            assert_eq!(get_safe_utf8_char_index(2, input)?, 5);
            Ok(())
        }

        #[test]
        fn errors_when_index_out_of_range() {
            let input = "abc";
            let err = get_safe_utf8_char_index(3, input).unwrap_err(); // len == 3, último índice válido == 2

            assert!(
                matches!(err.error_code, TextForgeErrorCode::IndexOutOfRange(_)),
                "expected IndexOutOfRange, got: {err:?}"
            );
        }

        #[test]
        fn errors_on_empty_input_for_any_index() {
            let input = "";
            let err = get_safe_utf8_char_index(0, input).unwrap_err();

            assert!(
                matches!(err.error_code, TextForgeErrorCode::IndexOutOfRange(_)),
                "expected IndexOutOfRange, got: {err:?}"
            );
        }
    }
}

use std::borrow::Cow;

use unicode_normalization::{
    IsNormalized, UnicodeNormalization, is_nfc_quick, is_nfd_quick, is_nfkc_quick, is_nfkd_quick,
};

use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_vec_len,
    },
};

/// UNORM — normalize a complete Unicode string to NFC, NFD, NFKC or NFKD.
///
/// Mode names are lowercase and validated before execution. Unresolved parameter
/// expressions can be retained for pipeline construction and serialization. Already-normalized
/// input retains its original ownership. No context or per-document state is needed.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::unorm::Unorm};
/// let token = Unorm::new("nfc").unwrap();
/// assert_eq!(token.transform("e\u{301}".into(), None).unwrap(), "é");
/// assert!(Unorm::new("unknown").is_err());
/// ```
#[derive(Clone)]
pub struct Unorm {
    mode: Option<&'static str>,
    params: Vec<TextForgeParamTypes>,
}

impl Unorm {
    /// Construct a normalizer for `nfc`, `nfd`, `nfkc` or `nfkd`.
    /// Unresolved interpolation is retained for export; direct execution requires a resolved mode.
    ///
    /// ```
    /// use textforge::tokens::{InstructionMethods, transforms::unorm::Unorm};
    /// let token = Unorm::new("nfkc").unwrap();
    /// assert_eq!(token.transform("Ａﬁ²".into(), None).unwrap(), "Afi2");
    /// ```
    pub fn new(mode: &str) -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[TextForgeParamTypes::String(mode.to_owned())])?;
        Ok(token)
    }
}

impl Default for Unorm {
    fn default() -> Self {
        Self {
            mode: Some("nfc"),
            params: vec![TextForgeParamTypes::String("nfc".to_owned())],
        }
    }
}

impl InstructionMethods for Unorm {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }

    fn get_string_repr(&self) -> &'static str {
        "unorm"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        let mode = match self.params.first() {
            Some(TextForgeParamTypes::String(mode)) => mode.as_str(),
            _ => "nfc",
        };
        format!("unorm {};\n", shell_words::quote(mode)).into()
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(
            params,
            1,
            "unorm",
            "Expected one Unicode normalization mode",
        )?;
        let mode = match &params[0] {
            TextForgeParamTypes::String(value) => match value.as_str() {
                "nfc" => Some("nfc"),
                "nfd" => Some("nfd"),
                "nfkc" => Some("nfkc"),
                "nfkd" => Some("nfkd"),
                value if value.contains("{{") || value.contains("[[") => None,
                _ => {
                    return Err(TextForgeError::new(
                        TextForgeErrorCode::InvalidParameters(
                            "Expected nfc, nfd, nfkc or nfkd".into(),
                        ),
                        "unorm",
                        value.clone(),
                    ));
                }
            },
            _ => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters(
                        "Unicode normalization mode must be text".into(),
                    ),
                    "unorm",
                    format!("{:?}", params[0]),
                ));
            }
        };
        self.mode = mode;
        self.params = params.to_vec();
        Ok(())
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mode = self.mode.ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::InvalidParameters(
                    "Unresolved Unicode normalization mode".into(),
                ),
                "unorm",
                match self.params.first() {
                    Some(TextForgeParamTypes::String(mode)) => mode.clone(),
                    _ => String::new(),
                },
            )
        })?;
        let quick = match mode {
            "nfd" => is_nfd_quick(input.chars()),
            "nfkc" => is_nfkc_quick(input.chars()),
            "nfkd" => is_nfkd_quick(input.chars()),
            _ => is_nfc_quick(input.chars()),
        };
        if quick == IsNormalized::Yes {
            return Ok(input);
        }
        let output = match mode {
            "nfd" => collect_if_changed(&input, input.nfd()),
            "nfkc" => collect_if_changed(&input, input.nfkc()),
            "nfkd" => collect_if_changed(&input, input.nfkd()),
            _ => collect_if_changed(&input, input.nfc()),
        };
        Ok(match output {
            Some(output) => Cow::Owned(output),
            None => input,
        })
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x41
    }

    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}

/// Compare the normalized iterator with the source until the first difference.
/// Retain the equal byte prefix and consume the remaining iterator only once.
/// This avoids normalizing twice after a QuickCheck `Maybe` result.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::unorm::Unorm};
/// assert_eq!(Unorm::new("nfc").unwrap().transform("e\u{301}".into(), None).unwrap(), "é");
/// ```
fn collect_if_changed(input: &str, mut normalized: impl Iterator<Item = char>) -> Option<String> {
    let mut source = input.chars();
    let mut equal_bytes = 0;
    for character in normalized.by_ref() {
        if source.next() == Some(character) {
            equal_bytes += character.len_utf8();
        } else {
            let mut output = String::with_capacity(input.len());
            output.push_str(&input[..equal_bytes]);
            output.push(character);
            output.extend(normalized);
            return Some(output);
        }
    }
    (equal_bytes != input.len()).then(|| input[..equal_bytes].to_owned())
}

#[cfg(test)]
mod test;

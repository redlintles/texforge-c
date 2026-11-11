use std::{fs::OpenOptions, io::Write, path::Path};

use crate::{
    parser::resolve_var::TokenWrapper,
    utils::{errors::TextForgeError, validations::check_pipeline_output_path},
};

pub fn write_bytecode_to_file(
    path: &Path,
    tokens: Vec<TokenWrapper>,
) -> Result<(), TextForgeError> {
    let mut encoded = Vec::new();
    write_bytecode(&mut encoded, &tokens)?;
    check_pipeline_output_path(path, None)?;

    let mut file = match OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
    {
        Ok(x) => x,
        Err(_) => {
            return Err(TextForgeError::new(
                crate::utils::errors::TextForgeErrorCode::FileOpeningError(
                    "Failed opening File".into(),
                ),
                "",
                format!("{:?}", path),
            ));
        }
    };

    file.write_all(&encoded).map_err(|error| {
        TextForgeError::new(
            crate::utils::errors::TextForgeErrorCode::FileWritingError(error.to_string().into()),
            "write_bytecode_to_file",
            path.display().to_string(),
        )
    })
}

/// Encode one complete pipeline, including when the sink accepts short writes.
fn write_bytecode(writer: &mut impl Write, tokens: &[TokenWrapper]) -> Result<(), TextForgeError> {
    let mut header: Vec<u8> = Vec::new();

    let magic_number: Vec<u8> = vec![38, 235, 245, 8, 244, 137, 1, 179];

    let protocol_version = 1_u64.to_be_bytes();

    let instruction_count = (tokens.len() as u32).to_be_bytes();

    header.extend_from_slice(&magic_number);
    header.extend_from_slice(&protocol_version);
    header.extend_from_slice(&instruction_count);

    match writer.write_all(&header) {
        Ok(_) => (),
        Err(_) => {
            return Err(TextForgeError::new(
                crate::utils::errors::TextForgeErrorCode::FileWritingError(
                    "Failed writing text to textforge file".into(),
                ),
                "Write bytecode to file",
                "Header writing error",
            ));
        }
    }

    for token in tokens.iter() {
        let line = token.to_bytecode_unresolved()?;

        match writer.write_all(&line) {
            Ok(_) => (),
            Err(_) => {
                return Err(TextForgeError::new(
                    crate::utils::errors::TextForgeErrorCode::FileWritingError(
                        "Failed writing text to textforge file".into(),
                    ),
                    "Write bytecode to file",
                    token.to_textforge_line(),
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tokens::transforms::atb::Atb;

    /// Test sink that accepts one byte per write and can simulate storage failure.
    /// The standard `Write::write_all` contract must retry these short writes.
    ///
    /// ```
    /// use std::io::Write;
    /// let mut bytes = Vec::new();
    /// bytes.write_all(b"complete record").unwrap();
    /// assert_eq!(bytes, b"complete record");
    /// ```
    struct ShortWriter {
        bytes: Vec<u8>,
        fail_after: Option<usize>,
    }

    impl Write for ShortWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self
                .fail_after
                .is_some_and(|limit| self.bytes.len() >= limit)
            {
                return Err(std::io::Error::other("simulated storage failure"));
            }
            if let Some(byte) = bytes.first() {
                self.bytes.push(*byte);
                Ok(1)
            } else {
                Ok(0)
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn retries_short_writes_for_header_and_instruction() {
        let token = TokenWrapper::new(Box::new(Atb::new("🍌")), None);
        let mut expected = vec![38, 235, 245, 8, 244, 137, 1, 179];
        expected.extend_from_slice(&1_u64.to_be_bytes());
        expected.extend_from_slice(&1_u32.to_be_bytes());
        expected.extend_from_slice(&token.to_bytecode_unresolved().unwrap());
        let mut writer = ShortWriter {
            bytes: Vec::new(),
            fail_after: None,
        };
        write_bytecode(&mut writer, &[token]).unwrap();
        assert_eq!(writer.bytes, expected);
    }

    #[test]
    fn reports_failures_during_header_and_instruction_writes() {
        for limit in [0, 21] {
            let mut writer = ShortWriter {
                bytes: Vec::new(),
                fail_after: Some(limit),
            };
            let error = write_bytecode(
                &mut writer,
                &[TokenWrapper::new(Box::new(Atb::new("🍌")), None)],
            )
            .unwrap_err();
            assert!(matches!(
                error.error_code,
                crate::utils::errors::TextForgeErrorCode::FileWritingError(_)
            ));
        }
    }
}

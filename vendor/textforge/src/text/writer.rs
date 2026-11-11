use crate::{
    parser::resolve_var::TokenWrapper,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_pipeline_output_path,
    },
};
use std::{fs::OpenOptions, io::Write, path::Path};

pub fn write_to_file(path: &Path, tokens: &[TokenWrapper]) -> Result<(), TextForgeError> {
    // Encode first: unsupported values must not truncate a previous pipeline or
    // leave an empty file behind. Instruction serialization is shared by all APIs.
    let mut encoded = String::new();
    for token in tokens {
        encoded.push_str(&token.to_text_line_unresolved()?);
    }
    check_pipeline_output_path(path, Some("textforge"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|error| {
            TextForgeError::new(
                TextForgeErrorCode::FileOpeningError(error.to_string().into()),
                "write_to_file",
                path.display().to_string(),
            )
        })?;
    file.write_all(encoded.as_bytes()).map_err(|error| {
        TextForgeError::new(
            TextForgeErrorCode::FileWritingError(error.to_string().into()),
            "write_to_file",
            path.display().to_string(),
        )
    })
}

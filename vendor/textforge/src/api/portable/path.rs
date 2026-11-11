use super::{BindingError, ErrorStage};
use crate::utils::errors::{TextForgeError, TextForgeErrorCode};
#[cfg(any(unix, windows))]
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// An owned native path, including non-UTF-8 Unix and native Windows names.
/// Constructors, rather than adapters, implement OS path conversion. A path from
/// another OS representation returns an error instead of lossy conversion.
///
/// ```
/// use textforge::api::portable::FilePath;
/// let path = FilePath::from_text("input.txt".into());
/// assert_eq!(path.to_text().as_deref(), Some("input.txt"));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePath(pub(super) PathBuf);

impl FilePath {
    pub fn from_text(path: String) -> Self {
        Self(path.into())
    }
    pub fn from_native_bytes(bytes: Vec<u8>) -> Result<Self, BindingError> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            Ok(Self(OsString::from_vec(bytes).into()))
        }
        #[cfg(not(unix))]
        {
            let _ = bytes;
            Err(unsupported("Unix byte paths"))
        }
    }
    pub fn from_windows_wide(wide: Vec<u16>) -> Result<Self, BindingError> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            Ok(Self(OsString::from_wide(&wide).into()))
        }
        #[cfg(not(windows))]
        {
            let _ = wide;
            Err(unsupported("Windows wide paths"))
        }
    }
    pub fn to_text(&self) -> Option<String> {
        self.0.to_str().map(str::to_owned)
    }
    pub fn to_native_bytes(&self) -> Result<Vec<u8>, BindingError> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Ok(self.0.as_os_str().as_bytes().to_vec())
        }
        #[cfg(not(unix))]
        {
            Err(unsupported("Unix byte paths"))
        }
    }
    pub fn to_windows_wide(&self) -> Result<Vec<u16>, BindingError> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            Ok(self.0.as_os_str().encode_wide().collect())
        }
        #[cfg(not(windows))]
        {
            Err(unsupported("Windows wide paths"))
        }
    }
    pub(super) fn as_path(&self) -> &Path {
        &self.0
    }
}

fn unsupported(representation: &str) -> BindingError {
    BindingError::from_core(
        ErrorStage::Conversion,
        TextForgeError::new(
            TextForgeErrorCode::ValidationError(
                format!("{representation} are not native on this target").into(),
            ),
            "FilePath",
            "path representation",
        ),
    )
}

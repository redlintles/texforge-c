#[macro_export]
macro_rules! parse_args {
    ($params:expr, $idx:expr, String, $msg:expr) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match &$params[$idx] {
            TextForgeParamTypes::String(payload) => payload.clone(),
            _ => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters($msg.into()),
                    "",
                    "",
                ));
            }
        }
    }};
    ($params:expr, $idx:expr, Usize, $msg:expr) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match &$params[$idx] {
            TextForgeParamTypes::Usize(payload) => payload.clone(),
            _ => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters($msg.into()),
                    "",
                    "",
                ));
            }
        }
    }};
    ($params:expr, $idx:expr, Token, $msg:expr) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match &$params[$idx] {
            TextForgeParamTypes::Token(payload) => payload.clone(),
            _ => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters($msg.into()),
                    "",
                    "",
                ));
            }
        }
    }};
    ($param:expr, String) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match &$param {
            TextForgeParamTypes::String(payload) => payload.clone(),
            _ => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters("Param must be of type Sring".into()),
                    "",
                    "",
                ));
            }
        }
    }};
    ($param:expr, Usize) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match $param {
            TextForgeParamTypes::Usize(payload) => payload.clone(),
            _ => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters("Param must be of type Usize".into()),
                    "",
                    "",
                ));
            }
        }
    }};
    ($param:expr, Token) => {{
        use $crate::parser::params::TextForgeParamTypes;
        use $crate::utils::errors::{TextForgeError, TextForgeErrorCode};
        match $param {
            TextForgeParamTypes::Token(payload) => payload.clone(),
            _ => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters("Param must be of type Token".into()),
                    "",
                    "",
                ));
            }
        }
    }};
}

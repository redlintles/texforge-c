#![forbid(unsafe_code)]

#[cfg(not(feature = "test_access"))]
mod context;
#[cfg(not(feature = "test_access"))]
mod globals;
#[cfg(not(feature = "test_access"))]
mod macros;
#[cfg(not(feature = "test_access"))]
mod parser;
#[cfg(not(feature = "test_access"))]
mod text;

#[cfg(feature = "test_access")]
pub mod context;
#[cfg(feature = "test_access")]
pub mod globals;
#[cfg(feature = "test_access")]
pub mod macros;

#[cfg(feature = "test_access")]
pub mod text;

#[cfg(feature = "test_access")]
pub mod parser;

// Public

pub mod api;
pub mod tokens;
pub mod utils;

// Bytecode
#[cfg(feature = "bytecode")]
pub mod bytecode;

#[cfg(feature = "watchers")]
pub mod watchers;

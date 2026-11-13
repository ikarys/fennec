//! Errors for the synchronous MCP client.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unauthorized,
    Failed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unauthorized => write!(f, "mcp http: 401 unauthorized"),
            Error::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Error::Failed(value)
    }
}

impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Error::Failed(value.to_string())
    }
}

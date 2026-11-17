use alloc::string::{String, ToString};
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotFound,
    InvalidData(String),
    Io(String),
    Unsupported,
}

pub type Result<T> = core::result::Result<T, Error>;

impl Error {
    /// Wrap any displayable error (backend, platform) as an I/O error.
    pub fn io(err: impl fmt::Display) -> Self {
        Error::Io(err.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound => f.write_str("not found"),
            Error::InvalidData(msg) => write!(f, "invalid data: {msg}"),
            Error::Io(msg) => write!(f, "I/O error: {msg}"),
            Error::Unsupported => f.write_str("unsupported"),
        }
    }
}

impl core::error::Error for Error {}

#[cfg(feature = "terminal")]
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => Error::NotFound,
            std::io::ErrorKind::InvalidData => Error::InvalidData(err.to_string()),
            _ => Error::Io(err.to_string()),
        }
    }
}

use alloc::string::String;
use alloc::fmt::{Write, Display};
use core::fmt::Formatter;

/// General Result type
pub type MResult<T> = Result<T, Error>;

/// General Error type
#[derive(Clone, Debug)]
pub enum Error {
    GraphicsAPIError { backend: &'static str, error: String },
    DataError { error: String }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::GraphicsAPIError { backend, error } => write!(f, "{backend} API error: {error}"),
            Self::DataError { error } => write!(f, "Data error: {error}")
        }
    }
}

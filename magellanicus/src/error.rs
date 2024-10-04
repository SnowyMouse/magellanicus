use alloc::fmt::Display;
use alloc::string::String;
use core::fmt::Formatter;

/// General Result type
pub type MResult<T> = Result<T, Error>;

/// General Error type
#[derive(Clone, Debug)]
pub enum Error {
    GraphicsAPIError { backend: &'static str, error: String },
    DataError { error: String }
}

impl Error {
    pub(crate) fn from_data_error_string(error: String) -> Self {
        Error::DataError { error }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::GraphicsAPIError { backend, error } => write!(f, "{backend} API error: {error}"),
            Self::DataError { error } => write!(f, "Data error: {error}")
        }
    }
}

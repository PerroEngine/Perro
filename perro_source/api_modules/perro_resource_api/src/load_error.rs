use std::fmt;

pub type LoadResult<T> = Result<T, LoadError>;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LoadError {
    Unsupported { op: &'static str },
    InvalidHandle { kind: &'static str, id: u64 },
    Read { path: String, message: String },
    Write { path: String, message: String },
    Utf8 { path: String, message: String },
    Parse { path: String, message: String },
    Prepare { message: String },
    Merge { message: String },
    Script { message: String },
    Legacy(String),
}

impl LoadError {
    pub fn legacy(message: impl Into<String>) -> Self {
        Self::Legacy(message.into())
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { op } => write!(f, "{op} is not supported"),
            Self::InvalidHandle { kind, id } => write!(f, "{kind} id `{id}` is not valid"),
            Self::Read { path, message } => write!(f, "failed to load `{path}`: {message}"),
            Self::Write { path, message } => write!(f, "failed to save `{path}`: {message}"),
            Self::Utf8 { path, message } => write!(f, "`{path}` is not valid UTF-8: {message}"),
            Self::Parse { path, message } => write!(f, "failed to parse `{path}`: {message}"),
            Self::Prepare { message } => f.write_str(message),
            Self::Merge { message } => f.write_str(message),
            Self::Script { message } => f.write_str(message),
            Self::Legacy(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<String> for LoadError {
    fn from(message: String) -> Self {
        Self::Legacy(message)
    }
}

impl From<&str> for LoadError {
    fn from(message: &str) -> Self {
        Self::Legacy(message.to_owned())
    }
}

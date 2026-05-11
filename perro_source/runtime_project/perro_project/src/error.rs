#[derive(Debug)]
pub enum ProjectError {
    Io(std::io::Error),
    ParseToml(toml::de::Error),
    MissingField(&'static str),
    InvalidField(&'static str, String),
    AlreadyExists(PathBuf),
}

impl Display for ProjectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::ParseToml(err) => write!(f, "{err}"),
            Self::MissingField(field) => write!(f, "missing required field `{field}`"),
            Self::InvalidField(field, reason) => write!(f, "invalid field `{field}`: {reason}"),
            Self::AlreadyExists(path) => {
                write!(f, "project directory already exists: {}", path.display())
            }
        }
    }
}

impl std::error::Error for ProjectError {}

impl From<std::io::Error> for ProjectError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for ProjectError {
    fn from(value: toml::de::Error) -> Self {
        Self::ParseToml(value)
    }
}

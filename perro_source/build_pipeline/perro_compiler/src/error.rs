#[derive(Debug)]
pub enum CompilerError {
    Io(std::io::Error),
    CargoFailed(i32),
    SceneParse(String),
}

impl Display for CompilerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::CargoFailed(code) => write!(f, "cargo build failed with exit code {code}"),
            Self::SceneParse(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CompilerError {}

impl From<std::io::Error> for CompilerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

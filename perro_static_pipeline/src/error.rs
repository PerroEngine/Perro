use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum StaticPipelineError {
    Io(std::io::Error),
    Image(image::ImageError),
    SceneParse(String),
}

impl Display for StaticPipelineError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Image(err) => write!(f, "{err}"),
            Self::SceneParse(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for StaticPipelineError {}

impl From<std::io::Error> for StaticPipelineError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<image::ImageError> for StaticPipelineError {
    fn from(value: image::ImageError) -> Self {
        Self::Image(value)
    }
}

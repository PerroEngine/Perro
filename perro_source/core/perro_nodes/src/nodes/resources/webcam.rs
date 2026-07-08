use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebcamConfig {
    pub device: Cow<'static, str>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub mirror: bool,
    pub cpu_frames: bool,
}

impl Default for WebcamConfig {
    fn default() -> Self {
        Self {
            device: Cow::Borrowed(""),
            width: 640,
            height: 480,
            fps: 30,
            mirror: false,
            cpu_frames: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Webcam {
    pub config: WebcamConfig,
    pub enabled: bool,
}

impl Default for Webcam {
    fn default() -> Self {
        Self {
            config: WebcamConfig::default(),
            enabled: true,
        }
    }
}

impl Webcam {
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

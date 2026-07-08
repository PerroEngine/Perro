use perro_ids::{TextureID, WebcamID};
pub use perro_nodes::WebcamConfig;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebcamDevice {
    pub slot: String,
    pub index: Option<u32>,
    pub name: String,
    pub description: String,
    pub extra: String,
}

impl WebcamDevice {
    #[inline]
    pub fn config(&self) -> WebcamConfig {
        WebcamConfig {
            device: self.slot.clone().into(),
            ..WebcamConfig::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebcamFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub trait WebcamAPI {
    fn webcam_devices(&self) -> Result<Vec<WebcamDevice>, String>;
    fn webcam_open(&self, config: WebcamConfig) -> Result<WebcamID, String>;
    fn webcam_default(&self) -> Result<WebcamID, String>;
    fn webcam_texture(&self, id: WebcamID) -> TextureID;
    fn webcam_frame_rgba(&self, id: WebcamID) -> Option<WebcamFrame>;
    fn webcam_is_open(&self, id: WebcamID) -> bool;
    fn webcam_last_error(&self, id: WebcamID) -> Option<String>;
    fn webcam_close(&self, id: WebcamID) -> bool;
}

pub struct WebcamModule<'res, R: WebcamAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: WebcamAPI + ?Sized> WebcamModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn devices(&self) -> Result<Vec<WebcamDevice>, String> {
        self.api.webcam_devices()
    }

    #[inline]
    pub fn open(&self, config: WebcamConfig) -> Result<WebcamID, String> {
        self.api.webcam_open(config)
    }

    #[inline]
    pub fn open_device(&self, device: &WebcamDevice) -> Result<WebcamID, String> {
        self.open(device.config())
    }

    #[inline]
    pub fn default(&self) -> Result<WebcamID, String> {
        self.api.webcam_default()
    }

    #[inline]
    pub fn texture(&self, id: WebcamID) -> TextureID {
        self.api.webcam_texture(id)
    }

    #[inline]
    pub fn frame_rgba(&self, id: WebcamID) -> Option<WebcamFrame> {
        self.api.webcam_frame_rgba(id)
    }

    #[inline]
    pub fn is_open(&self, id: WebcamID) -> bool {
        self.api.webcam_is_open(id)
    }

    #[inline]
    pub fn last_error(&self, id: WebcamID) -> Option<String> {
        self.api.webcam_last_error(id)
    }

    #[inline]
    pub fn close(&self, id: WebcamID) -> bool {
        self.api.webcam_close(id)
    }
}

#[macro_export]
macro_rules! webcam_open {
    ($res:expr, $cfg:expr) => {
        $res.Webcams().open($cfg)
    };
}

#[macro_export]
macro_rules! webcam_devices {
    ($res:expr) => {
        $res.Webcams().devices()
    };
}

#[macro_export]
macro_rules! webcam_open_device {
    ($res:expr, $device:expr) => {
        $res.Webcams().open_device($device)
    };
}

#[macro_export]
macro_rules! webcam_default {
    ($res:expr) => {
        $res.Webcams().default()
    };
}

#[macro_export]
macro_rules! webcam_texture {
    ($res:expr, $id:expr) => {
        $res.Webcams().texture($id)
    };
}

#[macro_export]
macro_rules! webcam_frame_rgba {
    ($res:expr, $id:expr) => {
        $res.Webcams().frame_rgba($id)
    };
}

use thiserror::Error;

/// Result type alias for JoyCon operations
pub type Result<T> = std::result::Result<T, JoyConError>;

/// Errors that can occur when working with Joy-Con controllers
#[derive(Error, Debug)]
pub enum JoyConError {
    #[error("HID error: {0}")]
    Hid(String),

    #[error("BLE error: {0}")]
    Ble(String),

    #[error("Device not found")]
    DeviceNotFound,

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<hidapi::HidError> for JoyConError {
    fn from(err: hidapi::HidError) -> Self {
        JoyConError::Hid(err.to_string())
    }
}

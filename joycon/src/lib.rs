//! # JoyCon
//!
//! An open-source Rust library for interfacing with Nintendo Switch Joy-Con controllers
//! (Joy-Con 1 and Joy-Con 2) on Windows.
//!
//! ## Overview
//!
//! **JoyCon** provides a foundation for developers and researchers interested in low-level
//! interaction with Nintendo Switch Joy-Cons, supporting both Bluetooth Classic (Joy-Con 1)
//! and Bluetooth Low Energy (Joy-Con 2).
//!
//! - **Joy-Con 1** devices require manual Bluetooth pairing to appear as HID devices on Windows.
//! - **Joy-Con 2** devices broadcast BLE advertisements when the sync button is held.

pub mod joycon;
pub mod joycon2;
pub mod error;
pub mod input_report;
pub mod calibration;

pub use joycon::JoyCon;
pub use joycon2::JoyCon2;
pub use error::{JoyConError, Result};
pub use input_report::{InputReport, Buttons, Stick, Gyro, Accel};
pub use calibration::{Calibration, CalibrationSample};

/// Joy-Con vendor ID (Nintendo) - for HID devices
pub const JOYCON_VENDOR_ID: u16 = 0x057E;

/// Joy-Con 1 product IDs
pub const JOYCON_1_LEFT_PID: u16 = 0x2006;
pub const JOYCON_1_RIGHT_PID: u16 = 0x2007;

/// Nintendo BLE Company ID (for Joy-Con 2 BLE advertisements)
pub const NINTENDO_BLE_CID: u16 = 0x0553;

/// Joy-Con 2 side identifiers in BLE manufacturer data
pub const JOYCON_R_SIDE: u8 = 0x66;
pub const JOYCON_L_SIDE: u8 = 0x67;

/// Scan for available Joy-Con 1 devices via HID enumeration
pub fn scan_joycon1_devices() -> Result<Vec<(String, u16, u16)>> {
    joycon::scan_devices()
}

/// Scan for available Joy-Con 2 devices via BLE advertisements
pub async fn scan_joycon2_devices() -> Result<Vec<String>> {
    joycon2::scan_devices().await
}


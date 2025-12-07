//! Input system for handling controller input
//!
//! This module provides a unified API for managing various controller types.
//! Currently supports Joy-Con controllers (both HID and BLE).

pub mod joycon;

pub use joycon::{InputReport, Buttons, Calibration, ControllerManager};
pub use crate::structs::{Vector2, Vector3};


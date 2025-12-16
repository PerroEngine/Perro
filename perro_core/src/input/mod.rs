//! Input system for handling controller input
//!
//! This module provides a unified API for managing various controller types.
//! Currently supports Joy-Con controllers (both HID and BLE).

pub mod joycon;
pub mod manager;

pub use crate::structs::{Vector2, Vector3};
pub use joycon::{Buttons, Calibration, ControllerManager, InputReport};
pub use manager::{InputManager, InputMap, InputSource, MouseButton, parse_input_source};

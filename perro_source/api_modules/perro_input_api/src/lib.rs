//! Public input scripting API.
//!
//! This crate exposes frame-stable input state to scripts. It keeps raw device
//! state, input-map actions, player bindings, and queued output commands
//! together behind [`InputWindow`].

// ---- Device state modules ----

mod frame;
mod gamepad;
mod input_map;
mod joycon;
mod keycode;
mod macros;
mod mouse_button;
mod player;
mod snapshot;
mod state;
mod types;
mod window;

// ---- Public device/state exports ----

pub use frame::*;
pub use gamepad::{GamepadAxis, GamepadButton, GamepadState};
pub use input_map::{InputAction, InputBinding, InputMap, action_hash};
pub use joycon::{JoyConButton, JoyConMouseSensor, JoyConSide, JoyConState};
pub use keycode::KeyCode;
pub use mouse_button::MouseButton;
use perro_structs::Vector2;
pub use perro_structs::{SignedUnit, SignedUnitVector2};
pub use player::{PlayerBinding, PlayerModule, PlayerState};
pub use snapshot::*;
pub use state::*;
use std::cell::RefCell;
pub use types::*;
pub use window::*;

#[cfg(test)]
mod tests;

/// Common imports for scripts that use input APIs.
pub mod prelude {
    pub use crate::{
        ActionModule, GamepadAxis, GamepadButton, GamepadIndex, GamepadModule, GamepadState,
        InputAPI, InputAction, InputBinding, InputMap, InputSnapshot, InputWindow, JoyConButton,
        JoyConIndex, JoyConModule, JoyConMouseSensor, JoyConSide, JoyConState, KeyCode, KeyModule,
        KeyboardModule, KeyboardState, MouseButton, MouseMode, MouseModule, MouseState,
        MouseStateModule, PlayerBinding, PlayerIndicatorSlot, PlayerModule, PlayerState,
        RumbleIntensity, action_down, action_hash, action_pressed, action_released, gamepad_accel,
        gamepad_down, gamepad_get, gamepad_gyro, gamepad_left_stick, gamepad_list, gamepad_pressed,
        gamepad_released, gamepad_right_stick, gamepad_set_rumble, joycon_accel, joycon_calibrated,
        joycon_calibrating, joycon_calibration_bias, joycon_connected, joycon_down,
        joycon_ensure_calibration, joycon_get, joycon_gyro, joycon_list, joycon_mouse_sensor,
        joycon_needs_calibration, joycon_pressed, joycon_released, joycon_request_calibration,
        joycon_set_indicator, joycon_set_rumble, joycon_side, joycon_stick, key_down, key_pressed,
        key_released, mouse_capture, mouse_confine, mouse_confine_hidden, mouse_delta, mouse_down,
        mouse_hide, mouse_mode, mouse_position, mouse_pressed, mouse_released, mouse_set_mode,
        mouse_show, mouse_wheel, player_bind, player_get, player_list, viewport_size,
    };
    pub use perro_structs::{SignedUnit, SignedUnitVector2, Unit, UnitVector2, Vector2};
}

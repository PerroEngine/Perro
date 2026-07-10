//! Compile guard for input macro path hygiene.
//!
//! User script crates depend only on `perro_api` + `perro_runtime`, so
//! `perro_structs` is not in their extern prelude. Any exported input macro
//! that expands to a bare `perro_structs::...` path breaks user scripts with
//! E0433 even though it compiles inside `perro_input_api` itself.
//!
//! This crate (`perro_scripting`) also does not depend on `perro_structs`,
//! so invoking every input macro here reproduces the user-script resolution
//! context: macros must reach engine types via `$crate::...` paths only.
//! Do not add `perro_structs` to this crate's dependencies, or this guard
//! stops working.

#![allow(dead_code)]

use perro_input_api::prelude::*;

fn exercise_joycon_macros<IP: InputAPI + ?Sized>(ipt: &InputWindow<'_, IP>) {
    let _ = joycon_list!(ipt);
    let _ = joycon_get!(ipt, 0);
    let _ = joycon_side!(ipt, 0);
    let _ = joycon_down!(ipt, 0, JoyConButton::Top);
    let _ = joycon_pressed!(ipt, 0, JoyConButton::Top);
    let _ = joycon_released!(ipt, 0, JoyConButton::Top);
    let _ = joycon_stick!(ipt, 0);
    let _ = joycon_mouse_sensor!(ipt, 0);
    let _ = joycon_gyro!(ipt, 0);
    let _ = joycon_accel!(ipt, 0);
    let _ = joycon_connected!(ipt, 0);
    let _ = joycon_calibrated!(ipt, 0);
    let _ = joycon_calibrating!(ipt, 0);
    let _ = joycon_needs_calibration!(ipt, 0);
    let _ = joycon_calibration_bias!(ipt, 0);
    joycon_request_calibration!(ipt, 0);
    let _ = joycon_ensure_calibration!(ipt, 0);
    joycon_set_rumble!(ipt, 0, 0.0, 0.0);
    joycon_set_indicator!(ipt, 0, 1);
}

fn exercise_gamepad_macros<IP: InputAPI + ?Sized>(ipt: &InputWindow<'_, IP>) {
    let _ = gamepad_list!(ipt);
    let _ = gamepad_get!(ipt, 0);
    let _ = gamepad_down!(ipt, 0, GamepadButton::Bottom);
    let _ = gamepad_pressed!(ipt, 0, GamepadButton::Bottom);
    let _ = gamepad_released!(ipt, 0, GamepadButton::Bottom);
    let _ = gamepad_left_stick!(ipt, 0);
    let _ = gamepad_right_stick!(ipt, 0);
    let _ = gamepad_gyro!(ipt, 0);
    let _ = gamepad_accel!(ipt, 0);
    gamepad_set_rumble!(ipt, 0, 0.0, 0.0);
}

fn exercise_key_action_mouse_macros<IP: InputAPI + ?Sized>(ipt: &InputWindow<'_, IP>) {
    let _ = key_down!(ipt, KeyCode::Space);
    let _ = key_pressed!(ipt, KeyCode::Space);
    let _ = key_released!(ipt, KeyCode::Space);
    let _ = action_down!(ipt, "jump");
    let _ = action_pressed!(ipt, "jump");
    let _ = action_released!(ipt, "jump");
    let _ = mouse_down!(ipt, MouseButton::Left);
    let _ = mouse_pressed!(ipt, MouseButton::Left);
    let _ = mouse_released!(ipt, MouseButton::Left);
    let _ = mouse_delta!(ipt);
    let _ = mouse_wheel!(ipt);
    let _ = mouse_position!(ipt);
    let _ = viewport_size!(ipt);
    let _ = mouse_mode!(ipt);
    mouse_set_mode!(ipt, MouseMode::Visible);
    mouse_show!(ipt);
    mouse_hide!(ipt);
    mouse_capture!(ipt);
    mouse_confine!(ipt);
    mouse_confine_hidden!(ipt);
}

#[test]
fn input_macros_expand_from_script_like_scope() {
    // compile-only guard: the exercise fns above must build without
    // perro_structs in this crate's extern prelude
}

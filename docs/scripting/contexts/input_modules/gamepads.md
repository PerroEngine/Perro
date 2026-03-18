# Gamepads Module

Access:

- `ipt.Gamepads()`

Purpose:

- Read raw gamepad input as the engine detects controllers.
- Use the player system to map engine-assigned indices to your game’s notion of a player.

Macros:

- `gamepad_list!(ipt) -> &[GamepadState]`
- `gamepad_get!(ipt, index) -> Option<&GamepadState>`
- `gamepad_down!(ipt, index, button) -> bool`
- `gamepad_pressed!(ipt, index, button) -> bool`
- `gamepad_released!(ipt, index, button) -> bool`
- `gamepad_left_stick!(ipt, index) -> Vector2`
- `gamepad_right_stick!(ipt, index) -> Vector2`
- `gamepad_gyro!(ipt, index) -> Vector3`
- `gamepad_accel!(ipt, index) -> Vector3`

Methods:

- `ipt.Gamepads().all() -> &[GamepadState]`
- `ipt.Gamepads().get(index) -> Option<&GamepadState>`

Common `GamepadState` methods:

- `state.is_button_down(button) -> bool`
- `state.is_button_pressed(button) -> bool`
- `state.is_button_released(button) -> bool`
- `state.axis(axis) -> f32`
- `state.left_stick() -> Vector2`
- `state.right_stick() -> Vector2`
- `state.gyro() -> Vector3`
- `state.accel() -> Vector3`

Inputs:

- `index: usize`
- `button: GamepadButton`
- `axis: GamepadAxis`

Bindings:

- Gamepad indices are assigned by the engine in connection/order-detected sequence.
- Map indices to players via `PlayerBinding::Gamepad { index }` when you need stable player associations.

Source of truth:

- `perro_source/api_modules/perro_input/src/gamepad.rs`

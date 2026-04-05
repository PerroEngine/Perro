# Joy-Cons Module

Access:
- `ipt.JoyCons()`

Macros:
- `joycon_list!(ipt) -> &[JoyConState]`
- `joycon_down!(ipt, index, button) -> bool`
- `joycon_get!(ipt, index) -> Option<&JoyConState>`
- `joycon_pressed!(ipt, index, button) -> bool`
- `joycon_released!(ipt, index, button) -> bool`
- `joycon_side!(ipt, index) -> Option<JoyConSide>`
- `joycon_stick!(ipt, index) -> Vector2`
- `joycon_gyro!(ipt, index) -> Vector3`
- `joycon_accel!(ipt, index) -> Vector3`
- `joycon_connected!(ipt, index) -> bool`
- `joycon_calibrated!(ipt, index) -> bool`
- `joycon_calibrating!(ipt, index) -> bool`
- `joycon_needs_calibration!(ipt, index) -> bool`
- `joycon_calibration_bias!(ipt, index) -> Vector3`
- `joycon_request_calibration!(ipt, index) -> ()`

Methods:
- `ipt.JoyCons().all() -> &[JoyConState]`
- `ipt.JoyCons().get(index) -> Option<&JoyConState>`

Common `JoyConState` methods:
- `state.side() -> JoyConSide`
- `state.connected() -> bool`
- `state.calibrated() -> bool`
- `state.calibration_in_progress() -> bool`
- `state.needs_calibration() -> bool`
- `state.calibration_bias() -> Vector3`
- `state.is_button_down(button) -> bool`
- `state.is_button_pressed(button) -> bool`
- `state.is_button_released(button) -> bool`
- `state.stick_x() -> f32`
- `state.stick_y() -> f32`
- `state.stick() -> Vector2`
- `state.gyro() -> Vector3`
- `state.accel() -> Vector3`

Inputs:
- `index: usize`
- `button: JoyConButton`

Bindings:
- Joy-Con indices are assigned by the engine in connection/order-detected sequence.
- Use the player system to map those indices to your game's notion of a player.
- Player bindings that use Joy-Cons are configured via `PlayerBinding::JoyConSingle { index }` and `PlayerBinding::JoyConPair { left, right }`.

Calibration behavior:
- Calibration files are stored at `user://calibrations/<SERIAL>.cal`.
- If a calibration file already exists for a connected Joy-Con serial, the engine auto-loads and auto-applies that bias on connect.
- Scripts are only needed to trigger first-time calibration and to display status.

When to use each calibration macro:
- `joycon_connected!`: device presence gate before showing Joy-Con UI.
- `joycon_needs_calibration!`: show "calibrate now" prompt.
- `joycon_request_calibration!`: start calibration workflow for that index.
- `joycon_calibrating!`: show in-progress state.
- `joycon_calibrated!`: hide prompts and allow normal gyro gameplay.
- `joycon_calibration_bias!`: debug/telemetry display of current bias.

`JoyConButton` mapping:
- `Top`: Left Joy-Con Up / Right Joy-Con X
- `Bottom`: Left Joy-Con Down / Right Joy-Con B
- `Left`: Left Joy-Con Left / Right Joy-Con Y
- `Right`: Left Joy-Con Right / Right Joy-Con A
- `Bumper`: Left Joy-Con L / Right Joy-Con R
- `Trigger`: Left Joy-Con ZL / Right Joy-Con ZR
- `Stick`: Stick press (both sides)
- `SL`: SL (both sides)
- `SR`: SR (both sides)
- `Start`: Left Joy-Con Minus / Right Joy-Con Plus
- `Meta`: Left Joy-Con Capture / Right Joy-Con Home

Source of truth:
- `perro_source/api_modules/perro_input/src/joycon.rs`

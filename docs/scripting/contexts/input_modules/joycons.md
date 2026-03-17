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

Methods:
- `ipt.JoyCons().all() -> &[JoyConState]`
- `ipt.JoyCons().get(index) -> Option<&JoyConState>`

Common `JoyConState` methods:
- `state.side() -> JoyConSide`
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

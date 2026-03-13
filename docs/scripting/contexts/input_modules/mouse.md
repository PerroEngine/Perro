# Mouse Module

Access:
- `ipt.Mouse()`

Macros:
- `mouse_down!(ipt, button) -> bool`
- `mouse_pressed!(ipt, button) -> bool`
- `mouse_released!(ipt, button) -> bool`
- `mouse_delta!(ipt) -> Vector2`
- `mouse_wheel!(ipt) -> Vector2`

Methods:
- `ipt.Mouse().down(button) -> bool`
- `ipt.Mouse().pressed(button) -> bool`
- `ipt.Mouse().released(button) -> bool`
- `ipt.Mouse().delta() -> Vector2`
- `ipt.Mouse().wheel() -> Vector2`

Inputs:
- `button: MouseButton`

Available `MouseButton` values:
- `MouseButton::Left`
- `MouseButton::Right`
- `MouseButton::Middle`
- `MouseButton::Back`
- `MouseButton::Forward`

Source of truth:
- `perro_source/api_modules/perro_input/src/mouse_button.rs`

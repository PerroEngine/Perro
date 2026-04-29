# Mouse Module

Access:
- `ipt.Mouse()`

Macros:
- `mouse_down!(ipt, button) -> bool`
- `mouse_pressed!(ipt, button) -> bool`
- `mouse_released!(ipt, button) -> bool`
- `mouse_delta!(ipt) -> Vector2`
- `mouse_wheel!(ipt) -> Vector2`
- `mouse_position!(ipt) -> Vector2`
- `mouse_mode!(ipt) -> MouseMode`
- `mouse_set_mode!(ipt, mode)`
- `mouse_show!(ipt)`
- `mouse_hide!(ipt)`
- `mouse_capture!(ipt)`
- `mouse_confine!(ipt)`
- `mouse_confine_hidden!(ipt)`

Methods:
- `ipt.Mouse().down(button) -> bool`
- `ipt.Mouse().pressed(button) -> bool`
- `ipt.Mouse().released(button) -> bool`
- `ipt.Mouse().delta() -> Vector2`
- `ipt.Mouse().wheel() -> Vector2`
- `ipt.Mouse().position() -> Vector2`
- `ipt.Mouse().mode() -> MouseMode`
- `ipt.Mouse().set_mode(mode)`
- `ipt.Mouse().show()`
- `ipt.Mouse().hide()`
- `ipt.Mouse().capture()`
- `ipt.Mouse().confine()`
- `ipt.Mouse().confine_hidden()`

Inputs:
- `button: MouseButton`

Coordinate units:
- `mouse_position!(ipt)` returns normalized viewport coordinates in `[0.0, 1.0]`.
- `(0.5, 0.5)` is the center of the viewport.
- X increases to the right; Y increases upward (top is near `1.0`, bottom is near `0.0`).
- `mouse_delta!(ipt)` is per-frame movement in pixels.

Mouse mode:
- Default mode is `MouseMode::Visible`.
- Window click focuses the window only.
- Capture is opt-in from script.
- `MouseMode::Visible` shows the cursor and does not grab it.
- `MouseMode::Hidden` hides the cursor and does not grab it.
- `MouseMode::Captured` hides the cursor and locks it for relative motion.
- `MouseMode::Confined` shows the cursor and keeps it in the window.
- `MouseMode::ConfinedHidden` hides the cursor and keeps it in the window.
- Escape releases capture back to `Visible`.
- Focus loss releases capture back to `Visible`.

Available `MouseButton` values:
- `MouseButton::Left`
- `MouseButton::Right`
- `MouseButton::Middle`
- `MouseButton::Back`
- `MouseButton::Forward`

Source of truth:
- `perro_source/api_modules/perro_input/src/mouse_button.rs`
- `perro_source/api_modules/perro_input/src/lib.rs`

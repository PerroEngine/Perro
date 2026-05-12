#[macro_export]
/// Signature:
/// - `key_down!(&InputWindow<_>, KeyCode) -> bool`
///
/// Usage:
/// - `key_down!(ipt, KeyCode::Space) -> bool`
///
/// `ipt` is usually the input parameter from lifecycle methods:
/// - `fn on_update(..., ctx: &mut ScriptContext<'_, RT, RS, IP>, ...)`
///
/// `KeyCode` is the keyboard-key enum (letters, numbers, arrows, function keys, etc.).
///
/// Checks whether a key is currently down.
macro_rules! key_down {
    ($ipt:expr, $key:expr) => {
        $ipt.Keys().down($key)
    };
}

#[macro_export]
/// Signature:
/// - `key_pressed!(&InputWindow<_>, KeyCode) -> bool`
///
/// Usage:
/// - `key_pressed!(ipt, KeyCode::Enter) -> bool`
///
/// `KeyCode` is the keyboard-key enum.
///
/// Checks whether a key was pressed this frame.
macro_rules! key_pressed {
    ($ipt:expr, $key:expr) => {
        $ipt.Keys().pressed($key)
    };
}

#[macro_export]
/// Signature:
/// - `key_released!(&InputWindow<_>, KeyCode) -> bool`
///
/// Usage:
/// - `key_released!(ipt, KeyCode::Escape) -> bool`
///
/// `KeyCode` is the keyboard-key enum.
///
/// Checks whether a key was released this frame.
macro_rules! key_released {
    ($ipt:expr, $key:expr) => {
        $ipt.Keys().released($key)
    };
}

#[macro_export]
/// Signature:
/// - `mouse_down!(&InputWindow<_>, MouseButton) -> bool`
///
/// Usage:
/// - `mouse_down!(ipt, MouseButton::Right) -> bool`
///
/// `MouseButton` is the mouse-button enum (`Left`, `Right`, `Middle`, and extras).
///
/// Checks whether a mouse button is currently down.
macro_rules! mouse_down {
    ($ipt:expr, $button:expr) => {
        $ipt.Mouse().down($button)
    };
}

#[macro_export]
/// Signature:
/// - `mouse_pressed!(&InputWindow<_>, MouseButton) -> bool`
///
/// Usage:
/// - `mouse_pressed!(ipt, MouseButton::Left) -> bool`
///
/// `MouseButton` is the mouse-button enum.
///
/// Checks whether a mouse button was pressed this frame.
macro_rules! mouse_pressed {
    ($ipt:expr, $button:expr) => {
        $ipt.Mouse().pressed($button)
    };
}

#[macro_export]
/// Signature:
/// - `mouse_released!(&InputWindow<_>, MouseButton) -> bool`
///
/// Usage:
/// - `mouse_released!(ipt, MouseButton::Left) -> bool`
///
/// `MouseButton` is the mouse-button enum.
///
/// Checks whether a mouse button was released this frame.
macro_rules! mouse_released {
    ($ipt:expr, $button:expr) => {
        $ipt.Mouse().released($button)
    };
}

#[macro_export]
/// Signature:
/// - `mouse_delta!(&InputWindow<_>) -> Vector2`
///
/// Usage:
/// - `mouse_delta!(ipt) -> Vector2`
macro_rules! mouse_delta {
    ($ipt:expr) => {
        $ipt.Mouse().delta()
    };
}

#[macro_export]
/// Signature:
/// - `mouse_wheel!(&InputWindow<_>) -> Vector2`
///
/// Usage:
/// - `mouse_wheel!(ipt) -> Vector2`
macro_rules! mouse_wheel {
    ($ipt:expr) => {
        $ipt.Mouse().wheel()
    };
}

#[macro_export]
/// Signature:
/// - `mouse_position!(&InputWindow<_>) -> Vector2`
///
/// Usage:
/// - `mouse_position!(ipt) -> Vector2`
macro_rules! mouse_position {
    ($ipt:expr) => {
        $ipt.Mouse().position()
    };
}

#[macro_export]
/// Signature:
/// - `viewport_size!(&InputWindow<_>) -> Vector2`
///
/// Usage:
/// - `viewport_size!(ipt) -> Vector2`
macro_rules! viewport_size {
    ($ipt:expr) => {
        $ipt.Mouse().viewport_size()
    };
}

#[macro_export]
/// Signature:
/// - `mouse_mode!(&InputWindow<_>) -> MouseMode`
///
/// Usage:
/// - `mouse_mode!(ipt) -> MouseMode`
macro_rules! mouse_mode {
    ($ipt:expr) => {
        $ipt.Mouse().mode()
    };
}

#[macro_export]
/// Signature:
/// - `mouse_set_mode!(&InputWindow<_>, MouseMode) -> ()`
///
/// Usage:
/// - `mouse_set_mode!(ipt, MouseMode::Captured)`
macro_rules! mouse_set_mode {
    ($ipt:expr, $mode:expr) => {{ $ipt.Mouse().set_mode($mode) }};
}

#[macro_export]
/// Signature:
/// - `mouse_show!(&InputWindow<_>) -> ()`
macro_rules! mouse_show {
    ($ipt:expr) => {{ $ipt.Mouse().show() }};
}

#[macro_export]
/// Signature:
/// - `mouse_hide!(&InputWindow<_>) -> ()`
macro_rules! mouse_hide {
    ($ipt:expr) => {{ $ipt.Mouse().hide() }};
}

#[macro_export]
/// Signature:
/// - `mouse_capture!(&InputWindow<_>) -> ()`
macro_rules! mouse_capture {
    ($ipt:expr) => {{ $ipt.Mouse().capture() }};
}

#[macro_export]
/// Signature:
/// - `mouse_confine!(&InputWindow<_>) -> ()`
macro_rules! mouse_confine {
    ($ipt:expr) => {{ $ipt.Mouse().confine() }};
}

#[macro_export]
/// Signature:
/// - `mouse_confine_hidden!(&InputWindow<_>) -> ()`
macro_rules! mouse_confine_hidden {
    ($ipt:expr) => {{ $ipt.Mouse().confine_hidden() }};
}

#[macro_export]
/// Signature:
/// - `joycon_request_calibration!(&InputWindow<_>, JoyConIndex) -> ()`
macro_rules! joycon_request_calibration {
    ($ipt:expr, $index:expr) => {{ $ipt.request_joycon_calibration($index) }};
}

#[macro_export]
macro_rules! gamepad_set_rumble {
    ($ipt:expr, $index:expr, $low:expr, $high:expr) => {{ $ipt.Gamepads().set_rumble($index, $low, $high) }};
}

#[macro_export]
macro_rules! joycon_set_rumble {
    ($ipt:expr, $index:expr, $low:expr, $high:expr) => {{ $ipt.JoyCons().set_rumble($index, $low, $high) }};
}

#[macro_export]
macro_rules! joycon_set_indicator {
    ($ipt:expr, $index:expr, $indicator:expr) => {{ $ipt.JoyCons().set_indicator($index, $indicator) }};
}

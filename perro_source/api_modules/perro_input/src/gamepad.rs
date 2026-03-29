#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    Bottom,
    Right,
    Left,
    Top,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    Start,
    Select,
    Home,
    Capture,
    L1,
    R1,
    L2,
    R2,
    L3,
    R3,
}

impl GamepadButton {
    pub const COUNT: usize = 18;

    #[inline]
    pub fn as_index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftTrigger,
    RightTrigger,
}

impl GamepadAxis {
    pub const COUNT: usize = 6;

    #[inline]
    pub fn as_index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug)]
pub struct GamepadState {
    buttons_down: [u64; GamepadState::BUTTON_WORDS],
    buttons_pressed: [u64; GamepadState::BUTTON_WORDS],
    buttons_released: [u64; GamepadState::BUTTON_WORDS],
    axes: [f32; GamepadAxis::COUNT],
    gyro: perro_structs::Vector3,
    accel: perro_structs::Vector3,
}

impl GamepadState {
    const BUTTON_WORDS: usize = GamepadButton::COUNT.div_ceil(64);

    pub fn new() -> Self {
        Self {
            buttons_down: [0; GamepadState::BUTTON_WORDS],
            buttons_pressed: [0; GamepadState::BUTTON_WORDS],
            buttons_released: [0; GamepadState::BUTTON_WORDS],
            axes: [0.0; GamepadAxis::COUNT],
            gyro: perro_structs::Vector3::new(0.0, 0.0, 0.0),
            accel: perro_structs::Vector3::new(0.0, 0.0, 0.0),
        }
    }

    #[inline(always)]
    pub fn begin_frame(&mut self) {
        self.buttons_pressed.fill(0);
        self.buttons_released.fill(0);
    }

    #[inline(always)]
    pub fn set_button_state(&mut self, button: GamepadButton, is_down: bool) {
        let idx = button.as_index();
        let word = idx / 64;
        let bit = 1_u64 << (idx % 64);
        let was_down = self.buttons_down[word] & bit != 0;

        if is_down {
            if !was_down {
                self.buttons_down[word] |= bit;
                self.buttons_pressed[word] |= bit;
            }
        } else if was_down {
            self.buttons_down[word] &= !bit;
            self.buttons_released[word] |= bit;
        }
    }

    #[inline(always)]
    pub fn set_axis(&mut self, axis: GamepadAxis, value: f32) {
        self.axes[axis.as_index()] = value;
    }

    #[inline(always)]
    pub fn set_gyro(&mut self, x: f32, y: f32, z: f32) {
        self.gyro = perro_structs::Vector3::new(x, y, z);
    }

    #[inline(always)]
    pub fn set_accel(&mut self, x: f32, y: f32, z: f32) {
        self.accel = perro_structs::Vector3::new(x, y, z);
    }

    #[inline(always)]
    pub fn is_button_down(&self, button: GamepadButton) -> bool {
        self.test(&self.buttons_down, button)
    }

    #[inline(always)]
    pub fn is_button_pressed(&self, button: GamepadButton) -> bool {
        self.test(&self.buttons_pressed, button)
    }

    #[inline(always)]
    pub fn is_button_released(&self, button: GamepadButton) -> bool {
        self.test(&self.buttons_released, button)
    }

    #[inline(always)]
    pub fn axis(&self, axis: GamepadAxis) -> f32 {
        self.axes[axis.as_index()]
    }

    #[inline(always)]
    pub fn left_stick(&self) -> perro_structs::Vector2 {
        perro_structs::Vector2::new(
            self.axis(GamepadAxis::LeftStickX),
            self.axis(GamepadAxis::LeftStickY),
        )
    }

    #[inline(always)]
    pub fn right_stick(&self) -> perro_structs::Vector2 {
        perro_structs::Vector2::new(
            self.axis(GamepadAxis::RightStickX),
            self.axis(GamepadAxis::RightStickY),
        )
    }

    #[inline(always)]
    pub fn gyro(&self) -> perro_structs::Vector3 {
        self.gyro
    }

    #[inline(always)]
    pub fn accel(&self) -> perro_structs::Vector3 {
        self.accel
    }

    #[inline(always)]
    fn test(&self, bits: &[u64; GamepadState::BUTTON_WORDS], button: GamepadButton) -> bool {
        let idx = button.as_index();
        let word = idx / 64;
        let bit = 1_u64 << (idx % 64);
        bits[word] & bit != 0
    }
}

impl Default for GamepadState {
    fn default() -> Self {
        Self::new()
    }
}

#[macro_export]
/// Signature:
/// - `gamepad_list!(&InputContext<_>) -> &[GamepadState]`
///
/// Usage:
/// - `gamepad_list!(ipt) -> &[GamepadState]`
macro_rules! gamepad_list {
    ($ipt:expr) => {{
        let gp = $ipt.Gamepads();
        gp.all()
    }};
}

#[macro_export]
/// Signature:
/// - `gamepad_get!(&InputContext<_>, GamepadIndex) -> Option<&GamepadState>`
///
/// Usage:
/// - `gamepad_get!(ipt, index) -> Option<&GamepadState>`
///
/// `GamepadIndex` is the gamepad slot/index (usually `usize`, for example `0`).
macro_rules! gamepad_get {
    ($ipt:expr, $index:expr) => {{
        let gp = $ipt.Gamepads();
        gp.get($index)
    }};
}

#[macro_export]
/// Signature:
/// - `gamepad_down!(&InputContext<_>, GamepadIndex, GamepadButton) -> bool`
///
/// Usage:
/// - `gamepad_down!(ipt, index, GamepadButton::Bottom) -> bool`
///
/// `GamepadButton` is the gamepad button enum (face buttons, d-pad, shoulders, triggers, sticks, etc.).
///
/// Example:
/// - `if gamepad_down!(ipt, 0, GamepadButton::Bottom) { ... }`
macro_rules! gamepad_down {
    ($ipt:expr, $index:expr, $button:expr) => {{
        let gp = $ipt.Gamepads();
        gp.get($index)
            .map(|gp| gp.is_button_down($button))
            .unwrap_or(false)
    }};
}

#[macro_export]
/// Signature:
/// - `gamepad_pressed!(&InputContext<_>, GamepadIndex, GamepadButton) -> bool`
///
/// Usage:
/// - `gamepad_pressed!(ipt, index, GamepadButton::Bottom) -> bool`
macro_rules! gamepad_pressed {
    ($ipt:expr, $index:expr, $button:expr) => {{
        let gp = $ipt.Gamepads();
        gp.get($index)
            .map(|gp| gp.is_button_pressed($button))
            .unwrap_or(false)
    }};
}

#[macro_export]
/// Signature:
/// - `gamepad_released!(&InputContext<_>, GamepadIndex, GamepadButton) -> bool`
///
/// Usage:
/// - `gamepad_released!(ipt, index, GamepadButton::Bottom) -> bool`
macro_rules! gamepad_released {
    ($ipt:expr, $index:expr, $button:expr) => {{
        let gp = $ipt.Gamepads();
        gp.get($index)
            .map(|gp| gp.is_button_released($button))
            .unwrap_or(false)
    }};
}

#[macro_export]
/// Signature:
/// - `gamepad_left_stick!(&InputContext<_>, GamepadIndex) -> Vector2`
///
/// Usage:
/// - `gamepad_left_stick!(ipt, index) -> Vector2`
macro_rules! gamepad_left_stick {
    ($ipt:expr, $index:expr) => {{
        let gp = $ipt.Gamepads();
        gp.get($index)
            .map(|gp| gp.left_stick())
            .unwrap_or(perro_structs::Vector2::new(0.0, 0.0))
    }};
}

#[macro_export]
/// Signature:
/// - `gamepad_right_stick!(&InputContext<_>, GamepadIndex) -> Vector2`
///
/// Usage:
/// - `gamepad_right_stick!(ipt, index) -> Vector2`
macro_rules! gamepad_right_stick {
    ($ipt:expr, $index:expr) => {{
        let gp = $ipt.Gamepads();
        gp.get($index)
            .map(|gp| gp.right_stick())
            .unwrap_or(perro_structs::Vector2::new(0.0, 0.0))
    }};
}

#[macro_export]
/// Signature:
/// - `gamepad_gyro!(&InputContext<_>, GamepadIndex) -> Vector3`
///
/// Usage:
/// - `gamepad_gyro!(ipt, index) -> Vector3`
macro_rules! gamepad_gyro {
    ($ipt:expr, $index:expr) => {{
        let gp = $ipt.Gamepads();
        gp.get($index)
            .map(|gp| gp.gyro())
            .unwrap_or(perro_structs::Vector3::new(0.0, 0.0, 0.0))
    }};
}

#[macro_export]
/// Signature:
/// - `gamepad_accel!(&InputContext<_>, GamepadIndex) -> Vector3`
///
/// Usage:
/// - `gamepad_accel!(ipt, index) -> Vector3`
macro_rules! gamepad_accel {
    ($ipt:expr, $index:expr) => {{
        let gp = $ipt.Gamepads();
        gp.get($index)
            .map(|gp| gp.accel())
            .unwrap_or(perro_structs::Vector3::new(0.0, 0.0, 0.0))
    }};
}

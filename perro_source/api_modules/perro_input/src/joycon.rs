#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JoyConSide {
    LJoyCon,
    RJoyCon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JoyConButton {
    /// Left Joy-Con: Up. Right Joy-Con: X.
    Top,
    /// Left Joy-Con: Down. Right Joy-Con: B.
    Bottom,
    /// Left Joy-Con: Left. Right Joy-Con: Y.
    Left,
    /// Left Joy-Con: Right. Right Joy-Con: A.
    Right,
    /// Left Joy-Con: L. Right Joy-Con: R.
    Bumper,
    /// Left Joy-Con: ZL. Right Joy-Con: ZR.
    Trigger,
    /// Left Joy-Con: Stick press. Right Joy-Con: Stick press.
    Stick,
    /// Left Joy-Con: SL. Right Joy-Con: SL.
    SL,
    /// Left Joy-Con: SR. Right Joy-Con: SR.
    SR,
    /// Left Joy-Con: Minus. Right Joy-Con: Plus.
    Start,
    /// Left Joy-Con: Capture. Right Joy-Con: Home.
    Meta,
}

impl JoyConButton {
    pub const COUNT: usize = 11;

    #[inline]
    pub fn as_index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug)]
pub struct JoyConState {
    side: JoyConSide,
    buttons_down: [u64; JoyConState::BUTTON_WORDS],
    buttons_pressed: [u64; JoyConState::BUTTON_WORDS],
    buttons_released: [u64; JoyConState::BUTTON_WORDS],
    connected: bool,
    calibrated: bool,
    calibration_in_progress: bool,
    calibration_requested: bool,
    stick_x: f32,
    stick_y: f32,
    calibration_bias: perro_structs::Vector3,
    gyro: perro_structs::Vector3,
    accel: perro_structs::Vector3,
}

impl JoyConState {
    const BUTTON_WORDS: usize = JoyConButton::COUNT.div_ceil(64);

    pub fn new(side: JoyConSide) -> Self {
        Self {
            side,
            buttons_down: [0; JoyConState::BUTTON_WORDS],
            buttons_pressed: [0; JoyConState::BUTTON_WORDS],
            buttons_released: [0; JoyConState::BUTTON_WORDS],
            connected: false,
            calibrated: false,
            calibration_in_progress: false,
            calibration_requested: false,
            stick_x: 0.0,
            stick_y: 0.0,
            calibration_bias: perro_structs::Vector3::new(0.0, 0.0, 0.0),
            gyro: perro_structs::Vector3::new(0.0, 0.0, 0.0),
            accel: perro_structs::Vector3::new(0.0, 0.0, 0.0),
        }
    }

    #[inline(always)]
    pub fn side(&self) -> JoyConSide {
        self.side
    }

    #[inline(always)]
    pub fn set_side(&mut self, side: JoyConSide) {
        if self.side == side {
            return;
        }
        self.side = side;
        self.buttons_down.fill(0);
        self.buttons_pressed.fill(0);
        self.buttons_released.fill(0);
    }

    #[inline(always)]
    pub fn begin_frame(&mut self) {
        self.buttons_pressed.fill(0);
        self.buttons_released.fill(0);
    }

    #[inline(always)]
    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    #[inline(always)]
    pub fn set_calibrated(&mut self, calibrated: bool) {
        self.calibrated = calibrated;
    }

    #[inline(always)]
    pub fn set_calibration_in_progress(&mut self, in_progress: bool) {
        self.calibration_in_progress = in_progress;
    }

    #[inline(always)]
    pub fn set_calibration_requested(&mut self, requested: bool) {
        self.calibration_requested = requested;
    }

    #[inline(always)]
    pub fn set_calibration_bias(&mut self, x: f32, y: f32, z: f32) {
        self.calibration_bias = perro_structs::Vector3::new(x, y, z);
    }

    #[inline(always)]
    pub fn set_button_state(&mut self, button: JoyConButton, is_down: bool) {
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
    pub fn set_stick(&mut self, x: f32, y: f32) {
        self.stick_x = x;
        self.stick_y = y;
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
    pub fn stick_x(&self) -> f32 {
        self.stick_x
    }

    #[inline(always)]
    pub fn stick_y(&self) -> f32 {
        self.stick_y
    }

    #[inline(always)]
    pub fn stick(&self) -> perro_structs::Vector2 {
        perro_structs::Vector2::new(self.stick_x, self.stick_y)
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
    pub fn connected(&self) -> bool {
        self.connected
    }

    #[inline(always)]
    pub fn calibrated(&self) -> bool {
        self.calibrated
    }

    #[inline(always)]
    pub fn calibration_in_progress(&self) -> bool {
        self.calibration_in_progress
    }

    #[inline(always)]
    pub fn calibration_requested(&self) -> bool {
        self.calibration_requested
    }

    #[inline(always)]
    pub fn needs_calibration(&self) -> bool {
        self.connected && !self.calibrated && !self.calibration_in_progress
    }

    #[inline(always)]
    pub fn calibration_bias(&self) -> perro_structs::Vector3 {
        self.calibration_bias
    }

    #[inline(always)]
    pub fn is_button_down(&self, button: JoyConButton) -> bool {
        self.test(&self.buttons_down, button)
    }

    #[inline(always)]
    pub fn is_button_pressed(&self, button: JoyConButton) -> bool {
        self.test(&self.buttons_pressed, button)
    }

    #[inline(always)]
    pub fn is_button_released(&self, button: JoyConButton) -> bool {
        self.test(&self.buttons_released, button)
    }

    #[inline(always)]
    fn test(&self, bits: &[u64; JoyConState::BUTTON_WORDS], button: JoyConButton) -> bool {
        let idx = button.as_index();
        let word = idx / 64;
        let bit = 1_u64 << (idx % 64);
        bits[word] & bit != 0
    }
}

impl Default for JoyConState {
    fn default() -> Self {
        Self::new(JoyConSide::LJoyCon)
    }
}

#[macro_export]
/// Signature:
/// - `joycon_list!(&InputWindow<_>) -> &[JoyConState]`
///
/// Usage:
/// - `joycon_list!(ipt) -> &[JoyConState]`
macro_rules! joycon_list {
    ($ipt:expr) => {{
        let jc = $ipt.JoyCons();
        jc.all()
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_get!(&InputWindow<_>, JoyConIndex) -> Option<&JoyConState>`
///
/// Usage:
/// - `joycon_get!(ipt, index) -> Option<&JoyConState>`
///
/// `JoyConIndex` is the Joy-Con slot/index (usually `usize`, for example `0`).
macro_rules! joycon_get {
    ($ipt:expr, $index:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index)
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_side!(&InputWindow<_>, JoyConIndex) -> Option<JoyConSide>`
///
/// Usage:
/// - `joycon_side!(ipt, index) -> Option<JoyConSide>`
///
/// `JoyConSide` tells you whether that entry is left or right Joy-Con.
macro_rules! joycon_side {
    ($ipt:expr, $index:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index).map(|jc| jc.side())
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_down!(&InputWindow<_>, JoyConIndex, JoyConButton) -> bool`
///
/// Usage:
/// - `joycon_down!(ipt, index, JoyConButton::Top) -> bool`
///
/// `JoyConButton` is the Joy-Con button enum (top/bottom/left/right, bumper/trigger, stick, SL/SR, start/meta).
macro_rules! joycon_down {
    ($ipt:expr, $index:expr, $button:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index)
            .map(|jc| jc.is_button_down($button))
            .unwrap_or(false)
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_pressed!(&InputWindow<_>, JoyConIndex, JoyConButton) -> bool`
///
/// Usage:
/// - `joycon_pressed!(ipt, index, JoyConButton::Top) -> bool`
macro_rules! joycon_pressed {
    ($ipt:expr, $index:expr, $button:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index)
            .map(|jc| jc.is_button_pressed($button))
            .unwrap_or(false)
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_released!(&InputWindow<_>, JoyConIndex, JoyConButton) -> bool`
///
/// Usage:
/// - `joycon_released!(ipt, index, JoyConButton::Top) -> bool`
macro_rules! joycon_released {
    ($ipt:expr, $index:expr, $button:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index)
            .map(|jc| jc.is_button_released($button))
            .unwrap_or(false)
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_stick!(&InputWindow<_>, JoyConIndex) -> Vector2`
///
/// Usage:
/// - `joycon_stick!(ipt, index) -> Vector2`
macro_rules! joycon_stick {
    ($ipt:expr, $index:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index)
            .map(|jc| jc.stick())
            .unwrap_or(perro_structs::Vector2::new(0.0, 0.0))
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_gyro!(&InputWindow<_>, JoyConIndex) -> Vector3`
///
/// Usage:
/// - `joycon_gyro!(ipt, index) -> Vector3`
macro_rules! joycon_gyro {
    ($ipt:expr, $index:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index)
            .map(|jc| jc.gyro())
            .unwrap_or(perro_structs::Vector3::new(0.0, 0.0, 0.0))
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_accel!(&InputWindow<_>, JoyConIndex) -> Vector3`
///
/// Usage:
/// - `joycon_accel!(ipt, index) -> Vector3`
macro_rules! joycon_accel {
    ($ipt:expr, $index:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index)
            .map(|jc| jc.accel())
            .unwrap_or(perro_structs::Vector3::new(0.0, 0.0, 0.0))
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_connected!(&InputWindow<_>, JoyConIndex) -> bool`
macro_rules! joycon_connected {
    ($ipt:expr, $index:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index).map(|jc| jc.connected()).unwrap_or(false)
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_calibrated!(&InputWindow<_>, JoyConIndex) -> bool`
macro_rules! joycon_calibrated {
    ($ipt:expr, $index:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index).map(|jc| jc.calibrated()).unwrap_or(false)
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_calibrating!(&InputWindow<_>, JoyConIndex) -> bool`
macro_rules! joycon_calibrating {
    ($ipt:expr, $index:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index)
            .map(|jc| jc.calibration_in_progress())
            .unwrap_or(false)
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_needs_calibration!(&InputWindow<_>, JoyConIndex) -> bool`
macro_rules! joycon_needs_calibration {
    ($ipt:expr, $index:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index)
            .map(|jc| jc.needs_calibration())
            .unwrap_or(false)
    }};
}

#[macro_export]
/// Signature:
/// - `joycon_calibration_bias!(&InputWindow<_>, JoyConIndex) -> Vector3`
macro_rules! joycon_calibration_bias {
    ($ipt:expr, $index:expr) => {{
        let jc = $ipt.JoyCons();
        jc.get($index)
            .map(|jc| jc.calibration_bias())
            .unwrap_or(perro_structs::Vector3::new(0.0, 0.0, 0.0))
    }};
}

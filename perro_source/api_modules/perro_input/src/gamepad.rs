#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    South,
    East,
    West,
    North,
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
    buttons_down: Vec<u64>,
    buttons_pressed: Vec<u64>,
    buttons_released: Vec<u64>,
    axes: [f32; GamepadAxis::COUNT],
}

impl GamepadState {
    pub fn new() -> Self {
        let words = GamepadButton::COUNT.div_ceil(64);
        Self {
            buttons_down: vec![0; words],
            buttons_pressed: vec![0; words],
            buttons_released: vec![0; words],
            axes: [0.0; GamepadAxis::COUNT],
        }
    }

    #[inline]
    pub fn begin_frame(&mut self) {
        self.buttons_pressed.fill(0);
        self.buttons_released.fill(0);
    }

    #[inline]
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

    #[inline]
    pub fn set_axis(&mut self, axis: GamepadAxis, value: f32) {
        self.axes[axis.as_index()] = value;
    }

    #[inline]
    pub fn is_button_down(&self, button: GamepadButton) -> bool {
        self.test(&self.buttons_down, button)
    }

    #[inline]
    pub fn is_button_pressed(&self, button: GamepadButton) -> bool {
        self.test(&self.buttons_pressed, button)
    }

    #[inline]
    pub fn is_button_released(&self, button: GamepadButton) -> bool {
        self.test(&self.buttons_released, button)
    }

    #[inline]
    pub fn axis(&self, axis: GamepadAxis) -> f32 {
        self.axes[axis.as_index()]
    }

    #[inline]
    fn test(&self, bits: &[u64], button: GamepadButton) -> bool {
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

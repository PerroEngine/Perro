#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JoyConButton {
    Up,
    Down,
    Left,
    Right,
    Minus,
    Plus,
    Home,
    Capture,
    L,
    R,
    ZL,
    ZR,
    Stick,
    SL,
    SR,
}

impl JoyConButton {
    pub const COUNT: usize = 15;

    #[inline]
    pub fn as_index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JoyConAxis {
    StickX,
    StickY,
}

impl JoyConAxis {
    pub const COUNT: usize = 2;

    #[inline]
    pub fn as_index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug)]
pub struct JoyConState {
    buttons_down: Vec<u64>,
    buttons_pressed: Vec<u64>,
    buttons_released: Vec<u64>,
    axes: [f32; JoyConAxis::COUNT],
}

impl JoyConState {
    pub fn new() -> Self {
        let words = JoyConButton::COUNT.div_ceil(64);
        Self {
            buttons_down: vec![0; words],
            buttons_pressed: vec![0; words],
            buttons_released: vec![0; words],
            axes: [0.0; JoyConAxis::COUNT],
        }
    }

    #[inline]
    pub fn begin_frame(&mut self) {
        self.buttons_pressed.fill(0);
        self.buttons_released.fill(0);
    }

    #[inline]
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

    #[inline]
    pub fn set_axis(&mut self, axis: JoyConAxis, value: f32) {
        self.axes[axis.as_index()] = value;
    }

    #[inline]
    pub fn is_button_down(&self, button: JoyConButton) -> bool {
        self.test(&self.buttons_down, button)
    }

    #[inline]
    pub fn is_button_pressed(&self, button: JoyConButton) -> bool {
        self.test(&self.buttons_pressed, button)
    }

    #[inline]
    pub fn is_button_released(&self, button: JoyConButton) -> bool {
        self.test(&self.buttons_released, button)
    }

    #[inline]
    pub fn axis(&self, axis: JoyConAxis) -> f32 {
        self.axes[axis.as_index()]
    }

    #[inline]
    fn test(&self, bits: &[u64], button: JoyConButton) -> bool {
        let idx = button.as_index();
        let word = idx / 64;
        let bit = 1_u64 << (idx % 64);
        bits[word] & bit != 0
    }
}

impl Default for JoyConState {
    fn default() -> Self {
        Self::new()
    }
}

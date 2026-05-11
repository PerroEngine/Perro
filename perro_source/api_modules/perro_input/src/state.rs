use super::*;

#[derive(Clone, Debug)]
pub struct KeyboardState {
    down: Vec<u64>,
    pressed: Vec<u64>,
    released: Vec<u64>,
    text_inputs: Vec<String>,
}

impl KeyboardState {
    pub fn new() -> Self {
        let words = KeyCode::COUNT.div_ceil(64);
        Self {
            down: vec![0; words],
            pressed: vec![0; words],
            released: vec![0; words],
            text_inputs: Vec::new(),
        }
    }

    #[inline]
    pub fn begin_frame(&mut self) {
        self.pressed.fill(0);
        self.released.fill(0);
        self.text_inputs.clear();
    }

    #[inline]
    pub fn set_key_state(&mut self, key: KeyCode, is_down: bool) {
        let idx = key.as_index();
        let word = idx / 64;
        let bit = 1_u64 << (idx % 64);
        let was_down = self.down[word] & bit != 0;

        if is_down {
            if !was_down {
                self.down[word] |= bit;
                self.pressed[word] |= bit;
            }
        } else if was_down {
            self.down[word] &= !bit;
            self.released[word] |= bit;
        }
    }

    #[inline]
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.test(&self.down, key)
    }

    #[inline]
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.test(&self.pressed, key)
    }

    #[inline]
    pub fn is_key_released(&self, key: KeyCode) -> bool {
        self.test(&self.released, key)
    }

    #[inline]
    pub fn push_text_input(&mut self, text: impl Into<String>) {
        let text = text.into();
        if !text.is_empty() {
            self.text_inputs.push(text);
        }
    }

    #[inline]
    pub fn text_inputs(&self) -> &[String] {
        &self.text_inputs
    }

    #[inline]
    fn test(&self, bits: &[u64], key: KeyCode) -> bool {
        let idx = key.as_index();
        let word = idx / 64;
        let bit = 1_u64 << (idx % 64);
        bits[word] & bit != 0
    }
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct MouseState {
    down: u8,
    pressed: u8,
    released: u8,
    delta_x: f32,
    delta_y: f32,
    wheel_x: f32,
    wheel_y: f32,
    position_x: f32,
    position_y: f32,
    viewport_width: f32,
    viewport_height: f32,
    mode: MouseMode,
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            down: 0,
            pressed: 0,
            released: 0,
            delta_x: 0.0,
            delta_y: 0.0,
            wheel_x: 0.0,
            wheel_y: 0.0,
            position_x: 0.0,
            position_y: 0.0,
            viewport_width: 1.0,
            viewport_height: 1.0,
            mode: MouseMode::Visible,
        }
    }

    #[inline]
    pub fn begin_frame(&mut self) {
        self.pressed = 0;
        self.released = 0;
        self.delta_x = 0.0;
        self.delta_y = 0.0;
        self.wheel_x = 0.0;
        self.wheel_y = 0.0;
    }

    #[inline]
    pub fn set_button_state(&mut self, button: MouseButton, is_down: bool) {
        let bit = button.bit();
        let was_down = self.down & bit != 0;

        if is_down {
            if !was_down {
                self.down |= bit;
                self.pressed |= bit;
            }
        } else if was_down {
            self.down &= !bit;
            self.released |= bit;
        }
    }

    #[inline]
    pub fn add_delta(&mut self, dx: f32, dy: f32) {
        self.delta_x += dx;
        self.delta_y += dy;
    }

    #[inline]
    pub fn add_wheel(&mut self, dx: f32, dy: f32) {
        self.wheel_x += dx;
        self.wheel_y += dy;
    }

    #[inline]
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position_x = x;
        self.position_y = y;
    }

    #[inline]
    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.viewport_width = (width.max(1)) as f32;
        self.viewport_height = (height.max(1)) as f32;
    }

    #[inline]
    pub fn set_mode(&mut self, mode: MouseMode) {
        self.mode = mode;
    }

    #[inline]
    pub fn mode(&self) -> MouseMode {
        self.mode
    }

    #[inline]
    pub fn is_button_down(&self, button: MouseButton) -> bool {
        self.down & button.bit() != 0
    }

    #[inline]
    pub fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.pressed & button.bit() != 0
    }

    #[inline]
    pub fn is_button_released(&self, button: MouseButton) -> bool {
        self.released & button.bit() != 0
    }

    #[inline]
    pub fn delta(&self) -> Vector2 {
        Vector2::new(self.delta_x, self.delta_y)
    }

    #[inline]
    pub fn wheel(&self) -> Vector2 {
        Vector2::new(self.wheel_x, self.wheel_y)
    }

    #[inline]
    pub fn position(&self) -> Vector2 {
        let x = (self.position_x / self.viewport_width).clamp(0.0, 1.0);
        let y = 1.0 - (self.position_y / self.viewport_height).clamp(0.0, 1.0);
        Vector2::new(x, y)
    }

    #[inline]
    pub fn viewport_size(&self) -> Vector2 {
        Vector2::new(self.viewport_width, self.viewport_height)
    }
}

impl Default for MouseState {
    fn default() -> Self {
        Self::new()
    }
}

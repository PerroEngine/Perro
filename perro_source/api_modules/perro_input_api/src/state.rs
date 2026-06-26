use super::*;

/// Keyboard button state for the current input frame.
///
/// `down` persists while a key is held. `pressed` and `released` are one-frame
/// edges and are cleared by [`KeyboardState::begin_frame`].
#[derive(Clone, Debug)]
pub struct KeyboardState {
    down: Vec<u64>,
    pressed: Vec<u64>,
    released: Vec<u64>,
    text_inputs: Vec<String>,
}

impl KeyboardState {
    // ---- Lifecycle ----

    /// Create an empty keyboard state with bit storage for every [`KeyCode`].
    pub fn new() -> Self {
        let words = KeyCode::COUNT.div_ceil(64);
        Self {
            down: vec![0; words],
            pressed: vec![0; words],
            released: vec![0; words],
            text_inputs: Vec::new(),
        }
    }

    /// Clear one-frame edges and text input before new input events arrive.
    #[inline]
    pub fn begin_frame(&mut self) {
        self.pressed.fill(0);
        self.released.fill(0);
        self.text_inputs.clear();
    }

    /// Release every held key and clear one-frame keyboard state.
    #[inline]
    pub fn clear(&mut self) {
        self.down.fill(0);
        self.pressed.fill(0);
        self.released.fill(0);
        self.text_inputs.clear();
    }

    // ---- Event input ----

    /// Apply a key transition and update one-frame pressed/released edges.
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

    // ---- Query ----

    /// Return `true` while the key is held.
    #[inline]
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.test(&self.down, key)
    }

    /// Return `true` only on the frame the key changes from up to down.
    #[inline]
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.test(&self.pressed, key)
    }

    /// Return `true` only on the frame the key changes from down to up.
    #[inline]
    pub fn is_key_released(&self, key: KeyCode) -> bool {
        self.test(&self.released, key)
    }

    /// Push text input received this frame.
    #[inline]
    pub fn push_text_input(&mut self, text: impl Into<String>) {
        let text = text.into();
        if !text.is_empty() {
            self.text_inputs.push(text);
        }
    }

    /// Return text input chunks received during the current frame.
    #[inline]
    pub fn text_inputs(&self) -> &[String] {
        &self.text_inputs
    }

    // ---- Bit helpers ----

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

/// Mouse button, motion, wheel, position, viewport, and mode state.
///
/// Button edges are one-frame values. Mouse position is stored in window pixels
/// and returned normalized to viewport space by [`MouseState::position`].
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
    // ---- Lifecycle ----

    /// Create empty mouse state with a 1x1 fallback viewport.
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

    /// Clear one-frame button edges, motion delta, and wheel delta.
    #[inline]
    pub fn begin_frame(&mut self) {
        self.pressed = 0;
        self.released = 0;
        self.clear_delta();
        self.wheel_x = 0.0;
        self.wheel_y = 0.0;
    }

    /// Release every held button and clear one-frame mouse state.
    #[inline]
    pub fn clear_buttons_and_motion(&mut self) {
        self.down = 0;
        self.pressed = 0;
        self.released = 0;
        self.clear_delta();
        self.wheel_x = 0.0;
        self.wheel_y = 0.0;
    }

    /// Clear only accumulated mouse motion delta.
    #[inline]
    pub fn clear_delta(&mut self) {
        self.delta_x = 0.0;
        self.delta_y = 0.0;
    }

    // ---- Event input ----

    /// Apply a mouse button transition and update one-frame edges.
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

    /// Add relative mouse movement in pixels.
    #[inline]
    pub fn add_delta(&mut self, dx: f32, dy: f32) {
        self.delta_x += dx;
        self.delta_y += dy;
    }

    /// Add scroll-wheel movement for this frame.
    #[inline]
    pub fn add_wheel(&mut self, dx: f32, dy: f32) {
        self.wheel_x += dx;
        self.wheel_y += dy;
    }

    /// Set absolute mouse position in window pixels.
    #[inline]
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position_x = x;
        self.position_y = y;
    }

    /// Set viewport size used for normalized mouse position.
    #[inline]
    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.viewport_width = (width.max(1)) as f32;
        self.viewport_height = (height.max(1)) as f32;
    }

    /// Set current mouse mode reported to scripts.
    #[inline]
    pub fn set_mode(&mut self, mode: MouseMode) {
        self.mode = mode;
    }

    // ---- Query ----

    /// Return current mouse mode.
    #[inline]
    pub fn mode(&self) -> MouseMode {
        self.mode
    }

    /// Return `true` while the button is held.
    #[inline]
    pub fn is_button_down(&self, button: MouseButton) -> bool {
        self.down & button.bit() != 0
    }

    /// Return `true` only on the frame the button changes from up to down.
    #[inline]
    pub fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.pressed & button.bit() != 0
    }

    /// Return `true` only on the frame the button changes from down to up.
    #[inline]
    pub fn is_button_released(&self, button: MouseButton) -> bool {
        self.released & button.bit() != 0
    }

    /// Return accumulated relative movement in pixels for this frame.
    #[inline]
    pub fn delta(&self) -> Vector2 {
        Vector2::new(self.delta_x, self.delta_y)
    }

    /// Return accumulated wheel movement for this frame.
    #[inline]
    pub fn wheel(&self) -> Vector2 {
        Vector2::new(self.wheel_x, self.wheel_y)
    }

    /// Return normalized viewport position where bottom-left is `(0, 0)`.
    #[inline]
    pub fn position(&self) -> Vector2 {
        let x = (self.position_x / self.viewport_width).clamp(0.0, 1.0);
        let y = 1.0 - (self.position_y / self.viewport_height).clamp(0.0, 1.0);
        Vector2::new(x, y)
    }

    /// Return viewport size in pixels.
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

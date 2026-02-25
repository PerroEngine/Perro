mod keycode;
mod mouse_button;

pub use keycode::KeyCode;
pub use mouse_button::MouseButton;

#[derive(Clone, Debug)]
pub struct InputSnapshot {
    down: Vec<u64>,
    pressed: Vec<u64>,
    released: Vec<u64>,
    mouse_down: u8,
    mouse_pressed: u8,
    mouse_released: u8,
    mouse_delta_x: f32,
    mouse_delta_y: f32,
}

impl InputSnapshot {
    pub fn new() -> Self {
        let words = KeyCode::COUNT.div_ceil(64);
        Self {
            down: vec![0; words],
            pressed: vec![0; words],
            released: vec![0; words],
            mouse_down: 0,
            mouse_pressed: 0,
            mouse_released: 0,
            mouse_delta_x: 0.0,
            mouse_delta_y: 0.0,
        }
    }

    #[inline]
    pub fn begin_frame(&mut self) {
        self.pressed.fill(0);
        self.released.fill(0);
        self.mouse_pressed = 0;
        self.mouse_released = 0;
        self.mouse_delta_x = 0.0;
        self.mouse_delta_y = 0.0;
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
    pub fn set_mouse_button_state(&mut self, button: MouseButton, is_down: bool) {
        let bit = button.bit();
        let was_down = self.mouse_down & bit != 0;

        if is_down {
            if !was_down {
                self.mouse_down |= bit;
                self.mouse_pressed |= bit;
            }
        } else if was_down {
            self.mouse_down &= !bit;
            self.mouse_released |= bit;
        }
    }

    #[inline]
    pub fn add_mouse_delta(&mut self, dx: f32, dy: f32) {
        self.mouse_delta_x += dx;
        self.mouse_delta_y += dy;
    }

    #[inline]
    pub fn is_mouse_down(&self, button: MouseButton) -> bool {
        self.mouse_down & button.bit() != 0
    }

    #[inline]
    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed & button.bit() != 0
    }

    #[inline]
    pub fn is_mouse_released(&self, button: MouseButton) -> bool {
        self.mouse_released & button.bit() != 0
    }

    #[inline]
    pub fn mouse_delta(&self) -> (f32, f32) {
        (self.mouse_delta_x, self.mouse_delta_y)
    }

    #[inline]
    fn test(&self, bits: &[u64], key: KeyCode) -> bool {
        let idx = key.as_index();
        let word = idx / 64;
        let bit = 1_u64 << (idx % 64);
        bits[word] & bit != 0
    }
}

impl Default for InputSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

pub trait InputAPI {
    fn is_key_down(&self, key: KeyCode) -> bool;
    fn is_key_pressed(&self, key: KeyCode) -> bool;
    fn is_key_released(&self, key: KeyCode) -> bool;
    fn is_mouse_down(&self, button: MouseButton) -> bool;
    fn is_mouse_pressed(&self, button: MouseButton) -> bool;
    fn is_mouse_released(&self, button: MouseButton) -> bool;
    fn mouse_delta(&self) -> (f32, f32);
}

impl InputAPI for InputSnapshot {
    #[inline]
    fn is_key_down(&self, key: KeyCode) -> bool {
        self.is_key_down(key)
    }

    #[inline]
    fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.is_key_pressed(key)
    }

    #[inline]
    fn is_key_released(&self, key: KeyCode) -> bool {
        self.is_key_released(key)
    }

    #[inline]
    fn is_mouse_down(&self, button: MouseButton) -> bool {
        self.is_mouse_down(button)
    }

    #[inline]
    fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.is_mouse_pressed(button)
    }

    #[inline]
    fn is_mouse_released(&self, button: MouseButton) -> bool {
        self.is_mouse_released(button)
    }

    #[inline]
    fn mouse_delta(&self) -> (f32, f32) {
        self.mouse_delta()
    }
}

pub struct InputContext<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

#[allow(non_snake_case)]
impl<'ipt, IP: InputAPI + ?Sized> InputContext<'ipt, IP> {
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    #[inline]
    pub fn Keys(&self) -> KeyModule<'_, IP> {
        KeyModule::new(self.ipt)
    }

    #[inline]
    pub fn Mouse(&self) -> MouseModule<'_, IP> {
        MouseModule::new(self.ipt)
    }
}

pub struct KeyModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> KeyModule<'ipt, IP> {
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    #[inline]
    pub fn down(&self, key: KeyCode) -> bool {
        self.ipt.is_key_down(key)
    }

    #[inline]
    pub fn pressed(&self, key: KeyCode) -> bool {
        self.ipt.is_key_pressed(key)
    }

    #[inline]
    pub fn released(&self, key: KeyCode) -> bool {
        self.ipt.is_key_released(key)
    }
}

pub struct MouseModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> MouseModule<'ipt, IP> {
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    #[inline]
    pub fn down(&self, button: MouseButton) -> bool {
        self.ipt.is_mouse_down(button)
    }

    #[inline]
    pub fn pressed(&self, button: MouseButton) -> bool {
        self.ipt.is_mouse_pressed(button)
    }

    #[inline]
    pub fn released(&self, button: MouseButton) -> bool {
        self.ipt.is_mouse_released(button)
    }

    #[inline]
    pub fn delta(&self) -> (f32, f32) {
        self.ipt.mouse_delta()
    }
}

pub mod prelude {
    pub use crate::{
        InputAPI, InputContext, InputSnapshot, KeyCode, KeyModule, MouseButton, MouseModule,
    };
}

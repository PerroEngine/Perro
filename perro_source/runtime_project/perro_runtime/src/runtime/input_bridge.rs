use super::Runtime;
use perro_input::{GamepadAxis, GamepadButton, KeyCode, MouseButton};

impl Runtime {
    #[inline]
    pub fn begin_input_frame(&mut self) {
        self.input.apply_queued_commands();
        self.input.begin_frame();
    }

    #[inline]
    pub fn set_key_state(&mut self, key: KeyCode, is_down: bool) {
        self.input.set_key_state(key, is_down);
    }

    #[inline]
    pub fn set_mouse_button_state(&mut self, button: MouseButton, is_down: bool) {
        self.input.set_mouse_button_state(button, is_down);
    }

    #[inline]
    pub fn add_mouse_delta(&mut self, dx: f32, dy: f32) {
        self.input.add_mouse_delta(dx, dy);
    }

    #[inline]
    pub fn add_mouse_wheel(&mut self, dx: f32, dy: f32) {
        self.input.add_mouse_wheel(dx, dy);
    }

    #[inline]
    pub fn set_mouse_position(&mut self, x: f32, y: f32) {
        self.input.set_mouse_position(x, y);
    }

    #[inline]
    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.input.set_viewport_size(width, height);
    }

    #[inline]
    pub fn set_gamepad_button_state(&mut self, index: usize, button: GamepadButton, is_down: bool) {
        self.input.set_gamepad_button_state(index, button, is_down);
    }

    #[inline]
    pub fn set_gamepad_axis(&mut self, index: usize, axis: GamepadAxis, value: f32) {
        self.input.set_gamepad_axis(index, axis, value);
    }

    #[inline]
    pub fn set_gamepad_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.input.set_gamepad_gyro(index, x, y, z);
    }

    #[inline]
    pub fn set_gamepad_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.input.set_gamepad_accel(index, x, y, z);
    }

    #[inline]
    pub fn set_joycon_button_state(
        &mut self,
        index: usize,
        button: perro_input::JoyConButton,
        is_down: bool,
    ) {
        self.input.set_joycon_button_state(index, button, is_down);
    }

    #[inline]
    pub fn set_joycon_stick(&mut self, index: usize, x: f32, y: f32) {
        self.input.set_joycon_stick(index, x, y);
    }

    #[inline]
    pub fn set_joycon_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.input.set_joycon_gyro(index, x, y, z);
    }

    #[inline]
    pub fn set_joycon_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.input.set_joycon_accel(index, x, y, z);
    }
}

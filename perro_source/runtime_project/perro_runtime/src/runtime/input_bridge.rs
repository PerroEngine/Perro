use super::Runtime;
use perro_input::{
    GamepadAxis, GamepadButton, GamepadRumbleRequest, JoyConIndicatorRequest, JoyConRumbleRequest,
    KeyCode, MouseButton, MouseMode, PlayerBinding, PlayerState,
};

impl Runtime {
    #[inline]
    pub fn begin_input_frame(&mut self) {
        self.input.apply_queued_commands();
        self.input.begin_frame();
    }

    #[inline]
    pub fn apply_input_commands(&mut self) {
        self.input.apply_queued_commands();
    }

    #[inline]
    pub fn set_key_state(&mut self, key: KeyCode, is_down: bool) {
        self.input.set_key_state(key, is_down);
    }

    #[inline]
    pub fn push_text_input(&mut self, text: impl Into<String>) {
        self.input.push_text_input(text);
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
    pub fn set_mouse_mode_state(&mut self, mode: MouseMode) {
        self.input.set_mouse_mode_state(mode);
    }

    #[inline]
    pub fn mouse_mode(&self) -> MouseMode {
        self.input.mouse_mode()
    }

    #[inline]
    pub fn take_mouse_mode_request(&mut self) -> Option<MouseMode> {
        self.input.take_mouse_mode_request()
    }

    #[inline]
    pub fn set_cursor_icon_request(&mut self, icon: perro_ui::CursorIcon) {
        self.cursor_icon_request = Some(icon);
    }

    #[inline]
    pub fn take_cursor_icon_request(&mut self) -> Option<perro_ui::CursorIcon> {
        self.cursor_icon_request.take()
    }

    #[inline]
    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        let old_size = self.input.viewport_size();
        self.input.set_viewport_size(width, height);
        self.resource_api.set_viewport_size(width, height);
        if self.input.viewport_size() != old_size {
            self.mark_ui_viewport_dirty();
        }
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
    pub fn set_joycon_side(&mut self, index: usize, side: perro_input::JoyConSide) {
        self.input.set_joycon_side(index, side);
    }

    #[inline]
    pub fn set_joycon_connected(&mut self, index: usize, connected: bool) {
        self.input.set_joycon_connected(index, connected);
    }

    #[inline]
    pub fn set_joycon_calibrated(&mut self, index: usize, calibrated: bool) {
        self.input.set_joycon_calibrated(index, calibrated);
    }

    #[inline]
    pub fn set_joycon_calibration_in_progress(&mut self, index: usize, in_progress: bool) {
        self.input
            .set_joycon_calibration_in_progress(index, in_progress);
    }

    #[inline]
    pub fn set_joycon_calibration_bias(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.input.set_joycon_calibration_bias(index, x, y, z);
    }

    #[inline]
    pub fn set_joycon_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.input.set_joycon_gyro(index, x, y, z);
    }

    #[inline]
    pub fn set_joycon_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.input.set_joycon_accel(index, x, y, z);
    }

    #[inline]
    pub fn take_joycon_calibration_requests(&mut self) -> Vec<usize> {
        self.input.take_joycon_calibration_requests()
    }

    #[inline]
    pub fn take_gamepad_rumble_requests(&mut self) -> Vec<GamepadRumbleRequest> {
        self.input.take_gamepad_rumble_requests()
    }

    #[inline]
    pub fn take_joycon_rumble_requests(&mut self) -> Vec<JoyConRumbleRequest> {
        self.input.take_joycon_rumble_requests()
    }

    #[inline]
    pub fn take_joycon_indicator_requests(&mut self) -> Vec<JoyConIndicatorRequest> {
        self.input.take_joycon_indicator_requests()
    }

    #[inline]
    pub fn players(&self) -> &[PlayerState] {
        self.input.players()
    }

    #[inline]
    pub fn bind_player(&mut self, index: usize, binding: PlayerBinding) {
        self.input.bind_player(index, binding);
    }
}

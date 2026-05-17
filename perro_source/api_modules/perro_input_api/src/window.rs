use super::*;

/// InputWindow is a wrapper around the InputAPI that provides access to various input-related sub-APIs. It is designed to be passed to scripts as part of the ScriptContext, allowing them to interact with input in a structured way.
pub struct InputWindow<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

#[allow(non_snake_case)]
impl<'ipt, IP: InputAPI + ?Sized> InputWindow<'ipt, IP> {
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

    #[inline]
    pub fn Keyboard(&self) -> KeyboardModule<'_, IP> {
        KeyboardModule::new(self.ipt)
    }

    #[inline]
    pub fn MouseState(&self) -> MouseStateModule<'_, IP> {
        MouseStateModule::new(self.ipt)
    }

    #[inline(always)]
    pub fn Gamepads(&self) -> GamepadModule<'_, IP> {
        GamepadModule::new(self.ipt)
    }

    #[inline(always)]
    pub fn JoyCons(&self) -> JoyConModule<'_, IP> {
        JoyConModule::new(self.ipt)
    }

    #[inline]
    /// Access Player bindings and state.
    pub fn Players(&self) -> PlayerModule<'_, IP> {
        PlayerModule::new(self.ipt)
    }

    #[inline]
    pub fn Actions(&self) -> ActionModule<'_, IP> {
        ActionModule::new(self.ipt)
    }

    #[inline]
    pub fn bind_player(&self, index: usize, binding: PlayerBinding) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::BindPlayer { index, binding });
        }
    }

    #[inline]
    pub fn request_joycon_calibration(&self, index: usize) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::RequestJoyConCalibration { index });
        }
    }

    #[inline]
    pub fn set_mouse_mode(&self, mode: MouseMode) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::SetMouseMode { mode });
        }
    }

    #[inline]
    pub fn mouse_mode(&self) -> MouseMode {
        self.ipt.mouse().mode()
    }

    #[inline]
    pub fn set_gamepad_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer.borrow_mut().push(InputCommand::SetGamepadRumble {
                index,
                rumble: RumbleIntensity::new(low_frequency, high_frequency),
            });
        }
    }

    #[inline]
    pub fn set_joycon_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer.borrow_mut().push(InputCommand::SetJoyConRumble {
                index,
                rumble: RumbleIntensity::new(low_frequency, high_frequency),
            });
        }
    }

    #[inline]
    pub fn set_joycon_indicator(&self, index: usize, indicator: u8) {
        let Some(indicator) = PlayerIndicatorSlot::from_slot_or_lamp_pattern(indicator) else {
            return;
        };
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::SetJoyConIndicator { index, indicator });
        }
    }

    #[inline]
    pub fn set_joycon_indicator_slot(&self, index: usize, slot: u8) {
        let Some(slot) = PlayerIndicatorSlot::from_slot(slot) else {
            return;
        };
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer.borrow_mut().push(InputCommand::SetJoyConIndicator {
                index,
                indicator: slot,
            });
        }
    }
}

pub struct ActionModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> ActionModule<'ipt, IP> {
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    #[inline]
    pub fn down(&self, name: &str) -> bool {
        self.down_hash(action_hash(name))
    }

    #[inline]
    pub fn pressed(&self, name: &str) -> bool {
        self.pressed_hash(action_hash(name))
    }

    #[inline]
    pub fn released(&self, name: &str) -> bool {
        self.released_hash(action_hash(name))
    }

    #[inline]
    pub fn down_hash(&self, name_hash: u64) -> bool {
        self.ipt.action_down_hash(name_hash)
    }

    #[inline]
    pub fn pressed_hash(&self, name_hash: u64) -> bool {
        self.ipt.action_pressed_hash(name_hash)
    }

    #[inline]
    pub fn released_hash(&self, name_hash: u64) -> bool {
        self.ipt.action_released_hash(name_hash)
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
        self.ipt.keyboard().is_key_down(key)
    }

    #[inline]
    pub fn pressed(&self, key: KeyCode) -> bool {
        self.ipt.keyboard().is_key_pressed(key)
    }

    #[inline]
    pub fn released(&self, key: KeyCode) -> bool {
        self.ipt.keyboard().is_key_released(key)
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
        self.ipt.mouse().is_button_down(button)
    }

    #[inline]
    pub fn pressed(&self, button: MouseButton) -> bool {
        self.ipt.mouse().is_button_pressed(button)
    }

    #[inline]
    pub fn released(&self, button: MouseButton) -> bool {
        self.ipt.mouse().is_button_released(button)
    }

    #[inline]
    pub fn delta(&self) -> Vector2 {
        self.ipt.mouse().delta()
    }

    #[inline]
    pub fn wheel(&self) -> Vector2 {
        self.ipt.mouse().wheel()
    }

    #[inline]
    pub fn position(&self) -> Vector2 {
        self.ipt.mouse().position()
    }

    #[inline]
    pub fn viewport_size(&self) -> Vector2 {
        self.ipt.mouse().viewport_size()
    }

    #[inline]
    pub fn mode(&self) -> MouseMode {
        self.ipt.mouse().mode()
    }

    #[inline]
    pub fn set_mode(&self, mode: MouseMode) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::SetMouseMode { mode });
        }
    }

    #[inline]
    pub fn show(&self) {
        self.set_mode(MouseMode::Visible);
    }

    #[inline]
    pub fn hide(&self) {
        self.set_mode(MouseMode::Hidden);
    }

    #[inline]
    pub fn capture(&self) {
        self.set_mode(MouseMode::Captured);
    }

    #[inline]
    pub fn confine(&self) {
        self.set_mode(MouseMode::Confined);
    }

    #[inline]
    pub fn confine_hidden(&self) {
        self.set_mode(MouseMode::ConfinedHidden);
    }
}

pub struct KeyboardModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> KeyboardModule<'ipt, IP> {
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    #[inline]
    pub fn state(&self) -> &'ipt KeyboardState {
        self.ipt.keyboard()
    }

    #[inline]
    pub fn down(&self, key: KeyCode) -> bool {
        self.ipt.keyboard().is_key_down(key)
    }

    #[inline]
    pub fn pressed(&self, key: KeyCode) -> bool {
        self.ipt.keyboard().is_key_pressed(key)
    }

    #[inline]
    pub fn released(&self, key: KeyCode) -> bool {
        self.ipt.keyboard().is_key_released(key)
    }
}

pub struct MouseStateModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> MouseStateModule<'ipt, IP> {
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    #[inline]
    pub fn state(&self) -> &'ipt MouseState {
        self.ipt.mouse()
    }

    #[inline]
    pub fn down(&self, button: MouseButton) -> bool {
        self.ipt.mouse().is_button_down(button)
    }

    #[inline]
    pub fn pressed(&self, button: MouseButton) -> bool {
        self.ipt.mouse().is_button_pressed(button)
    }

    #[inline]
    pub fn released(&self, button: MouseButton) -> bool {
        self.ipt.mouse().is_button_released(button)
    }

    #[inline]
    pub fn delta(&self) -> Vector2 {
        self.ipt.mouse().delta()
    }

    #[inline]
    pub fn wheel(&self) -> Vector2 {
        self.ipt.mouse().wheel()
    }

    #[inline]
    pub fn position(&self) -> Vector2 {
        self.ipt.mouse().position()
    }

    #[inline]
    pub fn viewport_size(&self) -> Vector2 {
        self.ipt.mouse().viewport_size()
    }

    #[inline]
    pub fn mode(&self) -> MouseMode {
        self.ipt.mouse().mode()
    }
}

pub struct GamepadModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> GamepadModule<'ipt, IP> {
    #[inline(always)]
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    #[inline(always)]
    pub fn all(&self) -> &'ipt [GamepadState] {
        self.ipt.gamepads()
    }

    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&'ipt GamepadState> {
        self.ipt.gamepads().get(index)
    }

    #[inline(always)]
    pub fn set_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer.borrow_mut().push(InputCommand::SetGamepadRumble {
                index,
                rumble: RumbleIntensity::new(low_frequency, high_frequency),
            });
        }
    }
}

pub struct JoyConModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> JoyConModule<'ipt, IP> {
    /// Creates a Joy-Con access wrapper.
    #[inline(always)]
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    #[inline(always)]
    /// Returns all Joy-Con states (each entry is a single Joy-Con controller).
    pub fn all(&self) -> &'ipt [JoyConState] {
        self.ipt.joycons()
    }

    #[inline(always)]
    /// Returns the Joy-Con at the given index.
    pub fn get(&self, index: usize) -> Option<&'ipt JoyConState> {
        self.ipt.joycons().get(index)
    }

    #[inline(always)]
    pub fn set_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer.borrow_mut().push(InputCommand::SetJoyConRumble {
                index,
                rumble: RumbleIntensity::new(low_frequency, high_frequency),
            });
        }
    }

    #[inline(always)]
    pub fn set_indicator(&self, index: usize, indicator: u8) {
        let Some(indicator) = PlayerIndicatorSlot::from_slot_or_lamp_pattern(indicator) else {
            return;
        };
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::SetJoyConIndicator { index, indicator });
        }
    }

    #[inline(always)]
    pub fn set_indicator_slot(&self, index: usize, slot: u8) {
        let Some(slot) = PlayerIndicatorSlot::from_slot(slot) else {
            return;
        };
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer.borrow_mut().push(InputCommand::SetJoyConIndicator {
                index,
                indicator: slot,
            });
        }
    }
}

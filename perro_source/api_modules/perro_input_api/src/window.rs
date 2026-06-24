use super::*;

/// Script-facing input facade.
///
/// `InputWindow` holds a read-only input API borrow for one script callback.
/// Device modules expose frame-stable input state, while mutating calls queue
/// commands for the input backend to apply later.
pub struct InputWindow<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

#[allow(non_snake_case)]
impl<'ipt, IP: InputAPI + ?Sized> InputWindow<'ipt, IP> {
    // ---- Construction ----

    /// Create an input window around an existing input API borrow.
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    // ---- Device modules ----

    /// Access keyboard key edge/down queries.
    #[inline]
    pub fn Keys(&self) -> KeyModule<'_, IP> {
        KeyModule::new(self.ipt)
    }

    /// Access mouse button, movement, wheel, position, and mode queries.
    #[inline]
    pub fn Mouse(&self) -> MouseModule<'_, IP> {
        MouseModule::new(self.ipt)
    }

    /// Access full keyboard state.
    #[inline]
    pub fn Keyboard(&self) -> KeyboardModule<'_, IP> {
        KeyboardModule::new(self.ipt)
    }

    /// Access full mouse state.
    #[inline]
    pub fn MouseState(&self) -> MouseStateModule<'_, IP> {
        MouseStateModule::new(self.ipt)
    }

    /// Access connected gamepad states.
    #[inline(always)]
    pub fn Gamepads(&self) -> GamepadModule<'_, IP> {
        GamepadModule::new(self.ipt)
    }

    /// Access connected Joy-Con states.
    #[inline(always)]
    pub fn JoyCons(&self) -> JoyConModule<'_, IP> {
        JoyConModule::new(self.ipt)
    }

    /// Access player bindings and player state.
    #[inline]
    pub fn Players(&self) -> PlayerModule<'_, IP> {
        PlayerModule::new(self.ipt)
    }

    /// Access input-map action queries.
    #[inline]
    pub fn Actions(&self) -> ActionModule<'_, IP> {
        ActionModule::new(self.ipt)
    }

    // ---- Output commands ----

    /// Queue a player binding change.
    #[inline]
    pub fn bind_player(&self, index: usize, binding: PlayerBinding) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::BindPlayer { index, binding });
        }
    }

    /// Queue a Joy-Con calibration request.
    #[inline]
    pub fn request_joycon_calibration(&self, index: usize) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::RequestJoyConCalibration { index });
        }
    }

    /// Queue Joy-Con calibration if the indexed device has no calibration.
    #[inline]
    pub fn ensure_joycon_calibration(&self, index: usize) -> bool {
        let needs_calibration = self
            .ipt
            .joycons()
            .get(index)
            .is_some_and(JoyConState::needs_calibration);
        if needs_calibration {
            self.request_joycon_calibration(index);
        }
        needs_calibration
    }

    /// Queue a mouse mode change.
    #[inline]
    pub fn set_mouse_mode(&self, mode: MouseMode) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::SetMouseMode { mode });
        }
    }

    /// Return current mouse mode from the snapshot.
    #[inline]
    pub fn mouse_mode(&self) -> MouseMode {
        self.ipt.mouse().mode()
    }

    /// Queue gamepad rumble for a device slot.
    #[inline]
    pub fn set_gamepad_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer.borrow_mut().push(InputCommand::SetGamepadRumble {
                index,
                rumble: RumbleIntensity::new(low_frequency, high_frequency),
            });
        }
    }

    /// Queue Joy-Con rumble for a device slot.
    #[inline]
    pub fn set_joycon_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer.borrow_mut().push(InputCommand::SetJoyConRumble {
                index,
                rumble: RumbleIntensity::new(low_frequency, high_frequency),
            });
        }
    }

    /// Queue Joy-Con indicator by slot or lamp bit pattern.
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

    /// Queue Joy-Con indicator by zero-based slot only.
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

/// Input-map action query module.
pub struct ActionModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> ActionModule<'ipt, IP> {
    /// Create an action module around an input API borrow.
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    /// Return `true` while any binding for the named action is held.
    #[inline]
    pub fn down(&self, name: &str) -> bool {
        self.down_hash(action_hash(name))
    }

    /// Return `true` when any binding for the named action is pressed this frame.
    #[inline]
    pub fn pressed(&self, name: &str) -> bool {
        self.pressed_hash(action_hash(name))
    }

    /// Return `true` when any binding for the named action is released this frame.
    #[inline]
    pub fn released(&self, name: &str) -> bool {
        self.released_hash(action_hash(name))
    }

    /// Return `true` while any binding for the hashed action is held.
    #[inline]
    pub fn down_hash(&self, name_hash: u64) -> bool {
        self.ipt.action_down_hash(name_hash)
    }

    /// Return `true` when any binding for the hashed action is pressed this frame.
    #[inline]
    pub fn pressed_hash(&self, name_hash: u64) -> bool {
        self.ipt.action_pressed_hash(name_hash)
    }

    /// Return `true` when any binding for the hashed action is released this frame.
    #[inline]
    pub fn released_hash(&self, name_hash: u64) -> bool {
        self.ipt.action_released_hash(name_hash)
    }
}

/// Compact keyboard key query module.
pub struct KeyModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> KeyModule<'ipt, IP> {
    /// Create a key module around an input API borrow.
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    /// Return `true` while the key is held.
    #[inline]
    pub fn down(&self, key: KeyCode) -> bool {
        self.ipt.keyboard().is_key_down(key)
    }

    /// Return `true` only on the frame the key changes from up to down.
    #[inline]
    pub fn pressed(&self, key: KeyCode) -> bool {
        self.ipt.keyboard().is_key_pressed(key)
    }

    /// Return `true` only on the frame the key changes from down to up.
    #[inline]
    pub fn released(&self, key: KeyCode) -> bool {
        self.ipt.keyboard().is_key_released(key)
    }
}

/// Mouse query and command module.
pub struct MouseModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> MouseModule<'ipt, IP> {
    /// Create a mouse module around an input API borrow.
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    /// Return `true` while the button is held.
    #[inline]
    pub fn down(&self, button: MouseButton) -> bool {
        self.ipt.mouse().is_button_down(button)
    }

    /// Return `true` only on the frame the button changes from up to down.
    #[inline]
    pub fn pressed(&self, button: MouseButton) -> bool {
        self.ipt.mouse().is_button_pressed(button)
    }

    /// Return `true` only on the frame the button changes from down to up.
    #[inline]
    pub fn released(&self, button: MouseButton) -> bool {
        self.ipt.mouse().is_button_released(button)
    }

    /// Return accumulated relative movement in pixels.
    #[inline]
    pub fn delta(&self) -> Vector2 {
        self.ipt.mouse().delta()
    }

    /// Return accumulated wheel movement.
    #[inline]
    pub fn wheel(&self) -> Vector2 {
        self.ipt.mouse().wheel()
    }

    /// Return normalized viewport position.
    #[inline]
    pub fn position(&self) -> Vector2 {
        self.ipt.mouse().position()
    }

    /// Return viewport size in pixels.
    #[inline]
    pub fn viewport_size(&self) -> Vector2 {
        self.ipt.mouse().viewport_size()
    }

    /// Return current mouse mode.
    #[inline]
    pub fn mode(&self) -> MouseMode {
        self.ipt.mouse().mode()
    }

    /// Queue a mouse mode change.
    #[inline]
    pub fn set_mode(&self, mode: MouseMode) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::SetMouseMode { mode });
        }
    }

    /// Queue visible cursor mode.
    #[inline]
    pub fn show(&self) {
        self.set_mode(MouseMode::Visible);
    }

    /// Queue hidden cursor mode.
    #[inline]
    pub fn hide(&self) {
        self.set_mode(MouseMode::Hidden);
    }

    /// Queue captured cursor mode.
    #[inline]
    pub fn capture(&self) {
        self.set_mode(MouseMode::Captured);
    }

    /// Queue confined cursor mode.
    #[inline]
    pub fn confine(&self) {
        self.set_mode(MouseMode::Confined);
    }

    /// Queue confined-hidden cursor mode.
    #[inline]
    pub fn confine_hidden(&self) {
        self.set_mode(MouseMode::ConfinedHidden);
    }
}

/// Full keyboard state module.
pub struct KeyboardModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> KeyboardModule<'ipt, IP> {
    /// Create a keyboard module around an input API borrow.
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    /// Return full keyboard state.
    #[inline]
    pub fn state(&self) -> &'ipt KeyboardState {
        self.ipt.keyboard()
    }

    /// Return `true` while the key is held.
    #[inline]
    pub fn down(&self, key: KeyCode) -> bool {
        self.ipt.keyboard().is_key_down(key)
    }

    /// Return `true` only on the frame the key changes from up to down.
    #[inline]
    pub fn pressed(&self, key: KeyCode) -> bool {
        self.ipt.keyboard().is_key_pressed(key)
    }

    /// Return `true` only on the frame the key changes from down to up.
    #[inline]
    pub fn released(&self, key: KeyCode) -> bool {
        self.ipt.keyboard().is_key_released(key)
    }
}

/// Full mouse state query module.
pub struct MouseStateModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> MouseStateModule<'ipt, IP> {
    /// Create a mouse-state module around an input API borrow.
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    /// Return full mouse state.
    #[inline]
    pub fn state(&self) -> &'ipt MouseState {
        self.ipt.mouse()
    }

    /// Return `true` while the button is held.
    #[inline]
    pub fn down(&self, button: MouseButton) -> bool {
        self.ipt.mouse().is_button_down(button)
    }

    /// Return `true` only on the frame the button changes from up to down.
    #[inline]
    pub fn pressed(&self, button: MouseButton) -> bool {
        self.ipt.mouse().is_button_pressed(button)
    }

    /// Return `true` only on the frame the button changes from down to up.
    #[inline]
    pub fn released(&self, button: MouseButton) -> bool {
        self.ipt.mouse().is_button_released(button)
    }

    /// Return accumulated relative movement in pixels.
    #[inline]
    pub fn delta(&self) -> Vector2 {
        self.ipt.mouse().delta()
    }

    /// Return accumulated wheel movement.
    #[inline]
    pub fn wheel(&self) -> Vector2 {
        self.ipt.mouse().wheel()
    }

    /// Return normalized viewport position.
    #[inline]
    pub fn position(&self) -> Vector2 {
        self.ipt.mouse().position()
    }

    /// Return viewport size in pixels.
    #[inline]
    pub fn viewport_size(&self) -> Vector2 {
        self.ipt.mouse().viewport_size()
    }

    /// Return current mouse mode.
    #[inline]
    pub fn mode(&self) -> MouseMode {
        self.ipt.mouse().mode()
    }
}

/// Gamepad state and command module.
pub struct GamepadModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> GamepadModule<'ipt, IP> {
    /// Create a gamepad module around an input API borrow.
    #[inline(always)]
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    /// Return all gamepad states.
    #[inline(always)]
    pub fn all(&self) -> &'ipt [GamepadState] {
        self.ipt.gamepads()
    }

    /// Return one gamepad state by slot.
    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&'ipt GamepadState> {
        self.ipt.gamepads().get(index)
    }

    /// Queue gamepad rumble for a device slot.
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

/// Joy-Con state and command module.
pub struct JoyConModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> JoyConModule<'ipt, IP> {
    /// Creates a Joy-Con access wrapper.
    #[inline(always)]
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    /// Returns all Joy-Con states (each entry is a single Joy-Con controller).
    #[inline(always)]
    pub fn all(&self) -> &'ipt [JoyConState] {
        self.ipt.joycons()
    }

    /// Returns the Joy-Con at the given index.
    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&'ipt JoyConState> {
        self.ipt.joycons().get(index)
    }

    /// Queue Joy-Con rumble for a device slot.
    #[inline(always)]
    pub fn set_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32) {
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer.borrow_mut().push(InputCommand::SetJoyConRumble {
                index,
                rumble: RumbleIntensity::new(low_frequency, high_frequency),
            });
        }
    }

    /// Queue Joy-Con indicator by slot or lamp bit pattern.
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

    /// Queue Joy-Con indicator by zero-based slot only.
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

    /// Queue Joy-Con calibration if the indexed device has no calibration.
    #[inline(always)]
    pub fn ensure_calibration(&self, index: usize) -> bool {
        let needs_calibration = self
            .ipt
            .joycons()
            .get(index)
            .is_some_and(JoyConState::needs_calibration);
        if needs_calibration && let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::RequestJoyConCalibration { index });
        }
        needs_calibration
    }
}

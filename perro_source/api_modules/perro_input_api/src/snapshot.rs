use super::*;

/// Complete input state visible to scripts for one frame.
///
/// `InputSnapshot` stores raw device state, derived action bits, player
/// bindings, and script-issued commands. Script-facing APIs borrow it through
/// [`InputWindow`]. Commands are queued through interior mutability and applied
/// at the next [`InputSnapshot::begin_frame`] call.
#[derive(Clone, Debug)]
pub struct InputSnapshot {
    keyboard: KeyboardState,
    mouse: MouseState,
    gamepads: Vec<GamepadState>,
    joycons: Vec<JoyConState>,
    players: Vec<PlayerState>,
    input_map: InputMap,
    action_down: Vec<u64>,
    action_pressed: Vec<u64>,
    action_released: Vec<u64>,
    rebind_action: Option<u64>,
    rebind_result: Option<RebindResult>,
    commands: RefCell<Vec<InputCommand>>,
    pending_mouse_mode: Option<MouseMode>,
    pending_gamepad_rumble: Vec<GamepadRumbleRequest>,
    pending_joycon_rumble: Vec<JoyConRumbleRequest>,
    pending_joycon_indicator: Vec<JoyConIndicatorRequest>,
}

impl InputSnapshot {
    // ---- Lifecycle ----

    /// Create an empty snapshot with no devices, players, or actions.
    pub fn new() -> Self {
        Self {
            keyboard: KeyboardState::new(),
            mouse: MouseState::new(),
            gamepads: Vec::new(),
            joycons: Vec::new(),
            players: Vec::new(),
            input_map: InputMap::new(),
            action_down: Vec::new(),
            action_pressed: Vec::new(),
            action_released: Vec::new(),
            rebind_action: None,
            rebind_result: None,
            commands: RefCell::new(Vec::new()),
            pending_mouse_mode: None,
            pending_gamepad_rumble: Vec::new(),
            pending_joycon_rumble: Vec::new(),
            pending_joycon_indicator: Vec::new(),
        }
    }

    /// Start a new frame.
    ///
    /// Applies queued script commands, clears one-frame device edges, clears
    /// one-frame action edges, and refreshes persistent action-down bits.
    #[inline]
    pub fn begin_frame(&mut self) {
        self.apply_queued_commands();
        self.keyboard.begin_frame();
        self.mouse.begin_frame();
        for pad in &mut self.gamepads {
            pad.begin_frame();
        }
        for jc in &mut self.joycons {
            jc.begin_frame();
        }
        for player in &mut self.players {
            player.begin_frame();
        }
        self.action_pressed.fill(0);
        self.action_released.fill(0);
        self.refresh_all_action_down();
    }

    /// Clear transient keyboard and mouse state when the app loses focus.
    #[inline]
    pub fn clear_keyboard_mouse_state(&mut self) {
        self.keyboard.clear();
        self.mouse.clear_buttons_and_motion();
        self.action_pressed.fill(0);
        self.action_released.fill(0);
        self.refresh_all_action_down();
    }

    // ---- Keyboard input ----

    /// Apply a key transition and refresh affected actions.
    #[inline]
    pub fn set_key_state(&mut self, key: KeyCode, is_down: bool) {
        self.keyboard.set_key_state(key, is_down);
        if self.keyboard.is_key_pressed(key) {
            self.capture_rebind(InputBinding::Key(key));
        }
        self.refresh_key_actions(key);
    }

    /// Add text input for this frame.
    #[inline]
    pub fn push_text_input(&mut self, text: impl Into<String>) {
        self.keyboard.push_text_input(text);
    }

    /// Return text input chunks received during this frame.
    #[inline]
    pub fn text_inputs(&self) -> &[String] {
        self.keyboard.text_inputs()
    }

    /// Return `true` while the key is held.
    #[inline]
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.keyboard.is_key_down(key)
    }

    /// Return `true` only on the frame the key changes from up to down.
    #[inline]
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keyboard.is_key_pressed(key)
    }

    /// Return `true` only on the frame the key changes from down to up.
    #[inline]
    pub fn is_key_released(&self, key: KeyCode) -> bool {
        self.keyboard.is_key_released(key)
    }

    // ---- Mouse input ----

    /// Apply a mouse button transition and refresh affected actions.
    #[inline]
    pub fn set_mouse_button_state(&mut self, button: MouseButton, is_down: bool) {
        self.mouse.set_button_state(button, is_down);
        if self.mouse.is_button_pressed(button) {
            self.capture_rebind(InputBinding::Mouse(button));
        }
        self.refresh_mouse_actions(button);
    }

    /// Add relative mouse movement in pixels for this frame.
    #[inline]
    pub fn add_mouse_delta(&mut self, dx: f32, dy: f32) {
        self.mouse.add_delta(dx, dy);
    }

    /// Clear relative mouse movement without starting a new frame.
    #[inline]
    pub fn clear_mouse_delta(&mut self) {
        self.mouse.clear_delta();
    }

    /// Add mouse wheel movement for this frame.
    #[inline]
    pub fn add_mouse_wheel(&mut self, dx: f32, dy: f32) {
        self.mouse.add_wheel(dx, dy);
    }

    /// Set absolute mouse position in window pixels.
    #[inline]
    pub fn set_mouse_position(&mut self, x: f32, y: f32) {
        self.mouse.set_position(x, y);
    }

    /// Set current mouse mode without queuing an output command.
    #[inline]
    pub fn set_mouse_mode_state(&mut self, mode: MouseMode) {
        self.mouse.set_mode(mode);
    }

    /// Return current mouse mode.
    #[inline]
    pub fn mouse_mode(&self) -> MouseMode {
        self.mouse.mode()
    }

    /// Drain the last queued mouse mode request, if any.
    #[inline]
    pub fn take_mouse_mode_request(&mut self) -> Option<MouseMode> {
        self.pending_mouse_mode.take()
    }

    /// Set viewport size used for normalized mouse position.
    #[inline]
    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.mouse.set_viewport_size(width, height);
    }

    // ---- Read-only views ----

    /// Return all gamepad states.
    #[inline]
    pub fn gamepads(&self) -> &[GamepadState] {
        &self.gamepads
    }

    /// Return all Joy-Con states.
    #[inline]
    pub fn joycons(&self) -> &[JoyConState] {
        &self.joycons
    }

    /// Return all player binding states.
    #[inline]
    pub fn players(&self) -> &[PlayerState] {
        &self.players
    }

    /// Return the active input map.
    #[inline]
    pub fn input_map(&self) -> &InputMap {
        &self.input_map
    }

    /// Replace the input map and rebuild all cached action state.
    #[inline]
    pub fn set_input_map(&mut self, input_map: InputMap) {
        self.input_map = input_map;
        self.resize_action_bits();
        self.refresh_all_action_states();
    }

    // ---- Mutable device slots ----

    /// Find the first Joy-Con with a matching side.
    #[inline]
    pub fn joycon_side(&self, side: JoyConSide) -> Option<&JoyConState> {
        self.joycons.iter().find(|jc| jc.side() == side)
    }

    /// Find the first mutable Joy-Con with a matching side.
    #[inline]
    pub fn joycon_side_mut(&mut self, side: JoyConSide) -> Option<&mut JoyConState> {
        self.joycons.iter_mut().find(|jc| jc.side() == side)
    }

    /// Return a mutable gamepad slot, creating empty slots as needed.
    #[inline]
    pub fn gamepad_mut(&mut self, index: usize) -> &mut GamepadState {
        if self.gamepads.len() <= index {
            self.gamepads.resize_with(index + 1, GamepadState::new);
        }
        &mut self.gamepads[index]
    }

    /// Return a mutable Joy-Con slot, creating empty slots as needed.
    #[inline]
    pub fn joycon_mut(&mut self, index: usize) -> &mut JoyConState {
        if self.joycons.len() <= index {
            self.joycons.resize_with(index + 1, || {
                if index.is_multiple_of(2) {
                    JoyConState::new(JoyConSide::LJoyCon)
                } else {
                    JoyConState::new(JoyConSide::RJoyCon)
                }
            });
        }
        &mut self.joycons[index]
    }

    /// Return a mutable player slot, creating empty slots as needed.
    #[inline]
    pub fn player_mut(&mut self, index: usize) -> &mut PlayerState {
        if self.players.len() <= index {
            self.players.resize_with(index + 1, PlayerState::new);
        }
        &mut self.players[index]
    }

    /// Bind a player slot to a device source.
    #[inline]
    pub fn bind_player(&mut self, index: usize, binding: PlayerBinding) {
        self.player_mut(index).set_binding(binding);
    }

    // ---- Queued script commands ----

    /// Apply and clear commands queued by [`InputWindow`].
    #[inline]
    pub fn apply_queued_commands(&mut self) {
        let mut pending = {
            let mut commands = self.commands.borrow_mut();
            if commands.is_empty() {
                return;
            }
            std::mem::take(&mut *commands)
        };
        for command in pending.drain(..) {
            match command {
                InputCommand::BindPlayer { index, binding } => {
                    self.bind_player(index, binding);
                }
                InputCommand::StartRebind { action_hash } => {
                    self.rebind_result = None;
                    self.rebind_action = self
                        .input_map
                        .action_by_hash(action_hash)
                        .map(|_| action_hash);
                }
                InputCommand::CancelRebind => {
                    self.rebind_action = None;
                }
                InputCommand::RequestJoyConCalibration { index } => {
                    let state = self.joycon_mut(index);
                    state.set_calibration_requested(true);
                }
                InputCommand::SetMouseMode { mode } => {
                    self.mouse.set_mode(mode);
                    self.pending_mouse_mode = Some(mode);
                }
                InputCommand::SetGamepadRumble { index, rumble } => {
                    self.pending_gamepad_rumble
                        .push(GamepadRumbleRequest { index, rumble });
                }
                InputCommand::SetJoyConRumble { index, rumble } => {
                    self.pending_joycon_rumble
                        .push(JoyConRumbleRequest { index, rumble });
                }
                InputCommand::SetJoyConIndicator { index, indicator } => {
                    self.pending_joycon_indicator
                        .push(JoyConIndicatorRequest { index, indicator });
                }
            }
        }
    }

    // ---- Gamepad input ----

    /// Apply a gamepad button transition and refresh affected actions.
    #[inline]
    pub fn set_gamepad_button_state(&mut self, index: usize, button: GamepadButton, is_down: bool) {
        self.gamepad_mut(index).set_button_state(button, is_down);
        if self.gamepads[index].is_button_pressed(button) {
            self.capture_rebind(InputBinding::Gamepad(button));
        }
        self.refresh_gamepad_actions(button);
    }

    /// Set a gamepad axis value.
    #[inline]
    pub fn set_gamepad_axis(&mut self, index: usize, axis: GamepadAxis, value: f32) {
        self.gamepad_mut(index).set_axis(axis, value);
    }

    /// Set gamepad gyro data.
    #[inline]
    pub fn set_gamepad_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.gamepad_mut(index).set_gyro(x, y, z);
    }

    /// Set gamepad accelerometer data.
    #[inline]
    pub fn set_gamepad_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.gamepad_mut(index).set_accel(x, y, z);
    }

    // ---- Joy-Con input ----

    /// Apply a Joy-Con button transition and refresh affected actions.
    #[inline]
    pub fn set_joycon_button_state(&mut self, index: usize, button: JoyConButton, is_down: bool) {
        let state = self.joycon_mut(index);
        state.set_button_state(button, is_down);
        if self.joycons[index].is_button_pressed(button) {
            self.capture_rebind(InputBinding::JoyCon(button));
        }
        self.refresh_joycon_actions(button);
    }

    /// Set Joy-Con side and refresh all action state.
    #[inline]
    pub fn set_joycon_side(&mut self, index: usize, side: JoyConSide) {
        let state = self.joycon_mut(index);
        state.set_side(side);
        self.refresh_all_action_states();
    }

    /// Set Joy-Con connection state.
    #[inline]
    pub fn set_joycon_connected(&mut self, index: usize, connected: bool) {
        let state = self.joycon_mut(index);
        state.set_connected(connected);
    }

    /// Set whether Joy-Con calibration is complete.
    #[inline]
    pub fn set_joycon_calibrated(&mut self, index: usize, calibrated: bool) {
        let state = self.joycon_mut(index);
        state.set_calibrated(calibrated);
    }

    /// Set whether Joy-Con calibration is currently active.
    #[inline]
    pub fn set_joycon_calibration_in_progress(&mut self, index: usize, in_progress: bool) {
        let state = self.joycon_mut(index);
        state.set_calibration_in_progress(in_progress);
    }

    /// Set Joy-Con calibration bias vector.
    #[inline]
    pub fn set_joycon_calibration_bias(&mut self, index: usize, x: f32, y: f32, z: f32) {
        let state = self.joycon_mut(index);
        state.set_calibration_bias(x, y, z);
    }

    /// Set Joy-Con mouse sensor data.
    #[inline]
    pub fn set_joycon_mouse_sensor(
        &mut self,
        index: usize,
        x: f32,
        y: f32,
        extra: f32,
        distance: f32,
    ) {
        let state = self.joycon_mut(index);
        state.set_mouse_sensor(x, y, extra, distance);
    }

    /// Set Joy-Con stick vector.
    #[inline]
    pub fn set_joycon_stick(&mut self, index: usize, x: f32, y: f32) {
        let state = self.joycon_mut(index);
        state.set_stick(x, y);
    }

    /// Set Joy-Con stick vector as packed signed unorm8 axes.
    #[inline]
    pub fn set_joycon_stick_unit(&mut self, index: usize, stick: perro_structs::SignedUnitVector2) {
        let state = self.joycon_mut(index);
        state.set_stick_unit(stick);
    }

    /// Set Joy-Con gyro data.
    #[inline]
    pub fn set_joycon_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        let state = self.joycon_mut(index);
        state.set_gyro(x, y, z);
    }

    /// Set Joy-Con accelerometer data.
    #[inline]
    pub fn set_joycon_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        let state = self.joycon_mut(index);
        state.set_accel(x, y, z);
    }

    // ---- Mouse queries ----

    /// Return `true` while the mouse button is held.
    #[inline]
    pub fn is_mouse_down(&self, button: MouseButton) -> bool {
        self.mouse.is_button_down(button)
    }

    /// Return `true` only on the frame the mouse button changes from up to down.
    #[inline]
    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse.is_button_pressed(button)
    }

    /// Return `true` only on the frame the mouse button changes from down to up.
    #[inline]
    pub fn is_mouse_released(&self, button: MouseButton) -> bool {
        self.mouse.is_button_released(button)
    }

    /// Return accumulated relative mouse movement in pixels.
    #[inline]
    pub fn mouse_delta(&self) -> Vector2 {
        self.mouse.delta()
    }

    /// Return accumulated mouse wheel movement.
    #[inline]
    pub fn mouse_wheel(&self) -> Vector2 {
        self.mouse.wheel()
    }

    /// Return normalized viewport mouse position.
    #[inline]
    pub fn mouse_position(&self) -> Vector2 {
        self.mouse.position()
    }

    /// Return viewport size in pixels.
    #[inline]
    pub fn viewport_size(&self) -> Vector2 {
        self.mouse.viewport_size()
    }

    // ---- Output command drains ----

    /// Drain Joy-Con calibration requests and clear request flags.
    #[inline]
    pub fn take_joycon_calibration_requests(&mut self) -> Vec<usize> {
        let mut out = Vec::new();
        for (index, joycon) in self.joycons.iter_mut().enumerate() {
            if joycon.calibration_requested() {
                out.push(index);
                joycon.set_calibration_requested(false);
            }
        }
        out
    }

    /// Drain pending gamepad rumble requests.
    #[inline]
    pub fn take_gamepad_rumble_requests(&mut self) -> Vec<GamepadRumbleRequest> {
        std::mem::take(&mut self.pending_gamepad_rumble)
    }

    /// Drain pending Joy-Con rumble requests.
    #[inline]
    pub fn take_joycon_rumble_requests(&mut self) -> Vec<JoyConRumbleRequest> {
        std::mem::take(&mut self.pending_joycon_rumble)
    }

    /// Drain pending Joy-Con indicator requests.
    #[inline]
    pub fn take_joycon_indicator_requests(&mut self) -> Vec<JoyConIndicatorRequest> {
        std::mem::take(&mut self.pending_joycon_indicator)
    }

    // ---- Action queries ----

    /// Return `true` while the next button press waits to bind an action.
    #[inline]
    pub fn is_rebinding(&self) -> bool {
        self.rebind_action.is_some()
    }

    /// Return the last completed live rebind.
    #[inline]
    pub fn rebind_result(&self) -> Option<&RebindResult> {
        self.rebind_result.as_ref()
    }

    /// Return `true` while any binding for the hashed action is held.
    #[inline]
    pub fn is_action_down_hash(&self, name_hash: u64) -> bool {
        self.input_map
            .action_index(name_hash)
            .is_some_and(|index| test_bit(&self.action_down, index))
    }

    /// Return `true` when any binding for the hashed action is pressed this frame.
    #[inline]
    pub fn is_action_pressed_hash(&self, name_hash: u64) -> bool {
        self.input_map
            .action_index(name_hash)
            .is_some_and(|index| test_bit(&self.action_pressed, index))
    }

    /// Return `true` when any binding for the hashed action is released this frame.
    #[inline]
    pub fn is_action_released_hash(&self, name_hash: u64) -> bool {
        self.input_map
            .action_index(name_hash)
            .is_some_and(|index| test_bit(&self.action_released, index))
    }

    // ---- Action cache maintenance ----

    fn capture_rebind(&mut self, binding: InputBinding) {
        let Some(action_hash) = self.rebind_action.take() else {
            return;
        };
        let Some(action) = self.input_map.action_by_hash(action_hash) else {
            return;
        };
        let action = action.name.clone();
        self.input_map.set_bindings_hash(action_hash, vec![binding]);
        self.rebind_result = Some(RebindResult {
            action,
            action_hash,
            binding,
        });
        self.refresh_all_action_states();
    }

    fn resize_action_bits(&mut self) {
        let words = self.input_map.action_count().div_ceil(64);
        self.action_down.resize(words, 0);
        self.action_pressed.resize(words, 0);
        self.action_released.resize(words, 0);
    }

    fn refresh_key_actions(&mut self, key: KeyCode) {
        for idx in 0..self.input_map.actions_for_key(key).len() {
            let action = self.input_map.actions_for_key(key)[idx];
            self.refresh_action_state(action);
        }
    }

    fn refresh_mouse_actions(&mut self, button: MouseButton) {
        for idx in 0..self.input_map.actions_for_mouse(button).len() {
            let action = self.input_map.actions_for_mouse(button)[idx];
            self.refresh_action_state(action);
        }
    }

    fn refresh_gamepad_actions(&mut self, button: GamepadButton) {
        for idx in 0..self.input_map.actions_for_gamepad(button).len() {
            let action = self.input_map.actions_for_gamepad(button)[idx];
            self.refresh_action_state(action);
        }
    }

    fn refresh_joycon_actions(&mut self, button: JoyConButton) {
        for idx in 0..self.input_map.actions_for_joycon(button).len() {
            let action = self.input_map.actions_for_joycon(button)[idx];
            self.refresh_action_state(action);
        }
    }

    fn refresh_all_action_down(&mut self) {
        for action in 0..self.input_map.action_count() {
            let down = self.compute_action_down(action);
            set_bit(&mut self.action_down, action, down);
        }
    }

    fn refresh_all_action_states(&mut self) {
        for action in 0..self.input_map.action_count() {
            self.refresh_action_state(action);
        }
    }

    fn refresh_action_state(&mut self, action: usize) {
        let down = self.compute_action_down(action);
        let pressed = self.compute_action_pressed(action);
        let released = self.compute_action_released(action);
        set_bit(&mut self.action_down, action, down);
        set_bit(&mut self.action_pressed, action, pressed);
        set_bit(&mut self.action_released, action, released);
    }

    fn compute_action_down(&self, action: usize) -> bool {
        self.input_map.actions().get(action).is_some_and(|action| {
            action.bindings.iter().any(|binding| match binding {
                InputBinding::Key(key) => self.keyboard.is_key_down(*key),
                InputBinding::Mouse(button) => self.mouse.is_button_down(*button),
                InputBinding::Gamepad(button) => {
                    self.gamepads.iter().any(|pad| pad.is_button_down(*button))
                }
                InputBinding::JoyCon(button) => self
                    .joycons
                    .iter()
                    .any(|joycon| joycon.is_button_down(*button)),
            })
        })
    }

    fn compute_action_pressed(&self, action: usize) -> bool {
        self.input_map.actions().get(action).is_some_and(|action| {
            action.bindings.iter().any(|binding| match binding {
                InputBinding::Key(key) => self.keyboard.is_key_pressed(*key),
                InputBinding::Mouse(button) => self.mouse.is_button_pressed(*button),
                InputBinding::Gamepad(button) => self
                    .gamepads
                    .iter()
                    .any(|pad| pad.is_button_pressed(*button)),
                InputBinding::JoyCon(button) => self
                    .joycons
                    .iter()
                    .any(|joycon| joycon.is_button_pressed(*button)),
            })
        })
    }

    fn compute_action_released(&self, action: usize) -> bool {
        self.input_map.actions().get(action).is_some_and(|action| {
            action.bindings.iter().any(|binding| match binding {
                InputBinding::Key(key) => self.keyboard.is_key_released(*key),
                InputBinding::Mouse(button) => self.mouse.is_button_released(*button),
                InputBinding::Gamepad(button) => self
                    .gamepads
                    .iter()
                    .any(|pad| pad.is_button_released(*button)),
                InputBinding::JoyCon(button) => self
                    .joycons
                    .iter()
                    .any(|joycon| joycon.is_button_released(*button)),
            })
        })
    }
}

// ---- Bit helpers ----

#[inline]
fn test_bit(bits: &[u64], index: usize) -> bool {
    let word = index / 64;
    let bit = 1_u64 << (index % 64);
    bits.get(word).is_some_and(|word| word & bit != 0)
}

#[inline]
fn set_bit(bits: &mut [u64], index: usize, enabled: bool) {
    let word = index / 64;
    let bit = 1_u64 << (index % 64);
    if let Some(value) = bits.get_mut(word) {
        if enabled {
            *value |= bit;
        } else {
            *value &= !bit;
        }
    }
}

impl Default for InputSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// Data produced when live rebinding captures a button press.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RebindResult {
    pub action: String,
    pub action_hash: u64,
    pub binding: InputBinding,
}

/// Input command queued by scripts and applied by the input backend.
#[derive(Clone, Debug)]
pub enum InputCommand {
    /// Bind a player slot to a device source.
    BindPlayer {
        index: usize,
        binding: PlayerBinding,
    },
    /// Listen for the next button press and replace an action's bindings.
    StartRebind { action_hash: u64 },
    /// Stop an active live rebind.
    CancelRebind,
    /// Request Joy-Con calibration for a slot.
    RequestJoyConCalibration { index: usize },
    /// Request mouse mode change.
    SetMouseMode { mode: MouseMode },
    /// Request gamepad rumble.
    SetGamepadRumble {
        index: usize,
        rumble: RumbleIntensity,
    },
    /// Request Joy-Con rumble.
    SetJoyConRumble {
        index: usize,
        rumble: RumbleIntensity,
    },
    /// Request Joy-Con player indicator change.
    SetJoyConIndicator {
        index: usize,
        indicator: PlayerIndicatorSlot,
    },
}

/// Read-only input contract used by [`InputWindow`].
///
/// Implementors expose device state and may optionally provide a command buffer
/// for script-issued output requests.
pub trait InputAPI {
    /// Return keyboard state.
    fn keyboard(&self) -> &KeyboardState;
    /// Return mouse state.
    fn mouse(&self) -> &MouseState;
    /// Return gamepad states.
    fn gamepads(&self) -> &[GamepadState];
    /// Return Joy-Con states.
    fn joycons(&self) -> &[JoyConState];
    /// Return player binding states.
    fn players(&self) -> &[PlayerState];
    /// Return input-map actions.
    fn input_map(&self) -> &InputMap;
    /// Return `true` while a live rebind waits for input.
    fn is_rebinding(&self) -> bool {
        false
    }
    /// Return the last completed live rebind.
    fn rebind_result(&self) -> Option<&RebindResult> {
        None
    }

    /// Return `true` while any binding for the hashed action is held.
    fn action_down_hash(&self, name_hash: u64) -> bool {
        self.input_map().down_hash(
            name_hash,
            self.keyboard(),
            self.mouse(),
            self.gamepads(),
            self.joycons(),
        )
    }

    /// Return `true` when any binding for the hashed action is pressed this frame.
    fn action_pressed_hash(&self, name_hash: u64) -> bool {
        self.input_map().pressed_hash(
            name_hash,
            self.keyboard(),
            self.mouse(),
            self.gamepads(),
            self.joycons(),
        )
    }

    /// Return `true` when any binding for the hashed action is released this frame.
    fn action_released_hash(&self, name_hash: u64) -> bool {
        self.input_map().released_hash(
            name_hash,
            self.keyboard(),
            self.mouse(),
            self.gamepads(),
            self.joycons(),
        )
    }

    /// Return a command buffer when scripts may queue output requests.
    #[inline]
    fn command_buffer(&self) -> Option<&RefCell<Vec<InputCommand>>> {
        None
    }
}

impl InputAPI for InputSnapshot {
    #[inline]
    fn keyboard(&self) -> &KeyboardState {
        &self.keyboard
    }

    #[inline]
    fn mouse(&self) -> &MouseState {
        &self.mouse
    }

    #[inline]
    fn gamepads(&self) -> &[GamepadState] {
        self.gamepads()
    }

    #[inline]
    fn joycons(&self) -> &[JoyConState] {
        self.joycons()
    }

    #[inline]
    fn players(&self) -> &[PlayerState] {
        self.players()
    }

    #[inline]
    fn input_map(&self) -> &InputMap {
        self.input_map()
    }

    #[inline]
    fn is_rebinding(&self) -> bool {
        self.is_rebinding()
    }

    #[inline]
    fn rebind_result(&self) -> Option<&RebindResult> {
        self.rebind_result()
    }

    #[inline]
    fn action_down_hash(&self, name_hash: u64) -> bool {
        self.is_action_down_hash(name_hash)
    }

    #[inline]
    fn action_pressed_hash(&self, name_hash: u64) -> bool {
        self.is_action_pressed_hash(name_hash)
    }

    #[inline]
    fn action_released_hash(&self, name_hash: u64) -> bool {
        self.is_action_released_hash(name_hash)
    }

    #[inline]
    fn command_buffer(&self) -> Option<&RefCell<Vec<InputCommand>>> {
        Some(&self.commands)
    }
}

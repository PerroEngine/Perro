use super::*;

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
    commands: RefCell<Vec<InputCommand>>,
    pending_mouse_mode: Option<MouseMode>,
    pending_gamepad_rumble: Vec<GamepadRumbleRequest>,
    pending_joycon_rumble: Vec<JoyConRumbleRequest>,
    pending_joycon_indicator: Vec<JoyConIndicatorRequest>,
}

impl InputSnapshot {
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
            commands: RefCell::new(Vec::new()),
            pending_mouse_mode: None,
            pending_gamepad_rumble: Vec::new(),
            pending_joycon_rumble: Vec::new(),
            pending_joycon_indicator: Vec::new(),
        }
    }

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

    #[inline]
    pub fn set_key_state(&mut self, key: KeyCode, is_down: bool) {
        self.keyboard.set_key_state(key, is_down);
        self.refresh_key_actions(key);
    }

    #[inline]
    pub fn push_text_input(&mut self, text: impl Into<String>) {
        self.keyboard.push_text_input(text);
    }

    #[inline]
    pub fn text_inputs(&self) -> &[String] {
        self.keyboard.text_inputs()
    }

    #[inline]
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.keyboard.is_key_down(key)
    }

    #[inline]
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keyboard.is_key_pressed(key)
    }

    #[inline]
    pub fn is_key_released(&self, key: KeyCode) -> bool {
        self.keyboard.is_key_released(key)
    }

    #[inline]
    pub fn set_mouse_button_state(&mut self, button: MouseButton, is_down: bool) {
        self.mouse.set_button_state(button, is_down);
        self.refresh_mouse_actions(button);
    }

    #[inline]
    pub fn add_mouse_delta(&mut self, dx: f32, dy: f32) {
        self.mouse.add_delta(dx, dy);
    }

    #[inline]
    pub fn clear_mouse_delta(&mut self) {
        self.mouse.clear_delta();
    }

    #[inline]
    pub fn add_mouse_wheel(&mut self, dx: f32, dy: f32) {
        self.mouse.add_wheel(dx, dy);
    }

    #[inline]
    pub fn set_mouse_position(&mut self, x: f32, y: f32) {
        self.mouse.set_position(x, y);
    }

    #[inline]
    pub fn set_mouse_mode_state(&mut self, mode: MouseMode) {
        self.mouse.set_mode(mode);
    }

    #[inline]
    pub fn mouse_mode(&self) -> MouseMode {
        self.mouse.mode()
    }

    #[inline]
    pub fn take_mouse_mode_request(&mut self) -> Option<MouseMode> {
        self.pending_mouse_mode.take()
    }

    #[inline]
    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.mouse.set_viewport_size(width, height);
    }

    #[inline]
    pub fn gamepads(&self) -> &[GamepadState] {
        &self.gamepads
    }

    #[inline]
    pub fn joycons(&self) -> &[JoyConState] {
        &self.joycons
    }

    #[inline]
    pub fn players(&self) -> &[PlayerState] {
        &self.players
    }

    #[inline]
    pub fn input_map(&self) -> &InputMap {
        &self.input_map
    }

    #[inline]
    pub fn set_input_map(&mut self, input_map: InputMap) {
        self.input_map = input_map;
        self.resize_action_bits();
        self.refresh_all_action_states();
    }

    #[inline]
    pub fn joycon_side(&self, side: JoyConSide) -> Option<&JoyConState> {
        self.joycons.iter().find(|jc| jc.side() == side)
    }

    #[inline]
    pub fn joycon_side_mut(&mut self, side: JoyConSide) -> Option<&mut JoyConState> {
        self.joycons.iter_mut().find(|jc| jc.side() == side)
    }

    #[inline]
    pub fn gamepad_mut(&mut self, index: usize) -> &mut GamepadState {
        if self.gamepads.len() <= index {
            self.gamepads.resize_with(index + 1, GamepadState::new);
        }
        &mut self.gamepads[index]
    }

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

    #[inline]
    pub fn player_mut(&mut self, index: usize) -> &mut PlayerState {
        if self.players.len() <= index {
            self.players.resize_with(index + 1, PlayerState::new);
        }
        &mut self.players[index]
    }

    #[inline]
    pub fn bind_player(&mut self, index: usize, binding: PlayerBinding) {
        self.player_mut(index).set_binding(binding);
    }

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

    #[inline]
    pub fn set_gamepad_button_state(&mut self, index: usize, button: GamepadButton, is_down: bool) {
        self.gamepad_mut(index).set_button_state(button, is_down);
        self.refresh_gamepad_actions(button);
    }

    #[inline]
    pub fn set_gamepad_axis(&mut self, index: usize, axis: GamepadAxis, value: f32) {
        self.gamepad_mut(index).set_axis(axis, value);
    }

    #[inline]
    pub fn set_gamepad_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.gamepad_mut(index).set_gyro(x, y, z);
    }

    #[inline]
    pub fn set_gamepad_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.gamepad_mut(index).set_accel(x, y, z);
    }

    #[inline]
    pub fn set_joycon_button_state(&mut self, index: usize, button: JoyConButton, is_down: bool) {
        let state = self.joycon_mut(index);
        state.set_button_state(button, is_down);
        self.refresh_joycon_actions(button);
    }

    #[inline]
    pub fn set_joycon_side(&mut self, index: usize, side: JoyConSide) {
        let state = self.joycon_mut(index);
        state.set_side(side);
        self.refresh_all_action_states();
    }

    #[inline]
    pub fn set_joycon_connected(&mut self, index: usize, connected: bool) {
        let state = self.joycon_mut(index);
        state.set_connected(connected);
    }

    #[inline]
    pub fn set_joycon_calibrated(&mut self, index: usize, calibrated: bool) {
        let state = self.joycon_mut(index);
        state.set_calibrated(calibrated);
    }

    #[inline]
    pub fn set_joycon_calibration_in_progress(&mut self, index: usize, in_progress: bool) {
        let state = self.joycon_mut(index);
        state.set_calibration_in_progress(in_progress);
    }

    #[inline]
    pub fn set_joycon_calibration_bias(&mut self, index: usize, x: f32, y: f32, z: f32) {
        let state = self.joycon_mut(index);
        state.set_calibration_bias(x, y, z);
    }

    #[inline]
    pub fn set_joycon_stick(&mut self, index: usize, x: f32, y: f32) {
        let state = self.joycon_mut(index);
        state.set_stick(x, y);
    }

    #[inline]
    pub fn set_joycon_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        let state = self.joycon_mut(index);
        state.set_gyro(x, y, z);
    }

    #[inline]
    pub fn set_joycon_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        let state = self.joycon_mut(index);
        state.set_accel(x, y, z);
    }

    #[inline]
    pub fn is_mouse_down(&self, button: MouseButton) -> bool {
        self.mouse.is_button_down(button)
    }

    #[inline]
    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse.is_button_pressed(button)
    }

    #[inline]
    pub fn is_mouse_released(&self, button: MouseButton) -> bool {
        self.mouse.is_button_released(button)
    }

    #[inline]
    pub fn mouse_delta(&self) -> Vector2 {
        self.mouse.delta()
    }

    #[inline]
    pub fn mouse_wheel(&self) -> Vector2 {
        self.mouse.wheel()
    }

    #[inline]
    pub fn mouse_position(&self) -> Vector2 {
        self.mouse.position()
    }

    #[inline]
    pub fn viewport_size(&self) -> Vector2 {
        self.mouse.viewport_size()
    }

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

    #[inline]
    pub fn take_gamepad_rumble_requests(&mut self) -> Vec<GamepadRumbleRequest> {
        std::mem::take(&mut self.pending_gamepad_rumble)
    }

    #[inline]
    pub fn take_joycon_rumble_requests(&mut self) -> Vec<JoyConRumbleRequest> {
        std::mem::take(&mut self.pending_joycon_rumble)
    }

    #[inline]
    pub fn take_joycon_indicator_requests(&mut self) -> Vec<JoyConIndicatorRequest> {
        std::mem::take(&mut self.pending_joycon_indicator)
    }

    #[inline]
    pub fn is_action_down_hash(&self, name_hash: u64) -> bool {
        self.input_map
            .action_index(name_hash)
            .is_some_and(|index| test_bit(&self.action_down, index))
    }

    #[inline]
    pub fn is_action_pressed_hash(&self, name_hash: u64) -> bool {
        self.input_map
            .action_index(name_hash)
            .is_some_and(|index| test_bit(&self.action_pressed, index))
    }

    #[inline]
    pub fn is_action_released_hash(&self, name_hash: u64) -> bool {
        self.input_map
            .action_index(name_hash)
            .is_some_and(|index| test_bit(&self.action_released, index))
    }

    fn resize_action_bits(&mut self) {
        let words = self.input_map.action_count().div_ceil(64);
        self.action_down.resize(words, 0);
        self.action_pressed.resize(words, 0);
        self.action_released.resize(words, 0);
    }

    fn refresh_key_actions(&mut self, key: KeyCode) {
        let actions = self.input_map.actions_for_key(key).to_vec();
        self.refresh_action_states(&actions);
    }

    fn refresh_mouse_actions(&mut self, button: MouseButton) {
        let actions = self.input_map.actions_for_mouse(button).to_vec();
        self.refresh_action_states(&actions);
    }

    fn refresh_gamepad_actions(&mut self, button: GamepadButton) {
        let actions = self.input_map.actions_for_gamepad(button).to_vec();
        self.refresh_action_states(&actions);
    }

    fn refresh_joycon_actions(&mut self, button: JoyConButton) {
        let actions = self.input_map.actions_for_joycon(button).to_vec();
        self.refresh_action_states(&actions);
    }

    fn refresh_all_action_down(&mut self) {
        let actions: Vec<usize> = (0..self.input_map.action_count()).collect();
        for action in actions {
            let down = self.compute_action_down(action);
            set_bit(&mut self.action_down, action, down);
        }
    }

    fn refresh_all_action_states(&mut self) {
        let actions: Vec<usize> = (0..self.input_map.action_count()).collect();
        self.refresh_action_states(&actions);
    }

    fn refresh_action_states(&mut self, actions: &[usize]) {
        for &action in actions {
            let down = self.compute_action_down(action);
            let pressed = self.compute_action_pressed(action);
            let released = self.compute_action_released(action);
            set_bit(&mut self.action_down, action, down);
            set_bit(&mut self.action_pressed, action, pressed);
            set_bit(&mut self.action_released, action, released);
        }
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

#[derive(Clone, Debug)]
pub enum InputCommand {
    BindPlayer {
        index: usize,
        binding: PlayerBinding,
    },
    RequestJoyConCalibration {
        index: usize,
    },
    SetMouseMode {
        mode: MouseMode,
    },
    SetGamepadRumble {
        index: usize,
        rumble: RumbleIntensity,
    },
    SetJoyConRumble {
        index: usize,
        rumble: RumbleIntensity,
    },
    SetJoyConIndicator {
        index: usize,
        indicator: PlayerIndicatorSlot,
    },
}

pub trait InputAPI {
    fn keyboard(&self) -> &KeyboardState;
    fn mouse(&self) -> &MouseState;
    fn gamepads(&self) -> &[GamepadState];
    fn joycons(&self) -> &[JoyConState];
    fn players(&self) -> &[PlayerState];
    fn input_map(&self) -> &InputMap;
    fn action_down_hash(&self, name_hash: u64) -> bool {
        self.input_map().down_hash(
            name_hash,
            self.keyboard(),
            self.mouse(),
            self.gamepads(),
            self.joycons(),
        )
    }
    fn action_pressed_hash(&self, name_hash: u64) -> bool {
        self.input_map().pressed_hash(
            name_hash,
            self.keyboard(),
            self.mouse(),
            self.gamepads(),
            self.joycons(),
        )
    }
    fn action_released_hash(&self, name_hash: u64) -> bool {
        self.input_map().released_hash(
            name_hash,
            self.keyboard(),
            self.mouse(),
            self.gamepads(),
            self.joycons(),
        )
    }
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

mod gamepad;
mod joycon;
mod keycode;
mod mouse_button;
mod player;

pub use gamepad::{GamepadAxis, GamepadButton, GamepadState};
pub use joycon::{JoyConButton, JoyConSide, JoyConState};
pub use keycode::KeyCode;
pub use mouse_button::MouseButton;
use perro_structs::Vector2;
pub use player::{PlayerBinding, PlayerModule, PlayerState};
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MouseMode {
    #[default]
    Visible,
    Hidden,
    Captured,
    Confined,
    ConfinedHidden,
}

impl MouseMode {
    #[inline]
    pub fn cursor_visible(self) -> bool {
        matches!(self, Self::Visible | Self::Confined)
    }

    #[inline]
    pub fn is_captured(self) -> bool {
        matches!(self, Self::Captured)
    }

    #[inline]
    pub fn is_confined(self) -> bool {
        matches!(self, Self::Confined | Self::ConfinedHidden)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GamepadIndex(pub usize);

impl From<usize> for GamepadIndex {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<GamepadIndex> for usize {
    fn from(value: GamepadIndex) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JoyConIndex(pub usize);

impl From<usize> for JoyConIndex {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<JoyConIndex> for usize {
    fn from(value: JoyConIndex) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RumbleIntensity {
    pub low_frequency: f32,
    pub high_frequency: f32,
}

impl RumbleIntensity {
    #[inline]
    pub fn new(low_frequency: f32, high_frequency: f32) -> Self {
        Self {
            low_frequency,
            high_frequency,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerIndicatorSlot(pub u8);
pub type PlayerIndicator = PlayerIndicatorSlot;

impl PlayerIndicatorSlot {
    pub const COUNT: u8 = 8;
    pub const LAMP_PATTERNS: [u8; Self::COUNT as usize] = [
        0b0001, 0b0011, 0b0111, 0b1111, 0b1001, 0b1010, 0b1011, 0b0110,
    ];

    #[inline]
    pub fn from_slot(slot: u8) -> Option<Self> {
        if slot < Self::COUNT {
            Some(Self(slot))
        } else {
            None
        }
    }

    #[inline]
    pub fn from_player_number(player_number: usize) -> Option<Self> {
        if (1..=(Self::COUNT as usize)).contains(&player_number) {
            Some(Self((player_number - 1) as u8))
        } else {
            None
        }
    }

    #[inline]
    pub fn from_lamp_pattern(pattern: u8) -> Option<Self> {
        Self::LAMP_PATTERNS
            .iter()
            .position(|&candidate| candidate == pattern)
            .map(|slot| Self(slot as u8))
    }

    #[inline]
    pub fn from_slot_or_lamp_pattern(value: u8) -> Option<Self> {
        Self::from_slot(value).or_else(|| Self::from_lamp_pattern(value))
    }

    #[inline]
    pub fn to_lamp_pattern(self) -> u8 {
        Self::LAMP_PATTERNS[self.0 as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GamepadRumbleRequest {
    pub index: usize,
    pub rumble: RumbleIntensity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JoyConRumbleRequest {
    pub index: usize,
    pub rumble: RumbleIntensity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JoyConIndicatorRequest {
    pub index: usize,
    pub indicator: PlayerIndicatorSlot,
}

#[derive(Clone, Debug)]
pub struct InputSnapshot {
    keyboard: KeyboardState,
    mouse: MouseState,
    gamepads: Vec<GamepadState>,
    joycons: Vec<JoyConState>,
    players: Vec<PlayerState>,
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
    }

    #[inline]
    pub fn set_key_state(&mut self, key: KeyCode, is_down: bool) {
        self.keyboard.set_key_state(key, is_down);
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
    }

    #[inline]
    pub fn add_mouse_delta(&mut self, dx: f32, dy: f32) {
        self.mouse.add_delta(dx, dy);
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
    }

    #[inline]
    pub fn set_joycon_side(&mut self, index: usize, side: JoyConSide) {
        let state = self.joycon_mut(index);
        state.set_side(side);
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
    fn command_buffer(&self) -> Option<&RefCell<Vec<InputCommand>>> {
        Some(&self.commands)
    }
}

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
            buffer.borrow_mut().push(InputCommand::SetJoyConIndicator {
                index,
                indicator,
            });
        }
    }

    #[inline]
    pub fn set_joycon_indicator_slot(&self, index: usize, slot: u8) {
        let Some(slot) = PlayerIndicatorSlot::from_slot(slot) else {
            return;
        };
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::SetJoyConIndicator { index, indicator: slot });
        }
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
            buffer.borrow_mut().push(InputCommand::SetJoyConIndicator {
                index,
                indicator,
            });
        }
    }

    #[inline(always)]
    pub fn set_indicator_slot(&self, index: usize, slot: u8) {
        let Some(slot) = PlayerIndicatorSlot::from_slot(slot) else {
            return;
        };
        if let Some(buffer) = self.ipt.command_buffer() {
            buffer
                .borrow_mut()
                .push(InputCommand::SetJoyConIndicator { index, indicator: slot });
        }
    }
}

#[macro_export]
/// Signature:
/// - `key_down!(&InputWindow<_>, KeyCode) -> bool`
///
/// Usage:
/// - `key_down!(ipt, KeyCode::Space) -> bool`
///
/// `ipt` is usually the input parameter from lifecycle methods:
/// - `fn on_update(..., ctx: &mut ScriptContext<'_, RT, RS, IP>, ...)`
///
/// `KeyCode` is the keyboard-key enum (letters, numbers, arrows, function keys, etc.).
///
/// Checks whether a key is currently down.
macro_rules! key_down {
    ($ipt:expr, $key:expr) => {
        $ipt.Keys().down($key)
    };
}

#[macro_export]
/// Signature:
/// - `key_pressed!(&InputWindow<_>, KeyCode) -> bool`
///
/// Usage:
/// - `key_pressed!(ipt, KeyCode::Enter) -> bool`
///
/// `KeyCode` is the keyboard-key enum.
///
/// Checks whether a key was pressed this frame.
macro_rules! key_pressed {
    ($ipt:expr, $key:expr) => {
        $ipt.Keys().pressed($key)
    };
}

#[macro_export]
/// Signature:
/// - `key_released!(&InputWindow<_>, KeyCode) -> bool`
///
/// Usage:
/// - `key_released!(ipt, KeyCode::Escape) -> bool`
///
/// `KeyCode` is the keyboard-key enum.
///
/// Checks whether a key was released this frame.
macro_rules! key_released {
    ($ipt:expr, $key:expr) => {
        $ipt.Keys().released($key)
    };
}

#[macro_export]
/// Signature:
/// - `mouse_down!(&InputWindow<_>, MouseButton) -> bool`
///
/// Usage:
/// - `mouse_down!(ipt, MouseButton::Right) -> bool`
///
/// `MouseButton` is the mouse-button enum (`Left`, `Right`, `Middle`, and extras).
///
/// Checks whether a mouse button is currently down.
macro_rules! mouse_down {
    ($ipt:expr, $button:expr) => {
        $ipt.Mouse().down($button)
    };
}

#[macro_export]
/// Signature:
/// - `mouse_pressed!(&InputWindow<_>, MouseButton) -> bool`
///
/// Usage:
/// - `mouse_pressed!(ipt, MouseButton::Left) -> bool`
///
/// `MouseButton` is the mouse-button enum.
///
/// Checks whether a mouse button was pressed this frame.
macro_rules! mouse_pressed {
    ($ipt:expr, $button:expr) => {
        $ipt.Mouse().pressed($button)
    };
}

#[macro_export]
/// Signature:
/// - `mouse_released!(&InputWindow<_>, MouseButton) -> bool`
///
/// Usage:
/// - `mouse_released!(ipt, MouseButton::Left) -> bool`
///
/// `MouseButton` is the mouse-button enum.
///
/// Checks whether a mouse button was released this frame.
macro_rules! mouse_released {
    ($ipt:expr, $button:expr) => {
        $ipt.Mouse().released($button)
    };
}

#[macro_export]
/// Signature:
/// - `mouse_delta!(&InputWindow<_>) -> Vector2`
///
/// Usage:
/// - `mouse_delta!(ipt) -> Vector2`
macro_rules! mouse_delta {
    ($ipt:expr) => {
        $ipt.Mouse().delta()
    };
}

#[macro_export]
/// Signature:
/// - `mouse_wheel!(&InputWindow<_>) -> Vector2`
///
/// Usage:
/// - `mouse_wheel!(ipt) -> Vector2`
macro_rules! mouse_wheel {
    ($ipt:expr) => {
        $ipt.Mouse().wheel()
    };
}

#[macro_export]
/// Signature:
/// - `mouse_position!(&InputWindow<_>) -> Vector2`
///
/// Usage:
/// - `mouse_position!(ipt) -> Vector2`
macro_rules! mouse_position {
    ($ipt:expr) => {
        $ipt.Mouse().position()
    };
}

#[macro_export]
/// Signature:
/// - `viewport_size!(&InputWindow<_>) -> Vector2`
///
/// Usage:
/// - `viewport_size!(ipt) -> Vector2`
macro_rules! viewport_size {
    ($ipt:expr) => {
        $ipt.Mouse().viewport_size()
    };
}

#[macro_export]
/// Signature:
/// - `mouse_mode!(&InputWindow<_>) -> MouseMode`
///
/// Usage:
/// - `mouse_mode!(ipt) -> MouseMode`
macro_rules! mouse_mode {
    ($ipt:expr) => {
        $ipt.Mouse().mode()
    };
}

#[macro_export]
/// Signature:
/// - `mouse_set_mode!(&InputWindow<_>, MouseMode) -> ()`
///
/// Usage:
/// - `mouse_set_mode!(ipt, MouseMode::Captured)`
macro_rules! mouse_set_mode {
    ($ipt:expr, $mode:expr) => {{ $ipt.Mouse().set_mode($mode) }};
}

#[macro_export]
/// Signature:
/// - `mouse_show!(&InputWindow<_>) -> ()`
macro_rules! mouse_show {
    ($ipt:expr) => {{ $ipt.Mouse().show() }};
}

#[macro_export]
/// Signature:
/// - `mouse_hide!(&InputWindow<_>) -> ()`
macro_rules! mouse_hide {
    ($ipt:expr) => {{ $ipt.Mouse().hide() }};
}

#[macro_export]
/// Signature:
/// - `mouse_capture!(&InputWindow<_>) -> ()`
macro_rules! mouse_capture {
    ($ipt:expr) => {{ $ipt.Mouse().capture() }};
}

#[macro_export]
/// Signature:
/// - `mouse_confine!(&InputWindow<_>) -> ()`
macro_rules! mouse_confine {
    ($ipt:expr) => {{ $ipt.Mouse().confine() }};
}

#[macro_export]
/// Signature:
/// - `mouse_confine_hidden!(&InputWindow<_>) -> ()`
macro_rules! mouse_confine_hidden {
    ($ipt:expr) => {{ $ipt.Mouse().confine_hidden() }};
}

#[macro_export]
/// Signature:
/// - `joycon_request_calibration!(&InputWindow<_>, JoyConIndex) -> ()`
macro_rules! joycon_request_calibration {
    ($ipt:expr, $index:expr) => {{ $ipt.request_joycon_calibration($index) }};
}

#[macro_export]
macro_rules! gamepad_set_rumble {
    ($ipt:expr, $index:expr, $low:expr, $high:expr) => {{
        $ipt.Gamepads().set_rumble($index, $low, $high)
    }};
}

#[macro_export]
macro_rules! joycon_set_rumble {
    ($ipt:expr, $index:expr, $low:expr, $high:expr) => {{
        $ipt.JoyCons().set_rumble($index, $low, $high)
    }};
}

#[macro_export]
macro_rules! joycon_set_indicator {
    ($ipt:expr, $index:expr, $indicator:expr) => {{
        $ipt.JoyCons().set_indicator($index, $indicator)
    }};
}

#[cfg(test)]
mod tests {
    use super::{InputWindow, InputSnapshot, MouseMode};

    #[test]
    fn mouse_mode_defaults_visible() {
        let input = InputSnapshot::new();

        assert_eq!(input.mouse_mode(), MouseMode::Visible);
    }

    #[test]
    fn mouse_mode_command_sets_state_and_request() {
        let mut input = InputSnapshot::new();
        {
            let ctx = InputWindow::new(&input);
            ctx.Mouse().capture();
        }

        input.apply_queued_commands();

        assert_eq!(input.mouse_mode(), MouseMode::Captured);
        assert_eq!(input.take_mouse_mode_request(), Some(MouseMode::Captured));
        assert_eq!(input.take_mouse_mode_request(), None);
    }

    #[test]
    fn mouse_mode_macro_queues_request() {
        let mut input = InputSnapshot::new();
        {
            let ctx = InputWindow::new(&input);
            mouse_set_mode!(&ctx, MouseMode::Confined);
        }

        input.apply_queued_commands();

        assert_eq!(mouse_mode!(InputWindow::new(&input)), MouseMode::Confined);
        assert_eq!(input.take_mouse_mode_request(), Some(MouseMode::Confined));
    }
}

pub mod prelude {
    pub use crate::{
        GamepadAxis, GamepadButton, GamepadIndex, GamepadModule, GamepadState, InputAPI,
        InputWindow, InputSnapshot, JoyConButton, JoyConIndex, JoyConModule, JoyConSide,
        JoyConState, KeyCode, KeyModule, KeyboardModule, KeyboardState, MouseButton, MouseMode,
        MouseModule, MouseState, MouseStateModule, PlayerBinding, PlayerIndicatorSlot, PlayerModule,
        PlayerState, RumbleIntensity,
        gamepad_accel, gamepad_down, gamepad_get, gamepad_gyro, gamepad_left_stick, gamepad_list,
        gamepad_pressed, gamepad_released, gamepad_right_stick, gamepad_set_rumble, joycon_accel,
        joycon_calibrated, joycon_calibrating,
        joycon_calibration_bias, joycon_connected, joycon_down, joycon_get, joycon_gyro,
        joycon_list, joycon_needs_calibration, joycon_pressed, joycon_released,
        joycon_request_calibration, joycon_set_indicator, joycon_set_rumble, joycon_side,
        joycon_stick, key_down, key_pressed, key_released, mouse_capture, mouse_confine,
        mouse_confine_hidden, mouse_delta, mouse_down, mouse_hide, mouse_mode, mouse_position,
        mouse_pressed, mouse_released, mouse_set_mode, mouse_show, mouse_wheel, player_bind,
        player_get, player_list, viewport_size,
    };
    pub use perro_structs::Vector2;
}

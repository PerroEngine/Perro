mod gamepad;
mod joycon;
mod keycode;
mod mouse_button;
mod player;

pub use gamepad::{GamepadAxis, GamepadButton, GamepadState};
pub use joycon::{
    JoyConButton, JoyConSide, JoyConState,
};
pub use keycode::KeyCode;
pub use mouse_button::MouseButton;
pub use player::{PlayerBinding, PlayerModule, PlayerState};
use perro_structs::Vector2;
use std::cell::RefCell;

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

#[derive(Clone, Debug)]
pub struct InputSnapshot {
    keyboard: KeyboardState,
    mouse: MouseState,
    gamepads: Vec<GamepadState>,
    joycons: Vec<JoyConState>,
    players: Vec<PlayerState>,
    commands: RefCell<Vec<InputCommand>>,
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
                if index % 2 == 0 {
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
            commands.drain(..).collect::<Vec<_>>()
        };
        for command in pending.drain(..) {
            match command {
                InputCommand::BindPlayer { index, binding } => {
                    self.bind_player(index, binding);
                }
            }
        }
    }

    #[inline]
    pub fn set_gamepad_button_state(
        &mut self,
        index: usize,
        button: GamepadButton,
        is_down: bool,
    ) {
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
    pub fn set_joycon_button_state(
        &mut self,
        index: usize,
        button: JoyConButton,
        is_down: bool,
    ) {
        let state = self.joycon_mut(index);
        state.set_button_state(button, is_down);
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
}

impl Default for InputSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub enum InputCommand {
    BindPlayer { index: usize, binding: PlayerBinding },
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

    #[inline]
    pub fn Keyboard(&self) -> KeyboardModule<'_, IP> {
        KeyboardModule::new(self.ipt)
    }

    #[inline]
    pub fn MouseState(&self) -> MouseStateModule<'_, IP> {
        MouseStateModule::new(self.ipt)
    }

    #[inline]
    pub fn Gamepads(&self) -> GamepadModule<'_, IP> {
        GamepadModule::new(self.ipt)
    }

    #[inline]
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
}

#[derive(Clone, Debug)]
pub struct KeyboardState {
    down: Vec<u64>,
    pressed: Vec<u64>,
    released: Vec<u64>,
}

impl KeyboardState {
    pub fn new() -> Self {
        let words = KeyCode::COUNT.div_ceil(64);
        Self {
            down: vec![0; words],
            pressed: vec![0; words],
            released: vec![0; words],
        }
    }

    #[inline]
    pub fn begin_frame(&mut self) {
        self.pressed.fill(0);
        self.released.fill(0);
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
        Vector2::new(self.position_x, self.position_y)
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
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    #[inline]
    pub fn all(&self) -> &'ipt [GamepadState] {
        self.ipt.gamepads()
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&'ipt GamepadState> {
        self.ipt.gamepads().get(index)
    }
}

pub struct JoyConModule<'ipt, IP: InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: InputAPI + ?Sized> JoyConModule<'ipt, IP> {
    /// Creates a Joy-Con access wrapper.
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    #[inline]
    /// Returns all Joy-Con states (each entry is a single Joy-Con controller).
    pub fn all(&self) -> &'ipt [JoyConState] {
        self.ipt.joycons()
    }

    #[inline]
    /// Returns the Joy-Con at the given index.
    pub fn get(&self, index: usize) -> Option<&'ipt JoyConState> {
        self.ipt.joycons().get(index)
    }
}

#[macro_export]
macro_rules! key_down {
    ($ipt:expr, $key:expr) => {
        $ipt.Keys().down($key)
    };
}

#[macro_export]
macro_rules! key_pressed {
    ($ipt:expr, $key:expr) => {
        $ipt.Keys().pressed($key)
    };
}

#[macro_export]
macro_rules! key_released {
    ($ipt:expr, $key:expr) => {
        $ipt.Keys().released($key)
    };
}

#[macro_export]
macro_rules! mouse_down {
    ($ipt:expr, $button:expr) => {
        $ipt.Mouse().down($button)
    };
}

#[macro_export]
macro_rules! mouse_pressed {
    ($ipt:expr, $button:expr) => {
        $ipt.Mouse().pressed($button)
    };
}

#[macro_export]
macro_rules! mouse_released {
    ($ipt:expr, $button:expr) => {
        $ipt.Mouse().released($button)
    };
}

#[macro_export]
macro_rules! mouse_delta {
    ($ipt:expr) => {
        $ipt.Mouse().delta()
    };
}

#[macro_export]
macro_rules! mouse_wheel {
    ($ipt:expr) => {
        $ipt.Mouse().wheel()
    };
}

#[macro_export]
macro_rules! mouse_position {
    ($ipt:expr) => {
        $ipt.Mouse().position()
    };
}

#[macro_export]
macro_rules! viewport_size {
    ($ipt:expr) => {
        $ipt.Mouse().viewport_size()
    };
}

pub mod prelude {
    pub use crate::{
        GamepadAxis, GamepadButton, GamepadIndex, GamepadModule, GamepadState, InputAPI,
        InputContext, InputSnapshot, JoyConButton, JoyConIndex, JoyConModule, JoyConSide,
        JoyConState, KeyCode, KeyModule, KeyboardModule, KeyboardState, MouseButton, MouseModule,
        MouseState, MouseStateModule, PlayerBinding, PlayerModule, PlayerState, key_down,
        key_pressed, key_released, mouse_delta, mouse_down, mouse_position, mouse_pressed,
        mouse_released, mouse_wheel, viewport_size, joycon_list, joycon_down, joycon_get,
        joycon_side, joycon_pressed, joycon_released, joycon_stick, joycon_gyro, joycon_accel,
        gamepad_list, gamepad_get, gamepad_down, gamepad_pressed, gamepad_released,
        gamepad_left_stick, gamepad_right_stick, gamepad_gyro, gamepad_accel, player_list,
        player_get, player_bind,
    };
    pub use perro_structs::Vector2;
}

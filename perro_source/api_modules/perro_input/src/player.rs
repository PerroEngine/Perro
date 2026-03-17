use crate::{GamepadState, JoyConState, KeyboardState, MouseState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlayerBinding {
    None,
    Kbm,
    Gamepad { index: usize },
    JoyConSingle { index: usize },
    JoyConPair { left: usize, right: usize },
}

#[derive(Clone, Debug)]
pub struct PlayerState {
    binding: PlayerBinding,
}

impl PlayerState {
    pub fn new() -> Self {
        Self {
            binding: PlayerBinding::None,
        }
    }

    #[inline]
    pub fn begin_frame(&mut self) {}

    #[inline]
    pub fn binding(&self) -> PlayerBinding {
        self.binding
    }

    #[inline]
    pub fn set_binding(&mut self, binding: PlayerBinding) {
        self.binding = binding;
    }

    #[inline]
    pub fn kbm<'a>(
        &self,
        keyboard: &'a KeyboardState,
        mouse: &'a MouseState,
    ) -> Option<(&'a KeyboardState, &'a MouseState)> {
        match self.binding {
            PlayerBinding::Kbm => Some((keyboard, mouse)),
            _ => None,
        }
    }

    #[inline]
    pub fn gamepad<'a>(&self, gamepads: &'a [GamepadState]) -> Option<&'a GamepadState> {
        match self.binding {
            PlayerBinding::Gamepad { index } => gamepads.get(index),
            _ => None,
        }
    }

    #[inline]
    pub fn joycon_single<'a>(&self, joycons: &'a [JoyConState]) -> Option<&'a JoyConState> {
        match self.binding {
            PlayerBinding::JoyConSingle { index } => joycons.get(index),
            _ => None,
        }
    }

    #[inline]
    pub fn joycon_pair<'a>(
        &self,
        joycons: &'a [JoyConState],
    ) -> Option<(&'a JoyConState, &'a JoyConState)> {
        match self.binding {
            PlayerBinding::JoyConPair { left, right } => {
                Some((joycons.get(left)?, joycons.get(right)?))
            }
            _ => None,
        }
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PlayerModule<'ipt, IP: crate::InputAPI + ?Sized> {
    ipt: &'ipt IP,
}

impl<'ipt, IP: crate::InputAPI + ?Sized> PlayerModule<'ipt, IP> {
    pub fn new(ipt: &'ipt IP) -> Self {
        Self { ipt }
    }

    #[inline]
    pub fn all(&self) -> &'ipt [PlayerState] {
        self.ipt.players()
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&'ipt PlayerState> {
        self.ipt.players().get(index)
    }
}

#[macro_export]
macro_rules! player_list {
    ($ipt:expr) => {
        $ipt.Players().all()
    };
}

#[macro_export]
macro_rules! player_get {
    ($ipt:expr, $index:expr) => {
        $ipt.Players().get($index)
    };
}

#[macro_export]
macro_rules! player_bind {
    ($ipt:expr, $index:expr, $binding:expr) => {
        $ipt.bind_player($index, $binding)
    };
}

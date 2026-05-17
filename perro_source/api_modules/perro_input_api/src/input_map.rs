use crate::{
    GamepadButton, GamepadState, JoyConButton, JoyConState, KeyCode, KeyboardState, MouseButton,
    MouseState,
};

pub const fn action_hash(name: &str) -> u64 {
    perro_ids::string_to_u64(name)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputMap {
    actions: Vec<InputAction>,
    action_slots: Vec<(u64, usize)>,
    key_actions: Vec<Vec<usize>>,
    mouse_actions: Vec<Vec<usize>>,
    gamepad_actions: Vec<Vec<usize>>,
    joycon_actions: Vec<Vec<usize>>,
}

impl Default for InputMap {
    fn default() -> Self {
        Self::from_actions(Vec::new())
    }
}

impl InputMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_actions(mut actions: Vec<InputAction>) -> Self {
        for action in &mut actions {
            action.name_hash = action_hash(&action.name);
        }
        let mut input_map = Self {
            actions,
            action_slots: Vec::new(),
            key_actions: vec![Vec::new(); KeyCode::COUNT],
            mouse_actions: vec![Vec::new(); MouseButton::COUNT],
            gamepad_actions: vec![Vec::new(); GamepadButton::COUNT],
            joycon_actions: vec![Vec::new(); JoyConButton::COUNT],
        };
        input_map.rebuild_indexes();
        input_map
    }

    #[inline]
    pub fn actions(&self) -> &[InputAction] {
        &self.actions
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    #[inline]
    pub fn action(&self, name: &str) -> Option<&InputAction> {
        self.action_by_hash(action_hash(name))
    }

    #[inline]
    pub fn action_by_hash(&self, name_hash: u64) -> Option<&InputAction> {
        self.action_index(name_hash)
            .map(|index| &self.actions[index])
    }

    #[inline]
    pub fn action_index(&self, name_hash: u64) -> Option<usize> {
        self.action_slots
            .binary_search_by_key(&name_hash, |(hash, _)| *hash)
            .ok()
            .map(|slot| self.action_slots[slot].1)
    }

    #[inline]
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    #[inline]
    pub fn actions_for_key(&self, key: KeyCode) -> &[usize] {
        &self.key_actions[key.as_index()]
    }

    #[inline]
    pub fn actions_for_mouse(&self, button: MouseButton) -> &[usize] {
        &self.mouse_actions[button as usize]
    }

    #[inline]
    pub fn actions_for_gamepad(&self, button: GamepadButton) -> &[usize] {
        &self.gamepad_actions[button.as_index()]
    }

    #[inline]
    pub fn actions_for_joycon(&self, button: JoyConButton) -> &[usize] {
        &self.joycon_actions[button.as_index()]
    }

    #[inline]
    pub fn down(
        &self,
        name: &str,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepads: &[GamepadState],
        joycons: &[JoyConState],
    ) -> bool {
        self.down_hash(action_hash(name), keyboard, mouse, gamepads, joycons)
    }

    #[inline]
    pub fn pressed(
        &self,
        name: &str,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepads: &[GamepadState],
        joycons: &[JoyConState],
    ) -> bool {
        self.pressed_hash(action_hash(name), keyboard, mouse, gamepads, joycons)
    }

    #[inline]
    pub fn released(
        &self,
        name: &str,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepads: &[GamepadState],
        joycons: &[JoyConState],
    ) -> bool {
        self.released_hash(action_hash(name), keyboard, mouse, gamepads, joycons)
    }

    #[inline]
    pub fn down_hash(
        &self,
        name_hash: u64,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepads: &[GamepadState],
        joycons: &[JoyConState],
    ) -> bool {
        self.action_by_hash(name_hash).is_some_and(|action| {
            action
                .bindings
                .iter()
                .any(|binding| binding.down(keyboard, mouse, gamepads, joycons))
        })
    }

    #[inline]
    pub fn pressed_hash(
        &self,
        name_hash: u64,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepads: &[GamepadState],
        joycons: &[JoyConState],
    ) -> bool {
        self.action_by_hash(name_hash).is_some_and(|action| {
            action
                .bindings
                .iter()
                .any(|binding| binding.pressed(keyboard, mouse, gamepads, joycons))
        })
    }

    #[inline]
    pub fn released_hash(
        &self,
        name_hash: u64,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepads: &[GamepadState],
        joycons: &[JoyConState],
    ) -> bool {
        self.action_by_hash(name_hash).is_some_and(|action| {
            action
                .bindings
                .iter()
                .any(|binding| binding.released(keyboard, mouse, gamepads, joycons))
        })
    }

    fn rebuild_indexes(&mut self) {
        self.action_slots = self
            .actions
            .iter()
            .enumerate()
            .map(|(index, action)| (action.name_hash, index))
            .collect();
        self.action_slots.sort_by_key(|(hash, _)| *hash);

        for (index, action) in self.actions.iter().enumerate() {
            for binding in &action.bindings {
                match binding {
                    InputBinding::Key(key) => self.key_actions[key.as_index()].push(index),
                    InputBinding::Mouse(button) => self.mouse_actions[*button as usize].push(index),
                    InputBinding::Gamepad(button) => {
                        self.gamepad_actions[button.as_index()].push(index)
                    }
                    InputBinding::JoyCon(button) => {
                        self.joycon_actions[button.as_index()].push(index)
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputAction {
    pub name: String,
    pub name_hash: u64,
    pub bindings: Vec<InputBinding>,
}

impl InputAction {
    pub fn new(name: impl Into<String>, bindings: Vec<InputBinding>) -> Self {
        let name = name.into();
        Self {
            name_hash: action_hash(&name),
            name,
            bindings,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputBinding {
    Key(KeyCode),
    Mouse(MouseButton),
    Gamepad(GamepadButton),
    JoyCon(JoyConButton),
}

impl InputBinding {
    #[inline]
    fn down(
        self,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepads: &[GamepadState],
        joycons: &[JoyConState],
    ) -> bool {
        match self {
            Self::Key(key) => keyboard.is_key_down(key),
            Self::Mouse(button) => mouse.is_button_down(button),
            Self::Gamepad(button) => gamepads.iter().any(|pad| pad.is_button_down(button)),
            Self::JoyCon(button) => joycons.iter().any(|joycon| joycon.is_button_down(button)),
        }
    }

    #[inline]
    fn pressed(
        self,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepads: &[GamepadState],
        joycons: &[JoyConState],
    ) -> bool {
        match self {
            Self::Key(key) => keyboard.is_key_pressed(key),
            Self::Mouse(button) => mouse.is_button_pressed(button),
            Self::Gamepad(button) => gamepads.iter().any(|pad| pad.is_button_pressed(button)),
            Self::JoyCon(button) => joycons
                .iter()
                .any(|joycon| joycon.is_button_pressed(button)),
        }
    }

    #[inline]
    fn released(
        self,
        keyboard: &KeyboardState,
        mouse: &MouseState,
        gamepads: &[GamepadState],
        joycons: &[JoyConState],
    ) -> bool {
        match self {
            Self::Key(key) => keyboard.is_key_released(key),
            Self::Mouse(button) => mouse.is_button_released(button),
            Self::Gamepad(button) => gamepads.iter().any(|pad| pad.is_button_released(button)),
            Self::JoyCon(button) => joycons
                .iter()
                .any(|joycon| joycon.is_button_released(button)),
        }
    }
}

use crate::App;
use perro_graphics::GraphicsBackend;

mod backend {
    use super::*;
    use gilrs::{Axis, Button, EventType, Gilrs, GamepadId};
    use perro_input::{GamepadAxis, GamepadButton, GamepadIndex};
    use std::collections::{HashMap, HashSet};

    const ALL_BUTTONS: [GamepadButton; GamepadButton::COUNT] = [
        GamepadButton::Bottom,
        GamepadButton::Right,
        GamepadButton::Left,
        GamepadButton::Top,
        GamepadButton::DpadUp,
        GamepadButton::DpadDown,
        GamepadButton::DpadLeft,
        GamepadButton::DpadRight,
        GamepadButton::Start,
        GamepadButton::Select,
        GamepadButton::Home,
        GamepadButton::Capture,
        GamepadButton::L1,
        GamepadButton::R1,
        GamepadButton::L2,
        GamepadButton::R2,
        GamepadButton::L3,
        GamepadButton::R3,
    ];

    const ALL_AXES: [GamepadAxis; GamepadAxis::COUNT] = [
        GamepadAxis::LeftStickX,
        GamepadAxis::LeftStickY,
        GamepadAxis::RightStickX,
        GamepadAxis::RightStickY,
        GamepadAxis::LeftTrigger,
        GamepadAxis::RightTrigger,
    ];
    const JOYCON_VENDOR_ID: u16 = 0x057E;
    const JOYCON_1_LEFT_PID: u16 = 0x2006;
    const JOYCON_1_RIGHT_PID: u16 = 0x2007;

    #[derive(Default)]
    pub struct GamepadBackend {
        gilrs: Option<Gilrs>,
        assigned: HashMap<GamepadId, usize>,
        free_indices: Vec<usize>,
        next_index: usize,
        down: HashSet<(GamepadId, GamepadButton)>,
        assigned_uuids: HashMap<GamepadId, [u8; 16]>,
        uuid_in_use: HashSet<[u8; 16]>,
    }

    impl GamepadBackend {
        pub fn begin_frame<B: GraphicsBackend>(&mut self, app: &mut App<B>) {
            self.ensure_gilrs();
            let Some(mut gilrs) = self.gilrs.take() else {
                return;
            };

            while let Some(event) = gilrs.next_event() {
                self.handle_event(app, &gilrs, event);
            }

            self.gilrs = Some(gilrs);
        }

        fn ensure_gilrs(&mut self) {
            if self.gilrs.is_some() {
                return;
            }
            if let Ok(gilrs) = Gilrs::new() {
                self.gilrs = Some(gilrs);
            }
        }

        fn handle_event<B: GraphicsBackend>(
            &mut self,
            app: &mut App<B>,
            gilrs: &Gilrs,
            event: gilrs::Event,
        ) {
            let id = event.id;
            match event.event {
                EventType::Connected => {
                    let gp = gilrs.gamepad(id);
                    if is_joycon(&gp) {
                        return;
                    }
                    if let Some(index) = self.assign_index_if_unique(gilrs, id) {
                        log_gamepad_connected(gilrs, id, index);
                        clear_gamepad(app, index);
                    }
                }
                EventType::Disconnected => {
                    if let Some(index) = self.assigned.remove(&id) {
                        self.free_indices.push(index);
                        clear_gamepad(app, index);
                    }
                    self.down.retain(|(gp_id, _)| *gp_id != id);
                    if let Some(uuid) = self.assigned_uuids.remove(&id) {
                        self.uuid_in_use.remove(&uuid);
                    }
                }
                EventType::ButtonPressed(button, _) => {
                    if let Some(mapped) = map_button(button) {
                        self.set_button(app, gilrs, id, mapped, true);
                    }
                }
                EventType::ButtonRepeated(button, _) => {
                    if let Some(mapped) = map_button(button) {
                        self.set_button(app, gilrs, id, mapped, true);
                    }
                }
                EventType::ButtonReleased(button, _) => {
                    if let Some(mapped) = map_button(button) {
                        self.set_button(app, gilrs, id, mapped, false);
                    }
                }
                EventType::ButtonChanged(button, value, _) => {
                    if let Some(mapped) = map_button(button) {
                        self.set_button(app, gilrs, id, mapped, value > 0.5);
                    }
                }
                EventType::AxisChanged(axis, value, _) => {
                    if let Some(mapped) = map_axis(axis) {
                        if let Some(index) = self.assign_index_if_unique(gilrs, id) {
                            app.set_gamepad_axis(index, mapped, value);
                        }
                    } else {
                        self.handle_dpad_axis(app, gilrs, id, axis, value);
                    }
                }
                _ => {}
            }
        }

        fn set_button<B: GraphicsBackend>(
            &mut self,
            app: &mut App<B>,
            gilrs: &Gilrs,
            id: GamepadId,
            button: GamepadButton,
            is_down: bool,
        ) {
            let Some(index) = self.assign_index_if_unique(gilrs, id) else {
                return;
            };
            let key = (id, button);
            let was_down = self.down.contains(&key);
            if was_down == is_down {
                return;
            }
            if is_down {
                self.down.insert(key);
            } else {
                self.down.remove(&key);
            }
            app.set_gamepad_button_state(index, button, is_down);
        }

        fn handle_dpad_axis<B: GraphicsBackend>(
            &mut self,
            app: &mut App<B>,
            gilrs: &Gilrs,
            id: GamepadId,
            axis: Axis,
            value: f32,
        ) {
            match axis {
                Axis::DPadX => {
                    self.set_button(app, gilrs, id, GamepadButton::DpadLeft, value < -0.5);
                    self.set_button(app, gilrs, id, GamepadButton::DpadRight, value > 0.5);
                }
                Axis::DPadY => {
                    self.set_button(app, gilrs, id, GamepadButton::DpadUp, value > 0.5);
                    self.set_button(app, gilrs, id, GamepadButton::DpadDown, value < -0.5);
                }
                _ => {}
            }
        }

        fn assign_index_if_unique(&mut self, gilrs: &Gilrs, id: GamepadId) -> Option<usize> {
            if let Some(idx) = self.assigned.get(&id) {
                return Some(*idx);
            }
            let gp = gilrs.gamepad(id);
            if is_joycon(&gp) {
                return None;
            }
            let uuid = gp.uuid();
            if self.uuid_in_use.contains(&uuid) {
                return None;
            }
            let index = self.assign_index(id);
            self.uuid_in_use.insert(uuid);
            self.assigned_uuids.insert(id, uuid);
            Some(index)
        }

        fn assign_index(&mut self, id: GamepadId) -> usize {
            if let Some(idx) = self.assigned.get(&id) {
                return *idx;
            }
            let index = if self.free_indices.is_empty() {
                let idx = self.next_index;
                self.next_index = self.next_index.saturating_add(1);
                idx
            } else {
                self.free_indices.sort_unstable();
                self.free_indices.remove(0)
            };
            self.assigned.insert(id, index);
            index
        }
    }

    fn map_button(button: Button) -> Option<GamepadButton> {
        let mapped = match button {
            Button::South => GamepadButton::Bottom,
            Button::East => GamepadButton::Right,
            Button::West => GamepadButton::Left,
            Button::North => GamepadButton::Top,
            Button::DPadUp => GamepadButton::DpadUp,
            Button::DPadDown => GamepadButton::DpadDown,
            Button::DPadLeft => GamepadButton::DpadLeft,
            Button::DPadRight => GamepadButton::DpadRight,
            Button::Start => GamepadButton::Start,
            Button::Select => GamepadButton::Select,
            Button::Mode => GamepadButton::Home,
            Button::LeftTrigger => GamepadButton::L1,
            Button::RightTrigger => GamepadButton::R1,
            Button::LeftTrigger2 => GamepadButton::L2,
            Button::RightTrigger2 => GamepadButton::R2,
            Button::LeftThumb => GamepadButton::L3,
            Button::RightThumb => GamepadButton::R3,
            _ => return None,
        };
        Some(mapped)
    }

    fn map_axis(axis: Axis) -> Option<GamepadAxis> {
        let mapped = match axis {
            Axis::LeftStickX => GamepadAxis::LeftStickX,
            Axis::LeftStickY => GamepadAxis::LeftStickY,
            Axis::RightStickX => GamepadAxis::RightStickX,
            Axis::RightStickY => GamepadAxis::RightStickY,
            Axis::LeftZ => GamepadAxis::LeftTrigger,
            Axis::RightZ => GamepadAxis::RightTrigger,
            _ => return None,
        };
        Some(mapped)
    }

    fn clear_gamepad<B: GraphicsBackend>(app: &mut App<B>, index: usize) {
        for button in ALL_BUTTONS {
            app.set_gamepad_button_state(index, button, false);
        }
        for axis in ALL_AXES {
            app.set_gamepad_axis(index, axis, 0.0);
        }
        app.set_gamepad_gyro(index, 0.0, 0.0, 0.0);
        app.set_gamepad_accel(index, 0.0, 0.0, 0.0);
    }

    fn log_gamepad_connected(gilrs: &Gilrs, id: GamepadId, index: usize) {
        let gp = gilrs.gamepad(id);
        let name = gp.name();
        let vendor = gp.vendor_id();
        let product = gp.product_id();
        let idx = GamepadIndex(index);
        eprintln!(
            "[gamepad] connected index={:?} name=\"{}\" vid={:?} pid={:?}",
            idx, name, vendor, product
        );
    }

    fn is_joycon(gp: &gilrs::Gamepad<'_>) -> bool {
        let Some(vendor) = gp.vendor_id() else {
            return false;
        };
        let Some(product) = gp.product_id() else {
            return false;
        };
        if vendor != JOYCON_VENDOR_ID {
            return false;
        }
        product == JOYCON_1_LEFT_PID || product == JOYCON_1_RIGHT_PID
    }
}

#[derive(Default)]
pub struct GamepadInput {
    backend: backend::GamepadBackend,
}

impl GamepadInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_frame<B: GraphicsBackend>(&mut self, app: &mut App<B>) {
        self.backend.begin_frame(app);
    }
}

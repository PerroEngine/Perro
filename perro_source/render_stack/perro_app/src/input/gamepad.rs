use crate::App;
use perro_graphics::GraphicsBackend;

mod backend {
    use super::*;
    use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};
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
    const STATE_SYNC_INTERVAL_FRAMES: u32 = 4;
    const IDLE_POLL_INTERVAL_FRAMES: u32 = 4;

    #[derive(Default)]
    pub struct GamepadBackend {
        gilrs: Option<Gilrs>,
        id_to_uuid: HashMap<GamepadId, [u8; 16]>,
        uuid_to_index: HashMap<[u8; 16], usize>,
        index_to_uuid: Vec<Option<[u8; 16]>>,
        free_indices: Vec<usize>,
        next_index: usize,
        down: HashSet<(GamepadId, GamepadButton)>,
        uuid_in_use: HashSet<[u8; 16]>,
        state_sync_frame_counter: u32,
        idle_poll_frame_counter: u32,
    }

    impl GamepadBackend {
        pub fn begin_frame<B: GraphicsBackend>(&mut self, app: &mut App<B>) {
            self.ensure_gilrs();
            let Some(mut gilrs) = self.gilrs.take() else {
                return;
            };

            // When no non-JoyCon gamepad is active, throttle gilrs polling.
            // This keeps hot-loop overhead low for KBM-only projects while still
            // discovering new controllers quickly.
            if self.uuid_in_use.is_empty() {
                self.idle_poll_frame_counter = self.idle_poll_frame_counter.wrapping_add(1);
                if !self
                    .idle_poll_frame_counter
                    .is_multiple_of(IDLE_POLL_INTERVAL_FRAMES)
                {
                    self.gilrs = Some(gilrs);
                    return;
                }
            }

            while let Some(event) = gilrs.next_event() {
                self.handle_event(app, &gilrs, event);
            }

            // Some controllers/drivers (notably on Windows) can miss or coalesce
            // button events. Keep a periodic sync as a safety net, but avoid
            // full per-frame scans when there are no active gamepads.
            self.state_sync_frame_counter = self.state_sync_frame_counter.wrapping_add(1);
            let should_sync = !self.uuid_in_use.is_empty()
                && self
                    .state_sync_frame_counter
                    .is_multiple_of(STATE_SYNC_INTERVAL_FRAMES);
            if should_sync {
                self.sync_buttons(app, &gilrs);
                self.sync_axes(app, &gilrs);
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
                    self.handle_disconnect(app, id);
                    self.down.retain(|(gp_id, _)| *gp_id != id);
                    if let Some(uuid) = self.id_to_uuid.remove(&id) {
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
            if let Some(uuid) = self.id_to_uuid.get(&id) {
                return self.uuid_to_index.get(uuid).copied();
            }
            let gp = gilrs.gamepad(id);
            if is_joycon(&gp) {
                return None;
            }
            let uuid = gp.uuid();
            if self.uuid_in_use.contains(&uuid) {
                return None;
            }
            let index = if let Some(idx) = self.uuid_to_index.get(&uuid) {
                *idx
            } else {
                self.assign_index(uuid)
            };
            self.free_indices.retain(|free| *free != index);
            self.uuid_in_use.insert(uuid);
            self.id_to_uuid.insert(id, uuid);
            Some(index)
        }

        fn handle_disconnect<B: GraphicsBackend>(&mut self, app: &mut App<B>, id: GamepadId) {
            let Some(uuid) = self.id_to_uuid.get(&id).copied() else {
                return;
            };
            let Some(index) = self.uuid_to_index.get(&uuid).copied() else {
                return;
            };
            if !self.free_indices.contains(&index) {
                self.free_indices.push(index);
            }
            clear_gamepad(app, index);
        }

        fn assign_index(&mut self, uuid: [u8; 16]) -> usize {
            const MAX_PERSISTENT_GAMEPAD_SLOTS: usize = 12;

            let index = if self.next_index < MAX_PERSISTENT_GAMEPAD_SLOTS {
                let idx = self.next_index;
                self.next_index = self.next_index.saturating_add(1);
                idx
            } else if !self.free_indices.is_empty() {
                self.free_indices.sort_unstable();
                let idx = self.free_indices.remove(0);
                if let Some(old_uuid) = self.index_to_uuid.get(idx).and_then(|v| *v) {
                    self.uuid_to_index.remove(&old_uuid);
                }
                idx
            } else {
                let idx = self.next_index;
                self.next_index = self.next_index.saturating_add(1);
                idx
            };

            if self.index_to_uuid.len() <= index {
                self.index_to_uuid.resize(index + 1, None);
            }
            self.index_to_uuid[index] = Some(uuid);
            self.uuid_to_index.insert(uuid, index);
            index
        }

        fn sync_buttons<B: GraphicsBackend>(&mut self, app: &mut App<B>, gilrs: &Gilrs) {
            let ids: Vec<GamepadId> = self.id_to_uuid.keys().copied().collect();
            for id in ids {
                let gp = gilrs.gamepad(id);
                if !gp.is_connected() || is_joycon(&gp) {
                    continue;
                }
                for button in ALL_BUTTONS {
                    let Some(gilrs_button) = map_button_to_gilrs(button) else {
                        continue;
                    };
                    let is_down = gp.is_pressed(gilrs_button);
                    self.set_button(app, gilrs, id, button, is_down);
                }
            }
        }

        fn sync_axes<B: GraphicsBackend>(&mut self, app: &mut App<B>, gilrs: &Gilrs) {
            let ids: Vec<GamepadId> = self.id_to_uuid.keys().copied().collect();
            for id in ids {
                let gp = gilrs.gamepad(id);
                if !gp.is_connected() || is_joycon(&gp) {
                    continue;
                }
                let Some(index) = self
                    .id_to_uuid
                    .get(&id)
                    .and_then(|u| self.uuid_to_index.get(u))
                    .copied()
                else {
                    continue;
                };
                for axis in ALL_AXES {
                    let Some(gilrs_axis) = map_axis_to_gilrs(axis) else {
                        continue;
                    };
                    let value = gp.value(gilrs_axis);
                    app.set_gamepad_axis(index, axis, value);
                }
            }
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

    fn map_button_to_gilrs(button: GamepadButton) -> Option<Button> {
        let mapped = match button {
            GamepadButton::Bottom => Button::South,
            GamepadButton::Right => Button::East,
            GamepadButton::Left => Button::West,
            GamepadButton::Top => Button::North,
            GamepadButton::DpadUp => Button::DPadUp,
            GamepadButton::DpadDown => Button::DPadDown,
            GamepadButton::DpadLeft => Button::DPadLeft,
            GamepadButton::DpadRight => Button::DPadRight,
            GamepadButton::Start => Button::Start,
            GamepadButton::Select => Button::Select,
            GamepadButton::Home => Button::Mode,
            GamepadButton::Capture => return None,
            GamepadButton::L1 => Button::LeftTrigger,
            GamepadButton::R1 => Button::RightTrigger,
            GamepadButton::L2 => Button::LeftTrigger2,
            GamepadButton::R2 => Button::RightTrigger2,
            GamepadButton::L3 => Button::LeftThumb,
            GamepadButton::R3 => Button::RightThumb,
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

    fn map_axis_to_gilrs(axis: GamepadAxis) -> Option<Axis> {
        let mapped = match axis {
            GamepadAxis::LeftStickX => Axis::LeftStickX,
            GamepadAxis::LeftStickY => Axis::LeftStickY,
            GamepadAxis::RightStickX => Axis::RightStickX,
            GamepadAxis::RightStickY => Axis::RightStickY,
            GamepadAxis::LeftTrigger => Axis::LeftZ,
            GamepadAxis::RightTrigger => Axis::RightZ,
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

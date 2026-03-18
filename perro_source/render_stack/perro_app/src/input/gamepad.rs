use crate::App;
use perro_graphics::GraphicsBackend;

mod backend {
    use super::*;
    use gilrs::{Axis, Button, EventType, Gilrs, GamepadId};
    use perro_input::{GamepadAxis, GamepadButton};
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

    #[derive(Default)]
    pub struct GamepadBackend {
        gilrs: Option<Gilrs>,
        assigned: HashMap<GamepadId, usize>,
        free_indices: Vec<usize>,
        next_index: usize,
        down: HashSet<(GamepadId, GamepadButton)>,
    }

    impl GamepadBackend {
        pub fn begin_frame<B: GraphicsBackend>(&mut self, app: &mut App<B>) {
            self.ensure_gilrs();
            let Some(mut gilrs) = self.gilrs.take() else {
                return;
            };

            while let Some(event) = gilrs.next_event() {
                self.handle_event(app, event);
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

        fn handle_event<B: GraphicsBackend>(&mut self, app: &mut App<B>, event: gilrs::Event) {
            let id = event.id;
            match event.event {
                EventType::Connected => {
                    let _ = self.assign_index(id);
                }
                EventType::Disconnected => {
                    if let Some(index) = self.assigned.remove(&id) {
                        self.free_indices.push(index);
                        clear_gamepad(app, index);
                    }
                    self.down.retain(|(gp_id, _)| *gp_id != id);
                }
                EventType::ButtonPressed(button, _) => {
                    if let Some(mapped) = map_button(button) {
                        if self.down.contains(&(id, mapped)) {
                            return;
                        }
                        self.down.insert((id, mapped));
                        let index = self.assign_index(id);
                        app.set_gamepad_button_state(index, mapped, true);
                    }
                }
                EventType::ButtonReleased(button, _) => {
                    if let Some(mapped) = map_button(button) {
                        self.down.remove(&(id, mapped));
                        let index = self.assign_index(id);
                        app.set_gamepad_button_state(index, mapped, false);
                    }
                }
                EventType::AxisChanged(axis, value, _) => {
                    if let Some(mapped) = map_axis(axis) {
                        let index = self.assign_index(id);
                        app.set_gamepad_axis(index, mapped, value);
                    }
                }
                _ => {}
            }
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

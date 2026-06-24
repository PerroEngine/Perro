use crate::{
    GamepadAxis, GamepadButton, InputSnapshot, JoyConButton, JoyConSide, KeyCode, MouseButton,
    MouseMode, PlayerBinding,
};
use perro_structs::SignedUnitVector2;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    Key {
        key: KeyCode,
        is_down: bool,
    },
    Text(String),
    MouseButton {
        button: MouseButton,
        is_down: bool,
    },
    MouseDelta {
        dx: f32,
        dy: f32,
    },
    MouseWheel {
        dx: f32,
        dy: f32,
    },
    MousePosition {
        x: f32,
        y: f32,
    },
    MouseMode(MouseMode),
    ViewportSize {
        width: u32,
        height: u32,
    },
    GamepadButton {
        index: usize,
        button: GamepadButton,
        is_down: bool,
    },
    GamepadAxis {
        index: usize,
        axis: GamepadAxis,
        value: f32,
    },
    GamepadGyro {
        index: usize,
        x: f32,
        y: f32,
        z: f32,
    },
    GamepadAccel {
        index: usize,
        x: f32,
        y: f32,
        z: f32,
    },
    JoyConButton {
        index: usize,
        button: JoyConButton,
        is_down: bool,
    },
    JoyConStick {
        index: usize,
        stick: SignedUnitVector2,
    },
    JoyConSide {
        index: usize,
        side: JoyConSide,
    },
    JoyConConnected {
        index: usize,
        connected: bool,
    },
    JoyConCalibrated {
        index: usize,
        calibrated: bool,
    },
    JoyConCalibrationInProgress {
        index: usize,
        in_progress: bool,
    },
    JoyConCalibrationBias {
        index: usize,
        x: f32,
        y: f32,
        z: f32,
    },
    JoyConGyro {
        index: usize,
        x: f32,
        y: f32,
        z: f32,
    },
    JoyConAccel {
        index: usize,
        x: f32,
        y: f32,
        z: f32,
    },
    JoyConMouseSensor {
        index: usize,
        x: f32,
        y: f32,
        extra: f32,
        distance: f32,
    },
    BindPlayer {
        index: usize,
        binding: PlayerBinding,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputFrame {
    events: Vec<InputEvent>,
    dropped_events: u64,
}

impl InputFrame {
    pub fn new(events: Vec<InputEvent>, dropped_events: u64) -> Self {
        Self {
            events,
            dropped_events,
        }
    }

    #[inline]
    pub fn events(&self) -> &[InputEvent] {
        &self.events
    }

    #[inline]
    pub fn dropped_events(&self) -> u64 {
        self.dropped_events
    }

    pub fn apply_to_snapshot(&self, snapshot: &mut InputSnapshot) {
        snapshot.begin_frame();
        for event in &self.events {
            apply_event(snapshot, event);
        }
    }
}

pub struct InputRingBuffer {
    events: VecDeque<InputEvent>,
    capacity: usize,
    dropped_events: u64,
}

impl InputRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
            dropped_events: 0,
        }
    }

    pub fn push(&mut self, event: InputEvent) {
        if self.capacity == 0 {
            self.dropped_events = self.dropped_events.saturating_add(1);
            return;
        }
        if self.events.len() == self.capacity {
            self.events.pop_front();
            self.dropped_events = self.dropped_events.saturating_add(1);
        }
        self.events.push_back(event);
    }

    pub fn seal_frame(&mut self) -> InputFrame {
        let dropped = self.dropped_events;
        self.dropped_events = 0;
        InputFrame::new(self.events.drain(..).collect(), dropped)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    #[inline]
    pub fn dropped_events(&self) -> u64 {
        self.dropped_events
    }
}

fn apply_event(snapshot: &mut InputSnapshot, event: &InputEvent) {
    match event {
        InputEvent::Key { key, is_down } => snapshot.set_key_state(*key, *is_down),
        InputEvent::Text(text) => snapshot.push_text_input(text.clone()),
        InputEvent::MouseButton { button, is_down } => {
            snapshot.set_mouse_button_state(*button, *is_down)
        }
        InputEvent::MouseDelta { dx, dy } => snapshot.add_mouse_delta(*dx, *dy),
        InputEvent::MouseWheel { dx, dy } => snapshot.add_mouse_wheel(*dx, *dy),
        InputEvent::MousePosition { x, y } => snapshot.set_mouse_position(*x, *y),
        InputEvent::MouseMode(mode) => snapshot.set_mouse_mode_state(*mode),
        InputEvent::ViewportSize { width, height } => snapshot.set_viewport_size(*width, *height),
        InputEvent::GamepadButton {
            index,
            button,
            is_down,
        } => snapshot.set_gamepad_button_state(*index, *button, *is_down),
        InputEvent::GamepadAxis { index, axis, value } => {
            snapshot.set_gamepad_axis(*index, *axis, *value)
        }
        InputEvent::GamepadGyro { index, x, y, z } => snapshot.set_gamepad_gyro(*index, *x, *y, *z),
        InputEvent::GamepadAccel { index, x, y, z } => {
            snapshot.set_gamepad_accel(*index, *x, *y, *z)
        }
        InputEvent::JoyConButton {
            index,
            button,
            is_down,
        } => snapshot.set_joycon_button_state(*index, *button, *is_down),
        InputEvent::JoyConStick { index, stick } => snapshot.set_joycon_stick_unit(*index, *stick),
        InputEvent::JoyConSide { index, side } => snapshot.set_joycon_side(*index, *side),
        InputEvent::JoyConConnected { index, connected } => {
            snapshot.set_joycon_connected(*index, *connected)
        }
        InputEvent::JoyConCalibrated { index, calibrated } => {
            snapshot.set_joycon_calibrated(*index, *calibrated)
        }
        InputEvent::JoyConCalibrationInProgress { index, in_progress } => {
            snapshot.set_joycon_calibration_in_progress(*index, *in_progress)
        }
        InputEvent::JoyConCalibrationBias { index, x, y, z } => {
            snapshot.set_joycon_calibration_bias(*index, *x, *y, *z)
        }
        InputEvent::JoyConGyro { index, x, y, z } => snapshot.set_joycon_gyro(*index, *x, *y, *z),
        InputEvent::JoyConAccel { index, x, y, z } => snapshot.set_joycon_accel(*index, *x, *y, *z),
        InputEvent::JoyConMouseSensor {
            index,
            x,
            y,
            extra,
            distance,
        } => snapshot.set_joycon_mouse_sensor(*index, *x, *y, *extra, *distance),
        InputEvent::BindPlayer { index, binding } => snapshot.bind_player(*index, *binding),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InputAPI, MouseButton};

    #[test]
    fn ring_drops_oldest_when_full() {
        let mut ring = InputRingBuffer::new(2);
        ring.push(InputEvent::Text("a".to_string()));
        ring.push(InputEvent::Text("b".to_string()));
        ring.push(InputEvent::Text("c".to_string()));

        let frame = ring.seal_frame();

        assert_eq!(frame.dropped_events(), 1);
        assert_eq!(
            frame.events(),
            &[
                InputEvent::Text("b".to_string()),
                InputEvent::Text("c".to_string())
            ]
        );
        assert!(ring.is_empty());
    }

    #[test]
    fn frame_preserves_pressed_released_and_delta_semantics() {
        let mut snapshot = InputSnapshot::new();
        InputFrame::new(
            vec![
                InputEvent::Key {
                    key: KeyCode::KeyA,
                    is_down: true,
                },
                InputEvent::MouseButton {
                    button: MouseButton::Left,
                    is_down: true,
                },
                InputEvent::MouseDelta { dx: 3.0, dy: -2.0 },
                InputEvent::Text("x".to_string()),
            ],
            0,
        )
        .apply_to_snapshot(&mut snapshot);

        assert!(snapshot.is_key_down(KeyCode::KeyA));
        assert!(snapshot.is_key_pressed(KeyCode::KeyA));
        assert!(snapshot.is_mouse_down(MouseButton::Left));
        assert!(snapshot.is_mouse_pressed(MouseButton::Left));
        assert_eq!(snapshot.mouse_delta().x, 3.0);
        assert_eq!(snapshot.mouse_delta().y, -2.0);
        assert_eq!(snapshot.keyboard().text_inputs(), &["x".to_string()]);

        InputFrame::default().apply_to_snapshot(&mut snapshot);

        assert!(snapshot.is_key_down(KeyCode::KeyA));
        assert!(!snapshot.is_key_pressed(KeyCode::KeyA));
        assert_eq!(snapshot.mouse_delta().x, 0.0);
        assert!(snapshot.keyboard().text_inputs().is_empty());

        InputFrame::new(
            vec![InputEvent::Key {
                key: KeyCode::KeyA,
                is_down: false,
            }],
            0,
        )
        .apply_to_snapshot(&mut snapshot);

        assert!(!snapshot.is_key_down(KeyCode::KeyA));
        assert!(snapshot.is_key_released(KeyCode::KeyA));
    }
}

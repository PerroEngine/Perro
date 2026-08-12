use crate::App;
use perro_graphics::GraphicsBackend;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use perro_input_api::{GamepadAxis, GamepadButton};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
trait GamepadSink {
    fn set_gamepad_connected(&mut self, index: usize, connected: bool);
    fn set_gamepad_button_state(&mut self, index: usize, button: GamepadButton, is_down: bool);
    fn set_gamepad_axis(&mut self, index: usize, axis: GamepadAxis, value: f32);
    fn set_gamepad_gyro(&mut self, index: usize, x: f32, y: f32, z: f32);
    fn set_gamepad_accel(&mut self, index: usize, x: f32, y: f32, z: f32);
    fn take_gamepad_rumble_requests(&mut self) -> Vec<perro_input_api::GamepadRumbleRequest>;
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
impl<B: GraphicsBackend> GamepadSink for App<B> {
    fn set_gamepad_connected(&mut self, index: usize, connected: bool) {
        App::set_gamepad_connected(self, index, connected);
    }

    fn set_gamepad_button_state(&mut self, index: usize, button: GamepadButton, is_down: bool) {
        App::set_gamepad_button_state(self, index, button, is_down);
    }

    fn set_gamepad_axis(&mut self, index: usize, axis: GamepadAxis, value: f32) {
        App::set_gamepad_axis(self, index, axis, value);
    }

    fn set_gamepad_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        App::set_gamepad_gyro(self, index, x, y, z);
    }

    fn set_gamepad_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        App::set_gamepad_accel(self, index, x, y, z);
    }

    fn take_gamepad_rumble_requests(&mut self) -> Vec<perro_input_api::GamepadRumbleRequest> {
        App::take_gamepad_rumble_requests(self)
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
mod backend {
    use super::*;
    use gilrs::ff::{BaseEffect, BaseEffectType, Effect, EffectBuilder, Repeat, Replay, Ticks};
    use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};
    use perro_input_api::{GamepadAxis, GamepadButton, GamepadIndex};
    #[cfg(target_os = "windows")]
    use rusty_xinput::XInputHandle;
    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};
    use std::sync::OnceLock;

    thread_local! {
        static PREINIT_GILRS: RefCell<Option<Gilrs>> = const { RefCell::new(None) };
    }

    pub(super) fn preinit() {
        PREINIT_GILRS.with(|slot| {
            if slot.borrow().is_none() {
                match Gilrs::new() {
                    Ok(gilrs) => {
                        let count = gilrs
                            .gamepads()
                            .filter(|(_, gamepad)| gamepad.is_connected())
                            .count();
                        eprintln!("[gamepad] backend preinit connected={count}");
                        *slot.borrow_mut() = Some(gilrs);
                    }
                    Err(err) => eprintln!("[gamepad][error] backend preinit failed: {err}"),
                }
            }
        });
    }

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
    const RUMBLE_PLAY_FOR_MS: u32 = 120;

    #[derive(Default)]
    pub struct GamepadBackend {
        gilrs: Option<Gilrs>,
        id_to_uuid: HashMap<GamepadId, [u8; 16]>,
        uuid_to_index: HashMap<[u8; 16], usize>,
        index_to_uuid: Vec<Option<[u8; 16]>>,
        free_indices: Vec<usize>,
        free_index_set: HashSet<usize>,
        next_index: usize,
        down_masks: HashMap<GamepadId, u32>,
        uuid_in_use: HashSet<[u8; 16]>,
        rumble_effects: HashMap<usize, Effect>,
        sync_ids: Vec<GamepadId>,
        state_sync_frame_counter: u32,
        gilrs_init_warned: bool,
        backend_ready_logged: bool,
        #[cfg(target_os = "windows")]
        xinput: Option<XInputHandle>,
        #[cfg(target_os = "windows")]
        xinput_connected: [bool; 4],
    }

    impl GamepadBackend {
        pub(super) fn begin_frame<S: GamepadSink>(&mut self, app: &mut S) {
            self.ensure_gilrs();
            self.consume_output_requests(app);
            let Some(mut gilrs) = self.gilrs.take() else {
                return;
            };

            // Poll every frame even while empty. Hotplug is an event-driven path,
            // so delaying this poll also delays gilrs' live device-table update.
            while let Some(event) = gilrs.next_event() {
                self.handle_event(app, &gilrs, event);
            }

            // `gilrs` does not guarantee a Connected event for every controller
            // that was already present when the backend started. Scan after the
            // event drain so the live connection flags are current.
            self.discover_connected(app, &gilrs);

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

            #[cfg(target_os = "windows")]
            self.poll_xinput(app);

            self.gilrs = Some(gilrs);
        }

        #[cfg(feature = "steamworks")]
        pub(super) fn collect_connected_indices(&self, out: &mut Vec<usize>) {
            out.clear();
            out.extend(
                self.uuid_in_use
                    .iter()
                    .filter_map(|uuid| self.uuid_to_index.get(uuid).copied()),
            );
        }

        fn consume_output_requests<S: GamepadSink>(&mut self, app: &mut S) {
            for req in app.take_gamepad_rumble_requests() {
                self.apply_rumble(
                    req.index,
                    req.rumble.low_frequency,
                    req.rumble.high_frequency,
                );
            }
        }

        fn apply_rumble(&mut self, index: usize, low_frequency: f32, high_frequency: f32) {
            let low = low_frequency.clamp(0.0, 1.0);
            let high = high_frequency.clamp(0.0, 1.0);

            if low <= f32::EPSILON && high <= f32::EPSILON {
                self.stop_rumble(index);
                return;
            }

            let Some(id) = self.find_gamepad_id_by_index(index) else {
                return;
            };
            self.stop_rumble(index);

            let Some(gilrs) = self.gilrs.as_mut() else {
                return;
            };
            let gp = gilrs.gamepad(id);
            if !gp.is_connected() || !gp.is_ff_supported() {
                return;
            }

            let duration = Ticks::from_ms(RUMBLE_PLAY_FOR_MS);
            let weak = magnitude_from_unit(low);
            let strong = magnitude_from_unit(high);
            let mut builder = EffectBuilder::new();
            if strong > 0 {
                builder.add_effect(BaseEffect {
                    kind: BaseEffectType::Strong { magnitude: strong },
                    scheduling: Replay {
                        play_for: duration,
                        ..Default::default()
                    },
                    ..Default::default()
                });
            }
            if weak > 0 {
                builder.add_effect(BaseEffect {
                    kind: BaseEffectType::Weak { magnitude: weak },
                    scheduling: Replay {
                        play_for: duration,
                        ..Default::default()
                    },
                    ..Default::default()
                });
            }
            builder.gamepads(&[id]).repeat(Repeat::Infinitely);
            if let Ok(effect) = builder.finish(gilrs) {
                let _ = effect.play();
                self.rumble_effects.insert(index, effect);
            }
        }

        fn stop_rumble(&mut self, index: usize) {
            if let Some(effect) = self.rumble_effects.remove(&index) {
                let _ = effect.stop();
            }
        }

        fn find_gamepad_id_by_index(&self, index: usize) -> Option<GamepadId> {
            for (id, uuid) in &self.id_to_uuid {
                if self.uuid_to_index.get(uuid).copied() == Some(index) {
                    return Some(*id);
                }
            }
            None
        }

        fn ensure_gilrs(&mut self) {
            if self.gilrs.is_some() {
                return;
            }
            if let Some(gilrs) = PREINIT_GILRS.with(|slot| slot.borrow_mut().take()) {
                self.log_backend_ready(&gilrs);
                self.gilrs = Some(gilrs);
                return;
            }
            match Gilrs::new() {
                Ok(gilrs) => {
                    self.log_backend_ready(&gilrs);
                    self.gilrs = Some(gilrs);
                }
                Err(err) => {
                    if !self.gilrs_init_warned {
                        eprintln!("[gamepad][error] backend init failed: {err}");
                        self.gilrs_init_warned = true;
                    }
                }
            }
        }

        fn log_backend_ready(&mut self, gilrs: &Gilrs) {
            if self.backend_ready_logged {
                return;
            }
            let count = gilrs
                .gamepads()
                .filter(|(_, gamepad)| gamepad.is_connected())
                .count();
            eprintln!("[gamepad] backend ready connected={count}");
            self.backend_ready_logged = true;
        }

        fn discover_connected<S: GamepadSink>(&mut self, app: &mut S, gilrs: &Gilrs) {
            let ids: Vec<_> = gilrs
                .gamepads()
                .filter_map(|(id, gamepad)| {
                    (gamepad.is_connected() && !is_joycon(&gamepad)).then_some(id)
                })
                .collect();
            for id in ids {
                if self.id_to_uuid.contains_key(&id) {
                    continue;
                }
                if let Some(index) = self.assign_index_if_unique(gilrs, id) {
                    log_gamepad_connected(gilrs, id, index);
                    clear_gamepad(app, index);
                    app.set_gamepad_connected(index, true);
                }
            }
        }

        fn handle_event<S: GamepadSink>(
            &mut self,
            app: &mut S,
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
                        app.set_gamepad_connected(index, true);
                    }
                }
                EventType::Disconnected => {
                    let disconnected_index = self
                        .id_to_uuid
                        .get(&id)
                        .and_then(|uuid| self.uuid_to_index.get(uuid).copied());
                    self.handle_disconnect(app, id);
                    if let Some(index) = disconnected_index {
                        self.stop_rumble(index);
                        app.set_gamepad_connected(index, false);
                    }
                    self.down_masks.remove(&id);
                    if let Some(uuid) = self.id_to_uuid.remove(&id) {
                        self.uuid_in_use.remove(&uuid);
                    }
                }
                EventType::ButtonPressed(button, _) => {
                    if let Some(mapped) = map_button(button) {
                        self.set_button(app, gilrs, id, mapped, true);
                        self.set_trigger_axis_from_button(app, gilrs, id, button, 1.0);
                        self.log_raw_like_state(gilrs, id, "button_pressed");
                    }
                }
                EventType::ButtonRepeated(button, _) => {
                    if let Some(mapped) = map_button(button) {
                        self.set_button(app, gilrs, id, mapped, true);
                        self.set_trigger_axis_from_button(app, gilrs, id, button, 1.0);
                        self.log_raw_like_state(gilrs, id, "button_repeated");
                    }
                }
                EventType::ButtonReleased(button, _) => {
                    if let Some(mapped) = map_button(button) {
                        self.set_button(app, gilrs, id, mapped, false);
                        self.set_trigger_axis_from_button(app, gilrs, id, button, 0.0);
                        self.log_raw_like_state(gilrs, id, "button_released");
                    }
                }
                EventType::ButtonChanged(button, value, _) => {
                    if let Some(mapped) = map_button(button) {
                        self.set_button(app, gilrs, id, mapped, value > 0.5);
                        self.set_trigger_axis_from_button(app, gilrs, id, button, value);
                        self.log_raw_like_state(gilrs, id, "button_changed");
                    }
                }
                EventType::AxisChanged(axis, value, _) => {
                    if let Some(mapped) = map_axis(axis) {
                        if let Some(index) = self.assign_index_if_unique(gilrs, id) {
                            app.set_gamepad_axis(index, mapped, value);
                            self.log_raw_like_state(gilrs, id, "axis_changed");
                        }
                    } else {
                        self.handle_dpad_axis(app, gilrs, id, axis, value);
                        self.log_raw_like_state(gilrs, id, "dpad_axis_changed");
                    }
                }
                _ => {}
            }
        }

        fn set_button<S: GamepadSink>(
            &mut self,
            app: &mut S,
            gilrs: &Gilrs,
            id: GamepadId,
            button: GamepadButton,
            is_down: bool,
        ) {
            let Some(index) = self.assign_index_if_unique(gilrs, id) else {
                return;
            };
            let bit = 1u32 << (button as usize);
            let current = self.down_masks.get(&id).copied().unwrap_or(0);
            let was_down = (current & bit) != 0;
            if was_down == is_down {
                return;
            }
            let mut next = current;
            if is_down {
                next |= bit;
            } else {
                next &= !bit;
            }
            if next == 0 {
                self.down_masks.remove(&id);
            } else {
                self.down_masks.insert(id, next);
            }
            app.set_gamepad_button_state(index, button, is_down);
        }

        fn handle_dpad_axis<S: GamepadSink>(
            &mut self,
            app: &mut S,
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

        fn set_trigger_axis_from_button<S: GamepadSink>(
            &mut self,
            app: &mut S,
            gilrs: &Gilrs,
            id: GamepadId,
            button: Button,
            value: f32,
        ) {
            let axis = match button {
                Button::LeftTrigger2 => Some(GamepadAxis::LeftTrigger),
                Button::RightTrigger2 => Some(GamepadAxis::RightTrigger),
                _ => None,
            };
            let Some(axis) = axis else {
                return;
            };
            let Some(index) = self.assign_index_if_unique(gilrs, id) else {
                return;
            };
            app.set_gamepad_axis(index, axis, value.clamp(0.0, 1.0));
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
            if self.free_index_set.remove(&index)
                && let Some(pos) = self.free_indices.iter().position(|&v| v == index)
            {
                self.free_indices.swap_remove(pos);
            }
            self.uuid_in_use.insert(uuid);
            self.id_to_uuid.insert(id, uuid);
            Some(index)
        }

        fn handle_disconnect<S: GamepadSink>(&mut self, app: &mut S, id: GamepadId) {
            let Some(uuid) = self.id_to_uuid.get(&id).copied() else {
                return;
            };
            let Some(index) = self.uuid_to_index.get(&uuid).copied() else {
                return;
            };
            if self.free_index_set.insert(index) {
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
                let idx = self.free_indices.pop().expect("checked non-empty");
                self.free_index_set.remove(&idx);
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

        fn sync_buttons<S: GamepadSink>(&mut self, app: &mut S, gilrs: &Gilrs) {
            self.sync_ids.clear();
            self.sync_ids.extend(self.id_to_uuid.keys().copied());
            while let Some(id) = self.sync_ids.pop() {
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

        fn sync_axes<S: GamepadSink>(&mut self, app: &mut S, gilrs: &Gilrs) {
            self.sync_ids.clear();
            self.sync_ids.extend(self.id_to_uuid.keys().copied());
            for id in self.sync_ids.iter().copied() {
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
                self.log_raw_like_state(gilrs, id, "sync_axes");
            }
        }

        #[cfg(target_os = "windows")]
        fn poll_xinput<S: GamepadSink>(&mut self, app: &mut S) {
            if self.xinput.is_none() {
                self.xinput = XInputHandle::load_default().ok();
            }
            let Some(handle) = self.xinput.as_ref() else {
                return;
            };
            let states: Vec<_> = (0..4_u32)
                .map(|slot| (slot as usize, handle.get_state(slot).ok()))
                .collect();

            for (slot, state) in states {
                let index = self.xinput_app_index(slot);
                let Some(state) = state else {
                    if self.xinput_connected[slot] {
                        clear_gamepad(app, index);
                        app.set_gamepad_connected(index, false);
                        eprintln!("[gamepad][xinput] disconnected slot={slot} index={index}");
                    }
                    self.xinput_connected[slot] = false;
                    continue;
                };

                if !self.xinput_connected[slot] {
                    app.set_gamepad_connected(index, true);
                    eprintln!("[gamepad][xinput] connected slot={slot} index={index}");
                }
                self.xinput_connected[slot] = true;
                let pad = state.raw.Gamepad;
                let buttons = pad.wButtons;
                for (button, mask) in [
                    (GamepadButton::DpadUp, 0x0001),
                    (GamepadButton::DpadDown, 0x0002),
                    (GamepadButton::DpadLeft, 0x0004),
                    (GamepadButton::DpadRight, 0x0008),
                    (GamepadButton::Start, 0x0010),
                    (GamepadButton::Select, 0x0020),
                    (GamepadButton::L3, 0x0040),
                    (GamepadButton::R3, 0x0080),
                    (GamepadButton::L1, 0x0100),
                    (GamepadButton::R1, 0x0200),
                    (GamepadButton::Bottom, 0x1000),
                    (GamepadButton::Right, 0x2000),
                    (GamepadButton::Left, 0x4000),
                    (GamepadButton::Top, 0x8000),
                ] {
                    app.set_gamepad_button_state(index, button, buttons & mask != 0);
                }
                let lt = pad.bLeftTrigger as f32 / u8::MAX as f32;
                let rt = pad.bRightTrigger as f32 / u8::MAX as f32;
                app.set_gamepad_button_state(index, GamepadButton::L2, lt > 0.5);
                app.set_gamepad_button_state(index, GamepadButton::R2, rt > 0.5);
                app.set_gamepad_axis(index, GamepadAxis::LeftTrigger, lt);
                app.set_gamepad_axis(index, GamepadAxis::RightTrigger, rt);
                app.set_gamepad_axis(index, GamepadAxis::LeftStickX, normalize_i16(pad.sThumbLX));
                app.set_gamepad_axis(index, GamepadAxis::LeftStickY, normalize_i16(pad.sThumbLY));
                app.set_gamepad_axis(index, GamepadAxis::RightStickX, normalize_i16(pad.sThumbRX));
                app.set_gamepad_axis(index, GamepadAxis::RightStickY, normalize_i16(pad.sThumbRY));
            }
        }

        #[cfg(target_os = "windows")]
        fn xinput_app_index(&self, slot: usize) -> usize {
            self.id_to_uuid
                .iter()
                .find(|(id, _)| usize::from(**id) == slot)
                .and_then(|(_, uuid)| self.uuid_to_index.get(uuid).copied())
                .unwrap_or(slot)
        }

        fn log_raw_like_state(&self, gilrs: &Gilrs, id: GamepadId, reason: &str) {
            if !raw_dump_enabled() {
                return;
            }
            let gp = gilrs.gamepad(id);
            if !gp.is_connected() || is_joycon(&gp) {
                return;
            }
            let Some(uuid) = self.id_to_uuid.get(&id) else {
                return;
            };
            let Some(index) = self.uuid_to_index.get(uuid).copied() else {
                return;
            };
            let down_mask = self.down_masks.get(&id).copied().unwrap_or(0);
            let lx = gp.value(Axis::LeftStickX);
            let ly = gp.value(Axis::LeftStickY);
            let rx = gp.value(Axis::RightStickX);
            let ry = gp.value(Axis::RightStickY);
            let lt = gp.value(Axis::LeftZ);
            let rt = gp.value(Axis::RightZ);
            eprintln!(
                "[gamepad][raw] reason={} index={} id={:?} name=\"{}\" down_mask=0x{:08X} axes={{lx:{:.3},ly:{:.3},rx:{:.3},ry:{:.3},lt:{:.3},rt:{:.3}}} gyro=(0.0,0.0,0.0) accel=(0.0,0.0,0.0) raw_bytes=<unavailable:gilrs>",
                reason,
                index,
                id,
                gp.name(),
                down_mask,
                lx,
                ly,
                rx,
                ry,
                lt,
                rt
            );
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

    fn clear_gamepad<S: GamepadSink>(app: &mut S, index: usize) {
        for button in ALL_BUTTONS {
            app.set_gamepad_button_state(index, button, false);
        }
        for axis in ALL_AXES {
            app.set_gamepad_axis(index, axis, 0.0);
        }
        app.set_gamepad_gyro(index, 0.0, 0.0, 0.0);
        app.set_gamepad_accel(index, 0.0, 0.0, 0.0);
    }

    #[cfg(target_os = "windows")]
    fn normalize_i16(value: i16) -> f32 {
        if value >= 0 {
            value as f32 / i16::MAX as f32
        } else {
            value as f32 / 32768.0
        }
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

    fn raw_dump_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            std::env::var("PERRO_INPUT_RAW_DUMP")
                .map(|v| {
                    let t = v.trim();
                    !(t.is_empty() || t == "0" || t.eq_ignore_ascii_case("false"))
                })
                .unwrap_or(false)
        })
    }

    #[inline]
    fn magnitude_from_unit(v: f32) -> u16 {
        (v.clamp(0.0, 1.0) * u16::MAX as f32).round() as u16
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod backend {
    #[derive(Default)]
    pub struct GamepadBackend;

    pub(super) fn preinit() {}

    impl GamepadBackend {
        pub fn begin_frame<S>(&mut self, _app: &mut S) {}

        #[cfg(feature = "steamworks")]
        pub fn collect_connected_indices(&self, out: &mut Vec<usize>) {
            out.clear();
        }
    }
}

pub(crate) fn preinit() {
    backend::preinit();
}

#[cfg(feature = "steamworks")]
#[derive(Default)]
struct SteamFallbackBackend {
    handle_to_index: std::collections::HashMap<u64, usize>,
    warned: bool,
}

#[cfg(feature = "steamworks")]
impl SteamFallbackBackend {
    fn begin_frame<B: GraphicsBackend>(&mut self, app: &mut App<B>, native_indices: &[usize]) {
        let gamepads = match perro_steamworks::input::fallback_gamepads(native_indices.len()) {
            Ok(gamepads) => gamepads,
            Err(perro_steamworks::SteamError::Disabled) => {
                self.clear_all(app, native_indices);
                return;
            }
            Err(err) => {
                if !self.warned {
                    eprintln!("[gamepad][warn] Steam Input fallback unavailable: {err}");
                    self.warned = true;
                }
                self.clear_all(app, native_indices);
                return;
            }
        };
        self.warned = false;
        let mut seen = std::collections::HashSet::with_capacity(gamepads.len());
        for gamepad in gamepads {
            let raw = gamepad.handle.raw();
            seen.insert(raw);
            let index = if let Some(index) = self.handle_to_index.get(&raw).copied()
                && !native_indices.contains(&index)
            {
                index
            } else {
                let index = self.allocate_index(native_indices, raw);
                self.handle_to_index.insert(raw, index);
                clear_steam_gamepad(app, index);
                index
            };
            write_steam_gamepad(app, index, &gamepad);
        }

        let removed: Vec<_> = self
            .handle_to_index
            .iter()
            .filter_map(|(handle, index)| (!seen.contains(handle)).then_some((*handle, *index)))
            .collect();
        for (handle, index) in removed {
            self.handle_to_index.remove(&handle);
            if !native_indices.contains(&index) {
                clear_steam_gamepad(app, index);
            }
        }
    }

    fn allocate_index(&self, native_indices: &[usize], current_handle: u64) -> usize {
        let used: std::collections::HashSet<_> = self
            .handle_to_index
            .iter()
            .filter_map(|(handle, index)| (*handle != current_handle).then_some(*index))
            .collect();
        (0..)
            .find(|index| !native_indices.contains(index) && !used.contains(index))
            .expect("gamepad index space exhausted")
    }

    fn clear_all<B: GraphicsBackend>(&mut self, app: &mut App<B>, native_indices: &[usize]) {
        if self.handle_to_index.is_empty() {
            return;
        }
        let indices: Vec<_> = self
            .handle_to_index
            .drain()
            .map(|(_, index)| index)
            .collect();
        for index in indices {
            if !native_indices.contains(&index) {
                clear_steam_gamepad(app, index);
            }
        }
    }
}

#[cfg(feature = "steamworks")]
const STEAM_BUTTONS: [GamepadButton; 18] = [
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

#[cfg(feature = "steamworks")]
const STEAM_AXES: [GamepadAxis; 6] = [
    GamepadAxis::LeftStickX,
    GamepadAxis::LeftStickY,
    GamepadAxis::RightStickX,
    GamepadAxis::RightStickY,
    GamepadAxis::LeftTrigger,
    GamepadAxis::RightTrigger,
];

#[cfg(feature = "steamworks")]
fn write_steam_gamepad<B: GraphicsBackend>(
    app: &mut App<B>,
    index: usize,
    gamepad: &perro_steamworks::input::FallbackGamepad,
) {
    app.set_gamepad_connected(index, true);
    for (button, down) in STEAM_BUTTONS.into_iter().zip(gamepad.buttons) {
        app.set_gamepad_button_state(index, button, down);
    }
    for (axis, value) in STEAM_AXES.into_iter().zip(gamepad.axes) {
        app.set_gamepad_axis(index, axis, value);
    }
    app.set_gamepad_gyro(
        index,
        gamepad.motion.rot_vel[0],
        gamepad.motion.rot_vel[1],
        gamepad.motion.rot_vel[2],
    );
    app.set_gamepad_accel(
        index,
        gamepad.motion.pos_accel[0],
        gamepad.motion.pos_accel[1],
        gamepad.motion.pos_accel[2],
    );
}

#[cfg(feature = "steamworks")]
fn clear_steam_gamepad<B: GraphicsBackend>(app: &mut App<B>, index: usize) {
    app.set_gamepad_connected(index, false);
    for button in STEAM_BUTTONS {
        app.set_gamepad_button_state(index, button, false);
    }
    for axis in STEAM_AXES {
        app.set_gamepad_axis(index, axis, 0.0);
    }
    app.set_gamepad_gyro(index, 0.0, 0.0, 0.0);
    app.set_gamepad_accel(index, 0.0, 0.0, 0.0);
}

#[derive(Default)]
pub struct GamepadInput {
    backend: backend::GamepadBackend,
    #[cfg(feature = "steamworks")]
    steam_fallback: SteamFallbackBackend,
    #[cfg(feature = "steamworks")]
    native_indices: Vec<usize>,
}

impl GamepadInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_frame<B: GraphicsBackend>(&mut self, app: &mut App<B>) {
        self.backend.begin_frame(app);
        #[cfg(feature = "steamworks")]
        {
            self.backend
                .collect_connected_indices(&mut self.native_indices);
            self.steam_fallback.begin_frame(app, &self.native_indices);
        }
    }
}

#[cfg(all(test, feature = "steamworks"))]
mod steam_fallback_tests {
    use super::SteamFallbackBackend;

    #[test]
    fn fallback_slot_uses_first_free_index() {
        let mut backend = SteamFallbackBackend::default();
        assert_eq!(backend.allocate_index(&[], 1), 0);
        backend.handle_to_index.insert(1, 0);
        assert_eq!(backend.allocate_index(&[], 2), 1);
    }

    #[test]
    fn fallback_slot_skips_native_indices() {
        let mut backend = SteamFallbackBackend::default();
        backend.handle_to_index.insert(1, 1);
        assert_eq!(backend.allocate_index(&[0, 2], 2), 3);
        assert_eq!(backend.allocate_index(&[0, 1], 1), 2);
    }
}

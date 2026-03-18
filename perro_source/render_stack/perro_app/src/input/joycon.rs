use crate::App;
use perro_graphics::GraphicsBackend;

mod backend {
    use super::*;
    use hidapi::HidApi;
    use perro_input::{JoyConButton, JoyConIndex, JoyConSide};
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    const JOYCON_VENDOR_ID: u16 = 0x057E;
    const JOYCON_1_LEFT_PID: u16 = 0x2006;
    const JOYCON_1_RIGHT_PID: u16 = 0x2007;
    const REPORT_LEN: usize = 64;
    const SCAN_INTERVAL: Duration = Duration::from_secs(2);
    const READ_TIMEOUT: Duration = Duration::from_millis(8);

    #[derive(Debug)]
    struct DeviceHandle {
        stop: Arc<AtomicBool>,
    }

    type ButtonBits = [bool; JoyConButton::COUNT];
    type JoyConEvent = (usize, JoyConSide, ButtonBits, f32, f32, f32, f32, f32, f32, f32, f32);

    #[derive(Default)]
    pub struct JoyConBackend {
        devices: HashMap<String, DeviceHandle>,
        assigned: HashMap<String, usize>,
        free_indices: Vec<usize>,
        next_index: usize,
        rx: Option<Receiver<JoyConEvent>>,
        tx: Option<Sender<JoyConEvent>>,
        last_scan: Option<Instant>,
        last_buttons: HashMap<(usize, JoyConSide), ButtonBits>,
    }

    impl JoyConBackend {

        pub fn begin_frame<B: GraphicsBackend>(&mut self, app: &mut App<B>) {
            self.ensure_channel();
            self.scan_if_needed();
            self.drain_events(app);
        }

        fn ensure_channel(&mut self) {
            if self.rx.is_some() {
                return;
            }
            let (tx, rx) = mpsc::channel();
            self.tx = Some(tx);
            self.rx = Some(rx);
        }

        fn scan_if_needed(&mut self) {
            let now = Instant::now();
            let scan_due = self
                .last_scan
                .map(|t| now.duration_since(t) >= SCAN_INTERVAL)
                .unwrap_or(true);
            if !scan_due {
                return;
            }
            self.last_scan = Some(now);

            let Ok(api) = HidApi::new() else {
                return;
            };

            let mut seen_serials = HashSet::new();
            for dev in api.device_list() {
                if dev.vendor_id() != JOYCON_VENDOR_ID {
                    continue;
                }
                let pid = dev.product_id();
                let side = match pid {
                    JOYCON_1_LEFT_PID => JoyConSide::LJoyCon,
                    JOYCON_1_RIGHT_PID => JoyConSide::RJoyCon,
                    _ => continue,
                };
                let Some(serial) = dev.serial_number().map(|s| s.to_string()) else {
                    continue;
                };
                seen_serials.insert(serial.clone());
                if self.devices.contains_key(&serial) {
                    continue;
                }
                let index = self.assign_index(&serial, side);
                log_joycon_connected(index, side, &serial);
                self.spawn_device_thread(serial, pid, side, index);
            }

            // Drop device handles not seen this scan.
            let stale: Vec<String> = self
                .devices
                .keys()
                .filter(|s| !seen_serials.contains(*s))
                .cloned()
                .collect();
            for serial in stale {
                if let Some(handle) = self.devices.remove(&serial) {
                    handle.stop.store(true, Ordering::Relaxed);
                }
                if let Some(index) = self.assigned.get(&serial).copied() {
                    if !self.free_indices.contains(&index) {
                        self.free_indices.push(index);
                    }
                    self.last_buttons.retain(|(idx, _), _| *idx != index);
                }
            }
        }

        fn assign_index(&mut self, serial: &str, _side: JoyConSide) -> usize {
            if let Some(idx) = self.assigned.get(serial) {
                self.free_indices.retain(|free| *free != *idx);
                return *idx;
            }
            const MAX_PERSISTENT_JOYCON_SLOTS: usize = 12;
            let index = if self.next_index < MAX_PERSISTENT_JOYCON_SLOTS {
                let idx = self.next_index;
                self.next_index = self.next_index.saturating_add(1);
                idx
            } else if !self.free_indices.is_empty() {
                self.free_indices.sort_unstable();
                self.free_indices.remove(0)
            } else {
                let idx = self.next_index;
                self.next_index = self.next_index.saturating_add(1);
                idx
            };
            self.assigned.insert(serial.to_string(), index);
            index
        }

        fn spawn_device_thread(&mut self, serial: String, pid: u16, side: JoyConSide, index: usize) {
            let Some(tx) = self.tx.clone() else {
                return;
            };
            let stop = Arc::new(AtomicBool::new(false));
            let stop_thread = Arc::clone(&stop);
            let serial_thread = serial.clone();
            thread::spawn(move || {
                let Ok(api) = HidApi::new() else {
                    return;
                };
                let Ok(device) = api.open_serial(JOYCON_VENDOR_ID, pid, &serial_thread) else {
                    return;
                };

                let _ = enable_sensors(&device);
                let mut buffer = [0u8; REPORT_LEN];
                while !stop_thread.load(Ordering::Relaxed) {
                    match device.read_timeout(&mut buffer, READ_TIMEOUT.as_millis() as i32) {
                        Ok(size) if size > 0 => {
                            let data = &buffer[..size];
                            if let Some(payload) = decode_report(data, side) {
                                let _ = tx.send((
                                    index,
                                    side,
                                    payload.0,
                                    payload.1,
                                    payload.2,
                                    payload.3,
                                    payload.4,
                                    payload.5,
                                    payload.6,
                                    payload.7,
                                    payload.8,
                                ));
                            }
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
            });
            self.devices.insert(serial, DeviceHandle { stop });
        }

        fn drain_events<B: GraphicsBackend>(&mut self, app: &mut App<B>) {
            let Some(rx) = self.rx.as_ref() else {
                return;
            };
            while let Ok(event) = rx.try_recv() {
                apply_report(app, event, &mut self.last_buttons);
            }
        }
    }

    fn apply_report<B: GraphicsBackend>(
        app: &mut App<B>,
        event: JoyConEvent,
        last_buttons: &mut HashMap<(usize, JoyConSide), ButtonBits>,
    ) {
        let (
            index,
            side,
            buttons,
            stick_x,
            stick_y,
            gyro_x,
            gyro_y,
            gyro_z,
            accel_x,
            accel_y,
            accel_z,
        ) = event;

        let key = (index, side);
        let prev = last_buttons.get(&key).copied();
        apply_buttons(app, index, side, &buttons, prev.as_ref());
        last_buttons.insert(key, buttons);
        app.set_joycon_stick(index, stick_x, stick_y);
        app.set_joycon_gyro(index, gyro_x, gyro_y, gyro_z);
        app.set_joycon_accel(index, accel_x, accel_y, accel_z);
    }

    fn apply_buttons<B: GraphicsBackend>(
        app: &mut App<B>,
        index: usize,
        side: JoyConSide,
        buttons: &ButtonBits,
        prev: Option<&ButtonBits>,
    ) {
        let map = |b: JoyConButton| buttons[b.as_index()];
        let changed = |b: JoyConButton| match prev {
            Some(prev_bits) => prev_bits[b.as_index()] != buttons[b.as_index()],
            None => true,
        };
        match side {
            JoyConSide::LJoyCon => {
                if changed(JoyConButton::Top) {
                    app.set_joycon_button_state(index, JoyConButton::Top, map(JoyConButton::Top));
                }
                if changed(JoyConButton::Bottom) {
                    app.set_joycon_button_state(index, JoyConButton::Bottom, map(JoyConButton::Bottom));
                }
                if changed(JoyConButton::Left) {
                    app.set_joycon_button_state(index, JoyConButton::Left, map(JoyConButton::Left));
                }
                if changed(JoyConButton::Right) {
                    app.set_joycon_button_state(index, JoyConButton::Right, map(JoyConButton::Right));
                }
                if changed(JoyConButton::Bumper) {
                    app.set_joycon_button_state(index, JoyConButton::Bumper, map(JoyConButton::Bumper));
                }
                if changed(JoyConButton::Trigger) {
                    app.set_joycon_button_state(index, JoyConButton::Trigger, map(JoyConButton::Trigger));
                }
                if changed(JoyConButton::Stick) {
                    app.set_joycon_button_state(index, JoyConButton::Stick, map(JoyConButton::Stick));
                }
                if changed(JoyConButton::SL) {
                    app.set_joycon_button_state(index, JoyConButton::SL, map(JoyConButton::SL));
                }
                if changed(JoyConButton::SR) {
                    app.set_joycon_button_state(index, JoyConButton::SR, map(JoyConButton::SR));
                }
                if changed(JoyConButton::Start) {
                    app.set_joycon_button_state(index, JoyConButton::Start, map(JoyConButton::Start));
                }
                if changed(JoyConButton::Meta) {
                    app.set_joycon_button_state(index, JoyConButton::Meta, map(JoyConButton::Meta));
                }
            }
            JoyConSide::RJoyCon => {
                if changed(JoyConButton::Top) {
                    app.set_joycon_button_state(index, JoyConButton::Top, map(JoyConButton::Top));
                }
                if changed(JoyConButton::Bottom) {
                    app.set_joycon_button_state(index, JoyConButton::Bottom, map(JoyConButton::Bottom));
                }
                if changed(JoyConButton::Left) {
                    app.set_joycon_button_state(index, JoyConButton::Left, map(JoyConButton::Left));
                }
                if changed(JoyConButton::Right) {
                    app.set_joycon_button_state(index, JoyConButton::Right, map(JoyConButton::Right));
                }
                if changed(JoyConButton::Bumper) {
                    app.set_joycon_button_state(index, JoyConButton::Bumper, map(JoyConButton::Bumper));
                }
                if changed(JoyConButton::Trigger) {
                    app.set_joycon_button_state(index, JoyConButton::Trigger, map(JoyConButton::Trigger));
                }
                if changed(JoyConButton::Stick) {
                    app.set_joycon_button_state(index, JoyConButton::Stick, map(JoyConButton::Stick));
                }
                if changed(JoyConButton::SL) {
                    app.set_joycon_button_state(index, JoyConButton::SL, map(JoyConButton::SL));
                }
                if changed(JoyConButton::SR) {
                    app.set_joycon_button_state(index, JoyConButton::SR, map(JoyConButton::SR));
                }
                if changed(JoyConButton::Start) {
                    app.set_joycon_button_state(index, JoyConButton::Start, map(JoyConButton::Start));
                }
                if changed(JoyConButton::Meta) {
                    app.set_joycon_button_state(index, JoyConButton::Meta, map(JoyConButton::Meta));
                }
            }
        }
    }

    fn decode_report(
        data: &[u8],
        side: JoyConSide,
    ) -> Option<(ButtonBits, f32, f32, f32, f32, f32, f32, f32, f32)> {
        if data.len() < 49 {
            return None;
        }

        let has_report_id = data[0] == 0x30;
        let offset = if has_report_id { 0 } else { 1 };

        let button_idx_right = 3_usize.checked_sub(offset)?;
        let button_idx_shared = 4_usize.checked_sub(offset)?;
        let button_idx_left = 5_usize.checked_sub(offset)?;
        let button_byte_right = *data.get(button_idx_right)?;
        let button_byte_shared = *data.get(button_idx_shared)?;
        let button_byte_left = *data.get(button_idx_left)?;

        let mut buttons = [false; JoyConButton::COUNT];

        match side {
            JoyConSide::LJoyCon => {
                set_button(&mut buttons, JoyConButton::Top, (button_byte_left & 0x02) != 0);
                set_button(&mut buttons, JoyConButton::Bottom, (button_byte_left & 0x01) != 0);
                set_button(&mut buttons, JoyConButton::Left, (button_byte_left & 0x08) != 0);
                set_button(&mut buttons, JoyConButton::Right, (button_byte_left & 0x04) != 0);
                set_button(&mut buttons, JoyConButton::Bumper, (button_byte_left & 0x40) != 0);
                set_button(&mut buttons, JoyConButton::Trigger, (button_byte_left & 0x80) != 0);
                set_button(&mut buttons, JoyConButton::SL, (button_byte_left & 0x20) != 0);
                set_button(&mut buttons, JoyConButton::SR, (button_byte_left & 0x10) != 0);
                set_button(&mut buttons, JoyConButton::Start, (button_byte_shared & 0x01) != 0);
                set_button(&mut buttons, JoyConButton::Meta, (button_byte_shared & 0x20) != 0);
                set_button(&mut buttons, JoyConButton::Stick, (button_byte_shared & 0x08) != 0);
            }
            JoyConSide::RJoyCon => {
                set_button(&mut buttons, JoyConButton::Top, (button_byte_right & 0x02) != 0);
                set_button(&mut buttons, JoyConButton::Bottom, (button_byte_right & 0x04) != 0);
                set_button(&mut buttons, JoyConButton::Left, (button_byte_right & 0x01) != 0);
                set_button(&mut buttons, JoyConButton::Right, (button_byte_right & 0x08) != 0);
                set_button(&mut buttons, JoyConButton::Bumper, (button_byte_right & 0x40) != 0);
                set_button(&mut buttons, JoyConButton::Trigger, (button_byte_right & 0x80) != 0);
                set_button(&mut buttons, JoyConButton::SL, (button_byte_right & 0x20) != 0);
                set_button(&mut buttons, JoyConButton::SR, (button_byte_right & 0x10) != 0);
                set_button(&mut buttons, JoyConButton::Start, (button_byte_shared & 0x02) != 0);
                set_button(&mut buttons, JoyConButton::Meta, (button_byte_shared & 0x10) != 0);
                set_button(&mut buttons, JoyConButton::Stick, (button_byte_shared & 0x04) != 0);
            }
        }

        let (stick_x, stick_y) = decode_stick(data, side, offset)?;

        let (accel_x, accel_y, accel_z) = decode_accel(data, offset);
        let (gyro_x, gyro_y, gyro_z) = decode_gyro(data, offset);

        Some((buttons, stick_x, stick_y, gyro_x, gyro_y, gyro_z, accel_x, accel_y, accel_z))
    }

    #[inline]
    fn set_button(bits: &mut ButtonBits, button: JoyConButton, value: bool) {
        bits[button.as_index()] = value;
    }

    fn decode_stick(data: &[u8], side: JoyConSide, offset: usize) -> Option<(f32, f32)> {
        let (stick_start, stick_end) = match side {
            JoyConSide::LJoyCon => (6_usize.checked_sub(offset)?, 9_usize.checked_sub(offset)?),
            JoyConSide::RJoyCon => (9_usize.checked_sub(offset)?, 12_usize.checked_sub(offset)?),
        };
        let stick_bytes = data.get(stick_start..stick_end)?;
        if stick_bytes.len() != 3 {
            return None;
        }
        let raw_x = (stick_bytes[0] as u16) | (((stick_bytes[1] & 0x0F) as u16) << 8);
        let raw_y = (((stick_bytes[1] & 0xF0) >> 4) as u16) | ((stick_bytes[2] as u16) << 4);
        let x_norm = (raw_x as f32 / 4095.0).clamp(0.0, 1.0);
        let y_norm = (raw_y as f32 / 4095.0).clamp(0.0, 1.0);
        let x = x_norm * 2.0 - 1.0;
        let y = y_norm * 2.0 - 1.0;
        Some((x, y))
    }

    fn decode_accel(data: &[u8], offset: usize) -> (f32, f32, f32) {
        let accel_start = match 13_usize.checked_sub(offset) {
            Some(v) => v,
            None => return (0.0, 0.0, 0.0),
        };
        let accel_end = match 19_usize.checked_sub(offset) {
            Some(v) => v,
            None => return (0.0, 0.0, 0.0),
        };
        if accel_end <= data.len() {
            let ax = i16::from_le_bytes([data[accel_start], data[accel_start + 1]]) as f32;
            let ay = i16::from_le_bytes([data[accel_start + 2], data[accel_start + 3]]) as f32;
            let az = i16::from_le_bytes([data[accel_start + 4], data[accel_start + 5]]) as f32;
            (ax, ay, az)
        } else {
            (0.0, 0.0, 0.0)
        }
    }

    fn decode_gyro(data: &[u8], offset: usize) -> (f32, f32, f32) {
        let gyro_start = match 19_usize.checked_sub(offset) {
            Some(v) => v,
            None => return (0.0, 0.0, 0.0),
        };
        let gyro_end = match 25_usize.checked_sub(offset) {
            Some(v) => v,
            None => return (0.0, 0.0, 0.0),
        };
        if gyro_end <= data.len() {
            let gx = i16::from_le_bytes([data[gyro_start], data[gyro_start + 1]]) as f32;
            let gy = i16::from_le_bytes([data[gyro_start + 2], data[gyro_start + 3]]) as f32;
            let gz = i16::from_le_bytes([data[gyro_start + 4], data[gyro_start + 5]]) as f32;
            (gx, gy, gz)
        } else {
            (0.0, 0.0, 0.0)
        }
    }

    fn enable_sensors(device: &hidapi::HidDevice) -> Result<(), hidapi::HidError> {
        let mut cmd = vec![0x01, 0x00];
        cmd.extend_from_slice(&[0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40]);
        cmd.push(0x40);
        cmd.push(0x01);
        device.write(&cmd)?;

        let mut cmd2 = vec![0x01, 0x01];
        cmd2.extend_from_slice(&[0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40]);
        cmd2.push(0x03);
        cmd2.push(0x30);
        device.write(&cmd2)?;

        Ok(())
    }

    fn log_joycon_connected(index: usize, side: JoyConSide, serial: &str) {
        let idx = JoyConIndex(index);
        eprintln!(
            "[joycon] connected index={:?} side={:?} serial={}",
            idx, side, serial
        );
    }
}

#[derive(Default)]
pub struct JoyConInput {
    backend: backend::JoyConBackend,
}

impl JoyConInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_frame<B: GraphicsBackend>(&mut self, app: &mut App<B>) {
        self.backend.begin_frame(app);
    }
}

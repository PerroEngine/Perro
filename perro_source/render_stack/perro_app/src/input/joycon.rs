use crate::App;
use perro_graphics::GraphicsBackend;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

mod backend {
    use super::*;
    use hidapi::HidApi;
    use perro_input::{JoyConButton, JoyConSide};

    const JOYCON_VENDOR_ID: u16 = 0x057E;
    const JOYCON_L_PID: u16 = 0x2006;
    const JOYCON_R_PID: u16 = 0x2007;
    const REPORT_LEN: usize = 64;
    const SCAN_INTERVAL: Duration = Duration::from_secs(2);
    const READ_TIMEOUT: Duration = Duration::from_millis(8);
    const MAX_PERSISTENT_JOYCON_SLOTS: usize = 12;

    type ButtonBits = [bool; JoyConButton::COUNT];
    type JoyConEvent = (usize, JoyConSide, JoyConInputData);

    #[derive(Debug)]
    struct DeviceHandle {
        stop: Arc<AtomicBool>,
    }

    #[derive(Debug, Clone)]
    struct JoyConInputData {
        buttons: ButtonBits,
        stick: (f32, f32),
        gyro: (f32, f32, f32),
        accel: (f32, f32, f32),
    }

    #[derive(Default)]
    pub struct JoyConBackend {
        devices: HashMap<String, DeviceHandle>,
        assigned: HashMap<String, usize>,
        free_indices: Vec<usize>,
        next_index: usize,
        rx: Option<Receiver<JoyConEvent>>,
        tx: Option<Sender<JoyConEvent>>,
        last_buttons: HashMap<(usize, JoyConSide), ButtonBits>,
        last_scan: Option<Instant>,
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

            let mut connected_serials: HashSet<String> = HashSet::new();

            for dev in api.device_list() {
                if dev.vendor_id() != JOYCON_VENDOR_ID {
                    continue;
                }

                let pid = dev.product_id();
                let side = match pid {
                    JOYCON_L_PID => JoyConSide::LJoyCon,
                    JOYCON_R_PID => JoyConSide::RJoyCon,
                    _ => continue,
                };

                let Some(serial) = dev.serial_number() else {
                    continue;
                };
                let serial = serial.to_string();
                connected_serials.insert(serial.clone());

                if self.devices.contains_key(&serial) {
                    continue;
                }

                let index = self.assign_index(&serial);
                log_joycon_connected(index, side, &serial);
                self.spawn_device_thread(serial.clone(), pid, side, index);
            }

            // Remove disconnected devices
            self.devices.retain(|serial, handle| {
                let connected = connected_serials.contains(serial);
                if !connected {
                    handle.stop.store(true, Ordering::Relaxed);
                    if let Some(index) = self.assigned.remove(serial) {
                        self.free_indices.push(index);
                        self.last_buttons.retain(|(idx, _), _| *idx != index);
                    }
                }
                connected
            });
        }

        fn assign_index(&mut self, serial: &str) -> usize {
            if let Some(idx) = self.assigned.get(serial) {
                self.free_indices.retain(|&free| free != *idx);
                return *idx;
            }

            let index = if self.next_index < MAX_PERSISTENT_JOYCON_SLOTS {
                let idx = self.next_index;
                self.next_index += 1;
                idx
            } else if let Some(free_idx) = self.free_indices.pop() {
                free_idx
            } else {
                let idx = self.next_index;
                self.next_index += 1;
                idx
            };

            self.assigned.insert(serial.to_string(), index);
            index
        }

        fn spawn_device_thread(
            &mut self,
            serial: String,
            pid: u16,
            side: JoyConSide,
            index: usize,
        ) {
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
                                let _ = tx.send((index, side, payload));
                            }
                        }
                        _ => {}
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
        let (index, side, data) = event;
        let key = (index, side);
        let prev = last_buttons.get(&key).copied();

        apply_buttons(app, index, &data.buttons, prev.as_ref());
        last_buttons.insert(key, data.buttons);

        app.set_joycon_stick(index, data.stick.0, data.stick.1);
        app.set_joycon_gyro(index, data.gyro.0, data.gyro.1, data.gyro.2);
        app.set_joycon_accel(index, data.accel.0, data.accel.1, data.accel.2);
    }

    fn apply_buttons<B: GraphicsBackend>(
        app: &mut App<B>,
        index: usize,
        buttons: &ButtonBits,
        prev: Option<&ButtonBits>,
    ) {
        for button in [
            JoyConButton::Top,
            JoyConButton::Bottom,
            JoyConButton::Left,
            JoyConButton::Right,
            JoyConButton::Bumper,
            JoyConButton::Trigger,
            JoyConButton::Stick,
            JoyConButton::SL,
            JoyConButton::SR,
            JoyConButton::Start,
            JoyConButton::Meta,
        ] {
            let is_pressed = buttons[button.as_index()];
            let was_pressed = prev.is_some_and(|prev_bits| prev_bits[button.as_index()]);
            if is_pressed != was_pressed {
                app.set_joycon_button_state(index, button, is_pressed);
            }
        }
    }

    fn decode_report(data: &[u8], side: JoyConSide) -> Option<JoyConInputData> {
        if data.len() < 49 {
            return None;
        }

        let offset = if data[0] == 0x30 { 1 } else { 0 };

        let (left_idx, shared_idx, right_idx) = if offset == 1 { (2, 3, 4) } else { (3, 4, 5) };

        let (btn_left, btn_shared, btn_right) = (
            *data.get(left_idx)?,
            *data.get(shared_idx)?,
            *data.get(right_idx)?,
        );

        let mut buttons = [false; JoyConButton::COUNT];
        match side {
            JoyConSide::LJoyCon => {
                buttons[JoyConButton::Top.as_index()] = (btn_left & 0x02) != 0;
                buttons[JoyConButton::Bottom.as_index()] = (btn_left & 0x01) != 0;
                buttons[JoyConButton::Left.as_index()] = (btn_left & 0x08) != 0;
                buttons[JoyConButton::Right.as_index()] = (btn_left & 0x04) != 0;
                buttons[JoyConButton::Bumper.as_index()] = (btn_left & 0x40) != 0;
                buttons[JoyConButton::Trigger.as_index()] = (btn_left & 0x80) != 0;
                buttons[JoyConButton::SL.as_index()] = (btn_left & 0x20) != 0;
                buttons[JoyConButton::SR.as_index()] = (btn_left & 0x10) != 0;
                buttons[JoyConButton::Start.as_index()] = (btn_shared & 0x01) != 0;
                buttons[JoyConButton::Meta.as_index()] = (btn_shared & 0x20) != 0;
                buttons[JoyConButton::Stick.as_index()] = (btn_shared & 0x08) != 0;
            }
            JoyConSide::RJoyCon => {
                buttons[JoyConButton::Top.as_index()] = (btn_right & 0x02) != 0;
                buttons[JoyConButton::Bottom.as_index()] = (btn_right & 0x04) != 0;
                buttons[JoyConButton::Left.as_index()] = (btn_right & 0x01) != 0;
                buttons[JoyConButton::Right.as_index()] = (btn_right & 0x08) != 0;
                buttons[JoyConButton::Bumper.as_index()] = (btn_right & 0x40) != 0;
                buttons[JoyConButton::Trigger.as_index()] = (btn_right & 0x80) != 0;
                buttons[JoyConButton::SL.as_index()] = (btn_right & 0x20) != 0;
                buttons[JoyConButton::SR.as_index()] = (btn_right & 0x10) != 0;
                buttons[JoyConButton::Start.as_index()] = (btn_shared & 0x02) != 0;
                buttons[JoyConButton::Meta.as_index()] = (btn_shared & 0x10) != 0;
                buttons[JoyConButton::Stick.as_index()] = (btn_shared & 0x04) != 0;
            }
        }

        let stick = decode_stick(data, side, offset)?;
        let accel = decode_accel(data, offset);
        let gyro = decode_gyro(data, offset);

        Some(JoyConInputData {
            buttons,
            stick,
            gyro,
            accel,
        })
    }

    fn decode_stick(data: &[u8], side: JoyConSide, offset: usize) -> Option<(f32, f32)> {
        let (start, end) = match side {
            JoyConSide::LJoyCon => (6_usize.checked_sub(offset)?, 9_usize.checked_sub(offset)?),
            JoyConSide::RJoyCon => (9_usize.checked_sub(offset)?, 12_usize.checked_sub(offset)?),
        };

        let stick_bytes = data.get(start..end)?;
        if stick_bytes.len() != 3 {
            return None;
        }

        let raw_x = (stick_bytes[0] as u16) | (((stick_bytes[1] & 0x0F) as u16) << 8);
        let raw_y = (((stick_bytes[1] & 0xF0) >> 4) as u16) | ((stick_bytes[2] as u16) << 4);

        let x_norm = (raw_x as f32 / 4095.0).clamp(0.0, 1.0);
        let y_norm = (raw_y as f32 / 4095.0).clamp(0.0, 1.0);

        Some((
            x_norm * 2.0 - 1.0, // Normalize to [-1, 1]
            y_norm * 2.0 - 1.0,
        ))
    }

    fn decode_accel(data: &[u8], offset: usize) -> (f32, f32, f32) {
        let start = match 13_usize.checked_sub(offset) {
            Some(v) => v,
            None => return (0.0, 0.0, 0.0),
        };

        if start + 5 < data.len() {
            let ax = i16::from_le_bytes([data[start], data[start + 1]]) as f32;
            let ay = i16::from_le_bytes([data[start + 2], data[start + 3]]) as f32;
            let az = i16::from_le_bytes([data[start + 4], data[start + 5]]) as f32;
            (ax, ay, az)
        } else {
            (0.0, 0.0, 0.0)
        }
    }

    fn decode_gyro(data: &[u8], offset: usize) -> (f32, f32, f32) {
        let start = match 19_usize.checked_sub(offset) {
            Some(v) => v,
            None => return (0.0, 0.0, 0.0),
        };

        if start + 5 < data.len() {
            let gx = i16::from_le_bytes([data[start], data[start + 1]]) as f32;
            let gy = i16::from_le_bytes([data[start + 2], data[start + 3]]) as f32;
            let gz = i16::from_le_bytes([data[start + 4], data[start + 5]]) as f32;
            (gx, gy, gz)
        } else {
            (0.0, 0.0, 0.0)
        }
    }

    fn enable_sensors(device: &hidapi::HidDevice) -> Result<(), hidapi::HidError> {
        const CMD_ENABLE_IMU: [u8; 12] = [
            0x01, 0x00, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01,
        ];
        const CMD_SET_REPORT_30: [u8; 12] = [
            0x01, 0x01, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40, 0x03, 0x30,
        ];

        device.write(&CMD_ENABLE_IMU)?;
        device.write(&CMD_SET_REPORT_30)?;
        Ok(())
    }

    fn log_joycon_connected(index: usize, side: JoyConSide, serial: &str) {
        eprintln!(
            "[joycon] connected index={} side={:?} serial={}",
            index, side, serial
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

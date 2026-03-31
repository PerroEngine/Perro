use crate::App;
use perro_graphics::GraphicsBackend;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

mod backend {
    use super::*;
    use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter};
    use btleplug::platform::Manager;
    use futures_util::stream::StreamExt;
    use hidapi::HidApi;
    use perro_input::{JoyConButton, JoyConSide};
    use tokio::runtime::Builder;
    use tokio::time::{self, Duration as TokioDuration};
    use uuid::Uuid;

    const JOYCON_VENDOR_ID: u16 = 0x057E;
    const JOYCON_L_PID: u16 = 0x2006;
    const JOYCON_R_PID: u16 = 0x2007;
    const NINTENDO_BLE_CID: u16 = 0x0553;
    const JOYCON2_R_SIDE: u8 = 0x66;
    const JOYCON2_L_SIDE: u8 = 0x67;
    const JOYCON2_INPUT_REPORT_05_UUID: Uuid = uuid::uuid!("ab7de9be-89fe-49ad-828f-118f09df7fd2");
    const JOYCON2_INPUT_REPORT_07_UUID: Uuid = uuid::uuid!("cc1bbbb5-7354-4d32-a716-a81cb241a32a");
    const JOYCON2_INPUT_REPORT_08_UUID: Uuid = uuid::uuid!("d5a9e01e-2ffc-4cca-b20c-8b67142bf442");
    const JOYCON2_WRITE_COMMAND_UUID: Uuid = uuid::uuid!("649d4ac9-8eb7-4e6c-af44-1ea54fe5f005");
    const REPORT_LEN: usize = 64;
    const SCAN_INTERVAL: Duration = Duration::from_secs(2);
    const READ_TIMEOUT: Duration = Duration::from_millis(8);
    const MAX_PERSISTENT_JOYCON_SLOTS: usize = 12;

    type ButtonBits = u16;

    enum JoyConEvent {
        Report {
            index: usize,
            side: JoyConSide,
            data: JoyConInputData,
        },
        Disconnected {
            index: usize,
        },
    }

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
    struct SlotAllocator {
        assigned: HashMap<String, usize>,
        free_indices: Vec<usize>,
        next_index: usize,
    }

    #[derive(Default)]
    pub struct JoyConBackend {
        devices: HashMap<String, DeviceHandle>,
        slots: Arc<Mutex<SlotAllocator>>,
        rx: Option<Receiver<JoyConEvent>>,
        tx: Option<Sender<JoyConEvent>>,
        last_buttons: HashMap<(usize, JoyConSide), ButtonBits>,
        last_scan: Option<Instant>,
        ble_started: bool,
        ble_stop: Option<Arc<AtomicBool>>,
    }

    const ALL_BUTTONS: [JoyConButton; JoyConButton::COUNT] = [
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
    ];

    impl JoyConBackend {
        pub fn begin_frame<B: GraphicsBackend>(&mut self, app: &mut App<B>) {
            self.ensure_channel();
            self.scan_if_needed(app);
            self.drain_events(app);
        }

        fn ensure_channel(&mut self) {
            if self.rx.is_some() {
                return;
            }
            let (tx, rx) = mpsc::channel();
            self.tx = Some(tx);
            self.rx = Some(rx);
            self.start_ble_worker_if_needed();
        }

        fn start_ble_worker_if_needed(&mut self) {
            if self.ble_started {
                return;
            }
            let Some(tx) = self.tx.clone() else {
                return;
            };
            let stop = Arc::new(AtomicBool::new(false));
            spawn_ble_manager_thread(tx, Arc::clone(&self.slots), Arc::clone(&stop));
            self.ble_stop = Some(stop);
            self.ble_started = true;
        }

        fn scan_if_needed<B: GraphicsBackend>(&mut self, app: &mut App<B>) {
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
                let slot_key = format!("hid:{serial}");
                connected_serials.insert(slot_key.clone());

                if self.devices.contains_key(&slot_key) {
                    continue;
                }

                let index = assign_slot(&self.slots, &slot_key);
                log_joycon_connected(index, side, &serial);
                self.spawn_device_thread(slot_key, serial, pid, side, index);
            }

            // Remove disconnected devices
            self.devices.retain(|slot_key, handle| {
                let connected = connected_serials.contains(slot_key);
                if !connected {
                    handle.stop.store(true, Ordering::Relaxed);
                    if let Some(index) = release_slot(&self.slots, slot_key) {
                        let _ = self.tx.as_ref().and_then(|tx| {
                            tx.send(JoyConEvent::Disconnected { index }).ok()
                        });
                        clear_joycon_index(app, index);
                        self.last_buttons.retain(|(idx, _), _| *idx != index);
                    }
                }
                connected
            });
        }

        fn spawn_device_thread(
            &mut self,
            slot_key: String,
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
                                let _ = tx.send(JoyConEvent::Report {
                                    index,
                                    side,
                                    data: payload,
                                });
                            }
                        }
                        _ => {}
                    }
                }

                let _ = tx.send(JoyConEvent::Disconnected { index });
            });

            self.devices.insert(slot_key, DeviceHandle { stop });
        }

        fn drain_events<B: GraphicsBackend>(&mut self, app: &mut App<B>) {
            let Some(rx) = self.rx.as_ref() else {
                return;
            };

            while let Ok(event) = rx.try_recv() {
                match event {
                    JoyConEvent::Report { index, side, data } => {
                        apply_report(app, index, side, data, &mut self.last_buttons);
                    }
                    JoyConEvent::Disconnected { index } => {
                        clear_joycon_index(app, index);
                        self.last_buttons.retain(|(idx, _), _| *idx != index);
                    }
                }
            }
        }
    }

    fn assign_slot(slots: &Arc<Mutex<SlotAllocator>>, key: &str) -> usize {
        let mut slots = slots.lock().expect("joycon slot allocator poisoned");
        if let Some(idx) = slots.assigned.get(key).copied() {
            slots.free_indices.retain(|free| *free != idx);
            return idx;
        }

        let index = if slots.next_index < MAX_PERSISTENT_JOYCON_SLOTS {
            let idx = slots.next_index;
            slots.next_index += 1;
            idx
        } else if let Some(free_idx) = slots.free_indices.pop() {
            free_idx
        } else {
            let idx = slots.next_index;
            slots.next_index += 1;
            idx
        };
        slots.assigned.insert(key.to_string(), index);
        index
    }

    fn release_slot(slots: &Arc<Mutex<SlotAllocator>>, key: &str) -> Option<usize> {
        let mut slots = slots.lock().ok()?;
        let index = slots.assigned.remove(key)?;
        if !slots.free_indices.contains(&index) {
            slots.free_indices.push(index);
        }
        Some(index)
    }

    fn spawn_ble_manager_thread(
        tx: Sender<JoyConEvent>,
        slots: Arc<Mutex<SlotAllocator>>,
        stop: Arc<AtomicBool>,
    ) {
        thread::spawn(move || {
            let Ok(rt) = Builder::new_current_thread().enable_all().build() else {
                return;
            };
            rt.block_on(async move {
                let Ok(manager) = Manager::new().await else {
                    return;
                };
                let Ok(adapters) = manager.adapters().await else {
                    return;
                };
                let Some(adapter) = adapters.into_iter().next() else {
                    return;
                };

                let mut known: HashSet<String> = HashSet::new();

                while !stop.load(Ordering::Relaxed) {
                    if let Ok(sl) = slots.lock() {
                        known.retain(|k| sl.assigned.contains_key(k));
                    }
                    let _ = adapter.start_scan(ScanFilter::default()).await;
                    time::sleep(TokioDuration::from_secs(2)).await;

                    let Ok(peripherals) = adapter.peripherals().await else {
                        continue;
                    };

                    for peripheral in peripherals {
                        let Some((side, serial)) = classify_joycon2_ble(&peripheral).await else {
                            continue;
                        };
                        let key = format!("ble:{serial}");
                        if known.contains(&key) {
                            continue;
                        }

                        if peripheral.connect().await.is_err() {
                            continue;
                        }
                        let _ = peripheral.discover_services().await;
                        let chars = peripheral.characteristics();
                        let input_char = chars.iter().find(|c| {
                            c.properties.contains(CharPropFlags::NOTIFY)
                                && (c.uuid == JOYCON2_INPUT_REPORT_05_UUID
                                    || c.uuid == JOYCON2_INPUT_REPORT_07_UUID
                                    || c.uuid == JOYCON2_INPUT_REPORT_08_UUID)
                        });

                        let Some(input_char) = input_char.cloned() else {
                            let _ = peripheral.disconnect().await;
                            continue;
                        };

                        if peripheral.subscribe(&input_char).await.is_err() {
                            let _ = peripheral.disconnect().await;
                            continue;
                        }

                        if let Some(cmd_char) =
                            chars.iter().find(|c| c.uuid == JOYCON2_WRITE_COMMAND_UUID)
                        {
                            let _ = peripheral
                                .write(
                                    cmd_char,
                                    &[
                                        0x0c, 0x91, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00,
                                        0x00, 0x00,
                                    ],
                                    btleplug::api::WriteType::WithoutResponse,
                                )
                                .await;
                            let _ = peripheral
                                .write(
                                    cmd_char,
                                    &[
                                        0x0c, 0x91, 0x01, 0x04, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00,
                                        0x00, 0x00,
                                    ],
                                    btleplug::api::WriteType::WithoutResponse,
                                )
                                .await;
                        }

                        let index = assign_slot(&slots, &key);
                        let tx_clone = tx.clone();
                        let slots_clone = Arc::clone(&slots);
                        let key_clone = key.clone();
                        let stop_clone = Arc::clone(&stop);
                        known.insert(key);

                        tokio::spawn(async move {
                            let Ok(mut notifications) = peripheral.notifications().await else {
                                let _ = tx_clone.send(JoyConEvent::Disconnected { index });
                                let _ = release_slot(&slots_clone, &key_clone);
                                return;
                            };
                            while !stop_clone.load(Ordering::Relaxed) {
                                match time::timeout(
                                    TokioDuration::from_secs(4),
                                    notifications.next(),
                                )
                                .await
                                {
                                    Ok(Some(packet)) => {
                                        if let Some(data) = decode_report(&packet.value, side) {
                                            let _ = tx_clone.send(JoyConEvent::Report {
                                                index,
                                                side,
                                                data,
                                            });
                                        }
                                    }
                                    Ok(None) | Err(_) => {
                                        break;
                                    }
                                }
                            }
                            let _ = peripheral.disconnect().await;
                            let _ = tx_clone.send(JoyConEvent::Disconnected { index });
                            let _ = release_slot(&slots_clone, &key_clone);
                        });
                    }
                }
            });
        });
    }

    async fn classify_joycon2_ble(
        peripheral: &btleplug::platform::Peripheral,
    ) -> Option<(JoyConSide, String)> {
        let props = peripheral.properties().await.ok().flatten()?;
        let data = props.manufacturer_data.get(&NINTENDO_BLE_CID)?;
        let side = if data.contains(&JOYCON2_L_SIDE) {
            JoyConSide::LJoyCon
        } else if data.contains(&JOYCON2_R_SIDE) {
            JoyConSide::RJoyCon
        } else {
            return None;
        };

        let serial = format!("{:?}", peripheral.id())
            .replace("PeripheralId(", "")
            .replace(')', "")
            .replace(':', "")
            .to_uppercase();

        Some((side, serial))
    }

    #[inline(always)]
    fn set_button_bit(bits: &mut ButtonBits, button: JoyConButton, is_down: bool) {
        let bit = 1u16 << (button.as_index() as u16);
        if is_down {
            *bits |= bit;
        } else {
            *bits &= !bit;
        }
    }

    fn apply_report<B: GraphicsBackend>(
        app: &mut App<B>,
        index: usize,
        side: JoyConSide,
        data: JoyConInputData,
        last_buttons: &mut HashMap<(usize, JoyConSide), ButtonBits>,
    ) {
        let key = (index, side);
        let prev = last_buttons.get(&key).copied();

        apply_buttons(app, index, data.buttons, prev);
        last_buttons.insert(key, data.buttons);

        app.set_joycon_stick(index, data.stick.0, data.stick.1);
        app.set_joycon_gyro(index, data.gyro.0, data.gyro.1, data.gyro.2);
        app.set_joycon_accel(index, data.accel.0, data.accel.1, data.accel.2);
    }

    fn clear_joycon_index<B: GraphicsBackend>(app: &mut App<B>, index: usize) {
        for button in ALL_BUTTONS {
            app.set_joycon_button_state(index, button, false);
        }
        app.set_joycon_stick(index, 0.0, 0.0);
        app.set_joycon_gyro(index, 0.0, 0.0, 0.0);
        app.set_joycon_accel(index, 0.0, 0.0, 0.0);
    }

    fn apply_buttons<B: GraphicsBackend>(
        app: &mut App<B>,
        index: usize,
        buttons: ButtonBits,
        prev: Option<ButtonBits>,
    ) {
        let prev_bits = prev.unwrap_or(0);
        let changed = buttons ^ prev_bits;
        if changed == 0 {
            return;
        }

        for button in ALL_BUTTONS {
            let bit = 1u16 << (button.as_index() as u16);
            if (changed & bit) != 0 {
                app.set_joycon_button_state(index, button, (buttons & bit) != 0);
            }
        }
    }

    fn decode_report(data: &[u8], side: JoyConSide) -> Option<JoyConInputData> {
        if let Some(decoded) = decode_report_joycon2(data, side) {
            return Some(decoded);
        }
        decode_report_joycon1(data, side)
    }

    fn decode_report_joycon1(data: &[u8], side: JoyConSide) -> Option<JoyConInputData> {
        if data.len() < 49 {
            return None;
        }

        let offset = if data[0] == 0x30 { 1 } else { 0 };

        let (left_idx, shared_idx, right_idx) = if offset == 1 { (2, 3, 4) } else { (3, 4, 5) };

        let (btn_left, btn_shared, btn_right) = (data[left_idx], data[shared_idx], data[right_idx]);

        let mut buttons: ButtonBits = 0;
        match side {
            JoyConSide::LJoyCon => {
                set_button_bit(&mut buttons, JoyConButton::Top, (btn_left & 0x02) != 0);
                set_button_bit(&mut buttons, JoyConButton::Bottom, (btn_left & 0x01) != 0);
                set_button_bit(&mut buttons, JoyConButton::Left, (btn_left & 0x08) != 0);
                set_button_bit(&mut buttons, JoyConButton::Right, (btn_left & 0x04) != 0);
                set_button_bit(&mut buttons, JoyConButton::Bumper, (btn_left & 0x40) != 0);
                set_button_bit(&mut buttons, JoyConButton::Trigger, (btn_left & 0x80) != 0);
                set_button_bit(&mut buttons, JoyConButton::SL, (btn_left & 0x20) != 0);
                set_button_bit(&mut buttons, JoyConButton::SR, (btn_left & 0x10) != 0);
                set_button_bit(&mut buttons, JoyConButton::Start, (btn_shared & 0x01) != 0);
                set_button_bit(&mut buttons, JoyConButton::Meta, (btn_shared & 0x20) != 0);
                set_button_bit(&mut buttons, JoyConButton::Stick, (btn_shared & 0x08) != 0);
            }
            JoyConSide::RJoyCon => {
                set_button_bit(&mut buttons, JoyConButton::Top, (btn_right & 0x02) != 0);
                set_button_bit(&mut buttons, JoyConButton::Bottom, (btn_right & 0x04) != 0);
                set_button_bit(&mut buttons, JoyConButton::Left, (btn_right & 0x01) != 0);
                set_button_bit(&mut buttons, JoyConButton::Right, (btn_right & 0x08) != 0);
                set_button_bit(&mut buttons, JoyConButton::Bumper, (btn_right & 0x40) != 0);
                set_button_bit(&mut buttons, JoyConButton::Trigger, (btn_right & 0x80) != 0);
                set_button_bit(&mut buttons, JoyConButton::SL, (btn_right & 0x20) != 0);
                set_button_bit(&mut buttons, JoyConButton::SR, (btn_right & 0x10) != 0);
                set_button_bit(&mut buttons, JoyConButton::Start, (btn_shared & 0x02) != 0);
                set_button_bit(&mut buttons, JoyConButton::Meta, (btn_shared & 0x10) != 0);
                set_button_bit(&mut buttons, JoyConButton::Stick, (btn_shared & 0x04) != 0);
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

    fn decode_report_joycon2(data: &[u8], side: JoyConSide) -> Option<JoyConInputData> {
        // Joy-Con 2 BLE reports in legacy code are report 0x05/0x08 with motion at 0x30..0x3B.
        let report_id = *data.first()?;
        if report_id != 0x05 && report_id != 0x08 {
            return None;
        }
        if data.len() < 0x3C {
            return None;
        }

        let is_left = matches!(side, JoyConSide::LJoyCon);
        let btn_offset = if is_left { 4 } else { 3 };
        let state = ((data[btn_offset] as u32) << 16)
            | ((data[btn_offset + 1] as u32) << 8)
            | (data[btn_offset + 2] as u32);

        let mut buttons: ButtonBits = 0;
        if is_left {
            set_button_bit(&mut buttons, JoyConButton::Top, (state & 0x000002) != 0);
            set_button_bit(&mut buttons, JoyConButton::Bottom, (state & 0x000001) != 0);
            set_button_bit(&mut buttons, JoyConButton::Left, (state & 0x000008) != 0);
            set_button_bit(&mut buttons, JoyConButton::Right, (state & 0x000004) != 0);
            set_button_bit(&mut buttons, JoyConButton::Bumper, (state & 0x000040) != 0);
            set_button_bit(&mut buttons, JoyConButton::Trigger, (state & 0x000080) != 0);
            set_button_bit(&mut buttons, JoyConButton::Stick, (state & 0x000800) != 0);
            set_button_bit(&mut buttons, JoyConButton::SL, (data[6] & 0x20) != 0);
            set_button_bit(&mut buttons, JoyConButton::SR, (data[6] & 0x10) != 0);
            set_button_bit(&mut buttons, JoyConButton::Start, (state & 0x000100) != 0);
            set_button_bit(&mut buttons, JoyConButton::Meta, (data[5] & 0x20) != 0);
        } else {
            set_button_bit(&mut buttons, JoyConButton::Top, (state & 0x000400) != 0);
            set_button_bit(&mut buttons, JoyConButton::Bottom, (state & 0x000200) != 0);
            set_button_bit(&mut buttons, JoyConButton::Left, (state & 0x000100) != 0);
            set_button_bit(&mut buttons, JoyConButton::Right, (state & 0x000800) != 0);
            set_button_bit(&mut buttons, JoyConButton::Bumper, (state & 0x004000) != 0);
            set_button_bit(&mut buttons, JoyConButton::Trigger, (state & 0x008000) != 0);
            set_button_bit(&mut buttons, JoyConButton::Stick, (state & 0x000004) != 0);
            set_button_bit(&mut buttons, JoyConButton::SL, (data[4] & 0x20) != 0);
            set_button_bit(&mut buttons, JoyConButton::SR, (data[4] & 0x10) != 0);
            set_button_bit(&mut buttons, JoyConButton::Start, (state & 0x000002) != 0);
            set_button_bit(&mut buttons, JoyConButton::Meta, (data[5] & 0x10) != 0);
        }

        let stick_offset = if is_left { 10 } else { 13 };
        let stick = {
            let raw = &data[stick_offset..stick_offset + 3];
            let x_raw = ((raw[1] & 0x0F) as u16) << 8 | raw[0] as u16;
            let y_raw = (raw[2] as u16) << 4 | ((raw[1] & 0xF0) >> 4) as u16;
            let x = ((x_raw as f32 / 4095.0).clamp(0.0, 1.0) - 0.5) * 2.0;
            let y = ((y_raw as f32 / 4095.0).clamp(0.0, 1.0) - 0.5) * 2.0;
            (x, y)
        };

        let accel = (
            i16::from_le_bytes([data[0x30], data[0x31]]) as f32,
            i16::from_le_bytes([data[0x32], data[0x33]]) as f32,
            i16::from_le_bytes([data[0x34], data[0x35]]) as f32,
        );

        const JOYCON2_GYRO_SCALE: f32 = 13.875;
        let gx_raw = i16::from_le_bytes([data[0x36], data[0x37]]) as f32 / JOYCON2_GYRO_SCALE;
        let gy_raw = i16::from_le_bytes([data[0x38], data[0x39]]) as f32 / JOYCON2_GYRO_SCALE;
        let gz_raw = i16::from_le_bytes([data[0x3A], data[0x3B]]) as f32 / JOYCON2_GYRO_SCALE;
        // Match legacy transform used before state-space mapping.
        let gyro = (gy_raw, gx_raw, -gz_raw);

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

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
    use std::sync::OnceLock;
    use tokio::runtime::Builder;
    use tokio::time::{self, Duration as TokioDuration, Instant as TokioInstant};
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
    const BLE_DISCOVERY_POLL: TokioDuration = TokioDuration::from_millis(20);
    const BLE_CHAR_DISCOVERY_RETRIES: u32 = 4;
    const BLE_CHAR_DISCOVERY_DELAY: TokioDuration = TokioDuration::from_millis(10);
    const MAX_PERSISTENT_JOYCON_SLOTS: usize = 12;
    const STICK_DEADZONE: f32 = 0.08;
    const STICK_AXIS_GAIN_POS: f32 = 1.85;
    const STICK_AXIS_GAIN_NEG: f32 = 1.45;
    const ACCEL_GRAVITY_SCALE: f32 = 0.2386;
    const ACCEL_ONE_G_TARGET: f32 = 1000.0;
    const GYRO_DEADZONE_DPS: f32 = 10.0;

    type ButtonBits = u16;

    enum JoyConEvent {
        Report {
            index: usize,
            side: JoyConSide,
            data: JoyConInputData,
            raw_report: Vec<u8>,
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
        gyro_bias: HashMap<(usize, JoyConSide), (f32, f32, f32)>,
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
                            if let Some(payload) = decode_report_hid(data, side) {
                                let _ = tx.send(JoyConEvent::Report {
                                    index,
                                    side,
                                    data: payload,
                                    raw_report: data.to_vec(),
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
                    JoyConEvent::Report {
                        index,
                        side,
                        data,
                        raw_report,
                    } => {
                        if raw_dump_enabled() {
                            log_raw_joycon_report(index, side, &raw_report, &data, "hid");
                        }
                        apply_report(
                            app,
                            index,
                            side,
                            data,
                            &mut self.last_buttons,
                            &mut self.gyro_bias,
                        );
                    }
                    JoyConEvent::Disconnected { index } => {
                        clear_joycon_index(app, index);
                        self.last_buttons.retain(|(idx, _), _| *idx != index);
                        self.gyro_bias.retain(|(idx, _), _| *idx != index);
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
                    eprintln!("[joycon2] BLE adapters unavailable");
                    return;
                };
                let Some(adapter) = adapters.into_iter().next() else {
                    eprintln!("[joycon2] no BLE adapter found");
                    return;
                };
                eprintln!("[joycon2] BLE worker started");
                let worker_started_at = TokioInstant::now();

                let mut known: HashSet<String> = HashSet::new();

                let _ = adapter.start_scan(ScanFilter::default()).await;
                while !stop.load(Ordering::Relaxed) {
                    if let Ok(sl) = slots.lock() {
                        known.retain(|k| sl.assigned.contains_key(k));
                    }
                    time::sleep(BLE_DISCOVERY_POLL).await;

                    let Ok(peripherals) = adapter.peripherals().await else {
                        continue;
                    };

                    for peripheral in peripherals {
                        let Some((side, serial, debug_tag)) = classify_joycon2_ble(&peripheral).await else {
                            continue;
                        };
                        let key = format!("ble:{serial}");
                        if known.contains(&key) {
                            continue;
                        }

                        if peripheral.connect().await.is_err() {
                            eprintln!("[joycon2] connect failed id={serial} tag={debug_tag}");
                            continue;
                        }
                        let connect_t0 = TokioInstant::now();
                        eprintln!(
                            "[joycon2][trace_v2] connected id={serial} side={side:?} tag={debug_tag} t={}ms",
                            worker_started_at.elapsed().as_millis()
                        );
                        let mut chars = peripheral.characteristics();
                        for _ in 0..BLE_CHAR_DISCOVERY_RETRIES {
                            if !chars.is_empty() {
                                break;
                            }
                            let _ = peripheral.discover_services().await;
                            chars = peripheral.characteristics();
                            if chars.is_empty() {
                                time::sleep(BLE_CHAR_DISCOVERY_DELAY).await;
                            }
                        }
                        let preferred_uuids: [Uuid; 3] = match side {
                            JoyConSide::LJoyCon => [
                                JOYCON2_INPUT_REPORT_07_UUID,
                                JOYCON2_INPUT_REPORT_05_UUID,
                                JOYCON2_INPUT_REPORT_08_UUID,
                            ],
                            JoyConSide::RJoyCon => [
                                JOYCON2_INPUT_REPORT_08_UUID,
                                JOYCON2_INPUT_REPORT_05_UUID,
                                JOYCON2_INPUT_REPORT_07_UUID,
                            ],
                        };
                        let input_char = preferred_uuids
                            .iter()
                            .find_map(|wanted| {
                                chars
                                    .iter()
                                    .find(|c| {
                                        c.properties.contains(CharPropFlags::NOTIFY)
                                            && c.uuid == *wanted
                                    })
                                    .cloned()
                            })
                            .or_else(|| {
                                chars.iter()
                                    .find(|c| {
                                        c.properties.contains(CharPropFlags::NOTIFY)
                                            && (c.uuid == JOYCON2_INPUT_REPORT_05_UUID
                                                || c.uuid == JOYCON2_INPUT_REPORT_07_UUID
                                                || c.uuid == JOYCON2_INPUT_REPORT_08_UUID)
                                    })
                                    .cloned()
                            })
                            .or_else(|| {
                                chars.iter()
                                    .find(|c| c.properties.contains(CharPropFlags::NOTIFY))
                                    .cloned()
                            });

                        let Some(input_char) = input_char else {
                            eprintln!(
                                "[joycon2] no notify characteristic id={serial} chars_seen={}",
                                chars.len()
                            );
                            let _ = peripheral.disconnect().await;
                            continue;
                        };

                        if peripheral.subscribe(&input_char).await.is_err() {
                            eprintln!("[joycon2] subscribe failed id={serial} uuid={}", input_char.uuid);
                            let _ = peripheral.disconnect().await;
                            continue;
                        }
                        eprintln!(
                            "[joycon2] subscribed id={serial} uuid={} t={}ms",
                            input_char.uuid,
                            connect_t0.elapsed().as_millis()
                        );

                        let cmd_char = chars
                            .iter()
                            .find(|c| c.uuid == JOYCON2_WRITE_COMMAND_UUID)
                            .cloned();
                        if let Some(cmd_char) = cmd_char.as_ref() {
                            send_joycon2_enable_sequence(&peripheral, cmd_char).await;
                            // Aggressive startup: fire a second pulse immediately.
                            send_joycon2_enable_sequence(&peripheral, cmd_char).await;
                        }

                        let index = assign_slot(&slots, &key);
                        let tx_clone = tx.clone();
                        let slots_clone = Arc::clone(&slots);
                        let key_clone = key.clone();
                        let stop_clone = Arc::clone(&stop);
                        known.insert(key);

                        tokio::spawn(async move {
                            let Ok(mut notifications) = peripheral.notifications().await else {
                                eprintln!("[joycon2] notifications stream failed id={key_clone}");
                                let _ = tx_clone.send(JoyConEvent::Disconnected { index });
                                let _ = release_slot(&slots_clone, &key_clone);
                                return;
                            };
                            let mut imu_active = false;
                            let mut last_enable_retry = TokioInstant::now();
                            let mut first_report_logged = false;
                            while !stop_clone.load(Ordering::Relaxed) {
                                match time::timeout(
                                    TokioDuration::from_secs(4),
                                    notifications.next(),
                                )
                                .await
                                {
                                    Ok(Some(packet)) => {
                                        if !first_report_logged {
                                            eprintln!(
                                                "[joycon2] first_report id={} t={}ms len={}",
                                                key_clone,
                                                connect_t0.elapsed().as_millis(),
                                                packet.value.len()
                                            );
                                            first_report_logged = true;
                                        }
                                        let rid = packet.value.first().copied().unwrap_or(0xFF);
                                        if let Some(data) = decode_report_ble(&packet.value, side) {
                                            let imu_zero = data.gyro.0 == 0.0
                                                && data.gyro.1 == 0.0
                                                && data.gyro.2 == 0.0
                                                && data.accel.0 == 0.0
                                                && data.accel.1 == 0.0
                                                && data.accel.2 == 0.0;
                                            if !imu_zero {
                                                if !imu_active {
                                                    eprintln!(
                                                        "[joycon2] imu_active id={} t={}ms",
                                                        key_clone,
                                                        connect_t0.elapsed().as_millis()
                                                    );
                                                }
                                                imu_active = true;
                                            } else if !imu_active
                                                && last_enable_retry.elapsed()
                                                    >= TokioDuration::from_millis(80)
                                            {
                                                if let Some(cmd_char) = cmd_char.as_ref() {
                                                    eprintln!(
                                                        "[joycon2] imu not active yet, retrying enable id={key_clone}"
                                                    );
                                                    send_joycon2_enable_sequence(
                                                        &peripheral,
                                                        cmd_char,
                                                    )
                                                    .await;
                                                }
                                                last_enable_retry = TokioInstant::now();
                                            }
                                            if raw_dump_enabled() {
                                                log_raw_joycon_report(
                                                    index,
                                                    side,
                                                    &packet.value,
                                                    &data,
                                                    "ble",
                                                );
                                            }
                                            eprintln!(
                                                "[joycon2][stream] id={} side={:?} report=0x{:02X} len={} buttons=0x{:04X} stick=({:.3},{:.3}) gyro=({:.1},{:.1},{:.1}) accel=({:.1},{:.1},{:.1})",
                                                key_clone,
                                                side,
                                                rid,
                                                packet.value.len(),
                                                data.buttons,
                                                data.stick.0,
                                                data.stick.1,
                                                data.gyro.0,
                                                data.gyro.1,
                                                data.gyro.2,
                                                data.accel.0,
                                                data.accel.1,
                                                data.accel.2
                                            );
                                            let _ = tx_clone.send(JoyConEvent::Report {
                                                index,
                                                side,
                                                data,
                                                raw_report: packet.value.clone(),
                                            });
                                        } else {
                                            eprintln!(
                                                "[joycon2][stream] undecoded id={} side={:?} report=0x{:02X} len={}",
                                                key_clone,
                                                side,
                                                rid,
                                                packet.value.len()
                                            );
                                        }
                                    }
                                    Ok(None) | Err(_) => {
                                        eprintln!("[joycon2] notifications timeout/ended id={key_clone}");
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
    ) -> Option<(JoyConSide, String, String)> {
        let props = peripheral.properties().await.ok().flatten()?;

        let mut side = None;
        let mut tag = String::new();

        if let Some(data) = props.manufacturer_data.get(&NINTENDO_BLE_CID) {
            if data.contains(&JOYCON2_L_SIDE) {
                side = Some(JoyConSide::LJoyCon);
                tag = "cid+side(L)".to_string();
            } else if data.contains(&JOYCON2_R_SIDE) {
                side = Some(JoyConSide::RJoyCon);
                tag = "cid+side(R)".to_string();
            } else {
                tag = "cid-no-side".to_string();
            }
        }

        if side.is_none()
            && let Some(name) = props.local_name.as_deref()
        {
            let lower = name.to_ascii_lowercase();
            if lower.contains("joy-con") || lower.contains("joycon") || lower.contains("nintendo")
            {
                if lower.contains("(l)") || lower.contains(" left") {
                    side = Some(JoyConSide::LJoyCon);
                    tag = format!("name(L):{name}");
                } else if lower.contains("(r)") || lower.contains(" right") {
                    side = Some(JoyConSide::RJoyCon);
                    tag = format!("name(R):{name}");
                }
            }
        }

        let side = match side {
            Some(s) => s,
            None => return None,
        };

        let serial = format!("{:?}", peripheral.id())
            .replace("PeripheralId(", "")
            .replace(')', "")
            .replace(':', "")
            .to_uppercase();

        Some((side, serial, tag))
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
        gyro_bias: &mut HashMap<(usize, JoyConSide), (f32, f32, f32)>,
    ) {
        let key = (index, side);
        let prev = last_buttons.get(&key).copied();

        apply_buttons(app, index, data.buttons, prev);
        last_buttons.insert(key, data.buttons);

        app.set_joycon_stick(index, data.stick.0, data.stick.1);
        let gyro = stabilize_gyro(data.gyro, data.accel, key, gyro_bias);
        app.set_joycon_gyro(index, gyro.0, gyro.1, gyro.2);
        app.set_joycon_accel(index, data.accel.0, data.accel.1, data.accel.2);
    }

    fn stabilize_gyro(
        raw_gyro: (f32, f32, f32),
        accel: (f32, f32, f32),
        key: (usize, JoyConSide),
        gyro_bias: &mut HashMap<(usize, JoyConSide), (f32, f32, f32)>,
    ) -> (f32, f32, f32) {
        let bias = gyro_bias.entry(key).or_insert((0.0, 0.0, 0.0));
        let amag = (accel.0 * accel.0 + accel.1 * accel.1 + accel.2 * accel.2).sqrt();
        let gmag =
            (raw_gyro.0 * raw_gyro.0 + raw_gyro.1 * raw_gyro.1 + raw_gyro.2 * raw_gyro.2).sqrt();

        // Learn bias only while likely still.
        let likely_still = (amag - ACCEL_ONE_G_TARGET).abs() < 180.0 && gmag < 260.0;
        if likely_still {
            const ALPHA: f32 = 0.03;
            bias.0 = bias.0 + (raw_gyro.0 - bias.0) * ALPHA;
            bias.1 = bias.1 + (raw_gyro.1 - bias.1) * ALPHA;
            bias.2 = bias.2 + (raw_gyro.2 - bias.2) * ALPHA;
        }

        let x = apply_deadzone(raw_gyro.0 - bias.0, GYRO_DEADZONE_DPS);
        let y = apply_deadzone(raw_gyro.1 - bias.1, GYRO_DEADZONE_DPS);
        let z = apply_deadzone(raw_gyro.2 - bias.2, GYRO_DEADZONE_DPS);
        (x, y, z)
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

    fn decode_report_hid(data: &[u8], side: JoyConSide) -> Option<JoyConInputData> {
        decode_report_joycon1(data, side)
    }

    fn decode_report_ble(data: &[u8], side: JoyConSide) -> Option<JoyConInputData> {
        decode_report_joycon2(data, side)
    }

    fn decode_report_joycon1(data: &[u8], side: JoyConSide) -> Option<JoyConInputData> {
        if data.len() < 49 {
            return None;
        }
        // HID Joy-Con 1 reads here are expected to be full 0x30 reports with report-id.
        // When report-id is present, force canonical offset=0 to avoid false IMU parsing.
        if data.first().copied() == Some(0x30) {
            return decode_report_joycon1_with_offset(data, side, 0);
        }
        decode_report_joycon1_with_offset(data, side, 1)
    }

    fn decode_report_joycon1_with_offset(
        data: &[u8],
        side: JoyConSide,
        offset: usize,
    ) -> Option<JoyConInputData> {

        // Joy-Con 1 layout:
        // byte 3 = right buttons, byte 4 = shared, byte 5 = left buttons
        let right_idx = 3usize.checked_sub(offset)?;
        let shared_idx = 4usize.checked_sub(offset)?;
        let left_idx = 5usize.checked_sub(offset)?;

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
        // BLE packets in this stream are usually raw report bodies (counter first),
        // not report-id-prefixed frames. Use stable alignment preference:
        // - if first byte looks like report-id, try shifted first
        // - otherwise try base-0 first
        let first = data.first().copied().unwrap_or(0xFF);
        if matches!(first, 0x05 | 0x07 | 0x08) {
            decode_report_joycon2_at_base(data, side, 1)
                .or_else(|| decode_report_joycon2_at_base(data, side, 0))
        } else {
            decode_report_joycon2_at_base(data, side, 0)
                .or_else(|| decode_report_joycon2_at_base(data, side, 1))
        }
    }

    fn decode_report_joycon2_at_base(
        data: &[u8],
        side: JoyConSide,
        base: usize,
    ) -> Option<JoyConInputData> {
        if data.len() < base + 0x3C {
            return None;
        }

        let is_left = matches!(side, JoyConSide::LJoyCon);
        let btn_offset = base + if is_left { 4 } else { 3 };
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
            set_button_bit(&mut buttons, JoyConButton::SL, (data[base + 6] & 0x20) != 0);
            set_button_bit(&mut buttons, JoyConButton::SR, (data[base + 6] & 0x10) != 0);
            set_button_bit(&mut buttons, JoyConButton::Start, (state & 0x000100) != 0);
            set_button_bit(&mut buttons, JoyConButton::Meta, (data[base + 5] & 0x20) != 0);
        } else {
            // Joy-Con 2 right face buttons: observed stream indicates Top/Bottom are inverted
            // vs legacy masks, so map accordingly.
            set_button_bit(&mut buttons, JoyConButton::Top, (state & 0x000200) != 0);
            set_button_bit(&mut buttons, JoyConButton::Bottom, (state & 0x000400) != 0);
            set_button_bit(&mut buttons, JoyConButton::Left, (state & 0x000100) != 0);
            set_button_bit(&mut buttons, JoyConButton::Right, (state & 0x000800) != 0);
            set_button_bit(&mut buttons, JoyConButton::Bumper, (state & 0x004000) != 0);
            set_button_bit(&mut buttons, JoyConButton::Trigger, (state & 0x008000) != 0);
            set_button_bit(&mut buttons, JoyConButton::Stick, (state & 0x000004) != 0);
            set_button_bit(&mut buttons, JoyConButton::SL, (data[base + 4] & 0x20) != 0);
            set_button_bit(&mut buttons, JoyConButton::SR, (data[base + 4] & 0x10) != 0);
            set_button_bit(&mut buttons, JoyConButton::Start, (state & 0x000002) != 0);
            set_button_bit(&mut buttons, JoyConButton::Meta, (data[base + 5] & 0x10) != 0);
        }

        let stick_offsets: &[usize] = if is_left {
            &[base + 10, base + 8]
        } else {
            &[base + 5, base + 13, base + 10]
        };
        let (x_raw, y_raw) = decode_stick_best_candidate(data, stick_offsets).unwrap_or((0, 0));
        let stick = if x_raw == 0 && y_raw == 0 {
            (0.0, 0.0)
        } else {
            let x = normalize_stick_axis(((x_raw as f32 / 4095.0).clamp(0.0, 1.0) - 0.5) * 2.0);
            let y = normalize_stick_axis(((y_raw as f32 / 4095.0).clamp(0.0, 1.0) - 0.5) * 2.0);
            (x, y)
        };

        let accel = normalize_accel_to_one_g(decode_joycon2_accel_best_candidate(data, base));
        let gyro = normalize_gyro_near_rest(decode_joycon2_gyro_best_candidate(data, base));

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

        let x = normalize_stick_axis(x_norm * 2.0 - 1.0);
        let y = normalize_stick_axis(y_norm * 2.0 - 1.0);
        Some((x, y))
    }

    fn decode_accel(data: &[u8], offset: usize) -> (f32, f32, f32) {
        let start = match 13_usize.checked_sub(offset) {
            Some(v) => v,
            None => return (0.0, 0.0, 0.0),
        };

        if start + 5 < data.len() {
            let ax =
                i16::from_le_bytes([data[start], data[start + 1]]) as f32 * ACCEL_GRAVITY_SCALE;
            let ay =
                i16::from_le_bytes([data[start + 2], data[start + 3]]) as f32 * ACCEL_GRAVITY_SCALE;
            let az =
                i16::from_le_bytes([data[start + 4], data[start + 5]]) as f32 * ACCEL_GRAVITY_SCALE;
            (ax, ay, az)
        } else {
            (0.0, 0.0, 0.0)
        }
    }

    #[inline(always)]
    fn apply_stick_deadzone(v: f32) -> f32 {
        let a = v.abs();
        if a < STICK_DEADZONE {
            0.0
        } else {
            v.signum() * ((a - STICK_DEADZONE) / (1.0 - STICK_DEADZONE))
        }
    }

    #[inline(always)]
    fn normalize_stick_axis(v: f32) -> f32 {
        let gain = if v >= 0.0 {
            STICK_AXIS_GAIN_POS
        } else {
            STICK_AXIS_GAIN_NEG
        };
        apply_stick_deadzone((v * gain).clamp(-1.0, 1.0)).clamp(-1.0, 1.0)
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
        // HID output report: [0x01, packet_no, rumble(8), subcmd, arg]
        const CMD_ENABLE_IMU: [u8; 12] = [
            0x01, 0x00, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40, 0x40, 0x01,
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

    fn log_raw_joycon_report(
        index: usize,
        side: JoyConSide,
        raw: &[u8],
        parsed: &JoyConInputData,
        source: &str,
    ) {
        let report_id = raw.first().copied().unwrap_or(0xFF);
        eprintln!(
            "[joycon][raw] src={} index={} side={:?} report=0x{:02X} len={} bytes={} buttons=0x{:04X} stick=({:.3},{:.3}) gyro=({:.1},{:.1},{:.1}) accel=({:.1},{:.1},{:.1})",
            source,
            index,
            side,
            report_id,
            raw.len(),
            hex_bytes(raw),
            parsed.buttons,
            parsed.stick.0,
            parsed.stick.1,
            parsed.gyro.0,
            parsed.gyro.1,
            parsed.gyro.2,
            parsed.accel.0,
            parsed.accel.1,
            parsed.accel.2
        );
    }

    fn hex_bytes(raw: &[u8]) -> String {
        raw.iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn decode_stick_raw_12(data: &[u8], offset: usize) -> Option<(u16, u16)> {
        let raw = data.get(offset..offset + 3)?;
        let x_raw = ((raw[1] & 0x0F) as u16) << 8 | raw[0] as u16;
        let y_raw = (raw[2] as u16) << 4 | ((raw[1] & 0xF0) >> 4) as u16;
        Some((x_raw, y_raw))
    }

    fn decode_stick_best_candidate(data: &[u8], offsets: &[usize]) -> Option<(u16, u16)> {
        let mut best: Option<(u16, u16)> = None;
        let mut best_score = f32::NEG_INFINITY;
        for &off in offsets {
            let Some((x, y)) = decode_stick_raw_12(data, off) else {
                continue;
            };
            if x == 0 && y == 0 {
                continue;
            }
            let dx = (x as f32 - 2048.0).abs() / 2048.0;
            let dy = (y as f32 - 2048.0).abs() / 2048.0;
            // Favor candidates closer to center by default to avoid pegged -1/-1 from bad offsets.
            let score = -(dx + dy);
            if score > best_score {
                best_score = score;
                best = Some((x, y));
            }
        }
        best
    }

    fn decode_joycon2_accel_best_candidate(data: &[u8], base: usize) -> (f32, f32, f32) {
        let motion_offsets = [base + 0x30, base + 0x2A, base + 0x24];
        let mut best = (0.0, 0.0, 0.0);
        let mut best_score = f32::INFINITY;
        for start in motion_offsets {
            if start + 5 >= data.len() {
                continue;
            }
            let ax = i16::from_le_bytes([data[start], data[start + 1]]) as f32 * ACCEL_GRAVITY_SCALE;
            let ay = i16::from_le_bytes([data[start + 2], data[start + 3]]) as f32 * ACCEL_GRAVITY_SCALE;
            let az = i16::from_le_bytes([data[start + 4], data[start + 5]]) as f32 * ACCEL_GRAVITY_SCALE;
            let amag = (ax * ax + ay * ay + az * az).sqrt();
            // Prefer realistic gravity magnitude region before normalization.
            let score = (amag - ACCEL_ONE_G_TARGET).abs();
            if score < best_score {
                best_score = score;
                best = (ax, ay, az);
            }
        }
        best
    }

    fn decode_joycon2_gyro_best_candidate(data: &[u8], base: usize) -> (f32, f32, f32) {
        const JOYCON2_GYRO_SCALE: f32 = 13.875;
        let motion_offsets = [base + 0x30, base + 0x2A, base + 0x24];
        let mut best = (0.0, 0.0, 0.0);
        let mut best_score = f32::INFINITY;

        for start in motion_offsets {
            if start + 11 >= data.len() {
                continue;
            }
            let gx_raw = i16::from_le_bytes([data[start + 6], data[start + 7]]) as f32 / JOYCON2_GYRO_SCALE;
            let gy_raw = i16::from_le_bytes([data[start + 8], data[start + 9]]) as f32 / JOYCON2_GYRO_SCALE;
            let gz_raw = i16::from_le_bytes([data[start + 10], data[start + 11]]) as f32 / JOYCON2_GYRO_SCALE;
            let gyro = (gy_raw, gx_raw, -gz_raw);
            let gmag = (gyro.0 * gyro.0 + gyro.1 * gyro.1 + gyro.2 * gyro.2).sqrt();
            // Parse gyro independently; choose the least explosive candidate.
            let score = gmag;
            if score < best_score {
                best_score = score;
                best = gyro;
            }
        }
        best
    }

    fn normalize_accel_to_one_g(accel: (f32, f32, f32)) -> (f32, f32, f32) {
        let amag = (accel.0 * accel.0 + accel.1 * accel.1 + accel.2 * accel.2).sqrt();
        if amag <= 0.001 {
            return accel;
        }
        let gain = (ACCEL_ONE_G_TARGET / amag).clamp(0.5, 2.0);
        (accel.0 * gain, accel.1 * gain, accel.2 * gain)
    }

    fn normalize_gyro_near_rest(gyro: (f32, f32, f32)) -> (f32, f32, f32) {
        (
            apply_deadzone(gyro.0, GYRO_DEADZONE_DPS),
            apply_deadzone(gyro.1, GYRO_DEADZONE_DPS),
            apply_deadzone(gyro.2, GYRO_DEADZONE_DPS),
        )
    }

    #[inline(always)]
    fn apply_deadzone(v: f32, dz: f32) -> f32 {
        if v.abs() < dz { 0.0 } else { v }
    }

    async fn send_joycon2_enable_sequence(
        peripheral: &btleplug::platform::Peripheral,
        cmd_char: &btleplug::api::Characteristic,
    ) {
        let _ = peripheral
            .write(
                cmd_char,
                &[
                    0x0c, 0x91, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00,
                ],
                btleplug::api::WriteType::WithoutResponse,
            )
            .await;
        let _ = peripheral
            .write(
                cmd_char,
                &[
                    0x0c, 0x91, 0x01, 0x04, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00,
                ],
                btleplug::api::WriteType::WithoutResponse,
            )
            .await;
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

use crate::App;
#[cfg(not(target_arch = "wasm32"))]
use crate::threaded::RenderThreadBridge;
use perro_graphics::GraphicsBackend;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use perro_input_api::InputEvent;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use perro_input_api::{
    JoyConButton, JoyConIndicatorRequest, JoyConRumbleRequest, JoyConSide, PlayerBinding,
    SignedUnitVector2,
};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
trait JoyConSink {
    fn set_joycon_button_state(&mut self, index: usize, button: JoyConButton, is_down: bool);
    fn set_joycon_stick(&mut self, index: usize, x: f32, y: f32);
    fn set_joycon_stick_unit(&mut self, index: usize, stick: SignedUnitVector2);
    fn set_joycon_side(&mut self, index: usize, side: JoyConSide);
    fn set_joycon_connected(&mut self, index: usize, connected: bool);
    fn set_joycon_calibrated(&mut self, index: usize, calibrated: bool);
    fn set_joycon_calibration_in_progress(&mut self, index: usize, in_progress: bool);
    fn set_joycon_calibration_bias(&mut self, index: usize, x: f32, y: f32, z: f32);
    fn set_joycon_gyro(&mut self, index: usize, x: f32, y: f32, z: f32);
    fn set_joycon_accel(&mut self, index: usize, x: f32, y: f32, z: f32);
    fn set_joycon_mouse_sensor(&mut self, index: usize, x: f32, y: f32, extra: f32, distance: f32);
    fn take_joycon_calibration_requests(&mut self) -> Vec<usize>;
    fn take_joycon_rumble_requests(&mut self) -> Vec<JoyConRumbleRequest>;
    fn take_joycon_indicator_requests(&mut self) -> Vec<JoyConIndicatorRequest>;
    fn for_each_player_binding(&self, f: &mut dyn FnMut(usize, PlayerBinding));
    fn bind_player(&mut self, index: usize, binding: PlayerBinding);
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
impl<B: GraphicsBackend> JoyConSink for App<B> {
    fn set_joycon_button_state(&mut self, index: usize, button: JoyConButton, is_down: bool) {
        App::set_joycon_button_state(self, index, button, is_down);
    }
    fn set_joycon_stick(&mut self, index: usize, x: f32, y: f32) {
        App::set_joycon_stick(self, index, x, y);
    }
    fn set_joycon_stick_unit(&mut self, index: usize, stick: SignedUnitVector2) {
        App::set_joycon_stick_unit(self, index, stick);
    }
    fn set_joycon_side(&mut self, index: usize, side: JoyConSide) {
        App::set_joycon_side(self, index, side);
    }
    fn set_joycon_connected(&mut self, index: usize, connected: bool) {
        App::set_joycon_connected(self, index, connected);
    }
    fn set_joycon_calibrated(&mut self, index: usize, calibrated: bool) {
        App::set_joycon_calibrated(self, index, calibrated);
    }
    fn set_joycon_calibration_in_progress(&mut self, index: usize, in_progress: bool) {
        App::set_joycon_calibration_in_progress(self, index, in_progress);
    }
    fn set_joycon_calibration_bias(&mut self, index: usize, x: f32, y: f32, z: f32) {
        App::set_joycon_calibration_bias(self, index, x, y, z);
    }
    fn set_joycon_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        App::set_joycon_gyro(self, index, x, y, z);
    }
    fn set_joycon_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        App::set_joycon_accel(self, index, x, y, z);
    }
    fn set_joycon_mouse_sensor(&mut self, index: usize, x: f32, y: f32, extra: f32, distance: f32) {
        App::set_joycon_mouse_sensor(self, index, x, y, extra, distance);
    }
    fn take_joycon_calibration_requests(&mut self) -> Vec<usize> {
        App::take_joycon_calibration_requests(self)
    }
    fn take_joycon_rumble_requests(&mut self) -> Vec<JoyConRumbleRequest> {
        App::take_joycon_rumble_requests(self)
    }
    fn take_joycon_indicator_requests(&mut self) -> Vec<JoyConIndicatorRequest> {
        App::take_joycon_indicator_requests(self)
    }
    fn for_each_player_binding(&self, f: &mut dyn FnMut(usize, PlayerBinding)) {
        for (index, player) in self.players().iter().enumerate() {
            f(index, player.get_binding());
        }
    }
    fn bind_player(&mut self, index: usize, binding: PlayerBinding) {
        App::bind_player(self, index, binding);
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
impl JoyConSink for RenderThreadBridge {
    fn set_joycon_button_state(&mut self, index: usize, button: JoyConButton, is_down: bool) {
        self.push_input_event(InputEvent::JoyConButton {
            index,
            button,
            is_down,
        });
    }
    fn set_joycon_stick(&mut self, index: usize, x: f32, y: f32) {
        self.set_joycon_stick_unit(index, SignedUnitVector2::new(x, y));
    }
    fn set_joycon_stick_unit(&mut self, index: usize, stick: SignedUnitVector2) {
        self.push_input_event(InputEvent::JoyConStick { index, stick });
    }
    fn set_joycon_side(&mut self, index: usize, side: JoyConSide) {
        self.push_input_event(InputEvent::JoyConSide { index, side });
    }
    fn set_joycon_connected(&mut self, index: usize, connected: bool) {
        self.push_input_event(InputEvent::JoyConConnected { index, connected });
    }
    fn set_joycon_calibrated(&mut self, index: usize, calibrated: bool) {
        self.push_input_event(InputEvent::JoyConCalibrated { index, calibrated });
    }
    fn set_joycon_calibration_in_progress(&mut self, index: usize, in_progress: bool) {
        self.push_input_event(InputEvent::JoyConCalibrationInProgress { index, in_progress });
    }
    fn set_joycon_calibration_bias(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.push_input_event(InputEvent::JoyConCalibrationBias { index, x, y, z });
    }
    fn set_joycon_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.push_input_event(InputEvent::JoyConGyro { index, x, y, z });
    }
    fn set_joycon_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.push_input_event(InputEvent::JoyConAccel { index, x, y, z });
    }
    fn set_joycon_mouse_sensor(&mut self, index: usize, x: f32, y: f32, extra: f32, distance: f32) {
        self.push_input_event(InputEvent::JoyConMouseSensor {
            index,
            x,
            y,
            extra,
            distance,
        });
    }
    fn take_joycon_calibration_requests(&mut self) -> Vec<usize> {
        Vec::new()
    }
    fn take_joycon_rumble_requests(&mut self) -> Vec<JoyConRumbleRequest> {
        Vec::new()
    }
    fn take_joycon_indicator_requests(&mut self) -> Vec<JoyConIndicatorRequest> {
        Vec::new()
    }
    fn for_each_player_binding(&self, _f: &mut dyn FnMut(usize, PlayerBinding)) {}
    fn bind_player(&mut self, index: usize, binding: PlayerBinding) {
        self.push_input_event(InputEvent::BindPlayer { index, binding });
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
mod joycon1;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
mod joycon2;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
mod shared;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
mod backend {
    // Public PC Joy-Con backend provenance:
    //
    // This backend exists for PC input and Perro's abstract input API. Tiernan
    // DeFranco first built this path as a standalone C++ research test project
    // in Summer 2025, then ported it to Rust and Perro. It maps raw Bluetooth
    // HID reports and BLE GATT notifications from Joy-Con devices into
    // perro_input_api controls. Public open source projects, including
    // JoyconPython and joycon2cpp, helped explain control reads, mappings,
    // player LEDs, and Joy-Con 2 rumble.
    //
    // This code does not use Nintendo SDK code, private Nintendo internals, or
    // NDA material; Tiernan does not have access to those materials at the time
    // this PC backend was written. If that access exists later, Tiernan will not
    // use it to update this public backend. Switch / Switch 2 builds will use a
    // separate private implementation that calls the official SDK directly.
    // Joy-Con 2 support here does not claim decryption work; it reads BLE
    // reports after normal OS pairing and uses observed public packet layouts.

    use super::*;
    use super::{joycon1, joycon2, shared};
    use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter};
    use btleplug::platform::Manager;
    use futures_util::stream::StreamExt;
    use hidapi::HidApi;
    use perro_input_api::{JoyConButton, JoyConSide, PlayerBinding, PlayerIndicatorSlot};
    use perro_io::data_local_dir;
    use serde::{Deserialize, Serialize};
    use shared::{ButtonBits, JoyConInputData, StickCalibration};
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::thread;
    use std::time::{Duration, Instant};
    use tokio::runtime::Builder;
    use tokio::time::{self, Duration as TokioDuration, Instant as TokioInstant};

    const REPORT_LEN: usize = 64;
    const SCAN_INTERVAL: Duration = Duration::from_secs(2);
    const READ_TIMEOUT: Duration = Duration::from_millis(8);
    const BLE_DISCOVERY_POLL: TokioDuration = TokioDuration::from_millis(20);
    const BLE_CHAR_DISCOVERY_RETRIES: u32 = 4;
    const BLE_CHAR_DISCOVERY_DELAY: TokioDuration = TokioDuration::from_millis(10);
    const IMU_ZERO_STUCK_THRESHOLD: Duration = Duration::from_millis(600);
    const IMU_ENABLE_RETRY_COOLDOWN: Duration = Duration::from_millis(250);
    const MAX_PERSISTENT_JOYCON_SLOTS: usize = 12;
    const CALIBRATION_STABLE_SECONDS: f32 = 3.0;
    const CALIBRATION_MAX_MAG_DPS: f32 = 12.0;
    const CALIBRATION_MAX_DELTA_DPS: f32 = 5.0;
    const CALIBRATION_FOLDER: &str = "Perro/calibrations";
    const MAX_JOYCON_EVENTS_PER_FRAME: usize = 256;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum CalibrationStatus {
        Missing,
        Calibrating,
        Calibrated,
    }

    #[derive(Debug, Clone, Copy)]
    struct CalibrationSession {
        started_at: Instant,
        last_sample: Option<(f32, f32, f32)>,
        sum: (f32, f32, f32),
        count: u32,
    }

    impl CalibrationSession {
        fn new() -> Self {
            Self {
                started_at: Instant::now(),
                last_sample: None,
                sum: (0.0, 0.0, 0.0),
                count: 0,
            }
        }
    }

    #[derive(Debug, Clone)]
    struct ConnectedJoyCon {
        index: usize,
        serial: String,
        output_tx: Option<Sender<DeviceCommand>>,
        stick_calibration: StickCalibration,
        stick_calibration_dirty: bool,
        last_stick_calibration_save: Instant,
        calibration_bias: (f32, f32, f32),
        status: CalibrationStatus,
        session: Option<CalibrationSession>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct GyroCalibrationFile {
        version: u32,
        bias_x: f32,
        bias_y: f32,
        bias_z: f32,
        #[serde(default = "default_stick_center")]
        stick_center_x: u16,
        #[serde(default = "default_stick_center")]
        stick_center_y: u16,
        #[serde(default = "default_stick_min")]
        stick_min_x: u16,
        #[serde(default = "default_stick_max")]
        stick_max_x: u16,
        #[serde(default = "default_stick_min")]
        stick_min_y: u16,
        #[serde(default = "default_stick_max")]
        stick_max_y: u16,
    }

    const fn default_stick_center() -> u16 {
        2048
    }

    const fn default_stick_min() -> u16 {
        300
    }

    const fn default_stick_max() -> u16 {
        3800
    }

    enum JoyConEvent {
        OutputReady {
            key: String,
            tx: Sender<DeviceCommand>,
        },
        Connected {
            index: usize,
            side: JoyConSide,
            serial: String,
            device_key: String,
        },
        Report {
            index: usize,
            side: JoyConSide,
            data: JoyConInputData,
            source: JoyConReportSource,
            raw_report: Option<RawJoyConReport>,
        },
        Disconnected {
            index: usize,
        },
    }

    #[derive(Clone, Copy)]
    enum JoyConReportSource {
        Hid,
        Ble,
    }

    impl JoyConReportSource {
        fn as_str(self) -> &'static str {
            match self {
                JoyConReportSource::Hid => "hid",
                JoyConReportSource::Ble => "ble",
            }
        }
    }

    #[derive(Clone, Copy)]
    struct RawJoyConReport {
        bytes: [u8; REPORT_LEN],
        len: usize,
    }

    impl RawJoyConReport {
        fn new(data: &[u8]) -> Option<Self> {
            if data.len() > REPORT_LEN {
                return None;
            }
            let mut bytes = [0u8; REPORT_LEN];
            bytes[..data.len()].copy_from_slice(data);
            Some(Self {
                bytes,
                len: data.len(),
            })
        }

        fn as_slice(&self) -> &[u8] {
            &self.bytes[..self.len]
        }
    }

    #[derive(Debug)]
    enum DeviceCommand {
        SetPlayerLamp {
            pattern: u8,
        },
        SetRumble {
            low_frequency: f32,
            high_frequency: f32,
        },
    }

    #[derive(Debug)]
    struct DeviceHandle {
        stop: Arc<AtomicBool>,
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
        connected: HashMap<(usize, JoyConSide), ConnectedJoyCon>,
        output_txs: HashMap<String, Sender<DeviceCommand>>,
        last_player_lamp: HashMap<usize, u8>,
        scan_connected_keys: HashSet<String>,
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
        pub(super) fn begin_frame<S: JoyConSink>(&mut self, app: &mut S) {
            self.ensure_channel();
            self.scan_if_needed(app);
            self.consume_calibration_requests(app);
            self.sync_player_binding_lamps(app);
            self.consume_output_requests(app);
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

        fn scan_if_needed<S: JoyConSink>(&mut self, app: &mut S) {
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

            self.scan_connected_keys.clear();

            for dev in api.device_list() {
                if dev.vendor_id() != joycon1::JOYCON_VENDOR_ID {
                    continue;
                }

                let pid = dev.product_id();
                let side = match pid {
                    joycon1::JOYCON_L_PID => JoyConSide::LJoyCon,
                    joycon1::JOYCON_R_PID => JoyConSide::RJoyCon,
                    _ => continue,
                };

                let Some(serial) = dev.serial_number() else {
                    continue;
                };
                let serial = serial.to_string();
                let slot_key = format!("hid:{serial}");
                self.scan_connected_keys.insert(slot_key.clone());

                if self.devices.contains_key(&slot_key) {
                    continue;
                }

                let index = assign_slot(&self.slots, &slot_key);
                log_joycon_connected(index, side, &serial);
                let _ = self.tx.as_ref().and_then(|tx| {
                    tx.send(JoyConEvent::Connected {
                        index,
                        side,
                        serial: serial.clone(),
                        device_key: slot_key.clone(),
                    })
                    .ok()
                });
                self.spawn_device_thread(slot_key, serial, pid, side, index);
            }

            // Remove disconnected devices
            self.devices.retain(|slot_key, handle| {
                let connected = self.scan_connected_keys.contains(slot_key);
                if !connected {
                    handle.stop.store(true, Ordering::Relaxed);
                    if let Some(index) = release_slot(&self.slots, slot_key) {
                        let _ = self
                            .tx
                            .as_ref()
                            .and_then(|tx| tx.send(JoyConEvent::Disconnected { index }).ok());
                        clear_joycon_index(app, index);
                        self.last_buttons.retain(|(idx, _), _| *idx != index);
                        self.connected.retain(|(idx, _), _| *idx != index);
                        self.output_txs.remove(slot_key);
                        self.last_player_lamp.remove(&index);
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
            let (cmd_tx, cmd_rx) = mpsc::channel();

            let stop = Arc::new(AtomicBool::new(false));
            let stop_thread = Arc::clone(&stop);
            let serial_thread = serial.clone();

            thread::spawn(move || {
                let Ok(api) = HidApi::new() else {
                    return;
                };

                let Ok(device) = api.open_serial(joycon1::JOYCON_VENDOR_ID, pid, &serial_thread)
                else {
                    return;
                };

                let _ = joycon1::enable_sensors(&device);
                let mut zero_started_at: Option<Instant> = None;
                let mut last_enable_retry = Instant::now() - IMU_ENABLE_RETRY_COOLDOWN;
                let mut buffer = [0u8; REPORT_LEN];
                let mut packet_number: u8 = 0;

                while !stop_thread.load(Ordering::Relaxed) {
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        match cmd {
                            DeviceCommand::SetPlayerLamp { pattern } => {
                                let _ = joycon1::send_hid_subcommand_with_rumble(
                                    &device,
                                    &mut packet_number,
                                    joycon1::JOYCON_SUBCMD_SET_PLAYER_LAMP,
                                    pattern,
                                );
                            }
                            DeviceCommand::SetRumble { .. } => {}
                        }
                    }
                    match device.read_timeout(&mut buffer, READ_TIMEOUT.as_millis() as i32) {
                        Ok(size) if size > 0 => {
                            let data = &buffer[..size];
                            if let Some(payload) = joycon1::decode_report_hid(data, side) {
                                if shared::imu_is_zero(payload.gyro, payload.accel) {
                                    let zero_start =
                                        zero_started_at.get_or_insert_with(Instant::now);
                                    if zero_start.elapsed() >= IMU_ZERO_STUCK_THRESHOLD
                                        && last_enable_retry.elapsed() >= IMU_ENABLE_RETRY_COOLDOWN
                                    {
                                        eprintln!(
                                            "[joycon] imu stuck at zero index={} side={:?}, retrying sensor enable",
                                            index, side
                                        );
                                        let _ = joycon1::enable_sensors(&device);
                                        last_enable_retry = Instant::now();
                                    }
                                } else {
                                    zero_started_at = None;
                                }
                                let _ = tx.send(JoyConEvent::Report {
                                    index,
                                    side,
                                    data: payload,
                                    source: JoyConReportSource::Hid,
                                    raw_report: raw_dump_enabled()
                                        .then(|| RawJoyConReport::new(data))
                                        .flatten(),
                                });
                            }
                        }
                        _ => {}
                    }
                }

                let _ = tx.send(JoyConEvent::Disconnected { index });
            });

            self.devices.insert(slot_key.clone(), DeviceHandle { stop });
            self.output_txs.insert(slot_key, cmd_tx);
        }

        fn drain_events<S: JoyConSink>(&mut self, app: &mut S) {
            let Some(rx) = self.rx.take() else {
                return;
            };

            for _ in 0..MAX_JOYCON_EVENTS_PER_FRAME {
                let Ok(event) = rx.try_recv() else {
                    break;
                };
                self.handle_event(app, event);
            }

            self.rx = Some(rx);
        }

        fn handle_event<S: JoyConSink>(&mut self, app: &mut S, event: JoyConEvent) {
            match event {
                JoyConEvent::OutputReady { key, tx } => {
                    self.output_txs.insert(key, tx);
                }
                JoyConEvent::Connected {
                    index,
                    side,
                    serial,
                    device_key,
                } => {
                    self.on_connected(app, index, side, serial, device_key);
                }
                JoyConEvent::Report {
                    index,
                    side,
                    data,
                    source,
                    raw_report,
                } => {
                    if let Some(raw_report) = raw_report.as_ref() {
                        log_raw_joycon_report(
                            index,
                            side,
                            raw_report.as_slice(),
                            &data,
                            source.as_str(),
                        );
                    }
                    apply_report(
                        app,
                        index,
                        side,
                        data,
                        &mut self.last_buttons,
                        &mut self.connected,
                    );
                }
                JoyConEvent::Disconnected { index } => {
                    clear_joycon_index(app, index);
                    self.last_buttons.retain(|(idx, _), _| *idx != index);
                    self.connected.retain(|(idx, _), _| *idx != index);
                    self.last_player_lamp.remove(&index);
                }
            }
        }

        fn consume_calibration_requests<S: JoyConSink>(&mut self, app: &mut S) {
            let requests = app.take_joycon_calibration_requests();
            for index in requests {
                self.start_calibration(app, index);
            }
        }

        fn consume_output_requests<S: JoyConSink>(&mut self, app: &mut S) {
            for req in app.take_joycon_rumble_requests() {
                self.apply_rumble(
                    req.index,
                    req.rumble.low_frequency,
                    req.rumble.high_frequency,
                );
            }
            for req in app.take_joycon_indicator_requests() {
                self.apply_indicator(req.index, req.indicator.to_lamp_pattern());
            }
        }

        fn apply_rumble(&mut self, index: usize, low_frequency: f32, high_frequency: f32) {
            for controller in self.connected.values() {
                if controller.index != index {
                    continue;
                }
                if let Some(tx) = controller.output_tx.as_ref() {
                    let _ = tx.send(DeviceCommand::SetRumble {
                        low_frequency,
                        high_frequency,
                    });
                }
            }
        }

        fn apply_indicator(&mut self, index: usize, indicator: u8) {
            self.last_player_lamp.insert(index, indicator);
            for controller in self.connected.values() {
                if controller.index != index {
                    continue;
                }
                if let Some(tx) = controller.output_tx.as_ref() {
                    let _ = tx.send(DeviceCommand::SetPlayerLamp { pattern: indicator });
                }
            }
        }

        fn sync_player_binding_lamps<S: JoyConSink>(&mut self, app: &mut S) {
            let mut desired = [None; MAX_PERSISTENT_JOYCON_SLOTS];
            app.for_each_player_binding(&mut |player_idx, binding| {
                let Some(indicator) = PlayerIndicatorSlot::from_player_number(player_idx + 1)
                else {
                    return;
                };
                let pattern = indicator.to_lamp_pattern();
                match binding {
                    PlayerBinding::JoyConSingle { index } => {
                        if let Some(slot) = desired.get_mut(index) {
                            *slot = Some(pattern);
                        } else if self.last_player_lamp.get(&index).copied() != Some(pattern) {
                            self.apply_indicator(index, pattern);
                        }
                    }
                    PlayerBinding::JoyConPair { left, right } => {
                        for index in [left, right] {
                            if let Some(slot) = desired.get_mut(index) {
                                *slot = Some(pattern);
                            } else if self.last_player_lamp.get(&index).copied() != Some(pattern) {
                                self.apply_indicator(index, pattern);
                            }
                        }
                    }
                    _ => {}
                }
            });
            for (index, pattern) in desired.into_iter().enumerate() {
                let Some(pattern) = pattern else {
                    continue;
                };
                if self.last_player_lamp.get(&index).copied() == Some(pattern) {
                    continue;
                }
                self.apply_indicator(index, pattern);
            }
        }

        fn start_calibration<S: JoyConSink>(&mut self, app: &mut S, index: usize) {
            if let Some((_, controller)) = self
                .connected
                .iter_mut()
                .find(|((idx, _), _)| *idx == index)
            {
                controller.status = CalibrationStatus::Calibrating;
                controller.session = Some(CalibrationSession::new());
                app.set_joycon_calibration_in_progress(index, true);
                app.set_joycon_calibrated(index, false);
                app.set_joycon_calibration_bias(index, 0.0, 0.0, 0.0);
            }
        }

        fn on_connected<S: JoyConSink>(
            &mut self,
            app: &mut S,
            index: usize,
            side: JoyConSide,
            serial: String,
            device_key: String,
        ) {
            let output_tx = self.output_txs.remove(&device_key);
            let file = load_calibration_file(&serial);
            let (status, bias, stick_calibration) = match file {
                Some(f) => {
                    let stick = StickCalibration {
                        center_x: f.stick_center_x,
                        center_y: f.stick_center_y,
                        min_x: f.stick_min_x,
                        max_x: f.stick_max_x,
                        min_y: f.stick_min_y,
                        max_y: f.stick_max_y,
                    };
                    (
                        CalibrationStatus::Calibrated,
                        (f.bias_x, f.bias_y, f.bias_z),
                        stick,
                    )
                }
                None => (
                    CalibrationStatus::Missing,
                    (0.0, 0.0, 0.0),
                    StickCalibration::default(),
                ),
            };
            eprintln!(
                "[joycon] calibration index={} side={:?} serial={} calibrated={} path={}",
                index,
                side,
                serial,
                status == CalibrationStatus::Calibrated,
                calibration_path_display(&serial)
            );
            self.connected.insert(
                (index, side),
                ConnectedJoyCon {
                    index,
                    serial,
                    output_tx,
                    stick_calibration,
                    stick_calibration_dirty: false,
                    last_stick_calibration_save: Instant::now(),
                    calibration_bias: bias,
                    status,
                    session: None,
                },
            );
            app.set_joycon_side(index, side);
            app.set_joycon_connected(index, true);
            app.set_joycon_calibration_in_progress(index, false);
            app.set_joycon_calibrated(index, status == CalibrationStatus::Calibrated);
            app.set_joycon_calibration_bias(index, bias.0, bias.1, bias.2);
            if !is_joycon_bound(app, index)
                && let Some(player_index) = first_unbound_player_slot(app)
            {
                app.bind_player(player_index, PlayerBinding::JoyConSingle { index });
            }
            if let Some(indicator) = PlayerIndicatorSlot::from_player_number(index + 1) {
                self.apply_indicator(index, indicator.to_lamp_pattern());
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
                    if let Ok(sl) = slots.try_lock() {
                        known.retain(|k| sl.assigned.contains_key(k));
                    }
                    time::sleep(BLE_DISCOVERY_POLL).await;

                    let Ok(peripherals) = adapter.peripherals().await else {
                        continue;
                    };

                    for peripheral in peripherals {
                        let Some((side, serial, debug_tag)) = joycon2::classify_joycon2_ble(&peripheral).await else {
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
                        let preferred_uuids: [uuid::Uuid; 3] = match side {
                            JoyConSide::LJoyCon => [
                                joycon2::JOYCON2_INPUT_REPORT_07_UUID,
                                joycon2::JOYCON2_INPUT_REPORT_05_UUID,
                                joycon2::JOYCON2_INPUT_REPORT_08_UUID,
                            ],
                            JoyConSide::RJoyCon => [
                                joycon2::JOYCON2_INPUT_REPORT_08_UUID,
                                joycon2::JOYCON2_INPUT_REPORT_05_UUID,
                                joycon2::JOYCON2_INPUT_REPORT_07_UUID,
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
                                            && (c.uuid == joycon2::JOYCON2_INPUT_REPORT_05_UUID
                                                || c.uuid == joycon2::JOYCON2_INPUT_REPORT_07_UUID
                                                || c.uuid == joycon2::JOYCON2_INPUT_REPORT_08_UUID)
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
                            .find(|c| c.uuid == joycon2::JOYCON2_WRITE_COMMAND_UUID)
                            .cloned();
                        let vibration_char = chars
                            .iter()
                            .find(|c| {
                                c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
                                    && c.uuid
                                        == match side {
                                            JoyConSide::LJoyCon => joycon2::JOYCON2_VIBRATION_L_UUID,
                                            JoyConSide::RJoyCon => joycon2::JOYCON2_VIBRATION_R_UUID,
                                        }
                            })
                            .cloned()
                            .or_else(|| {
                                chars
                                    .iter()
                                    .find(|c| {
                                        c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
                                            && (c.uuid == joycon2::JOYCON2_VIBRATION_L_UUID
                                                || c.uuid == joycon2::JOYCON2_VIBRATION_R_UUID)
                                    })
                                    .cloned()
                            });
                        let rumble_char = chars
                            .iter()
                            .find(|c| {
                                c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
                                    && c.uuid
                                        == match side {
                                            JoyConSide::LJoyCon => joycon2::JOYCON2_RUMBLE_L_UUID,
                                            JoyConSide::RJoyCon => joycon2::JOYCON2_RUMBLE_R_UUID,
                                        }
                            })
                            .cloned();
                        if let Some(cmd_char) = cmd_char.as_ref() {
                            joycon2::send_joycon2_enable_sequence(&peripheral, cmd_char).await;
                            // Aggressive startup: fire a second pulse immediately.
                            joycon2::send_joycon2_enable_sequence(&peripheral, cmd_char).await;
                        }
                        if let Some(init_char) = rumble_char.as_ref().or(cmd_char.as_ref()) {
                            joycon2::send_joycon2_full_init_sequence(&peripheral, init_char).await;
                        }

                        let index = assign_slot(&slots, &key);
                        let (cmd_tx, cmd_rx) = mpsc::channel();
                        let _ = tx.send(JoyConEvent::OutputReady {
                            key: key.clone(),
                            tx: cmd_tx,
                        });
                        let _ = tx.send(JoyConEvent::Connected {
                            index,
                            side,
                            serial: serial.clone(),
                            device_key: key.clone(),
                        });
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
                            let mut rumble_counter = 0u8;
                            let mut zero_started_at: Option<TokioInstant> = None;
                            let mut last_enable_retry =
                                TokioInstant::now() - TokioDuration::from_millis(250);
                            let mut first_report_logged = false;
                            while !stop_clone.load(Ordering::Relaxed) {
                                while let Ok(cmd) = cmd_rx.try_recv() {
                                    match cmd {
                                        DeviceCommand::SetPlayerLamp { pattern } => {
                                            if let Some(cmd_char) = cmd_char.as_ref() {
                                                joycon2::send_joycon2_player_lamp(
                                                    &peripheral,
                                                    cmd_char,
                                                    pattern,
                                                )
                                                .await;
                                            }
                                        }
                                        DeviceCommand::SetRumble {
                                            low_frequency,
                                            high_frequency,
                                        } => {
                                            if let Some(vibration_char) = vibration_char.as_ref() {
                                                joycon2::send_joycon2_rumble(
                                                    &peripheral,
                                                    vibration_char,
                                                    &mut rumble_counter,
                                                    low_frequency.max(high_frequency),
                                                )
                                                .await;
                                            }
                                        }
                                    }
                                }
                                match time::timeout(TokioDuration::from_millis(8), notifications.next()).await {
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
                                        if let Some(data) = joycon2::decode_report_ble(&packet.value, side) {
                                            let imu_zero = shared::imu_is_zero(data.gyro, data.accel);
                                            if !imu_zero {
                                                if !imu_active {
                                                    eprintln!(
                                                        "[joycon2] imu_active id={} t={}ms",
                                                        key_clone,
                                                        connect_t0.elapsed().as_millis()
                                                    );
                                                }
                                                imu_active = true;
                                                zero_started_at = None;
                                            } else {
                                                let zero_start =
                                                    zero_started_at.get_or_insert_with(TokioInstant::now);
                                                if zero_start.elapsed()
                                                    >= TokioDuration::from_millis(600)
                                                    && last_enable_retry.elapsed()
                                                        >= TokioDuration::from_millis(250)
                                                {
                                                    if let Some(cmd_char) = cmd_char.as_ref() {
                                                        eprintln!(
                                                            "[joycon2] imu stuck at zero, retrying enable id={key_clone}"
                                                        );
                                                        joycon2::send_joycon2_enable_sequence(
                                                            &peripheral,
                                                            cmd_char,
                                                        )
                                                        .await;
                                                    }
                                                    last_enable_retry = TokioInstant::now();
                                                }
                                            }
                                            let dump_raw = raw_dump_enabled();
                                            let _ = tx_clone.send(JoyConEvent::Report {
                                                index,
                                                side,
                                                data,
                                                source: JoyConReportSource::Ble,
                                                raw_report: dump_raw
                                                    .then(|| RawJoyConReport::new(&packet.value))
                                                    .flatten(),
                                            });
                                        } else if raw_dump_enabled() {
                                            eprintln!(
                                                "[joycon2][stream] undecoded id={} side={:?} report=0x{:02X} len={}",
                                                key_clone,
                                                side,
                                                rid,
                                                packet.value.len()
                                            );
                                        }
                                    }
                                    Ok(None) => {
                                        eprintln!("[joycon2] notifications timeout/ended id={key_clone}");
                                        break;
                                    }
                                    Err(_) => {}
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

    #[inline(always)]
    fn apply_report<S: JoyConSink>(
        app: &mut S,
        index: usize,
        side: JoyConSide,
        data: JoyConInputData,
        last_buttons: &mut HashMap<(usize, JoyConSide), ButtonBits>,
        connected: &mut HashMap<(usize, JoyConSide), ConnectedJoyCon>,
    ) {
        let key = (index, side);
        let prev = last_buttons.get(&key).copied();

        apply_buttons(app, index, data.buttons, prev);
        last_buttons.insert(key, data.buttons);

        let stick = connected
            .get_mut(&key)
            .and_then(|controller| {
                data.raw_stick.map(|raw| {
                    if matches!(controller.status, CalibrationStatus::Missing) {
                        controller.stick_calibration.set_center(raw);
                        controller.stick_calibration_dirty = true;
                    }
                    if controller.stick_calibration.learn_extents(raw) {
                        controller.stick_calibration_dirty = true;
                    }
                    if controller.stick_calibration_dirty
                        && controller.last_stick_calibration_save.elapsed()
                            >= Duration::from_secs(5)
                    {
                        save_calibration_file(
                            &controller.serial,
                            GyroCalibrationFile {
                                version: 1,
                                bias_x: controller.calibration_bias.0,
                                bias_y: controller.calibration_bias.1,
                                bias_z: controller.calibration_bias.2,
                                stick_center_x: controller.stick_calibration.center_x,
                                stick_center_y: controller.stick_calibration.center_y,
                                stick_min_x: controller.stick_calibration.min_x,
                                stick_max_x: controller.stick_calibration.max_x,
                                stick_min_y: controller.stick_calibration.min_y,
                                stick_max_y: controller.stick_calibration.max_y,
                            },
                        );
                        controller.stick_calibration_dirty = false;
                        controller.last_stick_calibration_save = Instant::now();
                    }
                    controller.stick_calibration.normalize(raw)
                })
            })
            .unwrap_or(data.stick);

        app.set_joycon_side(index, side);
        app.set_joycon_connected(index, true);
        app.set_joycon_stick_unit(index, stick);
        if let Some(mouse) = data.mouse {
            app.set_joycon_mouse_sensor(index, mouse.x, mouse.y, mouse.extra, mouse.distance);
        }
        let gyro = stabilize_gyro(app, key, data.gyro, connected);
        app.set_joycon_gyro(index, gyro.0, gyro.1, gyro.2);
        app.set_joycon_accel(index, data.accel.0, data.accel.1, data.accel.2);
    }

    fn stabilize_gyro<S: JoyConSink>(
        app: &mut S,
        key: (usize, JoyConSide),
        raw_gyro: (f32, f32, f32),
        connected: &mut HashMap<(usize, JoyConSide), ConnectedJoyCon>,
    ) -> (f32, f32, f32) {
        let Some(controller) = connected.get_mut(&key) else {
            return raw_gyro;
        };

        if controller.status == CalibrationStatus::Calibrating
            && let Some(session) = controller.session.as_mut()
        {
            let mag = (raw_gyro.0 * raw_gyro.0 + raw_gyro.1 * raw_gyro.1 + raw_gyro.2 * raw_gyro.2)
                .sqrt();
            let delta = session
                .last_sample
                .map(|prev| {
                    let dx = raw_gyro.0 - prev.0;
                    let dy = raw_gyro.1 - prev.1;
                    let dz = raw_gyro.2 - prev.2;
                    (dx * dx + dy * dy + dz * dz).sqrt()
                })
                .unwrap_or(0.0);

            let sample_is_steady =
                mag <= CALIBRATION_MAX_MAG_DPS && delta <= CALIBRATION_MAX_DELTA_DPS;
            if sample_is_steady {
                session.last_sample = Some(raw_gyro);
                session.sum.0 += raw_gyro.0;
                session.sum.1 += raw_gyro.1;
                session.sum.2 += raw_gyro.2;
                session.count = session.count.saturating_add(1);
            } else {
                *session = CalibrationSession::new();
            }

            if session.started_at.elapsed().as_secs_f32() >= CALIBRATION_STABLE_SECONDS
                && session.count > 0
            {
                let inv = 1.0 / (session.count as f32);
                let bias = (
                    session.sum.0 * inv,
                    session.sum.1 * inv,
                    session.sum.2 * inv,
                );
                controller.calibration_bias = bias;
                controller.status = CalibrationStatus::Calibrated;
                controller.session = None;
                save_calibration_file(
                    &controller.serial,
                    GyroCalibrationFile {
                        version: 1,
                        bias_x: bias.0,
                        bias_y: bias.1,
                        bias_z: bias.2,
                        stick_center_x: controller.stick_calibration.center_x,
                        stick_center_y: controller.stick_calibration.center_y,
                        stick_min_x: controller.stick_calibration.min_x,
                        stick_max_x: controller.stick_calibration.max_x,
                        stick_min_y: controller.stick_calibration.min_y,
                        stick_max_y: controller.stick_calibration.max_y,
                    },
                );
                app.set_joycon_calibration_in_progress(controller.index, false);
                app.set_joycon_calibrated(controller.index, true);
                app.set_joycon_calibration_bias(controller.index, bias.0, bias.1, bias.2);
            } else {
                app.set_joycon_calibration_in_progress(controller.index, true);
                app.set_joycon_calibrated(controller.index, false);
            }
        }

        let x = raw_gyro.0 - controller.calibration_bias.0;
        let y = raw_gyro.1 - controller.calibration_bias.1;
        let z = raw_gyro.2 - controller.calibration_bias.2;
        (x, y, z)
    }

    fn clear_joycon_index<S: JoyConSink>(app: &mut S, index: usize) {
        for button in ALL_BUTTONS {
            app.set_joycon_button_state(index, button, false);
        }
        app.set_joycon_connected(index, false);
        app.set_joycon_calibration_in_progress(index, false);
        app.set_joycon_calibrated(index, false);
        app.set_joycon_calibration_bias(index, 0.0, 0.0, 0.0);
        app.set_joycon_stick(index, 0.0, 0.0);
        app.set_joycon_mouse_sensor(index, 0.0, 0.0, 0.0, 0.0);
        app.set_joycon_gyro(index, 0.0, 0.0, 0.0);
        app.set_joycon_accel(index, 0.0, 0.0, 0.0);
    }

    fn apply_buttons<S: JoyConSink>(
        app: &mut S,
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

    #[inline(always)]
    fn is_joycon_bound<S: JoyConSink>(app: &S, joycon_index: usize) -> bool {
        let mut found = false;
        app.for_each_player_binding(&mut |_, binding| match binding {
            PlayerBinding::JoyConSingle { index } if index == joycon_index => found = true,
            PlayerBinding::JoyConPair { left, right }
                if left == joycon_index || right == joycon_index =>
            {
                found = true;
            }
            _ => {}
        });
        found
    }

    fn first_unbound_player_slot<S: JoyConSink>(app: &S) -> Option<usize> {
        let mut first_empty = None;
        let mut len = 0;
        app.for_each_player_binding(&mut |idx, binding| {
            len = idx + 1;
            if first_empty.is_none() && matches!(binding, PlayerBinding::None) {
                first_empty = Some(idx);
            }
        });
        if first_empty.is_some() {
            first_empty
        } else if len < 8 {
            Some(len)
        } else {
            None
        }
    }

    fn calibration_path(serial: &str) -> Option<std::path::PathBuf> {
        let safe_serial: String = serial
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let mut path = data_local_dir()?;
        for part in CALIBRATION_FOLDER.split('/') {
            path.push(part);
        }
        path.push(format!("{safe_serial}.cal"));
        Some(path)
    }

    fn calibration_path_display(serial: &str) -> String {
        calibration_path(serial)
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| format!("{CALIBRATION_FOLDER}/{serial}.cal"))
    }

    fn load_calibration_file(serial: &str) -> Option<GyroCalibrationFile> {
        let path = calibration_path(serial)?;
        let bytes = std::fs::read(path).ok()?;
        serde_json::from_slice::<GyroCalibrationFile>(&bytes).ok()
    }

    fn save_calibration_file(serial: &str, calibration: GyroCalibrationFile) {
        let Some(path) = calibration_path(serial) else {
            return;
        };
        if let Ok(data) = serde_json::to_vec_pretty(&calibration) {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, data);
        }
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
        let stick = parsed.stick.to_tuple();
        eprintln!(
            "[joycon][raw] src={} index={} side={:?} report=0x{:02X} len={} bytes={} buttons=0x{:04X} stick=({:.3},{:.3}) gyro=({:.1},{:.1},{:.1}) accel=({:.1},{:.1},{:.1})",
            source,
            index,
            side,
            report_id,
            raw.len(),
            HexBytes(raw),
            parsed.buttons,
            stick.0,
            stick.1,
            parsed.gyro.0,
            parsed.gyro.1,
            parsed.gyro.2,
            parsed.accel.0,
            parsed.accel.1,
            parsed.accel.2
        );
    }

    struct HexBytes<'a>(&'a [u8]);

    impl std::fmt::Display for HexBytes<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for (index, byte) in self.0.iter().enumerate() {
                if index != 0 {
                    f.write_str(" ")?;
                }
                write!(f, "{byte:02X}")?;
            }
            Ok(())
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod backend {
    #[derive(Default)]
    pub struct JoyConBackend;

    impl JoyConBackend {
        pub fn begin_frame<S>(&mut self, _app: &mut S) {}
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

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub fn begin_frame_threaded(&mut self, bridge: &RenderThreadBridge) {
        let mut bridge = bridge.clone();
        self.backend.begin_frame(&mut bridge);
    }

    #[cfg(all(
        not(target_arch = "wasm32"),
        not(any(target_os = "windows", target_os = "linux", target_os = "macos"))
    ))]
    pub fn begin_frame_threaded(&mut self, _bridge: &RenderThreadBridge) {}
}

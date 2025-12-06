//! Controller manager for handling multiple controllers
//!
//! Provides a unified API for scanning, connecting, and polling controllers.

use crate::input::joycon::{self, JoyCon, JoyCon2, InputReport, Calibration, CalibrationSample, JoyConError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::time::{Duration, interval};
use tokio::runtime::Handle;

/// Represents a connected Joy-Con controller
#[derive(Debug, Clone)]
pub struct ConnectedJoyCon {
    /// Serial number or address identifier
    pub serial: String,
    /// Whether this is a left Joy-Con
    pub is_left: bool,
    /// Whether this is a Joy-Con 2 (BLE) or Joy-Con 1 (HID)
    pub is_joycon2: bool,
    /// Latest input report
    pub latest_report: Option<InputReport>,
    /// Calibration data
    pub calibration: Calibration,
}

/// Helper to recover from poisoned mutexes
fn lock_mutex<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("[ControllerManager] WARNING: Mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    }
}

/// Controller manager that handles scanning, connecting, and polling controllers
pub struct ControllerManager {
    /// Connected Joy-Con 1 devices (HID) - stored separately to avoid Send/Sync issues
    joycon1_devices: Mutex<HashMap<String, JoyCon>>,
    /// Connected Joy-Con 2 devices (BLE) - use TokioMutex for async-safe locking
    joycon2_devices: Arc<TokioMutex<HashMap<String, JoyCon2>>>,
    /// Latest data from all connected controllers
    controller_data: Arc<Mutex<HashMap<String, ConnectedJoyCon>>>,
    /// Whether polling is enabled
    polling_enabled: Arc<Mutex<bool>>,
    /// Channel for sending input reports
    report_tx: Option<mpsc::UnboundedSender<(String, InputReport)>>,
    /// Handle to the polling task
    polling_handle: Option<tokio::task::JoinHandle<()>>,
    /// Tokio runtime handle for spawning async tasks
    runtime_handle: Handle,
    /// Channel for Joy-Con 1 reports (from background thread)
    joycon1_report_rx: Option<std::sync::mpsc::Receiver<(String, InputReport)>>,
    /// Handle to Joy-Con 1 polling thread
    joycon1_polling_handle: Option<std::thread::JoinHandle<()>>,
}

impl ControllerManager {
    /// Create a new controller manager
    pub fn new() -> Self {
        eprintln!("[ControllerManager::new] Creating ControllerManager...");
        
        // Try to get the current runtime handle, or create a new runtime
        let runtime_handle = match Handle::try_current() {
            Ok(handle) => {
                eprintln!("[ControllerManager::new] Using existing Tokio runtime handle");
                handle
            },
            Err(_) => {
                eprintln!("[ControllerManager::new] No Tokio runtime found, creating one in background thread...");
                // No runtime exists, create one in a background thread
                let (tx, rx) = std::sync::mpsc::channel();
                let thread_result = std::thread::Builder::new()
                    .name("joycon-runtime".to_string())
                    .spawn(move || {
                        eprintln!("[ControllerManager::new] Runtime thread started");
                        let rt = match tokio::runtime::Runtime::new() {
                            Ok(r) => {
                                eprintln!("[ControllerManager::new] Tokio runtime created in thread");
                                r
                            },
                            Err(e) => {
                                eprintln!("[ControllerManager::new] ERROR: Failed to create Tokio runtime: {:?}", e);
                                let _ = tx.send(Err(format!("Failed to create runtime: {:?}", e)));
                                return;
                            }
                        };
                        let handle = rt.handle().clone();
                        if tx.send(Ok(handle.clone())).is_err() {
                            eprintln!("[ControllerManager::new] ERROR: Failed to send runtime handle (receiver dropped)");
                            return;
                        }
                        eprintln!("[ControllerManager::new] Tokio runtime handle sent, keeping runtime alive...");
                        rt.block_on(async {
                            // Keep the runtime alive
                            loop {
                                tokio::time::sleep(Duration::from_secs(3600)).await;
                            }
                        });
                    });
                
                match thread_result {
                    Ok(_) => {
                        eprintln!("[ControllerManager::new] Runtime thread spawned successfully");
                    },
                    Err(e) => {
                        eprintln!("[ControllerManager::new] ERROR: Failed to spawn runtime thread: {:?}", e);
                        panic!("Failed to spawn runtime thread: {:?}", e);
                    }
                }
            
                match rx.recv() {
                    Ok(Ok(handle)) => {
                        eprintln!("[ControllerManager::new] Successfully received runtime handle");
                        handle
                    },
                    Ok(Err(e)) => {
                        eprintln!("[ControllerManager::new] ERROR: Runtime creation failed: {}", e);
                        panic!("Runtime creation failed: {}", e);
                    },
                    Err(e) => {
                        eprintln!("[ControllerManager::new] ERROR: Failed to receive runtime handle: {:?}", e);
                        panic!("Failed to receive runtime handle: {:?}", e);
                    }
                }
            }
        };
        
        eprintln!("[ControllerManager::new] Creating mutexes...");
        Self {
            joycon1_devices: Mutex::new(HashMap::new()),
            joycon2_devices: Arc::new(TokioMutex::new(HashMap::new())),
            controller_data: Arc::new(Mutex::new(HashMap::new())),
            polling_enabled: Arc::new(Mutex::new(false)),
            report_tx: None,
            polling_handle: None,
            runtime_handle,
            joycon1_report_rx: None,
            joycon1_polling_handle: None,
        }
    }

    /// Scan for Joy-Con 1 devices (HID)
    ///
    /// Returns a vector of tuples containing (serial_number, vendor_id, product_id)
    pub fn scan_joycon1(&self) -> Result<Vec<(String, u16, u16)>, JoyConError> {
        joycon::scan_joycon1_devices()
    }

    /// Scan for Joy-Con 2 devices (BLE)
    ///
    /// Returns a vector of device addresses/identifiers
    pub async fn scan_joycon2(&self) -> Result<Vec<String>, JoyConError> {
        joycon::scan_joycon2_devices().await
    }

    /// Connect to a Joy-Con 1 device
    pub fn connect_joycon1(&self, serial: &str, vid: u16, pid: u16) -> Result<(), JoyConError> {
        eprintln!("[connect_joycon1] Connecting to: serial={}, vid=0x{:04X}, pid=0x{:04X}", serial, vid, pid);
        
        eprintln!("[connect_joycon1] Creating JoyCon instance...");
        let joycon = match JoyCon::new(vid, pid, serial) {
            Ok(jc) => {
                eprintln!("[connect_joycon1] JoyCon instance created successfully");
                jc
            },
            Err(e) => {
                eprintln!("[connect_joycon1] ERROR: Failed to create JoyCon instance: {:?}", e);
                return Err(e);
            }
        };
        
        eprintln!("[connect_joycon1] Enabling sensors...");
        match joycon.enable_sensors() {
            Ok(_) => {
                eprintln!("[connect_joycon1] Sensors enabled successfully");
                // Small delay to let the Joy-Con process the command
                std::thread::sleep(std::time::Duration::from_millis(100));
            },
            Err(e) => {
                eprintln!("[connect_joycon1] ERROR: Failed to enable sensors: {:?}", e);
                return Err(e);
            }
        }
        
        let is_left = pid == joycon::JOYCON_1_LEFT_PID;
        eprintln!("[connect_joycon1] Device is {} Joy-Con", if is_left { "left" } else { "right" });
        
        eprintln!("[connect_joycon1] Adding to devices map...");
        let mut devices = lock_mutex(&self.joycon1_devices);
        devices.insert(serial.to_string(), joycon);
        eprintln!("[connect_joycon1] Device added to devices map");
        
        eprintln!("[connect_joycon1] Adding to controller_data map...");
        let mut data = lock_mutex(&self.controller_data);
        data.insert(serial.to_string(), ConnectedJoyCon {
            serial: serial.to_string(),
            is_left,
            is_joycon2: false,
            latest_report: None,
            calibration: Calibration::new(),
        });
        eprintln!("[connect_joycon1] Device added to controller_data map");
        
        eprintln!("[connect_joycon1] Connection successful!");
        Ok(())
    }

    /// Connect to a Joy-Con 2 device
    pub async fn connect_joycon2(&self, address: &str) -> Result<(), JoyConError> {
        let mut joycon2 = JoyCon2::connect(address).await?;
        joycon2.subscribe_to_inputs().await?;
        joycon2.enable_sensors().await?;
        
        let is_left = joycon2.is_left();
        let serial = joycon2.serial_number();
        
        let mut devices = self.joycon2_devices.lock().await;
        devices.insert(serial.clone(), joycon2);
        
        let mut data = self.controller_data.lock().unwrap();
        data.insert(serial.clone(), ConnectedJoyCon {
            serial,
            is_left,
            is_joycon2: true,
            latest_report: None,
            calibration: Calibration::new(),
        });
        
        Ok(())
    }

    /// Get all connected controllers' data
    ///
    /// Returns a vector of ConnectedJoyCon with the latest input reports
    pub fn get_data(&self) -> Vec<ConnectedJoyCon> {
        let data = self.controller_data.lock().unwrap();
        data.values().cloned().collect()
    }

    /// Calibrate a specific controller by serial number
    ///
    /// Collects samples over the specified duration and calculates offsets
    pub async fn calibrate(&self, serial: &str, duration_ms: u64) -> Result<(), JoyConError> {
        let mut samples = Vec::new();
        let start_time = std::time::Instant::now();
        let duration = Duration::from_millis(duration_ms);
        
        // Check if it's a Joy-Con 1 or 2
        let is_joycon2 = {
            let data = self.controller_data.lock().unwrap();
            data.get(serial)
                .map(|c| c.is_joycon2)
                .ok_or_else(|| JoyConError::DeviceNotFound)?
        };
        
        while start_time.elapsed() < duration {
            if is_joycon2 {
                let devices = self.joycon2_devices.lock().await;
                if let Some(joycon2) = devices.get(serial) {
                    match joycon2.read_notifications().await {
                        Ok(data) => {
                            let is_left = joycon2.is_left();
                            if let Ok(report) = InputReport::decode_joycon2(&data, is_left) {
                                samples.push(CalibrationSample::new(report.gyro, report.accel, report.stick));
                            }
                        }
                        Err(_) => {}
                    }
                }
            } else {
                let devices = self.joycon1_devices.lock().unwrap();
                if let Some(joycon) = devices.get(serial) {
                    let mut buffer = [0u8; 64];
                    if let Ok(bytes) = joycon.read_input_report(&mut buffer) {
                        if bytes > 0 {
                            if let Ok(report) = InputReport::decode(&buffer[..bytes], joycon.product_id()) {
                                samples.push(CalibrationSample::new(report.gyro, report.accel, report.stick));
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        if samples.is_empty() {
            return Err(JoyConError::Other("No samples collected during calibration".to_string()));
        }
        
        let calibration = Calibration::calculate_offset(&samples);
        
        let mut data = self.controller_data.lock().unwrap();
        if let Some(controller) = data.get_mut(serial) {
            controller.calibration = calibration;
        }
        
        Ok(())
    }

    /// Enable polling - starts async polling for all connected controllers
    pub fn enable_polling(&mut self) -> Result<(), JoyConError> {
        let mut enabled = self.polling_enabled.lock().unwrap();
        if *enabled {
            return Ok(()); // Already enabled
        }
        *enabled = true;
        drop(enabled);
        
        let (tx, _rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();
        self.report_tx = Some(tx);
        
        // Start Joy-Con 2 polling (BLE devices)
        let joycon2_devices = Arc::clone(&self.joycon2_devices);
        let controller_data = Arc::clone(&self.controller_data);
        let polling_enabled = Arc::clone(&self.polling_enabled);
        
        let handle = self.runtime_handle.spawn(async move {
            let mut interval = interval(Duration::from_millis(25)); // ~40Hz polling
            
            loop {
                interval.tick().await;
                
                // Check if polling is still enabled
                {
                    let enabled = polling_enabled.lock().unwrap();
                    if !*enabled {
                        break;
                    }
                }
                
                // Poll Joy-Con 2 devices (BLE) - use notification stream
                // Collect serials first, then lock per device to avoid borrowing issues
                let serials: Vec<String> = {
                    let devices = joycon2_devices.lock().await;
                    devices.keys().cloned().collect()
                };
                
                for serial in serials {
                    // Lock, get device info, unlock, then create future that locks again
                    let (is_left, has_device) = {
                        let devices = joycon2_devices.lock().await;
                        if let Some(joycon2) = devices.get(&serial) {
                            (joycon2.is_left(), true)
                        } else {
                            (false, false)
                        }
                    };
                    
                    if !has_device {
                        continue;
                    }
                    
                    // Create future that locks, gets device, and reads notification
                    let read_result = {
                        let devices_clone = Arc::clone(&joycon2_devices);
                        let serial_clone = serial.clone();
                        tokio::time::timeout(
                            Duration::from_millis(10),
                            async move {
                                let devices = devices_clone.lock().await;
                                if let Some(joycon2) = devices.get(&serial_clone) {
                                    joycon2.read_notifications().await
                                } else {
                                    Err(JoyConError::DeviceNotFound)
                                }
                            }
                        ).await
                    };
                    
                    if let Ok(Ok(data)) = read_result {
                        if let Ok(report) = InputReport::decode_joycon2(&data, is_left) {
                            // Apply calibration
                            let mut data = controller_data.lock().unwrap();
                            if let Some(controller) = data.get_mut(&serial) {
                                let (calibrated_gyro, _) = controller.calibration.apply(report.gyro, report.accel);
                                let mut calibrated_report = report.clone();
                                calibrated_report.gyro = calibrated_gyro;
                                controller.latest_report = Some(calibrated_report.clone());
                                
                                // Send to channel if receiver exists
                                if let Err(_) = tx_clone.send((serial.clone(), calibrated_report)) {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        });
        
        self.polling_handle = Some(handle);
        Ok(())
    }

    /// Poll Joy-Con 1 (HID) devices synchronously
    /// 
    /// This must be called from a synchronous context since HidDevice is not Send/Sync.
    /// Call this periodically (e.g., in your main game loop) to update Joy-Con 1 input data.
    /// 
    /// NOTE: This uses a blocking read (like the original project), which will wait for data.
    /// The Joy-Con sends reports when there's input change, so this will block until data arrives.
    /// This matches the behavior of the original joycon project.
    /// 
    /// WARNING: This will block the game loop until the Joy-Con sends a report.
    /// Consider calling this less frequently or only when you know input is expected.
    /// Poll Joy-Con 1 (HID) devices synchronously
    /// 
    /// Uses blocking read exactly like the original project
    pub fn poll_joycon1_sync(&self) {
        let devices = lock_mutex(&self.joycon1_devices);
        let mut data = lock_mutex(&self.controller_data);
        
        let device_count = devices.len();
        if device_count == 0 {
            return; // No devices to poll
        }
        
        for (serial, joycon) in devices.iter() {
            let mut buffer = [0u8; 64];
            
            // Use non-blocking read - returns immediately if no data available
            // This allows the update loop to run at full speed
            match joycon.read_input_report(&mut buffer) {
                Ok(bytes) => {
                    if bytes > 0 {
                        // Try to decode and store
                        match InputReport::decode(&buffer[..bytes], joycon.product_id()) {
                            Ok(report) => {
                                // Apply calibration
                                if let Some(controller) = data.get_mut(serial) {
                                    let (calibrated_gyro, calibrated_accel) = controller.calibration.apply(report.gyro, report.accel);
                                    
                                    // Apply stick calibration: subtract center offset to get values centered at 0
                                    // If center is 2000 and we read 2050, calibrated raw should be 50
                                    let stick_h_raw_calibrated = report.stick.horizontal as i32 - controller.calibration.stick_center.horizontal as i32;
                                    let stick_v_raw_calibrated = report.stick.vertical as i32 - controller.calibration.stick_center.vertical as i32;
                                    
                                    // Normalize to -1.0 to 1.0 range (estimate max range is ±1500 from center)
                                    const ESTIMATED_RANGE: f32 = 1500.0;
                                    let stick_h_norm = (stick_h_raw_calibrated as f32 / ESTIMATED_RANGE).clamp(-1.0, 1.0);
                                    let stick_v_norm = (stick_v_raw_calibrated as f32 / ESTIMATED_RANGE).clamp(-1.0, 1.0);
                                    
                                    // Apply deadzone: values between -50 and 50 raw units should be treated as 0
                                    // In normalized terms, that's 50/1500 = ~0.033
                                    const STICK_DEADZONE_RAW: i32 = 50;
                                    const STICK_DEADZONE_NORM: f32 = STICK_DEADZONE_RAW as f32 / ESTIMATED_RANGE; // ~0.033
                                    let stick_h = if stick_h_raw_calibrated.abs() < STICK_DEADZONE_RAW { 0.0 } else { stick_h_norm };
                                    let stick_v = if stick_v_raw_calibrated.abs() < STICK_DEADZONE_RAW { 0.0 } else { stick_v_norm };
                                    
                                    // For h_raw and v_raw, use the calibrated (offset) values
                                    let stick_h_raw = stick_h_raw_calibrated;
                                    let stick_v_raw = stick_v_raw_calibrated;
                                    
                                    // Apply gyro deadzone: values between -50 and 50 deg/s should be treated as 0
                                    const GYRO_DEADZONE: f32 = 50.0;
                                    let gyro_x = if calibrated_gyro.x.abs() < GYRO_DEADZONE { 0.0 } else { calibrated_gyro.x };
                                    let gyro_y = if calibrated_gyro.y.abs() < GYRO_DEADZONE { 0.0 } else { calibrated_gyro.y };
                                    let gyro_z = if calibrated_gyro.z.abs() < GYRO_DEADZONE { 0.0 } else { calibrated_gyro.z };
                                    
                                    // Build JSON with side-specific buttons
                                    let is_left = controller.is_left;
                                    let buttons_json = if is_left {
                                        serde_json::json!({
                                            "up": report.buttons.up,
                                            "down": report.buttons.down,
                                            "left": report.buttons.left,
                                            "right": report.buttons.right,
                                            "l": report.buttons.l,
                                            "zl": report.buttons.zl,
                                            "minus": report.buttons.minus,
                                            "capture": report.buttons.capture,
                                            "sl": report.buttons.sl,
                                            "sr": report.buttons.sr,
                                            "stick_press": report.buttons.stick_press,
                                        })
                                    } else {
                                        serde_json::json!({
                                            "a": report.buttons.a,
                                            "b": report.buttons.b,
                                            "x": report.buttons.x,
                                            "y": report.buttons.y,
                                            "r": report.buttons.r,
                                            "zr": report.buttons.zr,
                                            "home": report.buttons.home,
                                            "plus": report.buttons.plus,
                                            "sl": report.buttons.sl,
                                            "sr": report.buttons.sr,
                                            "stick_press": report.buttons.stick_press,
                                        })
                                    };
                                    
                                    // Store the report
                                    let mut calibrated_report = report.clone();
                                    calibrated_report.gyro = calibrated_gyro;
                                    controller.latest_report = Some(calibrated_report);
                                }
                            },
                            Err(_) => {
                                // Decode failed - ignore
                            }
                        }
                    }
                },
                Err(_) => {
                    // Read failed - ignore
                }
            }
        }
    }

    /// Disable polling - stops async polling
    pub fn disable_polling(&mut self) {
        {
            let mut enabled = self.polling_enabled.lock().unwrap();
            *enabled = false;
        }
        
        // Drop the channel to signal the polling task to stop
        self.report_tx = None;
        
        // Wait for the polling task to finish
        if let Some(handle) = self.polling_handle.take() {
            handle.abort();
        }
    }

    /// Check if polling is enabled
    pub fn is_polling_enabled(&self) -> bool {
        *self.polling_enabled.lock().unwrap()
    }
}

impl Default for ControllerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ControllerManager {
    fn drop(&mut self) {
        self.disable_polling();
    }
}


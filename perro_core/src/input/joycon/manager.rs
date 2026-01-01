//! Controller manager for handling multiple controllers
//!
//! Provides a unified API for scanning, connecting, and polling controllers.

use crate::input::joycon::{
    self, Calibration, CalibrationSample, InputReport, JoyCon, JoyCon2, JoyConError, JoyconSide,
    JoyconState, JoyconVersion,
};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};
use tokio::runtime::Handle;
use tokio::sync::{Mutex as TokioMutex, mpsc};
use tokio::time::{Duration, interval};

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
    /// Latest unified state (computed from latest_report)
    pub state: Option<JoyconState>,
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
            }
            Err(_) => {
                eprintln!(
                    "[ControllerManager::new] No Tokio runtime found, creating one in background thread..."
                );
                // No runtime exists, create one in a background thread
                let (tx, rx) = std::sync::mpsc::channel();
                let thread_result = std::thread::Builder::new()
                    .name("joycon-runtime".to_string())
                    .spawn(move || {
                        eprintln!("[ControllerManager::new] Runtime thread started");
                        // Use a multi-threaded runtime - it can handle spawn() from outside the runtime
                        // The key is to NOT block immediately - let worker threads start first
                        let rt = match tokio::runtime::Builder::new_multi_thread()
                            .worker_threads(2) // Use a small number of threads
                            .enable_all()
                            .build() {
                            Ok(r) => {
                                eprintln!("[ControllerManager::new] Tokio runtime created in thread (multi-threaded)");
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
                        // On a multi-threaded runtime, block_on blocks THIS thread but worker threads
                        // continue processing spawned tasks. We need to keep the runtime alive.
                        // Spawn a keepalive task first to ensure runtime is active
                        rt.spawn(async {
                            eprintln!("[ControllerManager::new] Keepalive task started - runtime worker threads are active!");
                            loop {
                                tokio::time::sleep(Duration::from_secs(3600)).await;
                            }
                        });
                        // Give worker threads a moment to start before blocking
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        // Now block on a never-ending future to keep the runtime alive
                        // Worker threads will continue processing spawned tasks
                        rt.block_on(async {
                            // Create a channel that will never receive anything to keep the runtime alive
                            let (_tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
                            // This will wait forever, keeping the runtime alive
                            // Worker threads will continue processing spawned tasks
                            let _ = rx.recv().await;
                        });
                    });

                match thread_result {
                    Ok(_) => {
                        eprintln!("[ControllerManager::new] Runtime thread spawned successfully");
                    }
                    Err(e) => {
                        eprintln!(
                            "[ControllerManager::new] ERROR: Failed to spawn runtime thread: {:?}",
                            e
                        );
                        panic!("Failed to spawn runtime thread: {:?}", e);
                    }
                }

                match rx.recv() {
                    Ok(Ok(handle)) => {
                        eprintln!("[ControllerManager::new] Successfully received runtime handle");
                        handle
                    }
                    Ok(Err(e)) => {
                        eprintln!(
                            "[ControllerManager::new] ERROR: Runtime creation failed: {}",
                            e
                        );
                        panic!("Runtime creation failed: {}", e);
                    }
                    Err(e) => {
                        eprintln!(
                            "[ControllerManager::new] ERROR: Failed to receive runtime handle: {:?}",
                            e
                        );
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

    /// Scan for Joy-Con 2 devices (BLE) - synchronous wrapper
    /// Blocks on the async operation using the runtime handle with a timeout
    pub fn scan_joycon2_sync(&self) -> Result<Vec<String>, JoyConError> {
        use tokio::sync::oneshot;
        // Spawn the async operation and use a oneshot channel to get the result
        let (tx, rx) = oneshot::channel();
        let handle = self.runtime_handle.clone();
        handle.spawn(async move {
            let result = joycon::scan_joycon2_devices().await;
            let _ = tx.send(result);
        });
        // Block on receiving the result with a timeout (BLE scan can take up to 5+ seconds)
        match rx.blocking_recv() {
            Ok(result) => result,
            Err(_) => {
                eprintln!(
                    "[scan_joycon2_sync] Timeout or channel error - no Joy-Con 2 devices found"
                );
                Ok(vec![]) // Return empty vec instead of error, so polling can continue
            }
        }
    }

    /// Connect to a Joy-Con 1 device
    pub fn connect_joycon1(&self, serial: &str, vid: u16, pid: u16) -> Result<(), JoyConError> {
        eprintln!(
            "[connect_joycon1] Connecting to: serial={}, vid=0x{:04X}, pid=0x{:04X}",
            serial, vid, pid
        );

        eprintln!("[connect_joycon1] Creating JoyCon instance...");
        let joycon = match JoyCon::new(vid, pid, serial) {
            Ok(jc) => {
                eprintln!("[connect_joycon1] JoyCon instance created successfully");
                jc
            }
            Err(e) => {
                eprintln!(
                    "[connect_joycon1] ERROR: Failed to create JoyCon instance: {:?}",
                    e
                );
                return Err(e);
            }
        };

        eprintln!("[connect_joycon1] Enabling sensors...");
        match joycon.enable_sensors() {
            Ok(_) => {
                eprintln!("[connect_joycon1] Sensors enabled successfully");
                // Small delay to let the Joy-Con process the command
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => {
                eprintln!("[connect_joycon1] ERROR: Failed to enable sensors: {:?}", e);
                return Err(e);
            }
        }

        let is_left = pid == joycon::JOYCON_1_LEFT_PID;
        eprintln!(
            "[connect_joycon1] Device is {} Joy-Con",
            if is_left { "left" } else { "right" }
        );

        eprintln!("[connect_joycon1] Adding to devices map...");
        let mut devices = lock_mutex(&self.joycon1_devices);
        devices.insert(serial.to_string(), joycon);
        eprintln!("[connect_joycon1] Device added to devices map");

        eprintln!("[connect_joycon1] Adding to controller_data map...");
        let mut data = lock_mutex(&self.controller_data);
        data.insert(
            serial.to_string(),
            ConnectedJoyCon {
                serial: serial.to_string(),
                is_left,
                is_joycon2: false,
                latest_report: None,
                calibration: Calibration::new(),
                state: None,
            },
        );
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
        data.insert(
            serial.clone(),
            ConnectedJoyCon {
                serial,
                is_left,
                is_joycon2: true,
                latest_report: None,
                calibration: Calibration::new(),
                state: None,
            },
        );

        Ok(())
    }

    /// Connect to a Joy-Con 2 device - synchronous wrapper
    /// Blocks on the async operation using the runtime handle
    pub fn connect_joycon2_sync(&self, address: &str) -> Result<(), JoyConError> {
        use tokio::sync::oneshot;
        // Spawn the async operation and use a oneshot channel to get the result
        let (tx, rx) = oneshot::channel();
        let handle = self.runtime_handle.clone();
        let address = address.to_string();
        let self_clone = self.controller_data.clone();
        let devices_clone = Arc::clone(&self.joycon2_devices);
        handle.spawn(async move {
            let result = async {
                let mut joycon2 = JoyCon2::connect(&address).await?;
                joycon2.subscribe_to_inputs().await?;
                joycon2.enable_sensors().await?;

                let is_left = joycon2.is_left();
                let serial = joycon2.serial_number();

                let mut devices = devices_clone.lock().await;
                devices.insert(serial.clone(), joycon2);

                let mut data = self_clone.lock().unwrap();
                data.insert(
                    serial.clone(),
                    ConnectedJoyCon {
                        serial,
                        is_left,
                        is_joycon2: true,
                        latest_report: None,
                        calibration: Calibration::new(),
                        state: None,
                    },
                );

                Ok::<(), JoyConError>(())
            }
            .await;
            let _ = tx.send(result);
        });
        // Block on receiving the result
        rx.blocking_recv()
            .unwrap_or(Err(JoyConError::Other("Channel closed".to_string())))
    }

    /// Get all connected controllers' data
    ///
    /// Returns a vector of ConnectedJoyCon with the latest input reports and unified state
    pub fn get_data(&self) -> Vec<ConnectedJoyCon> {
        let data = self.controller_data.lock().unwrap();
        data.values().cloned().collect()
    }

    /// Get all connected controllers' unified state
    ///
    /// Returns a vector of JoyconState for all connected controllers
    pub fn get_states(&self) -> Vec<JoyconState> {
        let data = self.controller_data.lock().unwrap();
        data.values().filter_map(|c| c.state.clone()).collect()
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
                                samples.push(CalibrationSample::new(
                                    report.gyro,
                                    report.accel,
                                    report.stick,
                                ));
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
                            if let Ok(report) =
                                InputReport::decode(&buffer[..bytes], joycon.product_id())
                            {
                                samples.push(CalibrationSample::new(
                                    report.gyro,
                                    report.accel,
                                    report.stick,
                                ));
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        if samples.is_empty() {
            return Err(JoyConError::Other(
                "No samples collected during calibration".to_string(),
            ));
        }

        let calibration = Calibration::calculate_offset(&samples);

        let mut data = self.controller_data.lock().unwrap();
        if let Some(controller) = data.get_mut(serial) {
            controller.calibration = calibration;
            // Recompute state if we have a latest report
            if let Some(ref report) = controller.latest_report {
                controller.state = Some(Self::compute_state(
                    report,
                    serial.to_string(),
                    controller.is_left,
                    controller.is_joycon2,
                ));
            }
        }

        Ok(())
    }

    /// Helper function to compute unified state from an input report
    fn compute_state(
        report: &InputReport,
        serial: String,
        is_left: bool,
        is_joycon2: bool,
    ) -> JoyconState {
        let side = if is_left {
            JoyconSide::Left
        } else {
            JoyconSide::Right
        };
        let version = if is_joycon2 {
            JoyconVersion::V2
        } else {
            JoyconVersion::V1
        };
        JoyconState::from_input_report(report, serial, side, version, true)
    }

    /// Enable polling - automatically scans, connects, and starts polling for all available controllers (both Joy-Con 1 and 2)
    pub fn enable_polling(&mut self) -> Result<(), JoyConError> {
        let mut enabled = self.polling_enabled.lock().unwrap();
        if *enabled {
            return Ok(()); // Already enabled
        }
        *enabled = true;
        drop(enabled);

        // Automatically scan and connect to all available Joy-Con 1 devices
        // Note: Scanning is done in enable_polling_impl for user feedback
        // Here we just connect to devices that were already scanned
        // Actually, we need to scan again here since we can't pass the results
        match self.scan_joycon1() {
            Ok(devices) => {
                println!("[Joy-Con 1] Connecting to {} device(s)...", devices.len());
                for (serial, vid, pid) in devices {
                    println!("[Joy-Con 1] Connecting to: {}...", serial);
                    match self.connect_joycon1(&serial, vid, pid) {
                        Ok(_) => {
                            println!("[Joy-Con 1] ✓ Successfully connected: {}", serial);
                        }
                        Err(e) => {
                            eprintln!("[Joy-Con 1] ✗ Failed to connect {}: {:?}", serial, e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[Joy-Con 1] ✗ Failed to scan: {:?}", e);
            }
        }

        // Automatically scan and connect to all available Joy-Con 2 devices
        // Run this in a background thread using block_on (like the original joycon crate does)
        // (BLE scan can take 5+ seconds, so we don't want to block)
        println!("[Joy-Con 2] Starting background scan (non-blocking, may take 5-10 seconds)...");
        let controller_data_clone = Arc::clone(&self.controller_data);
        let joycon2_devices_clone = Arc::clone(&self.joycon2_devices);
        let runtime_handle_clone = self.runtime_handle.clone();
        let polling_enabled_clone = Arc::clone(&self.polling_enabled);

        // Spawn a background thread that uses block_on to run the scan
        // This is how the original joycon crate does it (everything runs inside tokio::main)
        let runtime_for_block_on = runtime_handle_clone.clone();
        std::thread::Builder::new()
            .name("joycon2-scan".to_string())
            .spawn(move || {
                println!("[Joy-Con 2] Background scan thread started");
                // Use block_on to run the async scan - this ensures it actually executes
                runtime_for_block_on.block_on(async move {
            println!("[Joy-Con 2] ========================================");
            println!("[Joy-Con 2] BACKGROUND SCAN TASK STARTED!");
            println!("[Joy-Con 2] ========================================");
            // Small delay to let Joy-Con 1 connections complete first
            println!("[Joy-Con 2] Waiting 500ms before starting scan...");
            tokio::time::sleep(Duration::from_millis(500)).await;
            println!("[Joy-Con 2] Wait complete, starting scan now...");
            
            println!("[Joy-Con 2] Starting BLE scan (this will take ~5 seconds)...");
            println!("[Joy-Con 2] Make sure your Joy-Con 2 is in pairing mode (hold sync button)!");
            match joycon::scan_joycon2_devices().await {
                Ok(addresses) => {
                    println!("[Joy-Con 2] Scan completed. Found {} device(s)", addresses.len());
                    if addresses.is_empty() {
                        println!("[Joy-Con 2] ⚠ No devices found");
                        println!("[Joy-Con 2] Make sure:");
                        println!("[Joy-Con 2]   1. Joy-Con 2 is in pairing mode (hold sync button)");
                        println!("[Joy-Con 2]   2. Bluetooth is enabled on your computer");
                        println!("[Joy-Con 2]   3. Joy-Con 2 is not already paired to another device");
                    } else {
                        println!("[Joy-Con 2] ✓✓✓ FOUND {} Joy-Con 2 device(s)! ✓✓✓", addresses.len());
                        for (i, addr) in addresses.iter().enumerate() {
                            println!("[Joy-Con 2]   Device {}: ID={}", i + 1, addr);
                        }
                        println!("[Joy-Con 2] Attempting to connect to devices...");
                        for address in addresses {
                            // Connect synchronously using block_on (like the original joycon crate)
                            let address_clone = address.clone();
                            let devices_clone = Arc::clone(&joycon2_devices_clone);
                            let data_clone = Arc::clone(&controller_data_clone);
                            let runtime_for_connect = runtime_handle_clone.clone();
                            
                            println!("[Joy-Con 2] Connecting to: ID={}", address_clone);
                            // We're already in an async context, so just await directly
                            println!("[Joy-Con 2] ========================================");
                            println!("[Joy-Con 2] CONNECTION ATTEMPT");
                            println!("[Joy-Con 2] Found device with ID: {}", address_clone);
                            println!("[Joy-Con 2] IMPORTANT: Keep holding sync button on Joy-Con 2!");
                            println!("[Joy-Con 2] ========================================");
                            
                            // Try connecting immediately - JoyCon2::connect() will do its own scan
                            // but we want to connect as fast as possible while device is still in pairing mode
                            match JoyCon2::connect(&address_clone).await {
                                    Ok(mut joycon2) => {
                                        println!("[Joy-Con 2] ✓ BLE connection established!");
                                        println!("[Joy-Con 2] Subscribing to input notifications...");
                                        
                                        match joycon2.subscribe_to_inputs().await {
                                            Ok(_) => {
                                                println!("[Joy-Con 2] ✓ Subscribed to inputs");
                                                println!("[Joy-Con 2] Enabling sensors...");
                                                
                                                match joycon2.enable_sensors().await {
                                                    Ok(_) => {
                                                        let is_left = joycon2.is_left();
                                                        let serial = joycon2.serial_number();
                                                        let side_str = if is_left { "Left" } else { "Right" };
                                                        
                                                        println!("[Joy-Con 2] ✓ Sensors enabled");
                                                        println!("[Joy-Con 2] Storing device...");
                                                        
                                                        // Store the device
                                                        let mut devices = devices_clone.lock().await;
                                                        devices.insert(serial.clone(), joycon2);
                                                        drop(devices);
                                                        
                                                        // Add to controller data
                                                        let mut data = data_clone.lock().unwrap();
                                                        data.insert(serial.clone(), ConnectedJoyCon {
                                                            serial: serial.clone(),
                                                            is_left,
                                                            is_joycon2: true,
                                                            latest_report: None,
                                                            calibration: Calibration::new(),
                                                            state: None,
                                                        });
                                                        drop(data);
                                                        
                                                        println!("[Joy-Con 2] ========================================");
                                                        println!("[Joy-Con 2] ✓✓✓ FULLY CONNECTED AND READY! ✓✓✓");
                                                        println!("[Joy-Con 2] ========================================");
                                                        println!("[Joy-Con 2] Found and connected: ID={}", address_clone);
                                                        println!("[Joy-Con 2] Serial: {}", serial);
                                                        println!("[Joy-Con 2] Side: {}", side_str);
                                                        println!("[Joy-Con 2] Device will appear in get_data()");
                                                        println!("[Joy-Con 2] ========================================");
                                                    },
                                                    Err(e) => {
                                                        eprintln!("[Joy-Con 2] ✗✗✗ FAILED: Enable sensors");
                                                        eprintln!("[Joy-Con 2] Error: {:?}", e);
                                                        eprintln!("[Joy-Con 2] ========================================");
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                eprintln!("[Joy-Con 2] ✗✗✗ FAILED: Subscribe to inputs");
                                                eprintln!("[Joy-Con 2] Error: {:?}", e);
                                                eprintln!("[Joy-Con 2] ========================================");
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        eprintln!("[Joy-Con 2] ✗✗✗ FAILED: BLE Connection");
                                        eprintln!("[Joy-Con 2] Address: {}", address_clone);
                                        eprintln!("[Joy-Con 2] Error type: {:?}", e);
                                        eprintln!("[Joy-Con 2] Error message: {}", e);
                                        eprintln!("[Joy-Con 2] ========================================");
                                        eprintln!("[Joy-Con 2] Troubleshooting:");
                                        eprintln!("[Joy-Con 2]   - Make sure Joy-Con 2 is STILL in pairing mode");
                                        eprintln!("[Joy-Con 2]   - Try holding sync button again");
                                        eprintln!("[Joy-Con 2]   - Make sure it's not connected to Switch/other device");
                                        eprintln!("[Joy-Con 2] ========================================");
                                    }
                                }
                        }
                    }
                },
                Err(e) => {
                    eprintln!("[Joy-Con 2] ✗ Scan failed with error: {:?}", e);
                    eprintln!("[Joy-Con 2] Error details: {}", e);
                    println!("[Joy-Con 2] Common issues:");
                    println!("[Joy-Con 2]   - No BLE adapter found (check Bluetooth is enabled)");
                    println!("[Joy-Con 2]   - Joy-Con 2 not in pairing mode (hold sync button)");
                    println!("[Joy-Con 2]   - Joy-Con 2 already paired to another device");
                }
            }
                    println!("[Joy-Con 2] Background scan task completed");
                });
            })
            .expect("Failed to spawn Joy-Con 2 scan thread");

        println!("[Joy-Con 2] Background scan thread spawned");

        let (tx, _rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();
        self.report_tx = Some(tx);

        // Joy-Con 1 polling is handled automatically by the scene update loop
        // (called via poll_joycon1_sync() when polling is enabled)
        // This is necessary because HidDevice is not Send/Sync and must be accessed
        // from the main thread

        // Start Joy-Con 2 polling (BLE devices) in a background thread using block_on
        // This ensures the polling actually runs (like the scan thread)
        let joycon2_devices = Arc::clone(&self.joycon2_devices);
        let controller_data = Arc::clone(&self.controller_data);
        let polling_enabled = Arc::clone(&self.polling_enabled);
        let runtime_for_polling = self.runtime_handle.clone();

        std::thread::Builder::new()
            .name("joycon2-poll".to_string())
            .spawn(move || {
                println!("[Joy-Con 2] Polling thread started");
                // Use block_on to run the polling loop - this ensures it actually executes
                runtime_for_polling.block_on(async move {
                    let mut interval = interval(Duration::from_millis(1)); // ~1000Hz polling for lowest latency
                    
                    loop {
                        interval.tick().await;
                        
                        // Check if polling is still enabled
                        {
                            let enabled = polling_enabled.lock().unwrap();
                            if !*enabled {
                                println!("[Joy-Con 2] Polling disabled, stopping...");
                                break;
                            }
                        }
                        
                        // Poll Joy-Con 2 devices (BLE) - use read_notifications like the original crate
                        // Collect serials first, then lock per device to avoid borrowing issues
                        let serials: Vec<String> = {
                            let devices = joycon2_devices.lock().await;
                            devices.keys().cloned().collect()
                        };
                        
                        for serial in serials {
                            // Lock, get device info, unlock, then read notification
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
                            
                            // Read notification - the original crate just calls read_notifications() directly
                            // It blocks until data arrives (has 5s internal timeout)
                            // We'll call it directly like the original crate does - no timeout wrapper
                            let read_result = {
                                let devices_clone = Arc::clone(&joycon2_devices);
                                let serial_clone = serial.clone();
                                async move {
                                    let devices = devices_clone.lock().await;
                                    if let Some(joycon2) = devices.get(&serial_clone) {
                                        joycon2.read_notifications().await
                                    } else {
                                        Err(JoyConError::DeviceNotFound)
                                    }
                                }.await
                            };
                            
                            match read_result {
                                Ok(data) => {
                                    if let Ok(report) = InputReport::decode_joycon2(&data, is_left) {
                                        // Apply calibration
                                        let mut data = controller_data.lock().unwrap();
                                        if let Some(controller) = data.get_mut(&serial) {
                                            let (calibrated_gyro, _) = controller.calibration.apply(report.gyro, report.accel);
                                            let mut calibrated_report = report.clone();
                                            calibrated_report.gyro = calibrated_gyro;
                                            controller.latest_report = Some(calibrated_report.clone());
                                            // Compute unified state (connected: true since we got a report)
                                            controller.state = Some(Self::compute_state(&calibrated_report, serial.clone(), controller.is_left, controller.is_joycon2));
                                            
                                            // Send to channel if receiver exists
                                            if let Err(_) = tx_clone.send((serial.clone(), calibrated_report)) {
                                                break;
                                            }
                                        } else {
                                            // Controller not found in map - this shouldn't happen
                                            static MISSING_CONTROLLER_COUNT: AtomicU32 = AtomicU32::new(0);
                                            let missing_count = MISSING_CONTROLLER_COUNT.fetch_add(1, Ordering::Relaxed);
                                            if missing_count < 3 {
                                                println!("[Joy-Con 2] WARNING: Controller {} not found in controller_data map!", serial);
                                            }
                                        }
                                    } else {
                                        // Decode failed - print error details
                                        static DECODE_FAIL_COUNT: AtomicU32 = AtomicU32::new(0);
                                        let fail_count = DECODE_FAIL_COUNT.fetch_add(1, Ordering::Relaxed);
                                        if fail_count < 5 {
                                            println!("[Joy-Con 2] Decode failed for report (length: {}): {:?}", 
                                                data.len(),
                                                InputReport::decode_joycon2(&data, is_left).err());
                                            if data.len() >= 10 {
                                                println!("[Joy-Con 2] First 10 bytes: {:02x?}", &data[..10]);
                                            }
                                        }
                                    }
                                },
                                Err(e) => {
                                    // Read failed - log first few errors
                                    static READ_FAIL_COUNT: AtomicU32 = AtomicU32::new(0);
                                    let fail_count = READ_FAIL_COUNT.fetch_add(1, Ordering::Relaxed);
                                    if fail_count < 3 {
                                        println!("[Joy-Con 2] Read notification failed: {:?}", e);
                                    }
                                }
                            }
                        }
                    }
                });
            })
            .expect("Failed to spawn Joy-Con 2 polling thread");

        println!("[Joy-Con 2] Polling thread spawned");
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
                                    let (calibrated_gyro, _calibrated_accel) =
                                        controller.calibration.apply(report.gyro, report.accel);
                                    let mut calibrated_report = report.clone();
                                    calibrated_report.gyro = calibrated_gyro;
                                    controller.latest_report = Some(calibrated_report.clone());
                                    // Compute unified state
                                    controller.state = Some(Self::compute_state(
                                        &calibrated_report,
                                        serial.clone(),
                                        controller.is_left,
                                        controller.is_joycon2,
                                    ));
                                }
                            }
                            Err(_) => {
                                // Decode failed - ignore
                            }
                        }
                    }
                }
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

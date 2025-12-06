//! Example usage of the JoyCon library

use anyhow::Result;
use joycon::{scan_joycon1_devices, scan_joycon2_devices, JoyCon, JoyCon2, InputReport, Calibration, CalibrationSample, JOYCON_1_LEFT_PID, JOYCON_1_RIGHT_PID};
use std::collections::HashMap;
use std::io::Write;

/// Clear screen and move cursor to top-left, then move down past the header
fn clear_and_position() {
    // Clear entire screen and move cursor to top-left
    print!("\x1B[2J\x1B[1;1H");
    // Reprint the header message
    print!("Polling and reporting (Press Ctrl+C to exit)\n\n");
}

/// Create a visual spinner based on frame number
fn get_spinner(frame: u64) -> char {
    let spinners = ['|', '/', '-', '\\'];
    spinners[(frame % 4) as usize]
}

/// Create a simple ASCII visualization of stick position
fn visualize_stick(h_norm: f32, v_norm: f32) -> String {
    // Create a 9x9 grid for stick visualization
    let size = 9;
    let center = size / 2;
    
    // Calculate stick position on grid (-1 to 1 -> 0 to 8)
    let x = ((h_norm + 1.0) * (size - 1) as f32 / 2.0).round() as usize;
    let y = ((v_norm + 1.0) * (size - 1) as f32 / 2.0).round() as usize;
    let x = x.min(size - 1);
    let y = y.min(size - 1);
    
    let mut grid = vec![vec!['·'; size]; size];
    grid[y][x] = '●'; // Stick position
    grid[center][center] = if x == center && y == center { '●' } else { '+' }; // Center marker
    
    grid.iter()
        .map(|row| format!("  {}", row.iter().collect::<String>()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format input report as JSON with only relevant buttons for the side
fn format_report_json(serial: &str, side: &str, report: &InputReport, calibrated_gyro: &joycon::Gyro, calibrated_accel: &joycon::Accel, frame: u64) -> (String, usize) {
    let is_left = side == "Left";
    
    // Convert stick normalized values from 0-1 to -1 to 1 range
    let stick_h_norm = (report.stick.horizontal_norm - 0.5) * 2.0;
    let stick_v_norm = (report.stick.vertical_norm - 0.5) * 2.0;
    
    // Create visual indicators
    let spinner = get_spinner(frame);
    let stick_viz = visualize_stick(stick_h_norm, stick_v_norm);
    
    // Build buttons JSON based on side
    let buttons_json = if is_left {
        format!(
            r#"    "up": {},
    "down": {},
    "left": {},
    "right": {},
    "l": {},
    "zl": {},
    "minus": {},
    "capture": {},
    "sl": {},
    "sr": {},
    "stick_press": {}"#,
            report.buttons.up, report.buttons.down, report.buttons.left, report.buttons.right,
            report.buttons.l, report.buttons.zl, report.buttons.minus, report.buttons.capture,
            report.buttons.sl, report.buttons.sr, report.buttons.stick_press
        )
    } else {
        format!(
            r#"    "a": {},
    "b": {},
    "x": {},
    "y": {},
    "r": {},
    "zr": {},
    "home": {},
    "plus": {},
    "sl": {},
    "sr": {},
    "stick_press": {}"#,
            report.buttons.a, report.buttons.b, report.buttons.x, report.buttons.y,
            report.buttons.r, report.buttons.zr, report.buttons.home, report.buttons.plus,
            report.buttons.sl, report.buttons.sr, report.buttons.stick_press
        )
    };
    
    let json_str = format!(
        r#"Status: {} [Frame: {}]

Stick Visualization:
{}

{{
  "serial": "{}",
  "side": "{}",
  "buttons": {{
{}
  }},
  "stick": {{
    "raw": {{
      "horizontal": {},
      "vertical": {}
    }},
    "normalized": {{
      "horizontal": {:.6},
      "vertical": {:.6}
    }}
  }},
  "gyro": {{
    "x": {:.2},
    "y": {:.2},
    "z": {:.2}
  }},
  "accel": {{
    "x": {:.2},
    "y": {:.2},
    "z": {:.2}
  }},
  "battery_level": {},
  "charging": {}
}}"#,
        spinner, frame,
        stick_viz,
        serial, side,
        buttons_json,
        report.stick.horizontal, report.stick.vertical,
        stick_h_norm, stick_v_norm,
        calibrated_gyro.x, calibrated_gyro.y, calibrated_gyro.z,
        calibrated_accel.x, calibrated_accel.y, calibrated_accel.z,
        report.battery_level, report.charging
    );
    
    // Count the number of lines in the JSON output
    let line_count = json_str.matches('\n').count() + 1; // +1 for the last line without newline
    
    (json_str, line_count)
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("JoyCon Example\n");

    // Scan and connect to all Joy-Con 1 devices
    match scan_joycon1_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("No Joy-Con 1 devices found.");
                println!("Make sure your Joy-Con is paired and connected (solid light).");
            } else {

            println!("Found {} Joy-Con 1 device(s):", devices.len());
            for (i, (serial, vid, pid)) in devices.iter().enumerate() {
                let side_str = if *pid == JOYCON_1_LEFT_PID {
                    "Left"
                } else if *pid == JOYCON_1_RIGHT_PID {
                    "Right"
                } else {
                    "Unknown"
                };
                println!("  [{}] Serial: {}, VID: {:04X}, PID: {:04X}, Side: {}", 
                    i + 1, serial, vid, pid, side_str);
            }
            println!();

            // Connect to all devices
            let mut connected_devices: HashMap<String, (JoyCon, String)> = HashMap::new();
            
            for (serial, vid, pid) in &devices {
                let side_str = if *pid == JOYCON_1_LEFT_PID {
                    "Left"
                } else if *pid == JOYCON_1_RIGHT_PID {
                    "Right"
                } else {
                    "Unknown"
                };
                
                println!("Connecting to Joy-Con {} (Serial: {})...", side_str, serial);
                match JoyCon::new(*vid, *pid, serial) {
                    Ok(joycon) => {
                        println!("  ✓ Connected! Enabling sensors...");
                        if let Err(e) = joycon.enable_sensors() {
                            println!("  ⚠ Warning: Failed to enable sensors: {}", e);
                        } else {
                            println!("  ✓ Sensors enabled!");
                        }
                        connected_devices.insert(serial.clone(), (joycon, side_str.to_string()));
                    }
                    Err(e) => {
                        println!("  ✗ Failed to connect: {}", e);
                    }
                }
            }

            if connected_devices.is_empty() {
                println!("\nNo devices could be connected.");
                return Ok(());
            }

            println!("\n{} device(s) connected.", connected_devices.len());
            
            // Calibrate each device (collect samples and calculate offsets)
            let mut calibrations: HashMap<String, Calibration> = HashMap::new();
            println!("Calibrating devices (collecting samples for 0.5 seconds, keep controllers still)...");
            for (serial, (joycon, side)) in &connected_devices {
                match calibrate_device(joycon, 500) {
                    Ok(calibration) => {
                        println!("  ✓ {} ({}): Gyro offset X={:.2} Y={:.2} Z={:.2}, Accel offset X={:.2} Y={:.2} Z={:.2}, Stick center H={} V={}", 
                            serial, side,
                            calibration.gyro.x, calibration.gyro.y, calibration.gyro.z,
                            calibration.accel.x, calibration.accel.y, calibration.accel.z,
                            calibration.stick_center.horizontal, calibration.stick_center.vertical);
                        calibrations.insert(serial.clone(), calibration);
                    }
                    Err(e) => {
                        println!("  ⚠ Failed to calibrate {}: {}, using zero offsets", serial, e);
                        calibrations.insert(serial.clone(), Calibration::new());
                    }
                }
            }
            
            println!("\nCalibration complete! Starting continuous reporting...\n");
            
            // Read first input report from each device and display with calibrated values
            let mut buffers: HashMap<String, [u8; 64]> = HashMap::new();
            for serial in connected_devices.keys() {
                buffers.insert(serial.clone(), [0u8; 64]);
            }
            
            // Wait a bit for sensors to stabilize
            std::thread::sleep(std::time::Duration::from_millis(100));
            
            // Poll continuously and print report every 25ms, overwriting the same area
            println!("Polling and reporting (Press Ctrl+C to exit)\n");
            let mut frame = 0u64;
            loop {
                // Clear screen once at the start of each iteration (like Joy-Con 2 does)
                clear_and_position();
                
                // Process each device and print
                for (serial, (joycon, side)) in &connected_devices {
                    let buffer = buffers.get_mut(serial).unwrap();
                    match joycon.read_input_report(buffer) {
                        Ok(bytes) => {
                            if bytes > 0 {
                                if let Ok(report) = InputReport::decode(&buffer[..bytes], joycon.product_id()) {
                                    // Apply calibration offsets
                                    let default_cal = Calibration::new();
                                    let calibration = calibrations.get(serial).unwrap_or(&default_cal);
                                    let (calibrated_gyro, calibrated_accel) = calibration.apply(report.gyro, report.accel);
                                    
                                    // Format JSON output with visual indicators
                                    let (json_output, _) = format_report_json(serial, side, &report, &calibrated_gyro, &calibrated_accel, frame);
                                    
                                    // Print the JSON (screen already cleared at start of loop)
                                    print!("{}", json_output);
                                    
                                    let _ = std::io::stdout().flush();
                                }
                            }
                        }
                        Err(_) => {
                            // Silently skip read errors
                        }
                    }
                }
                
                frame += 1;
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            }
        }
        Err(e) => {
            println!("Error scanning Joy-Con 1: {}", e);
        }
    }

    // Now try Joy-Con 2 (BLE) devices
    handle_joycon2().await?;

    Ok(())
}

/// Calibrate a device by collecting samples over a duration and calculating offsets
/// 
/// # Arguments
/// * `joycon` - The Joy-Con device to calibrate
/// * `duration_ms` - How long to collect samples (in milliseconds)
/// 
/// # Returns
/// A `Calibration` struct containing the calculated offsets
fn calibrate_device(joycon: &JoyCon, duration_ms: u64) -> Result<Calibration> {
    let mut samples = Vec::new();
    let mut buffer = [0u8; 64];
    let start_time = std::time::Instant::now();
    let duration = std::time::Duration::from_millis(duration_ms);
    
    // Collect samples until duration is reached
    while start_time.elapsed() < duration {
        match joycon.read_input_report(&mut buffer) {
            Ok(bytes) => {
                if bytes > 0 {
                    if let Ok(report) = InputReport::decode(&buffer[..bytes], joycon.product_id()) {
                        samples.push(CalibrationSample::new(report.gyro, report.accel, report.stick));
                    }
                }
            }
            Err(_) => {
                // Continue on error, just skip this sample
            }
        }
        // Small sleep to avoid busy-waiting
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    
    if samples.is_empty() {
        return Err(anyhow::anyhow!("No samples collected during calibration"));
    }
    
    Ok(Calibration::calculate_offset(&samples))
}

// Joy-Con 2 (BLE) support
async fn handle_joycon2() -> Result<()> {
    println!("\n=== Joy-Con 2 (BLE) Devices ===");
    println!("(Hold the sync button on your Joy-Con 2 to enter pairing mode)");
    
    match scan_joycon2_devices().await {
        Ok(devices) => {
            if devices.is_empty() {
                println!("No Joy-Con 2 devices found.");
                println!("Make sure your Joy-Con 2 is in sync mode (holding sync button).");
                // Check if there's a Joy-Con 1 available instead
                println!("\nChecking for Joy-Con 1 devices...");
                match scan_joycon1_devices() {
                    Ok(joycon1_devices) => {
                        if !joycon1_devices.is_empty() {
                            println!("Found {} Joy-Con 1 device(s), using those instead.", joycon1_devices.len());
                            
                            // Connect to Joy-Con 1 devices
                            let mut connected_devices: HashMap<String, (JoyCon, String)> = HashMap::new();
                            for (serial, vid, pid) in &joycon1_devices {
                                let side_str = if *pid == JOYCON_1_LEFT_PID {
                                    "Left"
                                } else if *pid == JOYCON_1_RIGHT_PID {
                                    "Right"
                                } else {
                                    "Unknown"
                                };
                                
                                println!("Connecting to Joy-Con {} (Serial: {})...", side_str, serial);
                                match JoyCon::new(*vid, *pid, serial) {
                                    Ok(joycon) => {
                                        println!("  ✓ Connected! Enabling sensors...");
                                        if let Err(e) = joycon.enable_sensors() {
                                            println!("  ⚠ Warning: Failed to enable sensors: {}", e);
                                        } else {
                                            println!("  ✓ Sensors enabled!");
                                        }
                                        connected_devices.insert(serial.clone(), (joycon, side_str.to_string()));
                                    }
                                    Err(e) => {
                                        println!("  ✗ Failed to connect: {}", e);
                                    }
                                }
                            }
                            
                            if connected_devices.is_empty() {
                                println!("\nNo Joy-Con 1 devices could be connected.");
                                return Ok(());
                            }
                            
                            println!("\n{} device(s) connected.", connected_devices.len());
                            
                            // Calibrate each device
                            let mut calibrations: HashMap<String, Calibration> = HashMap::new();
                            println!("Calibrating devices (collecting samples for 0.5 seconds, keep controllers still)...");
                            for (serial, (joycon, side)) in &connected_devices {
                                match calibrate_device(joycon, 500) {
                                    Ok(calibration) => {
                                        println!("  ✓ {} ({}): Gyro offset X={:.2} Y={:.2} Z={:.2}, Accel offset X={:.2} Y={:.2} Z={:.2}, Stick center H={} V={}", 
                                            serial, side,
                                            calibration.gyro.x, calibration.gyro.y, calibration.gyro.z,
                                            calibration.accel.x, calibration.accel.y, calibration.accel.z,
                                            calibration.stick_center.horizontal, calibration.stick_center.vertical);
                                        calibrations.insert(serial.clone(), calibration);
                                    }
                                    Err(e) => {
                                        println!("  ⚠ Failed to calibrate {}: {}, using zero offsets", serial, e);
                                        calibrations.insert(serial.clone(), Calibration::new());
                                    }
                                }
                            }
                            
                            println!("\nCalibration complete! Starting continuous reporting...\n");
                            
                            // Read first input report from each device and display with calibrated values
                            let mut buffers: HashMap<String, [u8; 64]> = HashMap::new();
                            for serial in connected_devices.keys() {
                                buffers.insert(serial.clone(), [0u8; 64]);
                            }
                            
                            // Wait a bit for sensors to stabilize
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            
                            // Poll continuously and print report every 25ms, overwriting the same area
                            println!("Polling and reporting (Press Ctrl+C to exit)\n");
                            let mut frame = 0u64;
                            loop {
                                // Clear screen once at the start of each iteration
                                clear_and_position();
                                
                                // Process each device and print
                                for (serial, (joycon, side)) in &connected_devices {
                                    let buffer = buffers.get_mut(serial).unwrap();
                                    match joycon.read_input_report(buffer) {
                                        Ok(bytes) => {
                                            if bytes > 0 {
                                                if let Ok(report) = InputReport::decode(&buffer[..bytes], joycon.product_id()) {
                                                    // Apply calibration offsets
                                                    let default_cal = Calibration::new();
                                                    let calibration = calibrations.get(serial).unwrap_or(&default_cal);
                                                    let (calibrated_gyro, calibrated_accel) = calibration.apply(report.gyro, report.accel);
                                                    
                                                    // Format JSON output with visual indicators
                                                    let (json_output, _) = format_report_json(serial, side, &report, &calibrated_gyro, &calibrated_accel, frame);
                                    
                                                    // Print the new JSON
                                                    print!("{}", json_output);
                                    
                                                    let _ = std::io::stdout().flush();
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            // Silently skip read errors
                                        }
                                    }
                                }
                                
                                frame += 1;
                                tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
                            }
                        } else {
                            println!("No Joy-Con 1 devices found either.");
                        }
                    }
                    Err(_) => {
                        println!("Error scanning for Joy-Con 1 devices.");
                    }
                }
            } else {
                println!("Found {} Joy-Con 2 device(s):", devices.len());
                for (i, address) in devices.iter().enumerate() {
                    println!("  [{}] Address: {}", i + 1, address);
                }
                println!();
                
                // Try to connect to the first device
                if let Some(address) = devices.first() {
                    println!("Connecting to Joy-Con 2 (Address: {})...", address);
                    match JoyCon2::connect(address).await {
                        Ok(mut joycon2) => {
                            println!("  ✓ Connected!");
                            
                            // Try to subscribe to inputs
                            println!("  Attempting to subscribe to input characteristics...");
                            match joycon2.subscribe_to_inputs().await {
                                Ok(_) => {
                                    println!("  ✓ Subscribed to inputs!");
                                    
                                    // Enable motion sensors (gyro and accelerometer)
                                    if let Err(e) = joycon2.enable_sensors().await {
                                        println!("  ⚠ Warning: Failed to enable sensors: {}", e);
                                    } else {
                                        println!("  ✓ Motion sensors enabled!");
                                    }
                                    
                                    // Wait a bit for notifications to start
                                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                                    
                                    // Continuously read notifications
                                    println!("  Starting continuous input report reading...");
                                    println!("  (Press Ctrl+C to stop)\n");
                                    
                                    let is_left = joycon2.is_left();
                                    
                                    // Calibrate Joy-Con 2 (collect samples and calculate offsets)
                                    println!("  Calibrating device (collecting samples for 0.5 seconds, keep controller still)...");
                                    let mut calibration_samples = Vec::new();
                                    let calibration_start = std::time::Instant::now();
                                    let calibration_duration = std::time::Duration::from_millis(500);
                                    
                                    // Collect calibration samples - use read_notifications with error handling
                                    while calibration_start.elapsed() < calibration_duration {
                                        match joycon2.read_notifications().await {
                                            Ok(data) => {
                                                // Decode with error handling to prevent crashes
                                                match joycon::input_report::InputReport::decode_joycon2(&data, is_left) {
                                                    Ok(report) => {
                                                        calibration_samples.push(CalibrationSample::new(report.gyro, report.accel, report.stick));
                                                    }
                                                    Err(e) => {
                                                        // Log but don't crash - just skip this sample
                                                        eprintln!("  ⚠ Failed to decode report during calibration: {}", e);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                // Log but don't crash - just skip this sample
                                                eprintln!("  ⚠ Failed to read notification during calibration: {}", e);
                                            }
                                        }
                                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                                    }
                                    
                                    let calibration = if !calibration_samples.is_empty() {
                                        Calibration::calculate_offset(&calibration_samples)
                                    } else {
                                        Calibration::new()
                                    };
                                    
                                    println!("  ✓ Calibration complete! Gyro offset X={:.2} Y={:.2} Z={:.2}, Accel offset X={:.2} Y={:.2} Z={:.2}, Stick center H={} V={}\n",
                                        calibration.gyro.x, calibration.gyro.y, calibration.gyro.z,
                                        calibration.accel.x, calibration.accel.y, calibration.accel.z,
                                        calibration.stick_center.horizontal, calibration.stick_center.vertical);
                                    
                                    println!("Polling and reporting (Press Ctrl+C to exit)\n");
                                    let mut frame = 0u64;
                                    loop {
                                        // Clear screen once at the start of each iteration
                                        clear_and_position();
                                        
                                        match joycon2.read_notifications().await {
                                            Ok(data) => {
                                                // Decode the Joy-Con 2 input report
                                                match joycon::input_report::InputReport::decode_joycon2(&data, is_left) {
                                                    Ok(report) => {
                                                        // Apply calibration
                                                        let (calibrated_gyro, calibrated_accel) = calibration.apply(report.gyro, report.accel);
                                                        
                                                        // Format JSON output with visual indicators
                                                        let side_str = if is_left { "Left" } else { "Right" };
                                                        let (json_output, _) = format_report_json("Joy-Con 2", side_str, &report, &calibrated_gyro, &calibrated_accel, frame);
                                                        
                                                        // Print the new JSON
                                                        print!("{}", json_output);
                                                        
                                                        let _ = std::io::stdout().flush();
                                                    }
                                                    Err(_) => {
                                                        // Silently skip decode errors
                                                    }
                                                }
                                            }
                                            Err(_) => {
                                                // Silently skip read errors
                                            }
                                        }
                                        
                                        frame += 1;
                                        tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
                                    }
                                }
                                Err(e) => {
                                    println!("  ⚠ Failed to subscribe: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            println!("  ✗ Failed to connect: {}", e);
                        }
                    }
                }
            }
        }
        Err(e) => println!("Error scanning Joy-Con 2: {}", e),
    }
    
    Ok(())
}

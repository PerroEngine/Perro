//! Joy-Con 1 (HID) support
//!
//! Joy-Con 1 devices require manual Bluetooth pairing to appear as HID devices on Windows.
//! The Joy-Con light should be solid, otherwise the device won't appear in HID enumeration.

use crate::input::joycon::error::Result;
use crate::input::joycon::{JOYCON_1_LEFT_PID, JOYCON_1_RIGHT_PID, JOYCON_VENDOR_ID};
use hidapi::HidApi;
use std::sync::Arc;

/// Represents a Joy-Con 1 controller connected via HID
pub struct JoyCon {
    device: Arc<hidapi::HidDevice>,
    vendor_id: u16,
    product_id: u16,
    serial_number: String,
}

impl JoyCon {
    /// Create a new JoyCon instance from vendor ID, product ID, and serial number
    pub fn new(vendor_id: u16, product_id: u16, serial_number: &str) -> Result<Self> {
        let api = HidApi::new()?;
        let device = api.open_serial(vendor_id, product_id, serial_number)?;

        Ok(Self {
            device: Arc::new(device),
            vendor_id,
            product_id,
            serial_number: serial_number.to_string(),
        })
    }

    /// Read an input report from the Joy-Con
    ///
    /// Joy-Con input reports are typically 64 bytes
    /// Uses non-blocking read with 0ms timeout - returns immediately if no data is available
    /// This allows the update loop to run at full speed without blocking
    pub fn read_input_report(&self, buffer: &mut [u8]) -> Result<usize> {
        // Use non-blocking read with 0ms timeout - returns immediately if no data available
        // This prevents blocking the update loop while still reading data as fast as it arrives
        match self.device.read_timeout(buffer, 0) {
            Ok(bytes_read) => {
                Ok(bytes_read)
            },
            Err(e) => {
                // On timeout or error, return 0 bytes (not an error, just no data available)
                // This allows the update loop to continue without blocking
                if e.to_string().contains("timeout") || e.to_string().contains("timed out") {
                    Ok(0)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Enable 6-axis sensors (accelerometer and gyroscope)
    pub fn enable_sensors(&self) -> Result<()> {
        // Enable 6-axis sensor: subcommand 0x40 with argument 0x01
        let mut cmd = vec![0x01, 0x00]; // Report ID and packet number
        cmd.extend_from_slice(&[0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40]); // Rumble data
        cmd.push(0x40); // Subcommand: enable 6-axis
        cmd.push(0x01); // Argument: enable
        self.write_output_report(&cmd)?;
        
        // Switch to standard input report mode (0x30)
        let mut cmd2 = vec![0x01, 0x01]; // Report ID and packet number
        cmd2.extend_from_slice(&[0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40]); // Rumble data
        cmd2.push(0x03); // Subcommand: set input report mode
        cmd2.push(0x30); // Argument: standard input report
        self.write_output_report(&cmd2)?;
        
        Ok(())
    }

    /// Read and decode an input report into structured data
    pub fn read_decoded_report(&self) -> Result<crate::input::joycon::input_report::InputReport> {
        let mut buffer = [0u8; 64];
        let bytes_read = self.read_input_report(&mut buffer)?;
        crate::input::joycon::input_report::InputReport::decode(&buffer[..bytes_read], self.product_id)
    }

    /// Write an output report to the Joy-Con
    pub fn write_output_report(&self, data: &[u8]) -> Result<usize> {
        let bytes_written = self.device.write(data)?;
        Ok(bytes_written)
    }

    /// Get the vendor ID
    pub fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    /// Get the product ID
    pub fn product_id(&self) -> u16 {
        self.product_id
    }

    /// Get the serial number
    pub fn serial_number(&self) -> &str {
        &self.serial_number
    }

    /// Check if this is a left Joy-Con
    pub fn is_left(&self) -> bool {
        self.product_id == JOYCON_1_LEFT_PID
    }

    /// Check if this is a right Joy-Con
    pub fn is_right(&self) -> bool {
        self.product_id == JOYCON_1_RIGHT_PID
    }
}

/// Scan for available Joy-Con 1 devices
///
/// Returns a vector of tuples containing (serial_number, vendor_id, product_id)
pub fn scan_devices() -> Result<Vec<(String, u16, u16)>> {
    eprintln!("[scan_devices] Starting HID scan...");
    let api = match HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            eprintln!("[scan_devices] ERROR: HidApi::new() failed: {:?}", e);
            return Err(e.into());
        }
    };
    eprintln!("[scan_devices] HID API initialized");
    
    let mut devices = Vec::new();
    let mut total_count = 0;
    let mut nintendo_count = 0;

    eprintln!("[scan_devices] Enumerating devices...");
    for device_info in api.device_list() {
        total_count += 1;
        let vid = device_info.vendor_id();
        let pid = device_info.product_id();

        // Check if it's a Joy-Con 1 device
        if vid == JOYCON_VENDOR_ID {
            nintendo_count += 1;
            eprintln!("[scan_devices] Found Nintendo device: vid=0x{:04X}, pid=0x{:04X}", vid, pid);
            
            if pid == JOYCON_1_LEFT_PID || pid == JOYCON_1_RIGHT_PID {
                if let Some(serial) = device_info.serial_number() {
                    eprintln!("[scan_devices] ✓ FOUND JOY-CON: serial={}, vid=0x{:04X}, pid=0x{:04X}", serial, vid, pid);
                    devices.push((serial.to_string(), vid, pid));
                } else {
                    eprintln!("[scan_devices] ⚠ Joy-Con found but no serial number!");
                }
            }
        }
    }

    eprintln!("[scan_devices] Scan complete: {} total devices, {} Nintendo devices, {} Joy-Con devices", total_count, nintendo_count, devices.len());
    Ok(devices)
}


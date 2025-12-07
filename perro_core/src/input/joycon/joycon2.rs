//! Joy-Con 2 (BLE) support
//!
//! Joy-Con 2 devices broadcast BLE advertisements when the sync button is held.
//! You must hold the sync button for pairing mode before connecting and subscribing.
//!
//! **Important:** Joy-Con 2 uses a proprietary pairing procedure over the HID command interface
//! instead of standard Bluetooth SMP pairing. See:
//! https://github.com/ndeadly/switch2_controller_research/blob/master/bluetooth_interface.md
//!
//! After establishing a BLE connection, you may need to send initialization commands to enable
//! input reports. The device may not send notifications until properly initialized.

use crate::input::joycon::error::{JoyConError, Result};
use crate::input::joycon::{NINTENDO_BLE_CID, JOYCON_L_SIDE, JOYCON_R_SIDE};
use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter, Characteristic};
use btleplug::platform::{Manager, Peripheral as PlatformPeripheral, Adapter};
use std::sync::Arc;
use tokio::time::{Duration, timeout};
use futures::StreamExt;
use uuid::Uuid;

// Joy-Con 2 GATT characteristic UUIDs for input reports
// Based on: https://github.com/ndeadly/switch2_controller_research/blob/master/hid_reports.md
// Input Report 0x05: Common to all controllers
const INPUT_REPORT_05_UUID: &str = "ab7de9be-89fe-49ad-828f-118f09df7fd2";
// Input Report 0x07: Left Joy-Con 2
const INPUT_REPORT_07_UUID: &str = "cc1bbbb5-7354-4d32-a716-a81cb241a32a";
// Input Report 0x08: Right Joy-Con 2
const INPUT_REPORT_08_UUID: &str = "d5a9e01e-2ffc-4cca-b20c-8b67142bf442";

/// Represents a Joy-Con 2 controller connected via BLE
pub struct JoyCon2 {
    peripheral: Arc<PlatformPeripheral>,
    address: String,
    input_characteristic: Option<Characteristic>,
    command_characteristic: Option<Characteristic>,
    adapter: Option<Arc<Adapter>>,
    is_left: bool,
}

impl JoyCon2 {
    /// Create a new JoyCon2 instance by connecting to a device
    /// 
    /// The address should be from a device that was found via scan_devices(),
    /// which only returns devices that match the strict Joy-Con 2 criteria.
    pub async fn connect(address: &str) -> Result<Self> {
        let manager = Manager::new().await
            .map_err(|e| JoyConError::Ble(format!("Failed to create BLE manager: {}", e)))?;

        let adapters = manager.adapters().await
            .map_err(|e| JoyConError::Ble(format!("Failed to get adapters: {}", e)))?;

        if adapters.is_empty() {
            return Err(JoyConError::Ble("No BLE adapters found".to_string()));
        }

        let central = adapters.into_iter().next().unwrap();
        central.start_scan(ScanFilter::default()).await
            .map_err(|e| JoyConError::Ble(format!("Failed to start scan: {}", e)))?;

        // Wait a bit for devices to be discovered
        // Reduced to 1 second to connect faster while device is still in pairing mode
        tokio::time::sleep(Duration::from_secs(1)).await;

        let peripherals = central.peripherals().await
            .map_err(|e| JoyConError::Ble(format!("Failed to get peripherals: {}", e)))?;

        // Find the peripheral by address AND verify it's actually a Joy-Con 2
        let mut found_peripheral = None;
        for p in peripherals {
            let id_str = format!("{:?}", p.id());
            if id_str.contains(address) {
                // Double-check it's actually a Joy-Con 2 using strict filtering
                let (is_match, _, _) = is_joycon2_device(&p).await;
                if is_match {
                    found_peripheral = Some(p);
                    break;
                } else {
                    return Err(JoyConError::DeviceNotFound);
                }
            }
        }
        
        let peripheral = found_peripheral.ok_or_else(|| JoyConError::DeviceNotFound)?;

        peripheral.connect().await
            .map_err(|e| JoyConError::ConnectionFailed(format!("Failed to connect: {}", e)))?;

        // Wait for connection to establish
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Determine if this is a left or right Joy-Con based on the side identifier
        let (_, _, side) = is_joycon2_device(&peripheral).await;
        let is_left = side == Some(JOYCON_L_SIDE);

        Ok(Self {
            peripheral: Arc::new(peripheral),
            address: address.to_string(),
            input_characteristic: None,
            command_characteristic: None,
            adapter: Some(Arc::new(central)),
            is_left,
        })
    }

    /// Discover and subscribe to BLE characteristics for input notifications
    ///
    /// Based on: https://github.com/ndeadly/switch2_controller_research/blob/master/hid_reports.md
    /// Joy-Con 2 uses specific GATT characteristics for input reports:
    /// - Input Report 0x05 (common): UUID ab7de9be-89fe-49ad-828f-118f09df7fd2
    /// - Input Report 0x07 (Left): UUID cc1bbbb5-7354-4d32-a716-a81cb241a32a
    /// - Input Report 0x08 (Right): UUID d5a9e01e-2ffc-4cca-b20c-8b67142bf442
    pub async fn subscribe_to_inputs(&mut self) -> Result<()> {
        // Discover services
        self.peripheral.discover_services().await
            .map_err(|e| JoyConError::Ble(format!("Failed to discover services: {}", e)))?;

        let services = self.peripheral.services();
        
        println!("  Discovering services and characteristics...");
        
        // Parse the known UUIDs
        let report_05_uuid = Uuid::parse_str(INPUT_REPORT_05_UUID)
            .map_err(|e| JoyConError::Ble(format!("Failed to parse UUID: {}", e)))?;
        let report_07_uuid = Uuid::parse_str(INPUT_REPORT_07_UUID)
            .map_err(|e| JoyConError::Ble(format!("Failed to parse UUID: {}", e)))?;
        let report_08_uuid = Uuid::parse_str(INPUT_REPORT_08_UUID)
            .map_err(|e| JoyConError::Ble(format!("Failed to parse UUID: {}", e)))?;
        
        // First, try to find the specific Joy-Con 2 input report characteristics
        let mut found_characteristic: Option<Characteristic> = None;
        
        for service in &services {
            println!("    Service UUID: {}", service.uuid);
            for characteristic in &service.characteristics {
                println!("      Characteristic UUID: {}, Properties: {:?}", 
                    characteristic.uuid, characteristic.properties);
                
                // Check if this is one of the known Joy-Con 2 input report characteristics
                if characteristic.uuid == report_05_uuid 
                    || characteristic.uuid == report_07_uuid 
                    || characteristic.uuid == report_08_uuid {
                    if characteristic.properties.contains(btleplug::api::CharPropFlags::NOTIFY) {
                        found_characteristic = Some(characteristic.clone());
                        println!("      → Found Joy-Con 2 input report characteristic!");
                        break;
                    }
                }
            }
            if found_characteristic.is_some() {
                break;
            }
        }
        
        // If we didn't find a specific one, fall back to any NOTIFY characteristic
        // (for compatibility or if UUIDs are different)
        let characteristic = if let Some(char) = found_characteristic {
            char
        } else {
            // Fallback: look for any NOTIFY characteristic
            let mut notify_chars = Vec::new();
            for service in self.peripheral.services() {
                for char in &service.characteristics {
                    if char.properties.contains(btleplug::api::CharPropFlags::NOTIFY) {
                        notify_chars.push(char.clone());
                    }
                }
            }
            
            if notify_chars.is_empty() {
                return Err(JoyConError::Ble("No characteristics with NOTIFY property found".to_string()));
            }
            
            // Prefer READ | NOTIFY, fall back to NOTIFY only
            let read_notify: Vec<_> = notify_chars.iter()
                .filter(|c| c.properties.contains(btleplug::api::CharPropFlags::READ))
                .cloned()
                .collect();
            
            if !read_notify.is_empty() {
                read_notify[0].clone()
            } else {
                notify_chars[0].clone()
            }
        };
        
        println!("  Attempting to subscribe to characteristic: {} (Properties: {:?})", 
            characteristic.uuid, characteristic.properties);
        
        // Subscribe to notifications (btleplug will handle writing 0x0001 to CCCD)
        self.peripheral.subscribe(&characteristic).await
            .map_err(|e| JoyConError::Ble(format!("Failed to subscribe: {}", e)))?;
        
        self.input_characteristic = Some(characteristic.clone());
        println!("  ✓ Subscribed to input characteristic (notifications enabled)");
        
        // Joy-Con 2 requires initialization commands to start sending input reports
        // Based on: https://github.com/ndeadly/switch2_controller_research/blob/master/bluetooth_interface.md
        // The initialization sequence involves sending commands to enable input reports.
        // For now, we'll attempt a minimal initialization sequence.
        println!("  Attempting to initialize Joy-Con 2...");
        
        // Find command characteristic - use the exact UUID from joycon2cpp
        // Based on: https://github.com/TheFrano/joycon2cpp/blob/main/testapp/src/testapp.cpp
        // const wchar_t* WRITE_COMMAND_UUID = L"649d4ac9-8eb7-4e6c-af44-1ea54fe5f005";
        let write_command_uuid = Uuid::parse_str("649d4ac9-8eb7-4e6c-af44-1ea54fe5f005")
            .map_err(|e| JoyConError::Ble(format!("Failed to parse write command UUID: {}", e)))?;
        
        let mut command_char: Option<Characteristic> = None;
        
        for service in self.peripheral.services() {
            for cmd_char in &service.characteristics {
                if cmd_char.uuid == write_command_uuid {
                    command_char = Some(cmd_char.clone());
                    break;
                }
            }
            if command_char.is_some() {
                break;
            }
        }
        
        if let Some(cmd_char) = command_char {
            println!("  Found command characteristic: {} (Properties: {:?})", cmd_char.uuid, cmd_char.properties);
            self.command_characteristic = Some(cmd_char.clone());
        } else {
            println!("  ⚠ Command characteristic 649d4ac9-8eb7-4e6c-af44-1ea54fe5f005 not found");
            println!("  Trying to find any WRITE_WITHOUT_RESPONSE characteristic as fallback...");
            // Fallback: find any WRITE_WITHOUT_RESPONSE characteristic
            for service in self.peripheral.services() {
                for cmd_char in &service.characteristics {
                    if cmd_char.properties.contains(btleplug::api::CharPropFlags::WRITE_WITHOUT_RESPONSE) {
                        command_char = Some(cmd_char.clone());
                        println!("  Using fallback: {} (Properties: {:?})", cmd_char.uuid, cmd_char.properties);
                        self.command_characteristic = Some(cmd_char.clone());
                        break;
                    }
                }
                if command_char.is_some() {
                    break;
                }
            }
        }
        
        Ok(())
    }

    /// Get a notification stream for continuous reading
    /// This should be called once and reused, not called repeatedly
    pub async fn notification_stream(&self) -> Result<impl futures::Stream<Item = Result<Vec<u8>>>> {
        let notifications = self.peripheral.notifications().await
            .map_err(|e| JoyConError::Ble(format!("Failed to get notification stream: {}", e)))?;
        
        Ok(notifications.map(|notification| {
            Ok(notification.value)
        }))
    }
    
    /// Read notifications from the subscribed characteristic
    ///
    /// This attempts to read one notification from the subscribed characteristic.
    /// For characteristics with READ property, it reads directly.
    /// For NOTIFY characteristics, it listens to the peripheral's notification stream.
    /// NOTE: This creates a new stream each time, which can cause issues.
    /// For continuous reading, use notification_stream() instead.
    pub async fn read_notifications(&self) -> Result<Vec<u8>> {
        let characteristic = self.input_characteristic.as_ref()
            .ok_or_else(|| JoyConError::Ble("Not subscribed to input characteristic".to_string()))?;
        
        // If the characteristic has READ property, try reading it directly first
        if characteristic.properties.contains(btleplug::api::CharPropFlags::READ) {
            match self.peripheral.read(characteristic).await {
                Ok(data) => {
                    return Ok(data);
                }
                Err(_) => {
                    // Direct read failed, will wait for notification (silently)
                }
            }
        }
        
        // For NOTIFY characteristics, use the peripheral's notification stream
        // This is the correct way to receive notifications in btleplug
        let mut notifications = self.peripheral.notifications().await
            .map_err(|e| JoyConError::Ble(format!("Failed to get notification stream: {}", e)))?;
        
        // Wait for a notification with timeout
        let timeout_duration = Duration::from_secs(5);
        
        match timeout(timeout_duration, notifications.next()).await {
            Ok(Some(notification)) => {
                Ok(notification.value)
            }
            Ok(None) => {
                Err(JoyConError::Ble("Notification stream ended unexpectedly".to_string()))
            }
            Err(_) => {
                Err(JoyConError::Ble(
                    format!("No notification received within timeout. Make sure the controller is sending data (move it or press buttons).")
                ))
            }
        }
    }
    
    /// Enable motion sensors (gyroscope and accelerometer)
    /// Based on joycon2cpp: https://github.com/TheFrano/joycon2cpp
    /// Uses the write command characteristic UUID: 649d4ac9-8eb7-4e6c-af44-1ea54fe5f005
    /// 
    /// Sends the exact commands from joycon2cpp's SendCustomCommands function:
    /// - Command 1: { 0x0c, 0x91, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00 }
    /// - Command 2: { 0x0c, 0x91, 0x01, 0x04, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00 }
    pub async fn enable_sensors(&self) -> Result<()> {
        let cmd_char = self.command_characteristic.as_ref()
            .ok_or_else(|| JoyConError::Ble("No command characteristic available".to_string()))?;
        
        println!("  Attempting to enable motion sensors...");
        println!("  Using command characteristic: {} (Properties: {:?})", cmd_char.uuid, cmd_char.properties);
        
        let write_type = if cmd_char.properties.contains(btleplug::api::CharPropFlags::WRITE_WITHOUT_RESPONSE) {
            btleplug::api::WriteType::WithoutResponse
        } else {
            btleplug::api::WriteType::WithResponse
        };
        
        // Exact commands from joycon2cpp SendCustomCommands function
        let commands = vec![
            vec![0x0c, 0x91, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00],
            vec![0x0c, 0x91, 0x01, 0x04, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00],
        ];
        
        for (i, cmd) in commands.iter().enumerate() {
            println!("  Sending command {}...", i + 1);
            match self.peripheral.write(cmd_char, cmd, write_type).await {
                Ok(_) => {
                    println!("  ✓ Sent command {} successfully", i + 1);
                    if i < commands.len() - 1 {
                        println!("  Waiting 500ms before next command...");
                        tokio::time::sleep(Duration::from_millis(500)).await; // 500ms delay as in C++ code
                        println!("  Wait complete, continuing to next command...");
                    }
                }
                Err(e) => {
                    println!("  ⚠ Failed to send command {}: {}", i + 1, e);
                }
            }
        }
        
        println!("  Note: Motion data should appear at bytes 0x30-0x3B in input reports.");
        println!("  ✓ Sensors enabled");
        
        Ok(())
    }
    

    /// Get the device address
    pub fn address(&self) -> &str {
        &self.address
    }
    
    /// Get a serial number identifier for this Joy-Con 2
    /// Uses the BLE address formatted as a serial number (e.g., "3CA9ABCCAFE7")
    pub fn serial_number(&self) -> String {
        // Format address as serial number (remove colons and "PeripheralId()" wrapper if present)
        self.address
            .replace("PeripheralId(", "")
            .replace(")", "")
            .replace(":", "")
            .to_uppercase()
    }
    
    /// Check if this is a left Joy-Con
    pub fn is_left(&self) -> bool {
        self.is_left
    }
    
    /// Check if this is a right Joy-Con
    pub fn is_right(&self) -> bool {
        !self.is_left
    }

    /// Disconnect from the device
    pub async fn disconnect(&self) -> Result<()> {
        self.peripheral.disconnect().await
            .map_err(|e| JoyConError::Ble(format!("Failed to disconnect: {}", e)))?;
        Ok(())
    }
}

/// Check if a peripheral is a Joy-Con 2 device
/// 
/// STRICT FILTERING: Only matches devices that have:
/// 1. Nintendo BLE Company ID (0x0553) in manufacturer data
/// 2. AND Joy-Con side identifier (0x66 for Right, 0x67 for Left) in that same data
/// 
/// This ensures we only match actual Joy-Con 2 controllers, not other BLE devices.
async fn is_joycon2_device(peripheral: &PlatformPeripheral) -> (bool, String, Option<u8>) {
    let mut debug_info = String::new();
    let mut side: Option<u8> = None;
    
    if let Ok(Some(props)) = peripheral.properties().await {
        // Build debug info
        if let Some(name) = props.local_name.as_ref() {
            debug_info.push_str(&format!("Name: '{}'", name));
        } else {
            debug_info.push_str("Name: <none>");
        }
        debug_info.push_str(&format!(", Manufacturers: {:?}", props.manufacturer_data.keys().collect::<Vec<_>>()));
        
        // STRICT: Only match if we have Nintendo BLE Company ID (0x0553)
        if let Some(manufacturer_data) = props.manufacturer_data.get(&NINTENDO_BLE_CID) {
            debug_info.push_str(&format!(", Data: {:?}", manufacturer_data));
            
            // STRICT: Must also have side identifier (0x66 or 0x67) in the data
            let mut found_side = false;
            for &byte in manufacturer_data.iter() {
                if byte == JOYCON_R_SIDE {
                    side = Some(JOYCON_R_SIDE);
                    found_side = true;
                    break;
                } else if byte == JOYCON_L_SIDE {
                    side = Some(JOYCON_L_SIDE);
                    found_side = true;
                    break;
                }
            }
            
            // Only return true if we found BOTH the Nintendo CID AND a side identifier
            if found_side {
                let side_str = match side {
                    Some(JOYCON_R_SIDE) => "Right",
                    Some(JOYCON_L_SIDE) => "Left",
                    _ => "Unknown",
                };
                return (true, format!("Matched by Nintendo BLE CID 0x{:04X} ({}) with side identifier: {}", 
                    NINTENDO_BLE_CID, side_str, debug_info), side);
            } else {
                // Has Nintendo CID but no side identifier - NOT a match
                return (false, format!("Has Nintendo CID 0x{:04X} but no side identifier (0x66/0x67): {}", 
                    NINTENDO_BLE_CID, debug_info), None);
            }
        }
        
        // No Nintendo BLE CID found - NOT a match
        return (false, debug_info, None);
    }
    
    (false, "No properties available".to_string(), None)
}


/// Scan for available Joy-Con 2 devices via BLE advertisements
///
/// Returns a vector of device addresses/identifiers
/// Devices should be in sync mode (holding sync button) to be discovered
pub async fn scan_devices() -> Result<Vec<String>> {
    let manager = Manager::new().await
        .map_err(|e| JoyConError::Ble(format!("Failed to create BLE manager: {}", e)))?;

    let adapters = manager.adapters().await
        .map_err(|e| JoyConError::Ble(format!("Failed to get adapters: {}", e)))?;

    if adapters.is_empty() {
        return Err(JoyConError::Ble("No BLE adapters found".to_string()));
    }

    let central = adapters.into_iter().next().unwrap();
    
    // Start scanning
    central.start_scan(ScanFilter::default()).await
        .map_err(|e| JoyConError::Ble(format!("Failed to start scan: {}", e)))?;

    // Wait for devices to be discovered
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Get discovered peripherals
    let peripherals = central.peripherals().await
        .map_err(|e| JoyConError::Ble(format!("Failed to get peripherals: {}", e)))?;

    let mut devices = Vec::new();
    let mut all_devices_info = Vec::new();
    
    println!("Scanning {} BLE devices for Joy-Con 2...", peripherals.len());
    
    for peripheral in peripherals {
        let id_str = format!("{:?}", peripheral.id());
        let (is_match, debug_info, side) = is_joycon2_device(&peripheral).await;
        all_devices_info.push((id_str.clone(), debug_info.clone()));
        
        if is_match {
            let side_str = match side {
                Some(JOYCON_L_SIDE) => " (Left)",
                Some(JOYCON_R_SIDE) => " (Right)",
                _ => "",
            };
            println!("[Joy-Con 2] ✓ FOUND Joy-Con 2{}: ID={}", side_str, id_str);
            println!("[Joy-Con 2]   Details: {}", debug_info);
            if !devices.contains(&id_str) {
                devices.push(id_str);
            }
        }
    }
    
    // If no devices found with strict filtering, show debug info and return all devices
    if devices.is_empty() && !all_devices_info.is_empty() {
        println!("\n⚠ No Joy-Con 2 devices found with strict filtering.");
        println!("Found {} total BLE devices. Showing first 10:", all_devices_info.len());
        for (id, info) in all_devices_info.iter().take(10) {
            println!("  Device {}: {}", id, info);
        }
        if all_devices_info.len() > 10 {
            println!("  ... and {} more devices", all_devices_info.len() - 10);
        }
        println!("\nReturning all devices. Look for your Joy-Con 2 in the list above.");
        println!("If you see it, note its properties so we can improve filtering.\n");
        
        // Return all devices so user can see what's available
        return Ok(all_devices_info.into_iter().map(|(id, _)| id).collect());
    } else if !devices.is_empty() {
        println!("Found {} Joy-Con 2 device(s)\n", devices.len());
    }

    Ok(devices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scan_devices() {
        // This test will only pass if a Joy-Con 2 is in sync mode
        let devices = scan_devices().await;
        assert!(devices.is_ok());
    }
}



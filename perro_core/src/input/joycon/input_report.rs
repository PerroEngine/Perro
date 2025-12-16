//! Joy-Con input report decoder
//!
//! Decodes the raw 64-byte HID input reports into structured button, stick, and sensor data.

use crate::input::joycon::error::Result;
use crate::input::joycon::{JOYCON_1_LEFT_PID, JOYCON_1_RIGHT_PID};
use crate::structs::{Vector2, Vector3};

/// Gyroscope scale factor: divide raw i16 values by this to get degrees per second
/// Joy-Con gyro raw values need scaling. If rotations are too sensitive, increase this value.
/// If rotations are too slow or not moving, decrease this value.
/// Typical range: 10-30 depending on sensor calibration
const JOYCON_GYRO_SCALE: f32 = 15.0;

/// Remap stick percent value with deadzone and range expansion
/// - Deadzone: 45-55% maps to 50% (center)
/// - Values outside deadzone are remapped to use more of 0-100% range
pub(crate) fn remap_stick_percent(raw_percent: f32) -> u8 {
    const DEADZONE_LOW: f32 = 45.0;
    const DEADZONE_HIGH: f32 = 55.0;
    const CENTER: f32 = 50.0;

    if raw_percent >= DEADZONE_LOW && raw_percent <= DEADZONE_HIGH {
        // In deadzone - map to center
        CENTER as u8
    } else if raw_percent < DEADZONE_LOW {
        // Below deadzone - remap from [0, 45] to [0, 50]
        // This stretches the lower range to use more of 0-50%
        let normalized = raw_percent / DEADZONE_LOW; // 0.0 to 1.0
        // Apply a curve to bias towards 0 (more sensitive near edges)
        let curved = normalized * normalized; // Quadratic curve
        (curved * CENTER) as u8
    } else {
        // Above deadzone - remap from [55, 100] to [50, 100]
        // This stretches the upper range to use more of 50-100%
        let normalized = (raw_percent - DEADZONE_HIGH) / (100.0 - DEADZONE_HIGH); // 0.0 to 1.0
        // Apply a curve to bias towards 100 (more sensitive near edges)
        let curved = normalized * normalized; // Quadratic curve
        (CENTER + (curved * CENTER)) as u8
    }
}

/// Decoded Joy-Con input report
#[derive(Debug, Clone)]
pub struct InputReport {
    /// Button states
    pub buttons: Buttons,
    /// Analog stick position as Vector2 (normalized -1.0 to 1.0, where 0.0 is center)
    pub stick: Vector2,
    /// Gyroscope data (degrees per second) as Vector3
    pub gyro: Vector3,
    /// Accelerometer data (g-force) as Vector3
    pub accel: Vector3,
    /// Battery level (0-4, where 4 is full)
    pub battery_level: u8,
    /// Whether the device is charging
    pub charging: bool,
}

/// Button states - different buttons available on Left vs Right Joy-Con
#[derive(Debug, Clone, Default)]
pub struct Buttons {
    // Right Joy-Con buttons
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    pub r: bool,
    pub zr: bool,
    pub home: bool,
    pub plus: bool,

    // Left Joy-Con buttons
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub l: bool,
    pub zl: bool,
    pub minus: bool,
    pub capture: bool,

    // Shared buttons
    pub sl: bool,
    pub sr: bool,
    pub stick_press: bool, // Left stick on left, right stick on right
}

impl InputReport {
    /// Decode a raw Joy-Con 1 input report buffer into structured data
    pub fn decode(data: &[u8], product_id: u16) -> Result<Self> {
        if data.len() < 49 {
            return Err(crate::input::joycon::error::JoyConError::InvalidData(
                format!(
                    "Input report too short: {} bytes (expected at least 49)",
                    data.len()
                ),
            ));
        }

        let is_left = product_id == JOYCON_1_LEFT_PID;
        let is_right = product_id == JOYCON_1_RIGHT_PID;

        // Joy-Con input report format (based on JoyConGD C++ code):
        // Byte 0: Report ID (0x30 for standard input report) - may be present
        // Byte 1: Timer/connection info
        // Byte 2: Battery (bit 4 = charging, bits 5-7 = level)
        // Byte 3: Right Joy-Con buttons (Y, X, B, A, SR, SL, R, ZR)
        // Byte 4: Shared buttons (Minus, Plus, R-Stick, L-Stick, Home, Capture)
        // Byte 5: Left Joy-Con buttons (Down, Up, Right, Left, SR, SL, L, ZL)
        // Bytes 6-8: Left stick (3 bytes, 12 bits X + 12 bits Y)
        // Bytes 9-11: Right stick (3 bytes, 12 bits X + 12 bits Y)
        // Bytes 13-18: Accelerometer (6 bytes, 3 x int16 little-endian)
        // Bytes 19-24: Gyroscope (6 bytes, 3 x int16 little-endian)

        // Auto-detect if report ID is present (0x30 at byte 0)
        // C++ code uses absolute byte indices (2, 3, 4, 5, etc.) assuming report ID is at byte 0
        // If report ID is NOT present, all indices are shifted by -1
        // So byte 2 becomes byte 1, byte 3 becomes byte 2, etc.
        let has_report_id = data[0] == 0x30;
        let offset = if has_report_id { 0 } else { 1 };
        // If report ID is present: offset = 0 (use indices as-is)
        // If report ID is NOT present: offset = 1 (subtract 1 from all indices)

        // Extract button states from bytes 3, 4, 5 (absolute indices like C++ code)
        // C++ uses: getNBitFromInputReport(3, ...), getNBitFromInputReport(4, ...), getNBitFromInputReport(5, ...)
        let button_idx_right = 3 - offset;
        let button_idx_shared = 4 - offset;
        let button_idx_left = 5 - offset;
        let button_byte_right = data[button_idx_right]; // Byte 3: Right buttons
        let button_byte_shared = data[button_idx_shared]; // Byte 4: Shared buttons  
        let button_byte_left = data[button_idx_left]; // Byte 5: Left buttons

        let mut buttons = Buttons::default();

        // Buttons are active HIGH (1 = pressed, 0 = not pressed)
        // Based on JoyConGD C++ code:

        if is_right {
            // Right Joy-Con buttons from byte 3
            buttons.y = (button_byte_right & 0x01) != 0;
            buttons.x = (button_byte_right & 0x02) != 0;
            buttons.b = (button_byte_right & 0x04) != 0;
            buttons.a = (button_byte_right & 0x08) != 0;
            buttons.sr = (button_byte_right & 0x10) != 0;
            buttons.sl = (button_byte_right & 0x20) != 0;
            buttons.r = (button_byte_right & 0x40) != 0;
            buttons.zr = (button_byte_right & 0x80) != 0;

            // Shared buttons from byte 4
            buttons.plus = (button_byte_shared & 0x02) != 0;
            buttons.stick_press = (button_byte_shared & 0x04) != 0; // R-Stick (bit 2) - Python uses (4, 2, 1)
            buttons.home = (button_byte_shared & 0x10) != 0;
        } else if is_left {
            // Left Joy-Con buttons from byte 5
            buttons.down = (button_byte_left & 0x01) != 0;
            buttons.up = (button_byte_left & 0x02) != 0;
            buttons.right = (button_byte_left & 0x04) != 0;
            buttons.left = (button_byte_left & 0x08) != 0;
            buttons.sr = (button_byte_left & 0x10) != 0;
            buttons.sl = (button_byte_left & 0x20) != 0;
            buttons.l = (button_byte_left & 0x40) != 0;
            buttons.zl = (button_byte_left & 0x80) != 0;

            // Shared buttons from byte 4
            buttons.minus = (button_byte_shared & 0x01) != 0;
            buttons.stick_press = (button_byte_shared & 0x08) != 0; // L-Stick (bit 3) - try bit 3 since right uses bit 2
            buttons.capture = (button_byte_shared & 0x20) != 0;
        }

        // Extract analog stick - bytes 6-8 for left, 9-11 for right (absolute indices)
        // Format: 3 bytes per stick, 12 bits X + 12 bits Y
        // Byte 0: X low 8 bits
        // Byte 1: X high 4 bits (low nibble) + Y low 4 bits (high nibble)
        // Byte 2: Y high 8 bits
        let stick = if is_left {
            // Left stick at bytes 6-8 (absolute indices like C++ code)
            let stick_start = 6 - offset;
            let stick_end = 9 - offset;
            if stick_end > data.len() {
                return Err(crate::input::joycon::error::JoyConError::InvalidData(
                    "Input report too short for left stick data".to_string(),
                ));
            }
            let stick_bytes = &data[stick_start..stick_end];
            let raw_x = (stick_bytes[0] as u16) | (((stick_bytes[1] & 0x0F) as u16) << 8);
            let raw_y = (((stick_bytes[1] & 0xF0) >> 4) as u16) | ((stick_bytes[2] as u16) << 4);

            // Normalize: 0-4095 -> 0.0-1.0 (center ~2048 = 0.5)
            let x_norm = raw_x as f32 / 4095.0;
            let y_norm = raw_y as f32 / 4095.0;
            // Convert to -1.0 to 1.0 range (center at 0.0)
            let x = (x_norm - 0.5) * 2.0;
            let y = (y_norm - 0.5) * 2.0;

            Vector2::new(x, y)
        } else {
            // Right stick at bytes 9-11 (absolute indices like C++ code)
            let stick_start = 9 - offset;
            let stick_end = 12 - offset;
            if stick_end > data.len() {
                return Err(crate::input::joycon::error::JoyConError::InvalidData(
                    "Input report too short for right stick data".to_string(),
                ));
            }
            let stick_bytes = &data[stick_start..stick_end];
            let raw_x = (stick_bytes[0] as u16) | (((stick_bytes[1] & 0x0F) as u16) << 8);
            let raw_y = (((stick_bytes[1] & 0xF0) >> 4) as u16) | ((stick_bytes[2] as u16) << 4);

            // Normalize: 0-4095 -> 0.0-1.0 (center ~2048 = 0.5)
            let x_norm = raw_x as f32 / 4095.0;
            let y_norm = raw_y as f32 / 4095.0;
            // Convert to -1.0 to 1.0 range (center at 0.0)
            let x = (x_norm - 0.5) * 2.0;
            let y = (y_norm - 0.5) * 2.0;

            Vector2::new(x, y)
        };

        // Extract battery and charging info from byte 2 (absolute index like C++ code)
        // Based on JoyConGD: bit 4 = charging, bits 5-7 = level (3 bits, 0-7)
        let battery_byte = data[2 - offset];
        let charging = (battery_byte & 0x10) != 0; // Bit 4
        let battery_raw = ((battery_byte & 0xE0) >> 5) as u8; // Bits 5-7 (3 bits)
        // Map 0-7 to 0-4 scale
        let battery_level = match battery_raw {
            0 | 1 => 0,
            2 | 3 => 1,
            4 | 5 => 2,
            6 => 3,
            7 => 4,
            _ => 0,
        };

        // Extract accelerometer data - bytes 13-18 (absolute indices like C++ code)
        // Based on JoyConGD: reads bytes individually and combines as little-endian int16
        // Accel X: bytes 13 (LSB), 14 (MSB)
        // Accel Y: bytes 15 (LSB), 16 (MSB)
        // Accel Z: bytes 17 (LSB), 18 (MSB)
        let accel = {
            let accel_start = 13 - offset;
            let accel_end = 19 - offset;
            if accel_end <= data.len() {
                let accel_x = i16::from_le_bytes([data[accel_start], data[accel_start + 1]]) as f32;
                let accel_y =
                    i16::from_le_bytes([data[accel_start + 2], data[accel_start + 3]]) as f32;
                let accel_z =
                    i16::from_le_bytes([data[accel_start + 4], data[accel_start + 5]]) as f32;
                Vector3::new(accel_x, accel_y, accel_z)
            } else {
                // Return zeros if data not available
                Vector3::zero()
            }
        };

        // Extract gyroscope data - bytes 19-24 (absolute indices like C++ code)
        // Based on JoyConGD: reads bytes individually and combines as little-endian int16
        // Gyro X: bytes 19 (LSB), 20 (MSB)
        // Gyro Y: bytes 21 (LSB), 22 (MSB)
        // Gyro Z: bytes 23 (LSB), 24 (MSB)
        // Joy-Con 1 gyro raw values need to be scaled to convert to degrees/second
        // Divide raw i16 values by JOYCON_GYRO_SCALE to get degrees per second
        let gyro = {
            let gyro_start = 19 - offset;
            let gyro_end = 25 - offset;
            if gyro_end <= data.len() {
                let gyro_x = i16::from_le_bytes([data[gyro_start], data[gyro_start + 1]]) as f32
                    / JOYCON_GYRO_SCALE;
                let gyro_y = i16::from_le_bytes([data[gyro_start + 2], data[gyro_start + 3]])
                    as f32
                    / JOYCON_GYRO_SCALE;
                let gyro_z = i16::from_le_bytes([data[gyro_start + 4], data[gyro_start + 5]])
                    as f32
                    / JOYCON_GYRO_SCALE;
                Vector3::new(gyro_x, gyro_y, gyro_z)
            } else {
                // Return zeros if data not available
                Vector3::zero()
            }
        };

        Ok(InputReport {
            buttons,
            stick,
            gyro,
            accel,
            battery_level,
            charging,
        })
    }

    /// Decode a raw Joy-Con 2 input report (Report 0x05) into structured data
    ///
    /// Based on: https://github.com/ndeadly/switch2_controller_research/blob/master/hid_reports.md
    /// Input Report 0x05 format:
    /// - Offset 0x0: Counter (4 bytes)
    /// - Offset 0x4: Buttons (4 bytes) - Bitfield
    /// - Offset 0x8: Unknown (2 bytes)
    /// - Offset 0xA: Left Analog Stick (3 bytes) - Packed 12-bit values
    /// - Offset 0xD: Right Analog Stick (3 bytes) - Packed 12-bit values
    /// - Offset 0x1F: Battery Voltage (2 bytes) - in mV
    /// - Offset 0x21: Charging State/Rate (1 byte)
    /// - Offset 0x2A: Motion Data (18 bytes) - if feature bit 2 is set
    pub fn decode_joycon2(data: &[u8], is_left: bool) -> Result<Self> {
        if data.len() < 43 {
            return Err(crate::input::joycon::error::JoyConError::InvalidData(
                format!("Joy-Con 2 input report too short: {} bytes", data.len()),
            ));
        }

        // Based on joycon2cpp C++ code:
        // int btnOffset = isLeft ? 4 : 3;
        // uint32_t state = (buffer[btnOffset] << 16) | (buffer[btnOffset + 1] << 8) | buffer[btnOffset + 2];
        // This creates a 24-bit value from 3 consecutive bytes
        let btn_offset = if is_left { 4 } else { 3 };

        if data.len() < btn_offset + 3 {
            return Err(crate::input::joycon::error::JoyConError::InvalidData(
                format!(
                    "Joy-Con 2 input report too short for buttons: {} bytes (need at least {})",
                    data.len(),
                    btn_offset + 3
                ),
            ));
        }

        // Build 24-bit state value: high byte << 16 | mid byte << 8 | low byte
        let state = ((data[btn_offset] as u32) << 16)
            | ((data[btn_offset + 1] as u32) << 8)
            | (data[btn_offset + 2] as u32);

        let mut buttons = Buttons::default();

        // Button masks from C++ code
        const BUTTON_A_MASK_RIGHT: u32 = 0x000800;
        const BUTTON_B_MASK_RIGHT: u32 = 0x000200;
        const BUTTON_X_MASK_RIGHT: u32 = 0x000400;
        const BUTTON_Y_MASK_RIGHT: u32 = 0x000100;
        const BUTTON_PLUS_MASK_RIGHT: u32 = 0x000002;
        const BUTTON_R_MASK_RIGHT: u32 = 0x004000;
        const BUTTON_STICK_MASK_RIGHT: u32 = 0x000004;
        const BUTTON_UP_MASK_LEFT: u32 = 0x000002;
        const BUTTON_DOWN_MASK_LEFT: u32 = 0x000001;
        const BUTTON_LEFT_MASK_LEFT: u32 = 0x000008;
        const BUTTON_RIGHT_MASK_LEFT: u32 = 0x000004;
        const BUTTON_MINUS_MASK_LEFT: u32 = 0x000100;
        const BUTTON_L_MASK_LEFT: u32 = 0x000040;
        const BUTTON_STICK_MASK_LEFT: u32 = 0x000800;

        // From decode_triggers_shoulders: ZL=0x000080 (bit 7), ZR=0x008000 (bit 15)
        // L=0x000040 (bit 6), R=0x004000 (bit 14) when upright
        const ZL_MASK: u32 = 0x000080;
        const ZR_MASK: u32 = 0x008000;

        if is_left {
            buttons.up = (state & BUTTON_UP_MASK_LEFT) != 0;
            buttons.down = (state & BUTTON_DOWN_MASK_LEFT) != 0;
            buttons.left = (state & BUTTON_LEFT_MASK_LEFT) != 0;
            buttons.right = (state & BUTTON_RIGHT_MASK_LEFT) != 0;
            buttons.minus = (state & BUTTON_MINUS_MASK_LEFT) != 0;
            buttons.l = (state & BUTTON_L_MASK_LEFT) != 0;
            buttons.stick_press = (state & BUTTON_STICK_MASK_LEFT) != 0;
            buttons.zl = (state & ZL_MASK) != 0;
            // SL/SR/Capture need to be checked from the original bytes
            // Based on previous working mappings, they're in bytes 5-6
            if data.len() > 6 {
                buttons.sl = (data[6] & 0x20) != 0;
                buttons.sr = (data[6] & 0x10) != 0;
            }
            if data.len() > 5 {
                buttons.capture = (data[5] & 0x20) != 0;
            }
        } else {
            buttons.a = (state & BUTTON_A_MASK_RIGHT) != 0;
            // B and X are flipped - swap them
            buttons.b = (state & BUTTON_X_MASK_RIGHT) != 0;
            buttons.x = (state & BUTTON_B_MASK_RIGHT) != 0;
            buttons.y = (state & BUTTON_Y_MASK_RIGHT) != 0;
            buttons.plus = (state & BUTTON_PLUS_MASK_RIGHT) != 0;
            buttons.r = (state & BUTTON_R_MASK_RIGHT) != 0;
            buttons.stick_press = (state & BUTTON_STICK_MASK_RIGHT) != 0;
            buttons.zr = (state & ZR_MASK) != 0;

            // SL/SR/Home need to be checked from the original bytes
            if data.len() > 4 {
                buttons.sl = (data[4] & 0x20) != 0;
                buttons.sr = (data[4] & 0x10) != 0;
            }
            if data.len() > 5 {
                buttons.home = (data[5] & 0x10) != 0;
            }
        }

        // Stick decoding - based on C++ code:
        // const uint8_t* data = isLeft ? &buffer[10] : &buffer[13];
        // int x_raw = ((data[1] & 0x0F) << 8) | data[0];
        // int y_raw = (data[2] << 4) | ((data[1] & 0xF0) >> 4);
        let stick_data_offset = if is_left { 10 } else { 13 };

        let (stick_x, stick_y) = if data.len() >= stick_data_offset + 3 {
            let stick_data = &data[stick_data_offset..stick_data_offset + 3];
            // Format: 3 bytes per stick, 12 bits X + 12 bits Y
            // Byte 0: X low 8 bits
            // Byte 1: X high 4 bits (low nibble) + Y low 4 bits (high nibble)
            // Byte 2: Y high 8 bits
            let x_raw = ((stick_data[1] & 0x0F) as u16) << 8 | stick_data[0] as u16;
            let y_raw = ((stick_data[2] as u16) << 4) | ((stick_data[1] & 0xF0) >> 4) as u16;
            (x_raw, y_raw)
        } else {
            (0, 0)
        };

        let max = 4095.0;
        let horizontal_norm = (stick_x as f32 / max).clamp(0.0, 1.0);
        let vertical_norm = (stick_y as f32 / max).clamp(0.0, 1.0);
        // Convert to -1.0 to 1.0 range (center at 0.0)
        let x = (horizontal_norm - 0.5) * 2.0;
        let y = (vertical_norm - 0.5) * 2.0;

        let stick = Vector2::new(x, y);

        // Battery & charging - approximate (same offsets for both formats)
        let battery_voltage = if data.len() >= 33 {
            u16::from_le_bytes([data[31], data[32]])
        } else {
            0
        };
        let charging_byte = if data.len() >= 34 { data[33] } else { 0 };
        let charging = (charging_byte & 0x20) != 0;
        let battery_level = if battery_voltage >= 4000 {
            4
        } else if battery_voltage >= 3800 {
            3
        } else if battery_voltage >= 3600 {
            2
        } else if battery_voltage >= 3400 {
            1
        } else {
            0
        };

        // Motion data decoding
        // Based on: https://github.com/ndeadly/switch2_controller_research/blob/master/hid_reports.md
        // Motion data is at 0x30-0x3B (same for both Report 0x05 and 0x08)
        // Accelerometer: Use raw signed 16-bit values to match Joy-Con 1 format (around 4000 when flat)
        // Gyroscope: Scale to degrees/second
        // Joy-Con 2 uses a different base scale than Joy-Con 1
        // Scale adjustment: if 90° physical = 135° virtual, reduce sensitivity by 1.5x
        // 9.25 * 1.5 = 13.875
        const JOYCON2_GYRO_SCALE: f32 = 13.875; // Adjusted: 90° physical = 90° virtual
        let (gyro, accel) = if data.len() >= 0x3C {
            // Accelerometer: raw values (no scaling) - matches Joy-Con 1 format
            let accel_x = i16::from_le_bytes([data[0x30], data[0x31]]) as f32;
            let accel_y = i16::from_le_bytes([data[0x32], data[0x33]]) as f32;
            let accel_z = i16::from_le_bytes([data[0x34], data[0x35]]) as f32;

            // Gyroscope: convert from raw i16 to degrees/second
            // Joy-Con 2 has different axis orientation than Joy-Con 1
            // Transform axes here so state.rs mapping produces correct result
            // state.rs does: -y → X, -z → Y, x → Z
            // We want: -x_raw → X, z_raw → Y, y_raw → Z
            // So set: y = x_raw, z = -z_raw, x = y_raw
            let gyro_x_raw =
                i16::from_le_bytes([data[0x36], data[0x37]]) as f32 / JOYCON2_GYRO_SCALE;
            let gyro_y_raw =
                i16::from_le_bytes([data[0x38], data[0x39]]) as f32 / JOYCON2_GYRO_SCALE;
            let gyro_z_raw =
                i16::from_le_bytes([data[0x3A], data[0x3B]]) as f32 / JOYCON2_GYRO_SCALE;

            // Transform: state.rs will do -y → X, -z → Y, x → Z
            // We want: -x_raw → X, z_raw → Y, y_raw → Z
            // So: y = x_raw (state.rs does -y = -x_raw → X ✓)
            //     z = -z_raw (state.rs does -z = -(-z_raw) = z_raw → Y ✓)
            //     x = y_raw (state.rs does x = y_raw → Z ✓)
            let gyro = Vector3::new(
                gyro_y_raw,  // x → Z (state.rs: x → Z)
                gyro_x_raw,  // y → X (state.rs: -y → X, so -gyro_x_raw → X)
                -gyro_z_raw, // z → Y (state.rs: -z → Y, so -(-gyro_z_raw) = gyro_z_raw → Y)
            );

            (gyro, Vector3::new(accel_x, accel_y, accel_z))
        } else {
            (Vector3::zero(), Vector3::zero())
        };

        Ok(InputReport {
            buttons,
            stick,
            gyro,
            accel,
            battery_level,
            charging,
        })
    }
}

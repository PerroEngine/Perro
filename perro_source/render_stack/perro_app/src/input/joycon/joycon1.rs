use super::shared::{self, ButtonBits, JoyConInputData};
use perro_input_api::{JoyConButton, JoyConSide, SignedUnitVector2};

pub(super) const JOYCON_VENDOR_ID: u16 = 0x057E;
pub(super) const JOYCON_L_PID: u16 = 0x2006;
pub(super) const JOYCON_R_PID: u16 = 0x2007;
const JOYCON1_GYRO_SCALE: f32 = 15.0;
pub(super) const JOYCON_SUBCMD_SET_PLAYER_LAMP: u8 = 0x30;

pub(super) fn decode_report_hid(data: &[u8], side: JoyConSide) -> Option<JoyConInputData> {
    decode_report_joycon1(data, side)
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
            shared::set_button_bit(&mut buttons, JoyConButton::Top, (btn_left & 0x02) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Bottom, (btn_left & 0x01) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Left, (btn_left & 0x08) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Right, (btn_left & 0x04) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Bumper, (btn_left & 0x40) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Trigger, (btn_left & 0x80) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::SL, (btn_left & 0x20) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::SR, (btn_left & 0x10) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Start, (btn_shared & 0x01) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Meta, (btn_shared & 0x20) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Stick, (btn_shared & 0x08) != 0);
        }
        JoyConSide::RJoyCon => {
            shared::set_button_bit(&mut buttons, JoyConButton::Top, (btn_right & 0x02) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Bottom, (btn_right & 0x04) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Left, (btn_right & 0x01) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Right, (btn_right & 0x08) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Bumper, (btn_right & 0x40) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Trigger, (btn_right & 0x80) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::SL, (btn_right & 0x20) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::SR, (btn_right & 0x10) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Start, (btn_shared & 0x02) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Meta, (btn_shared & 0x10) != 0);
            shared::set_button_bit(&mut buttons, JoyConButton::Stick, (btn_shared & 0x04) != 0);
        }
    }

    let stick = decode_stick(data, side, offset)?;
    let accel = decode_accel(data, offset);
    let gyro = decode_gyro(data, offset);

    Some(JoyConInputData {
        buttons,
        stick,
        raw_stick: None,
        mouse: None,
        gyro,
        accel,
    })
}

fn decode_stick(data: &[u8], side: JoyConSide, offset: usize) -> Option<SignedUnitVector2> {
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

    let x = shared::normalize_stick_axis(x_norm * 2.0 - 1.0);
    let y = shared::normalize_stick_axis(y_norm * 2.0 - 1.0);
    Some(SignedUnitVector2::new(x, y))
}

fn decode_accel(data: &[u8], offset: usize) -> (f32, f32, f32) {
    let start = match 13_usize.checked_sub(offset) {
        Some(v) => v,
        None => return (0.0, 0.0, 0.0),
    };

    if start + 5 < data.len() {
        let ax =
            i16::from_le_bytes([data[start], data[start + 1]]) as f32 * shared::ACCEL_GRAVITY_SCALE;
        let ay = i16::from_le_bytes([data[start + 2], data[start + 3]]) as f32
            * shared::ACCEL_GRAVITY_SCALE;
        let az = i16::from_le_bytes([data[start + 4], data[start + 5]]) as f32
            * shared::ACCEL_GRAVITY_SCALE;
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
        let gx = i16::from_le_bytes([data[start], data[start + 1]]) as f32 / JOYCON1_GYRO_SCALE;
        let gy = i16::from_le_bytes([data[start + 2], data[start + 3]]) as f32 / JOYCON1_GYRO_SCALE;
        let gz = i16::from_le_bytes([data[start + 4], data[start + 5]]) as f32 / JOYCON1_GYRO_SCALE;
        (gx, gy, gz)
    } else {
        (0.0, 0.0, 0.0)
    }
}

pub(super) fn enable_sensors(device: &hidapi::HidDevice) -> Result<(), hidapi::HidError> {
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

pub(super) fn send_hid_subcommand_with_rumble(
    device: &hidapi::HidDevice,
    packet_number: &mut u8,
    subcommand: u8,
    arg: u8,
) -> Result<(), hidapi::HidError> {
    let mut report = [0u8; 12];
    report[0] = 0x01;
    report[1] = *packet_number;
    // Neutral rumble frame bytes.
    report[2..10].copy_from_slice(&[0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40]);
    report[10] = subcommand;
    report[11] = arg;
    *packet_number = packet_number.wrapping_add(1) & 0x0F;
    device.write(&report)?;
    Ok(())
}

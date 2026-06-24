use super::shared::{self, ButtonBits, JoyConInputData, MouseSensorData, RawStick};
use btleplug::api::Peripheral as _;
use perro_input_api::{JoyConButton, JoyConSide, SignedUnitVector2};
use uuid::Uuid;

const NINTENDO_BLE_CID: u16 = 0x0553;
const JOYCON2_R_SIDE: u8 = 0x66;
const JOYCON2_L_SIDE: u8 = 0x67;
pub(super) const JOYCON2_INPUT_REPORT_05_UUID: Uuid =
    uuid::uuid!("ab7de9be-89fe-49ad-828f-118f09df7fd2");
pub(super) const JOYCON2_INPUT_REPORT_07_UUID: Uuid =
    uuid::uuid!("cc1bbbb5-7354-4d32-a716-a81cb241a32a");
pub(super) const JOYCON2_INPUT_REPORT_08_UUID: Uuid =
    uuid::uuid!("d5a9e01e-2ffc-4cca-b20c-8b67142bf442");
pub(super) const JOYCON2_WRITE_COMMAND_UUID: Uuid =
    uuid::uuid!("649d4ac9-8eb7-4e6c-af44-1ea54fe5f005");
pub(super) const JOYCON2_VIBRATION_L_UUID: Uuid =
    uuid::uuid!("289326cb-a471-485d-a8f4-240c14f18241");
pub(super) const JOYCON2_VIBRATION_R_UUID: Uuid =
    uuid::uuid!("fa19b0fb-cd1f-46a7-84a1-bbb09e00c149");
pub(super) const JOYCON2_RUMBLE_L_UUID: Uuid = uuid::uuid!("ce49a830-dced-48ae-931e-c8cf88aadbea");
pub(super) const JOYCON2_RUMBLE_R_UUID: Uuid = uuid::uuid!("65a724b3-f1e7-4a61-8078-a342376b27ff");

pub(super) async fn classify_joycon2_ble(
    peripheral: &btleplug::platform::Peripheral,
) -> Option<(JoyConSide, String, &'static str)> {
    let props = peripheral.properties().await.ok().flatten()?;

    let mut side = None;
    let mut tag = "";

    if let Some(data) = props.manufacturer_data.get(&NINTENDO_BLE_CID) {
        if data.contains(&JOYCON2_L_SIDE) {
            side = Some(JoyConSide::LJoyCon);
            tag = "cid+side(L)";
        } else if data.contains(&JOYCON2_R_SIDE) {
            side = Some(JoyConSide::RJoyCon);
            tag = "cid+side(R)";
        } else {
            tag = "cid-no-side";
        }
    }

    if side.is_none()
        && let Some(name) = props.local_name.as_deref()
        && (contains_ascii_case_insensitive(name, "joy-con")
            || contains_ascii_case_insensitive(name, "joycon")
            || contains_ascii_case_insensitive(name, "nintendo"))
    {
        if contains_ascii_case_insensitive(name, "(l)")
            || contains_ascii_case_insensitive(name, " left")
        {
            side = Some(JoyConSide::LJoyCon);
            tag = "name(L)";
        } else if contains_ascii_case_insensitive(name, "(r)")
            || contains_ascii_case_insensitive(name, " right")
        {
            side = Some(JoyConSide::RJoyCon);
            tag = "name(R)";
        }
    }

    let side = match side {
        Some(s) => s,
        None => return None,
    };

    let mut serial = format!("{:?}", peripheral.id());
    if serial.starts_with("PeripheralId(") && serial.ends_with(')') {
        serial.truncate(serial.len() - 1);
        serial.drain(.."PeripheralId(".len());
    }
    serial.retain(|ch| ch != ':');
    serial.make_ascii_uppercase();

    Some((side, serial, tag))
}

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

pub(super) fn decode_report_ble(data: &[u8], side: JoyConSide) -> Option<JoyConInputData> {
    decode_report_joycon2(data, side)
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
        shared::set_button_bit(&mut buttons, JoyConButton::Top, (state & 0x000002) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Bottom, (state & 0x000001) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Left, (state & 0x000008) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Right, (state & 0x000004) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Bumper, (state & 0x000040) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Trigger, (state & 0x000080) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Stick, (state & 0x000800) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::SL, (data[base + 6] & 0x20) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::SR, (data[base + 6] & 0x10) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Start, (state & 0x000100) != 0);
        shared::set_button_bit(
            &mut buttons,
            JoyConButton::Meta,
            (data[base + 5] & 0x20) != 0,
        );
    } else {
        // Joy-Con 2 right face buttons: observed stream indicates Top/Bottom are inverted
        // vs legacy masks, so map accordingly.
        shared::set_button_bit(&mut buttons, JoyConButton::Top, (state & 0x000200) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Bottom, (state & 0x000400) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Left, (state & 0x000100) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Right, (state & 0x000800) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Bumper, (state & 0x004000) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Trigger, (state & 0x008000) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Stick, (state & 0x000004) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::SL, (data[base + 4] & 0x20) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::SR, (data[base + 4] & 0x10) != 0);
        shared::set_button_bit(&mut buttons, JoyConButton::Start, (state & 0x000002) != 0);
        shared::set_button_bit(
            &mut buttons,
            JoyConButton::Meta,
            (data[base + 5] & 0x10) != 0,
        );
    }

    let stick_offsets: &[usize] = if is_left {
        &[base + 10, base + 8]
    } else {
        &[base + 5, base + 13, base + 10]
    };
    let raw_stick = decode_stick_best_candidate(data, stick_offsets)
        .map(|(x, y)| RawStick { x, y })
        .filter(|raw| raw.x != 0 || raw.y != 0);
    let stick = if let Some(raw) = raw_stick {
        let x = shared::normalize_stick_axis(((raw.x as f32 / 4095.0).clamp(0.0, 1.0) - 0.5) * 2.0);
        let y = shared::normalize_stick_axis(((raw.y as f32 / 4095.0).clamp(0.0, 1.0) - 0.5) * 2.0);
        SignedUnitVector2::new(x, y)
    } else {
        SignedUnitVector2::ZERO
    };

    let mouse = decode_mouse_sensor(data, base);
    let accel = normalize_accel_to_one_g(decode_joycon2_accel_best_candidate(data, base));
    let gyro = decode_joycon2_gyro_best_candidate(data, base);

    Some(JoyConInputData {
        buttons,
        stick,
        raw_stick,
        mouse,
        gyro,
        accel,
    })
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

fn decode_mouse_sensor(data: &[u8], base: usize) -> Option<MouseSensorData> {
    let start = base + 0x0E;
    if start + 7 >= data.len() {
        return None;
    }
    let x = i16::from_le_bytes([data[start], data[start + 1]]) as f32;
    let y = i16::from_le_bytes([data[start + 2], data[start + 3]]) as f32;
    let extra = i16::from_le_bytes([data[start + 4], data[start + 5]]) as f32;
    let distance = u16::from_le_bytes([data[start + 6], data[start + 7]]) as f32;
    Some(MouseSensorData {
        x,
        y,
        extra,
        distance,
    })
}

fn decode_joycon2_accel_best_candidate(data: &[u8], base: usize) -> (f32, f32, f32) {
    let motion_offsets = [base + 0x30, base + 0x2A, base + 0x24];
    let mut best = (0.0, 0.0, 0.0);
    let mut best_score = f32::INFINITY;
    for start in motion_offsets {
        if start + 5 >= data.len() {
            continue;
        }
        let ax =
            i16::from_le_bytes([data[start], data[start + 1]]) as f32 * shared::ACCEL_GRAVITY_SCALE;
        let ay = i16::from_le_bytes([data[start + 2], data[start + 3]]) as f32
            * shared::ACCEL_GRAVITY_SCALE;
        let az = i16::from_le_bytes([data[start + 4], data[start + 5]]) as f32
            * shared::ACCEL_GRAVITY_SCALE;
        let amag = (ax * ax + ay * ay + az * az).sqrt();
        // Prefer realistic gravity magnitude region before normalization.
        let score = (amag - shared::ACCEL_ONE_G_TARGET).abs();
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
        let gx_raw =
            i16::from_le_bytes([data[start + 6], data[start + 7]]) as f32 / JOYCON2_GYRO_SCALE;
        let gy_raw =
            i16::from_le_bytes([data[start + 8], data[start + 9]]) as f32 / JOYCON2_GYRO_SCALE;
        let gz_raw =
            i16::from_le_bytes([data[start + 10], data[start + 11]]) as f32 / JOYCON2_GYRO_SCALE;
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
    let gain = (shared::ACCEL_ONE_G_TARGET / amag).clamp(0.5, 2.0);
    (accel.0 * gain, accel.1 * gain, accel.2 * gain)
}

pub(super) async fn send_joycon2_player_lamp(
    peripheral: &btleplug::platform::Peripheral,
    cmd_char: &btleplug::api::Characteristic,
    pattern: u8,
) {
    let mut report = [0u8; 16];
    report[..8].copy_from_slice(&[0x09, 0x91, 0x01, 0x07, 0x00, 0x08, 0x00, 0x00]);
    report[8] = pattern;
    let _ = peripheral
        .write(cmd_char, &report, btleplug::api::WriteType::WithoutResponse)
        .await;
}

pub(super) async fn send_joycon2_rumble(
    peripheral: &btleplug::platform::Peripheral,
    vibration_char: &btleplug::api::Characteristic,
    counter: &mut u8,
    amplitude: f32,
) {
    let mut report = [0u8; 42];
    let amp = amplitude.clamp(0.0, 1.0);
    if amp >= 0.01 {
        let sample = if amp > 0.5 {
            [0x93, 0x35, 0x36, 0x1C, 0x0D]
        } else {
            [0x4B, 0x7D, 0x80, 0x5A, 0x02]
        };
        report[1] = 0x50 | (*counter & 0x0F);
        report[2..7].copy_from_slice(&sample);
        *counter = counter.wrapping_add(1) & 0x0F;
    }
    let _ = peripheral
        .write(
            vibration_char,
            &report,
            btleplug::api::WriteType::WithoutResponse,
        )
        .await;
}

pub(super) async fn send_joycon2_enable_sequence(
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

pub(super) async fn send_joycon2_full_init_sequence(
    peripheral: &btleplug::platform::Peripheral,
    init_char: &btleplug::api::Characteristic,
) {
    const INIT_COMMANDS: &[&[u8]] = &[
        &[0x07, 0x91, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00],
        &[
            0x02, 0x91, 0x01, 0x04, 0x00, 0x08, 0x00, 0x00, 0x40, 0x7E, 0x00, 0x00, 0x00, 0x30,
            0x01, 0x00,
        ],
        &[0x10, 0x91, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00],
        &[0x16, 0x91, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00],
        &[
            0x0A, 0x91, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
        ],
        &[
            0x09, 0x91, 0x01, 0x07, 0x00, 0x08, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
        &[
            0x0C, 0x91, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0x37, 0x00, 0x00, 0x00,
        ],
        &[
            0x02, 0x91, 0x01, 0x04, 0x00, 0x08, 0x00, 0x00, 0x40, 0x7E, 0x00, 0x00, 0x80, 0x30,
            0x01, 0x00,
        ],
        &[
            0x02, 0x91, 0x01, 0x04, 0x00, 0x08, 0x00, 0x00, 0x40, 0x7E, 0x00, 0x00, 0x40, 0xC0,
            0x1F, 0x00,
        ],
        &[
            0x02, 0x91, 0x01, 0x04, 0x00, 0x08, 0x00, 0x00, 0x10, 0x7E, 0x00, 0x00, 0x40, 0x30,
            0x01, 0x00,
        ],
        &[
            0x02, 0x91, 0x01, 0x04, 0x00, 0x08, 0x00, 0x00, 0x18, 0x7E, 0x00, 0x00, 0x00, 0x31,
            0x01, 0x00,
        ],
        &[0x11, 0x91, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00],
        &[
            0x02, 0x91, 0x01, 0x04, 0x00, 0x08, 0x00, 0x00, 0x20, 0x7E, 0x00, 0x00, 0x60, 0x30,
            0x01, 0x00,
        ],
        &[
            0x0A, 0x91, 0x01, 0x08, 0x00, 0x14, 0x00, 0x00, 0x01, 0x59, 0x09, 0x00, 0x00, 0xFF,
            0xFF, 0xFF, 0xFF, 0x35, 0x00, 0x46, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        &[0x11, 0x91, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00],
        &[
            0x0C, 0x91, 0x01, 0x04, 0x00, 0x04, 0x00, 0x00, 0x37, 0x00, 0x00, 0x00,
        ],
    ];

    for command in INIT_COMMANDS {
        let mut report = vec![0u8; 17];
        report.extend_from_slice(command);
        let _ = peripheral
            .write(
                init_char,
                &report,
                btleplug::api::WriteType::WithoutResponse,
            )
            .await;
        tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
    }
}

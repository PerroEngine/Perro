use perro_input_api::{JoyConButton, SignedUnitVector2};

pub(super) const STICK_DEADZONE: f32 = 0.08;
pub(super) const STICK_AXIS_GAIN_POS: f32 = 1.85;
pub(super) const STICK_AXIS_GAIN_NEG: f32 = 1.45;
pub(super) const ACCEL_GRAVITY_SCALE: f32 = 0.2386;
pub(super) const ACCEL_ONE_G_TARGET: f32 = 1000.0;

pub(super) type ButtonBits = u16;

#[derive(Debug, Clone)]
pub(super) struct JoyConInputData {
    pub(super) buttons: ButtonBits,
    pub(super) stick: SignedUnitVector2,
    pub(super) raw_stick: Option<RawStick>,
    pub(super) mouse: Option<MouseSensorData>,
    pub(super) gyro: (f32, f32, f32),
    pub(super) accel: (f32, f32, f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RawStick {
    pub(super) x: u16,
    pub(super) y: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct MouseSensorData {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) extra: f32,
    pub(super) distance: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct StickCalibration {
    pub(super) center_x: u16,
    pub(super) center_y: u16,
    pub(super) min_x: u16,
    pub(super) max_x: u16,
    pub(super) min_y: u16,
    pub(super) max_y: u16,
}

impl StickCalibration {
    pub(super) const fn default() -> Self {
        Self {
            center_x: 2048,
            center_y: 2048,
            min_x: 300,
            max_x: 3800,
            min_y: 300,
            max_y: 3800,
        }
    }

    pub(super) fn set_center(&mut self, raw: RawStick) {
        self.center_x = raw.x;
        self.center_y = raw.y;
        self.min_x = self.min_x.min(raw.x.saturating_sub(256));
        self.max_x = self.max_x.max(raw.x.saturating_add(256).min(4095));
        self.min_y = self.min_y.min(raw.y.saturating_sub(256));
        self.max_y = self.max_y.max(raw.y.saturating_add(256).min(4095));
    }

    pub(super) fn learn_extents(&mut self, raw: RawStick) -> bool {
        let mut changed = false;
        if raw.x < self.min_x {
            self.min_x = raw.x;
            changed = true;
        }
        if raw.x > self.max_x {
            self.max_x = raw.x;
            changed = true;
        }
        if raw.y < self.min_y {
            self.min_y = raw.y;
            changed = true;
        }
        if raw.y > self.max_y {
            self.max_y = raw.y;
            changed = true;
        }
        changed
    }

    pub(super) fn normalize(&self, raw: RawStick) -> SignedUnitVector2 {
        let x = calibrated_axis(raw.x, self.center_x, self.min_x, self.max_x);
        let y = calibrated_axis(raw.y, self.center_y, self.min_y, self.max_y);
        SignedUnitVector2::new(normalize_stick_axis(x), normalize_stick_axis(y))
    }
}

fn calibrated_axis(raw: u16, center: u16, min_val: u16, max_val: u16) -> f32 {
    if min_val >= max_val {
        return ((raw as f32 / 4095.0).clamp(0.0, 1.0) - 0.5) * 2.0;
    }
    if raw >= center {
        let range = max_val.saturating_sub(center).max(1) as f32;
        ((raw - center) as f32 / range).clamp(0.0, 1.0)
    } else {
        let range = center.saturating_sub(min_val).max(1) as f32;
        -((center - raw) as f32 / range).clamp(0.0, 1.0)
    }
}

pub(super) fn set_button_bit(bits: &mut ButtonBits, button: JoyConButton, is_down: bool) {
    let bit = 1u16 << (button.as_index() as u16);
    if is_down {
        *bits |= bit;
    } else {
        *bits &= !bit;
    }
}

fn apply_stick_deadzone(v: f32) -> f32 {
    let a = v.abs();
    if a < STICK_DEADZONE {
        0.0
    } else {
        v.signum() * ((a - STICK_DEADZONE) / (1.0 - STICK_DEADZONE))
    }
}

pub(super) fn normalize_stick_axis(v: f32) -> f32 {
    let gain = if v >= 0.0 {
        STICK_AXIS_GAIN_POS
    } else {
        STICK_AXIS_GAIN_NEG
    };
    apply_stick_deadzone((v * gain).clamp(-1.0, 1.0)).clamp(-1.0, 1.0)
}

pub(super) fn imu_is_zero(gyro: (f32, f32, f32), accel: (f32, f32, f32)) -> bool {
    gyro.0 == 0.0
        && gyro.1 == 0.0
        && gyro.2 == 0.0
        && accel.0 == 0.0
        && accel.1 == 0.0
        && accel.2 == 0.0
}

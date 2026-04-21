use std::ops::{Add, Div, Mul, Sub};

trait FloatScalar:
    Copy
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
{
    const ZERO: Self;
    const HALF: Self;
    const ONE: Self;
    const TWO: Self;
    const THREE: Self;
    const EPSILON: Self;

    fn abs(self) -> Self;
    fn asin(self) -> Self;
    fn sin(self) -> Self;
}

impl FloatScalar for f32 {
    const ZERO: Self = 0.0;
    const HALF: Self = 0.5;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;
    const THREE: Self = 3.0;
    const EPSILON: Self = f32::EPSILON;

    #[inline]
    fn abs(self) -> Self {
        self.abs()
    }

    #[inline]
    fn asin(self) -> Self {
        self.asin()
    }

    #[inline]
    fn sin(self) -> Self {
        self.sin()
    }
}

impl FloatScalar for f64 {
    const ZERO: Self = 0.0;
    const HALF: Self = 0.5;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;
    const THREE: Self = 3.0;
    const EPSILON: Self = f64::EPSILON;

    #[inline]
    fn abs(self) -> Self {
        self.abs()
    }

    #[inline]
    fn asin(self) -> Self {
        self.asin()
    }

    #[inline]
    fn sin(self) -> Self {
        self.sin()
    }
}

#[inline]
fn clamp_core<T: FloatScalar>(value: T, min: T, max: T) -> T {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[inline]
fn clamp01_core<T: FloatScalar>(value: T) -> T {
    clamp_core(value, T::ZERO, T::ONE)
}

#[inline]
fn lerp_core<T: FloatScalar>(start: T, end: T, t: T) -> T {
    start + (end - start) * t
}

#[inline]
fn ilerp_core<T: FloatScalar>(start: T, end: T, value: T) -> T {
    let span = end - start;
    if span.abs() <= T::EPSILON {
        T::ZERO
    } else {
        (value - start) / span
    }
}

#[inline]
fn smoothstep01_core<T: FloatScalar>(value: T) -> T {
    value * value * (T::THREE - T::TWO * value)
}

#[inline]
fn smoothstep_core<T: FloatScalar>(edge0: T, edge1: T, value: T) -> T {
    let t = clamp01_core(ilerp_core(edge0, edge1, value));
    smoothstep01_core(t)
}

#[inline]
fn ismoothstep01_core<T: FloatScalar>(value: T) -> T {
    let x = clamp01_core(value);
    T::HALF - ((T::ONE - T::TWO * x).asin() / T::THREE).sin()
}

#[inline]
pub const fn deg_to_rad(degrees: f32) -> f32 {
    degrees.to_radians()
}

#[inline]
pub const fn rad_to_deg(radians: f32) -> f32 {
    radians.to_degrees()
}

#[inline]
pub fn clamp01(value: f32) -> f32 {
    clamp01_core(value)
}

#[inline]
pub fn lerp(start: f32, end: f32, t: f32) -> f32 {
    lerp_core(start, end, t)
}

#[inline]
pub fn ilerp(start: f32, end: f32, value: f32) -> f32 {
    ilerp_core(start, end, value)
}

#[inline]
pub fn remap(in_min: f32, in_max: f32, out_min: f32, out_max: f32, value: f32) -> f32 {
    let t = ilerp_core(in_min, in_max, value);
    lerp_core(out_min, out_max, t)
}

#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    smoothstep_core(edge0, edge1, value)
}

#[inline]
pub fn slerp(start: f32, end: f32, t: f32) -> f32 {
    lerp_core(start, end, smoothstep_core(0.0, 1.0, t))
}

#[inline]
pub fn islerp(start: f32, end: f32, value: f32) -> f32 {
    let smooth_t = clamp01_core(ilerp_core(start, end, value));
    ismoothstep(0.0, 1.0, smooth_t)
}

#[inline]
pub fn ismoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = clamp01_core(ilerp_core(edge0, edge1, value));
    ismoothstep01_core(t)
}

#[inline]
pub fn angle_diff_rad(from: f32, to: f32) -> f32 {
    wrap_angle_rad(to - from)
}

#[inline]
pub fn angle_diff_deg(from: f32, to: f32) -> f32 {
    wrap_angle_deg(to - from)
}

#[inline]
pub fn lerp_angle_rad(from: f32, to: f32, t: f32) -> f32 {
    from + angle_diff_rad(from, to) * t
}

#[inline]
pub fn lerp_angle_deg(from: f32, to: f32, t: f32) -> f32 {
    from + angle_diff_deg(from, to) * t
}

#[inline]
pub fn wrap_angle_rad(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

#[inline]
pub fn wrap_angle_deg(angle: f32) -> f32 {
    (angle + 180.0).rem_euclid(360.0) - 180.0
}

#[inline]
pub fn approach(current: f32, target: f32, max_delta: f32) -> f32 {
    if max_delta <= 0.0 {
        return current;
    }

    let delta = target - current;
    if delta.abs() <= max_delta {
        target
    } else {
        current + delta.signum() * max_delta
    }
}

#[inline]
pub fn damp(current: f32, target: f32, lambda: f32, delta_time: f32) -> f32 {
    if lambda <= 0.0 || delta_time <= 0.0 {
        return current;
    }

    let k = 1.0 - (-lambda * delta_time).exp();
    lerp_core(current, target, k)
}

#[inline]
pub fn smooth_damp(
    current: f32,
    target: f32,
    current_velocity: f32,
    smooth_time: f32,
    max_speed: f32,
    delta_time: f32,
) -> (f32, f32) {
    if delta_time <= 0.0 {
        return (current, current_velocity);
    }

    let smooth_time = smooth_time.max(0.0001);
    let omega = 2.0 / smooth_time;
    let x = omega * delta_time;
    let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);

    let mut change = current - target;
    let original_target = target;
    let max_change = max_speed.max(0.0) * smooth_time;
    change = change.clamp(-max_change, max_change);
    let adjusted_target = current - change;

    let temp = (current_velocity + omega * change) * delta_time;
    let mut new_velocity = (current_velocity - omega * temp) * exp;
    let mut output = adjusted_target + (change + temp) * exp;

    if (original_target - current > 0.0) == (output > original_target) {
        output = original_target;
        new_velocity = 0.0;
    }

    (output, new_velocity)
}

#[inline]
pub fn repeat(value: f32, length: f32) -> f32 {
    if length <= 0.0 {
        return 0.0;
    }
    value.rem_euclid(length)
}

#[inline]
pub fn ping_pong(value: f32, length: f32) -> f32 {
    if length <= 0.0 {
        return 0.0;
    }

    let v = repeat(value, length * 2.0);
    length - (v - length).abs()
}

#[inline]
pub fn nearly_eq(a: f32, b: f32, epsilon: f32) -> bool {
    (a - b).abs() <= epsilon.abs()
}

#[cfg(test)]
#[path = "../tests/unit/math_tests.rs"]
mod tests;

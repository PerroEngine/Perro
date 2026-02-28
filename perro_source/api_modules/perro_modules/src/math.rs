#[inline]
pub const fn deg_to_rad(degrees: f32) -> f32 {
    degrees.to_radians()
}

#[inline]
pub const fn rad_to_deg(radians: f32) -> f32 {
    radians.to_degrees()
}

#[macro_export]
macro_rules! deg_to_rad {
    ($degrees:expr) => {
        $crate::math::deg_to_rad($degrees as f32)
    };
}

#[macro_export]
macro_rules! rad_to_deg {
    ($radians:expr) => {
        $crate::math::rad_to_deg($radians as f32)
    };
}

#[cfg(test)]
#[path = "../tests/unit/math_tests.rs"]
mod tests;

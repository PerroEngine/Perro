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
mod tests {
    #[test]
    fn deg_to_rad_matches_std() {
        let expected = 180.0f32.to_radians();
        let actual = super::deg_to_rad(180.0);
        assert!((actual - expected).abs() < 1.0e-6);
    }

    #[test]
    fn rad_to_deg_matches_std() {
        let expected = std::f32::consts::PI.to_degrees();
        let actual = super::rad_to_deg(std::f32::consts::PI);
        assert!((actual - expected).abs() < 1.0e-6);
    }
}

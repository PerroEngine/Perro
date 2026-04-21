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

#[test]
fn clamp01_clamps_values() {
    assert_eq!(super::clamp01(-1.0), 0.0);
    assert_eq!(super::clamp01(0.25), 0.25);
    assert_eq!(super::clamp01(2.0), 1.0);
}

#[test]
fn generic_math_core_supports_f64() {
    let t = super::ilerp_core(10.0_f64, 20.0_f64, 15.0_f64);
    assert!((t - 0.5).abs() < 1.0e-12);
    let s = super::smoothstep_core(0.0_f64, 1.0_f64, 0.5_f64);
    assert!((s - 0.5).abs() < 1.0e-12);
}

#[test]
fn lerp_and_ilerp_match() {
    let mid = super::lerp(10.0, 20.0, 0.5);
    assert!((mid - 15.0).abs() < 1.0e-6);

    let t = super::ilerp(10.0, 20.0, 15.0);
    assert!((t - 0.5).abs() < 1.0e-6);
}

#[test]
fn ilerp_handles_degenerate_range() {
    assert_eq!(super::ilerp(2.0, 2.0, 3.0), 0.0);
}

#[test]
fn slerp_and_islerp_match() {
    for t in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        let value = super::slerp(10.0, 20.0, t);
        let back = super::islerp(10.0, 20.0, value);
        assert!((back - t).abs() < 1.0e-5, "t={t} value={value} back={back}");
    }

    let curved = super::slerp(0.0, 10.0, 0.25);
    assert!((curved - 1.5625).abs() < 1.0e-6);
}

#[test]
fn smoothstep_and_ismoothstep_match() {
    for t in [0.0f32, 0.2, 0.5, 0.8, 1.0] {
        let smooth = super::smoothstep(0.0, 1.0, t);
        let back = super::ismoothstep(0.0, 1.0, smooth);
        assert!(
            (back - t).abs() < 1.0e-5,
            "t={t} smooth={smooth} back={back}"
        );
    }
}

#[test]
fn angle_diff_and_lerp_angle_follow_shortest_path() {
    let diff = super::angle_diff_deg(170.0, -170.0);
    assert!((diff - 20.0).abs() < 1.0e-6);

    let mid = super::lerp_angle_deg(170.0, -170.0, 0.5);
    assert!((super::wrap_angle_deg(mid) - (-180.0)).abs() < 1.0e-4);

    let diff_rad = super::angle_diff_rad(3.0, -3.0);
    assert!(diff_rad.abs() < 1.0);
}

#[test]
fn damp_moves_toward_target() {
    let next = super::damp(0.0, 10.0, 4.0, 1.0 / 60.0);
    assert!(next > 0.0 && next < 10.0);
    assert_eq!(super::damp(1.0, 10.0, 0.0, 1.0 / 60.0), 1.0);
}

#[test]
fn smooth_damp_converges() {
    let mut value = 0.0;
    let mut velocity = 0.0;

    for _ in 0..180 {
        let (next_value, next_velocity) =
            super::smooth_damp(value, 10.0, velocity, 0.2, 100.0, 1.0 / 60.0);
        value = next_value;
        velocity = next_velocity;
    }

    assert!((value - 10.0).abs() < 0.05, "value={value}");
    let (same_value, same_velocity) = super::smooth_damp(2.0, 10.0, 1.0, 0.2, 100.0, 0.0);
    assert_eq!(same_value, 2.0);
    assert_eq!(same_velocity, 1.0);
}

#[test]
fn repeat_and_ping_pong_work() {
    assert!((super::repeat(7.5, 2.0) - 1.5).abs() < 1.0e-6);
    assert!((super::repeat(-0.5, 2.0) - 1.5).abs() < 1.0e-6);
    assert!((super::ping_pong(2.5, 2.0) - 1.5).abs() < 1.0e-6);
}

#[test]
fn nearly_eq_works() {
    assert!(super::nearly_eq(1.0, 1.0001, 0.001));
    assert!(!super::nearly_eq(1.0, 1.1, 0.001));
}

#[test]
fn remap_maps_between_ranges() {
    let out = super::remap(0.0, 10.0, 100.0, 200.0, 5.0);
    assert!((out - 150.0).abs() < 1.0e-6);
}

#[test]
fn smoothstep_clamps_and_smooths() {
    assert_eq!(super::smoothstep(0.0, 1.0, -5.0), 0.0);
    assert_eq!(super::smoothstep(0.0, 1.0, 5.0), 1.0);
    let middle = super::smoothstep(0.0, 1.0, 0.5);
    assert!((middle - 0.5).abs() < 1.0e-6);
}

#[test]
fn wrap_angle_rad_wraps_to_pi_range() {
    let wrapped = super::wrap_angle_rad(std::f32::consts::PI * 2.5);
    assert!((wrapped - std::f32::consts::FRAC_PI_2).abs() < 1.0e-6);
    assert!((-std::f32::consts::PI..std::f32::consts::PI).contains(&wrapped));
}

#[test]
fn wrap_angle_deg_wraps_to_180_range() {
    let wrapped = super::wrap_angle_deg(450.0);
    assert!((wrapped - 90.0).abs() < 1.0e-6);
    assert!((-180.0..180.0).contains(&wrapped));
}

#[test]
fn approach_moves_without_overshoot() {
    assert_eq!(super::approach(0.0, 10.0, 3.0), 3.0);
    assert_eq!(super::approach(9.0, 10.0, 3.0), 10.0);
    assert_eq!(super::approach(0.0, 10.0, 0.0), 0.0);
}

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

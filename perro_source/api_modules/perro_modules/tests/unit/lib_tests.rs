#[test]
fn log_macros_typecheck_and_forward() {
    let v = 42;
    crate::log_print!("print {v}");
    crate::log_info!("info {v}");
    crate::log_warn!("warn {v}");
    crate::log_error!("error {v}");
}

#[test]
fn math_macros_typecheck_and_forward() {
    let degrees = 180.0;
    let radians = std::f32::consts::PI;
    let _ = crate::math::deg_to_rad(degrees);
    let _ = crate::math::rad_to_deg(radians);
    let _ = crate::math::clamp01(2.0);
    let _ = crate::math::lerp(0.0, 1.0, 0.5);
    let _ = crate::math::ilerp(0.0, 10.0, 5.0);
    let _ = crate::math::slerp(0.0, 1.0, 0.5);
    let _ = crate::math::islerp(0.0, 1.0, 0.5);
    let _ = crate::math::ismoothstep(0.0, 1.0, 0.5);
    let _ = crate::math::angle_diff_rad(0.0, 1.0);
    let _ = crate::math::angle_diff_deg(0.0, 1.0);
    let _ = crate::math::lerp_angle_rad(0.0, 1.0, 0.5);
    let _ = crate::math::lerp_angle_deg(0.0, 1.0, 0.5);
    let _ = crate::math::remap(0.0, 1.0, 10.0, 20.0, 0.25);
    let _ = crate::math::smoothstep(0.0, 1.0, 0.5);
    let _ = crate::math::wrap_angle_rad(std::f32::consts::PI * 2.0);
    let _ = crate::math::wrap_angle_deg(540.0);
    let _ = crate::math::approach(0.0, 1.0, 0.1);
    let _ = crate::math::damp(0.0, 1.0, 4.0, 1.0 / 60.0);
    let _ = crate::math::smooth_damp(0.0, 1.0, 0.0, 0.2, 10.0, 1.0 / 60.0);
    let _ = crate::math::repeat(1.5, 1.0);
    let _ = crate::math::ping_pong(1.5, 1.0);
    let _ = crate::math::nearly_eq(1.0, 1.0, 0.0);
}

#[test]
fn random_api_typecheck_and_forward() {
    let seed = 42;
    let mut rng = crate::random::SeededRng::new(seed);

    let _ = crate::random::hash_u32(seed);
    let _ = crate::random::hash(seed);
    let _ = crate::random::hash_u128(seed as u128);
    let _ = crate::random::hash64_u64(seed as u64);
    let _ = crate::random::hash64_str("perro");
    let _ = crate::random::hash_combine(seed, seed + 1);
    let _ = crate::random::hash2_u32(seed, seed + 1);
    let _ = crate::random::hash_str("perro");
    let _ = crate::random::rand01(seed);
    let _ = crate::random::rand11(seed);
    let _ = crate::random::rand_range_f32(0.0, 1.0, seed);
    let _ = crate::random::rand_range(0.0f32, 1.0f32, seed);
    let _ = crate::random::rand_range_u32(0, 10, seed);
    let _ = crate::random::rand_range_i32(-10, 10, seed);
    let _ = crate::random::chance(0.5, seed);
    let _ = crate::random::choose_index(4, seed);
    let _ = crate::random::rand01_stream(seed, 2);
    let _ = crate::random::rand11_stream(seed, 2);
    let _ = crate::random::rand_unit_vec2(seed);
    let _ = crate::random::rand_unit_vec3(seed);
    let _ = crate::random::rand_in_circle(seed);
    let mut arr = [1, 2, 3];
    crate::random::shuffle(seed, &mut arr);
    let _ = rng.next_range_u32(0, 10);
    let _ = rng.next_range(0_u32, 10_u32);
    let _ = rng.next_range_i32(-10, 10);
    let _ = rng.next_range_f32(-1.0, 1.0);
    let _ = rng.next_chance(0.5);
    let _ = rng.next_index(4);
    let _ = rng.next_u32();
}

#[test]
fn hash_u32_known_values() {
    assert_eq!(super::hash_u32(0), 0x0000_0000);
    assert_eq!(super::hash_u32(1), 0x6889_90c0);
    assert_eq!(super::hash_u32(2), 0xd113_2181);
    assert_eq!(super::hash_u32(12_345), 0x912e_fcf7);
    assert_eq!(super::hash_u32(u32::MAX), 0x6768_824a);
}

#[test]
fn hash_helpers_cover_common_types() {
    assert_eq!(super::hash_str("perro"), super::hash_bytes(b"perro"));
    assert_eq!(super::hash64_str("perro"), super::hash64_bytes(b"perro"));
    assert_eq!(super::hash_i32(-42), super::hash_u32((-42_i32) as u32));
    assert_eq!(super::hash_i64(-42), super::hash_u64((-42_i64) as u64));
    assert_ne!(super::hash_u128(1), super::hash_u128(2));
    assert_ne!(super::hash64_u128(1), super::hash64_u128(2));
    assert_ne!(super::hash_bool(false), super::hash_bool(true));
    assert_ne!(super::hash_f32(1.0), super::hash_f32(2.0));
}

#[test]
fn generic_hash_and_range_work() {
    assert_eq!(super::hash(7_u32), super::hash_u32(7));
    assert_eq!(super::hash(-7_i32), super::hash_i32(-7));
    assert_eq!(super::hash(7_u64), super::hash_u64(7));

    let a: u32 = super::rand_range(10, 20, 55);
    let b: i32 = super::rand_range(-10, 10, 55);
    let c: f32 = super::rand_range(-2.0, 2.0, 55);
    assert!((10..20).contains(&a));
    assert!((-10..10).contains(&b));
    assert!((-2.0..=2.0).contains(&c));
}

#[test]
fn hash_combine_and_grid_hashes_change_per_input() {
    assert_ne!(super::hash_combine(1, 2), super::hash_combine(2, 1));
    assert_ne!(super::hash_combine3(1, 2, 3), super::hash_combine3(1, 2, 4));
    assert_ne!(
        super::hash_combine4(1, 2, 3, 4),
        super::hash_combine4(1, 2, 3, 5)
    );
    assert_ne!(super::hash2_u32(10, 20), super::hash2_u32(20, 10));
    assert_ne!(super::hash3_u32(10, 20, 30), super::hash3_u32(10, 20, 31));
}

#[test]
fn scalar_rand_stays_in_range() {
    for seed in [0, 1, 2, 3, 4, 7, 13, 37, 65_535, u32::MAX] {
        let r01 = super::rand01(seed);
        let r11 = super::rand11(seed);
        assert!((0.0..=1.0).contains(&r01), "seed={seed} r01={r01}");
        assert!((-1.0..=1.0).contains(&r11), "seed={seed} r11={r11}");
    }
}

#[test]
fn random_ranges_and_chance_work() {
    for seed in 0..128 {
        let uf = super::rand_range_f32(-2.0, 4.0, seed);
        let ui = super::rand_range_i32(-10, 10, seed);
        let uu = super::rand_range_u32(10, 20, seed);
        assert!((-2.0..=4.0).contains(&uf));
        assert!((-10..10).contains(&ui));
        assert!((10..20).contains(&uu));
    }

    assert!(!super::chance(0.0, 7));
    assert!(super::chance(1.0, 7));
}

#[test]
fn random_choose_shuffle_and_vectors_work() {
    assert_eq!(super::choose_index(0, 7), None);
    let idx = super::choose_index(10, 7).unwrap();
    assert!(idx < 10);

    let mut a = [1, 2, 3, 4, 5, 6];
    let mut b = [1, 2, 3, 4, 5, 6];
    super::shuffle(33, &mut a);
    super::shuffle(33, &mut b);
    assert_eq!(a, b);

    let (x2, y2) = super::rand_unit_vec2(9);
    assert!(((x2 * x2 + y2 * y2) - 1.0).abs() < 1.0e-5);

    let (xc, yc) = super::rand_in_circle(9);
    assert!(xc * xc + yc * yc <= 1.0 + 1.0e-6);

    let (x3, y3, z3) = super::rand_unit_vec3(9);
    assert!(((x3 * x3 + y3 * y3 + z3 * z3) - 1.0).abs() < 1.0e-5);
}

#[test]
fn stream_rand_stays_repeatable() {
    for index in 0..32 {
        assert_eq!(
            super::rand_u32_stream(123, index),
            super::rand_u32_stream(123, index)
        );

        let r01 = super::rand01_stream(123, index);
        let r11 = super::rand11_stream(123, index);
        assert!((0.0..=1.0).contains(&r01), "index={index} r01={r01}");
        assert!((-1.0..=1.0).contains(&r11), "index={index} r11={r11}");
    }
}

#[test]
fn seeded_rng_repeatable() {
    let mut a = super::SeededRng::new(1337);
    let mut b = super::SeededRng::new(1337);

    for _ in 0..64 {
        assert_eq!(a.next_u32(), b.next_u32());
        assert_eq!(a.next_01(), b.next_01());
        assert_eq!(a.next_11(), b.next_11());
    }
}

#[test]
fn seeded_rng_reseed_reset_sequence() {
    let mut rng = super::SeededRng::new(99);
    let first = rng.next_u32();
    let second = rng.next_u32();

    rng.reseed(99);
    assert_eq!(rng.next_u32(), first);
    assert_eq!(rng.next_u32(), second);
}

#[test]
fn seeded_rng_helpers_work() {
    let mut rng = super::SeededRng::new(11);
    assert!((0.0..=1.0).contains(&rng.next_range_f32(0.0, 1.0)));
    assert!((10..20).contains(&rng.next_range_u32(10, 20)));
    assert!((-20..20).contains(&rng.next_range_i32(-20, 20)));
    let generic: u32 = rng.next_range(0, 5);
    assert!((0..5).contains(&generic));
    assert!(rng.next_index(5).unwrap() < 5);
    let _ = rng.next_chance(0.5);
}

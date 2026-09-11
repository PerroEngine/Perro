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

#[test]
fn file_dir_helpers_return_sorted_disk_paths() {
    let root = std::env::temp_dir().join(format!("perro_modules_file_dir_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("b")).expect("test setup must succeed");
    std::fs::create_dir_all(root.join("a")).expect("test setup must succeed");
    std::fs::write(root.join("b").join("item.txt"), "ok").expect("test setup must succeed");

    let root_text = root.to_string_lossy().to_string();
    assert!(crate::file::exists(&root_text));
    assert!(crate::file::is_dir(&root_text));
    assert!(crate::file::is_file(
        root.join("b").join("item.txt").to_string_lossy().as_ref()
    ));

    let direct = crate::file::read_dir(&root_text).expect("test setup must succeed");
    assert_eq!(direct.len(), 2);
    assert!(direct[0].ends_with('a'));
    assert!(direct[1].ends_with('b'));

    let walked = crate::file::walk_dir(&root_text).expect("test setup must succeed");
    assert!(walked.iter().any(|path| path.ends_with("item.txt")));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn demo_file_and_zip_share_user_subdirectory() {
    let name = format!("perro_modules_demo_alias_{}", std::process::id());
    crate::file::set_project_root_disk(".", &name);
    let root = std::path::PathBuf::from(crate::file::resolve_path_string("user://"));
    assert_eq!(root.file_name().expect("test setup must succeed"), "data");
    assert_eq!(
        root.parent()
            .expect("test setup must succeed")
            .file_name()
            .expect("test setup must succeed"),
        name.as_str()
    );

    crate::file::save_string("demo://save.txt", "demo progress").expect("demo save");
    assert_eq!(
        crate::file::load_string("user://demo/save.txt").expect("test setup must succeed"),
        "demo progress"
    );
    crate::file::save_string("user://demo/save.txt", "full game import")
        .expect("test setup must succeed");
    assert_eq!(
        crate::file::load_string("demo://save.txt").expect("test setup must succeed"),
        "full game import"
    );
    assert!(crate::file::exists("demo://save.txt"));
    crate::file::save_string("user://settings.txt", "shared").expect("test setup must succeed");
    assert!(!crate::file::exists("demo://settings.txt"));

    crate::zip::write_files("demo://backup.zip", &[("demo://save.txt", "save.txt")])
        .expect("test setup must succeed");
    assert_eq!(
        crate::zip::list("user://demo/backup.zip").expect("test setup must succeed"),
        vec!["save.txt"]
    );
    crate::zip::extract_all("demo://backup.zip", "demo://restore")
        .expect("test setup must succeed");
    assert_eq!(
        crate::file::load_string("user://demo/restore/save.txt").expect("test setup must succeed"),
        "full game import"
    );
    assert!(crate::file::save_string("demo://../escape.txt", "bad").is_err());
    assert!(crate::zip::write_files("demo://../escape.zip", &[]).is_err());
    std::fs::remove_dir_all(root.parent().expect("test setup must succeed"))
        .expect("test setup must succeed");
}

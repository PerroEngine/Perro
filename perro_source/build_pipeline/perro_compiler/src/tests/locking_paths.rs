mod locking_paths {
    use super::*;

    #[test]
    fn generated_script_write_lock_times_out_when_live() {
        let target = unique_temp_path("write_lock_live").join("script.rs");
        let lock_path = target.with_extension("write-lock");
        std::fs::create_dir_all(&lock_path).expect("create live lock");

        let err = match super::super::WriteLock::acquire_with_policy(
            &target,
            std::time::Duration::ZERO,
            std::time::Duration::MAX,
        ) {
            Ok(_) => panic!("live lock must time out"),
            Err(err) => err,
        };

        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
        std::fs::remove_dir_all(target.parent().expect("temp parent")).expect("remove fixture");
    }

    #[test]
    fn generated_script_write_lock_reclaims_stale_lock() {
        let target = unique_temp_path("write_lock_stale").join("script.rs");
        let lock_path = target.with_extension("write-lock");
        std::fs::create_dir_all(&lock_path).expect("create stale lock");

        let guard = super::super::WriteLock::acquire_with_policy(
            &target,
            std::time::Duration::ZERO,
            std::time::Duration::ZERO,
        )
        .expect("reclaim stale lock");

        assert!(lock_path.is_dir());
        drop(guard);
        assert!(!lock_path.exists());
        std::fs::remove_dir_all(target.parent().expect("temp parent")).expect("remove fixture");
    }

    #[test]
    fn dlc_pack_pointer_callbacks_require_unsafe_calls() {
        let pack_dir = unique_temp_path("dlc_pack_unsafe_callbacks");
        super::super::write_dlc_pack_lib(
            std::path::Path::new("project"),
            "Expansion",
            std::path::Path::new("dlc"),
            &pack_dir,
            &[],
        )
        .expect("write pack source");
        let source = std::fs::read_to_string(pack_dir.join("src/lib.rs"))
            .expect("read generated pack source");

        for callback in [
            "perro_dlc_pack_lookup_mesh",
            "perro_dlc_pack_lookup_collision_trimesh",
            "perro_dlc_pack_lookup_skeleton",
            "perro_dlc_pack_lookup_texture",
            "perro_dlc_pack_lookup_audio",
            "perro_dlc_pack_lookup_shader",
            "perro_dlc_pack_lookup",
        ] {
            assert!(
                source.contains(&format!("pub unsafe extern \"C\" fn {callback}")),
                "safe raw-pointer callback emitted for {callback}"
            );
        }
        assert!(source.contains("pub mesh_lookup: unsafe extern \"C\" fn"));
        std::fs::remove_dir_all(pack_dir).expect("remove fixture");
    }

    #[test]
    fn dlc_script_sync_rejects_names_that_escape_or_corrupt_generated_paths() {
        let root = unique_temp_dir("perro_compiler_invalid_dlc_name");
        std::fs::create_dir_all(&root).expect("test setup/result must succeed");

        for name in [
            "",
            ".",
            "..",
            "../escape",
            "..\\escape",
            "self",
            "SELF",
            "bad\"name",
            "bad\nname",
        ] {
            assert!(
                sync_dlc_scripts(&root, name).is_err(),
                "accepted `{name:?}`"
            );
        }

        assert!(!root.join(".perro/escape").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn emits_obfuscated_static_steam_app_id_fn() {
        let src = emit_static_steam_app_id_fn(Some(480), "Game");

        assert!(src.contains("fn steam_app_id() -> u32"));
        assert!(src.contains("const DATA_A: u32 = 0x"));
        assert!(src.contains("std::hint::black_box(DATA_A)"));
        assert!(!src.contains("480u32"));
        assert!(!src.contains("Some(480"));
    }

    #[test]
    fn native_output_names_group_by_host_and_suffix_bin_with_version() {
        let host = super::super::rustc_default_host_triple()
            .and_then(|triple| target_slug_from_triple(&triple))
            .unwrap_or_else(|| format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH));
        assert_eq!(
            native_output_folder_name("My Game", None),
            format!("My_Game-{host}")
        );
        assert_eq!(
            native_output_artifact_name("My Game", Some("1.0"), None),
            format!("My_Game-{host}-v1.0")
        );
        assert_eq!(
            native_output_artifact_name("Game", None, None),
            format!("Game-{host}-v0.1.0")
        );
    }

    #[test]
    fn target_slug_from_triple_uses_rust_target_os_and_arch() {
        assert_eq!(
            target_slug_from_triple("x86_64-pc-windows-msvc").as_deref(),
            Some("windows-x86_64")
        );
        assert_eq!(
            target_slug_from_triple("aarch64-pc-windows-msvc").as_deref(),
            Some("windows-aarch64")
        );
        assert_eq!(
            target_slug_from_triple("aarch64-apple-darwin").as_deref(),
            Some("macos-aarch64")
        );
        assert_eq!(
            target_slug_from_triple("x86_64-unknown-linux-gnu").as_deref(),
            Some("linux-x86_64")
        );
    }

    #[test]
    fn native_output_names_use_requested_target() {
        assert_eq!(
            native_output_folder_name("My Game", Some("i686-pc-windows-msvc")),
            "My_Game-windows-i686"
        );
        assert_eq!(
            native_output_artifact_name("My Game", Some("2.0"), Some("aarch64-apple-darwin")),
            "My_Game-macos-aarch64-v2.0"
        );
        assert_eq!(
            target_binary_name("game", Some("x86_64-pc-windows-msvc")),
            "game.exe"
        );
        assert_eq!(
            target_binary_name("game", Some("x86_64-unknown-linux-gnu")),
            "game"
        );
    }

    #[test]
    fn native_target_triple_rejects_paths_and_flags() {
        assert!(validate_native_target_triple("x86_64-pc-windows-msvc").is_ok());
        assert!(validate_native_target_triple("../release").is_err());
        assert!(validate_native_target_triple("-Zbuild-std").is_err());
        assert!(validate_native_target_triple("windows").is_err());
    }

    #[test]
    fn steam_runtime_name_uses_target_arch() {
        assert_eq!(
            steam_runtime_library_name(Some("i686-pc-windows-msvc")),
            Some("steam_api.dll")
        );
        assert_eq!(
            steam_runtime_library_name(Some("x86_64-pc-windows-msvc")),
            Some("steam_api64.dll")
        );
        assert_eq!(
            steam_runtime_library_name(Some("aarch64-unknown-linux-gnu")),
            Some("libsteam_api.so")
        );
    }

    #[test]
    fn android_export_uses_exact_apk_path() {
        let root = unique_temp_path("android_exact_apk");
        std::fs::create_dir_all(&root).expect("project dir");
        ensure_project_toml(&root, "Android Pick").expect("project toml");
        let manifest_dir = root.join(".perro/project");
        std::fs::create_dir_all(&manifest_dir).expect("manifest dir");
        std::fs::write(
            manifest_dir.join("Cargo.toml"),
            r#"[package]
    name = "android_pick"
    version = "0.1.0"

    [lib]
    name = "main"

    [package.metadata.android]
    apk_name = "main"
    "#,
        )
        .expect("manifest");
        let cfg = load_project_toml(&root).expect("load project");
        sync_android_project_manifest(
            &root,
            &cfg,
            ProjectBuildOptions::new(false, false).with_target(ProjectBuildTarget::Android),
        )
        .expect("sync Android manifest");

        let target = root.join("target");
        let exact = target.join("release/apk/main.apk");
        let unrelated = target.join("other/release/newer.apk");
        std::fs::create_dir_all(exact.parent().expect("exact parent")).expect("exact dir");
        std::fs::create_dir_all(unrelated.parent().expect("other parent")).expect("other dir");
        std::fs::write(&exact, b"current project").expect("exact apk");
        std::fs::write(&unrelated, b"other project").expect("other apk");

        let selected = android_apk_artifact_path(&root, &target, true).expect("artifact path");
        assert_eq!(selected, exact);
        export_project_android_bundle(&root, &selected, false).expect("export exact apk");
        assert_eq!(
            std::fs::read(root.join(".output/android/Android Pick.apk")).expect("exported apk"),
            b"current project"
        );

        std::fs::remove_file(&exact).expect("remove exact apk");
        assert!(export_project_android_bundle(&root, &selected, false).is_err());
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn normalizes_script_cargo_paths_to_project_relative_slashes() {
        let project = std::path::Path::new("D:/Game");
        let crate_dir = project.join(".perro/scripts");
        let input = "src\\scripts\\../../../../res/scripts/game_manager.rs:1929:68: error: bad\n";
        let out = normalize_cargo_output_paths(project, Some(&crate_dir), input);
        assert_eq!(out, "res/scripts/game_manager.rs:1929:68: error: bad\n");
    }

    #[test]
    fn normalizes_nested_script_cargo_paths_to_project_relative_slashes() {
        let project = std::path::Path::new("D:/Game");
        let crate_dir = project.join(".perro/scripts");
        let input = " --> src\\scripts\\ai\\../../../../../res/scripts/ai/brain.rs:7:3\n";
        let out = normalize_cargo_output_paths(project, Some(&crate_dir), input);
        assert_eq!(out, " --> res/scripts/ai/brain.rs:7:3\n");
    }

    #[test]
    fn transpiles_controller_methods_into_call_method_arms() {
        let source = r#"
    use perro_api::prelude::*;

    #[State]
    pub struct ArcherControllerState {
    #[default = false]
    pub enabled: bool,
    }

    lifecycle!({
    fn on_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
    });

    methods!({
    fn bind_agent(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _agent_id: NodeID,
    ) {}

    fn set_player_index(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _player_index: i32,
    ) {}

    fn set_turn_enabled(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _enabled: bool,
    ) {}
    });
    "#;

        let transpiled = transpile_frontend_script(source, "res://tests/controller.rs");
        assert_methods_emitted(
            &transpiled,
            &["bind_agent", "set_player_index", "set_turn_enabled"],
        );
    }

    #[test]
    fn transpiles_state_fields_with_expose_marker() {
        let source = r#"
    use perro_api::prelude::*;

    #[State]
    pub struct PlayerState {
    #[default(100.0)]
    #[expose]
    health: f32,

    #[default(240.0)]
    #[expose]
    speed: f32,

    velocity: Vector2,
    grounded: bool,
    }

    lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {}
    });
    "#;

        let transpiled = transpile_frontend_script(source, "res://tests/player.rs");
        assert!(transpiled.contains("Box::new(<PlayerState as Default>::default())"));
        assert!(
            transpiled.contains("const __PERRO_VAR_HEALTH: ScriptMemberID = var!(\"health\");")
        );
        assert!(transpiled.contains("const __PERRO_VAR_SPEED: ScriptMemberID = var!(\"speed\");"));
        assert!(
            transpiled.contains("const __PERRO_VAR_VELOCITY: ScriptMemberID = var!(\"velocity\");")
        );
        assert!(
            transpiled.contains("const __PERRO_VAR_GROUNDED: ScriptMemberID = var!(\"grounded\");")
        );
    }

    #[test]
    fn transpiles_ai_methods_into_call_method_arms() {
        let source = r#"
    use perro_api::prelude::*;

    #[derive(Variant, Clone, Copy)]
    pub struct AgentRef {
    pub agent_id: NodeID,
    }

    impl Default for AgentRef {
    fn default() -> Self {
        Self {
            agent_id: NodeID::nil(),
        }
    }
    }

    #[derive(Variant, Clone, Copy)]
    pub struct AimPlan {
    pub has_plan: bool,
    }

    impl Default for AimPlan {
    fn default() -> Self {
        Self { has_plan: false }
    }
    }

    #[State]
    pub struct ArcherAiBrainState {
    #[default = false]
    pub enabled: bool,
    #[default = AgentRef::default()]
    pub agent_ref: AgentRef,
    #[default = AimPlan::default()]
    pub plan: AimPlan,
    }

    lifecycle!({
    fn on_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
    });

    methods!({
    fn bind_agent(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _agent_id: NodeID,
    ) {}

    fn set_turn_enabled(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _enabled: bool,
    ) {}

    fn set_ai_skill(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _skill: f32,
    ) {}

    fn reset_plan(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
    });
    "#;

        let transpiled = transpile_frontend_script(source, "res://tests/ai.rs");
        assert_methods_emitted(
            &transpiled,
            &[
                "bind_agent",
                "set_turn_enabled",
                "set_ai_skill",
                "reset_plan",
            ],
        );
    }

    #[test]
    fn transpiles_methods_even_with_braces_in_strings_comments_and_raw_strings() {
        let source = r###"
    use perro_api::prelude::*;

    #[State]
    pub struct WeirdState {
    #[default = false]
    pub enabled: bool,
    }

    lifecycle!({
    fn on_update(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        // comment with misleading delimiters: methods!({ fn nope( ) { } });
        let _a = "format-like braces {x} and parens (y) should not affect parser";
        let _b = r#"raw string with fake delimiters: methods!({ fn fake() {} })"#;
        let _c = br##"byte raw with nested hashes and braces { } ) ("##;
        let _d = "emoji \u{1F3F9} and braces {{{}}}";
    }
    });

    methods!({
    fn alpha(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}

    fn beta(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _enabled: bool,
    ) {}
    });
    "###;

        let transpiled = transpile_frontend_script(source, "res://tests/weird.rs");
        assert_methods_emitted(&transpiled, &["alpha", "beta"]);
    }

    #[test]
    fn transpiles_bool_method_return_into_variant() {
        let source = r#"
    use perro_api::prelude::*;

    methods!({
    fn is_ready(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) -> bool {
        let _ = ctx.id;
        true
    }
    });
    "#;

        let transpiled = transpile_frontend_script(source, "res://tests/bool_return.rs");
        assert!(transpiled.contains("Variant::from(self.is_ready(ctx))"));
        assert!(!transpiled.contains("self.is_ready(ctx);\n                Variant::Null"));
    }

    #[test]
    fn typed_param_binding_uses_first_for_zero_index() {
        let first = generate_call_param_binding(
            0,
            &ScriptMethodParam {
                name: "sport".to_string(),
                ty: "Sport".to_string(),
            },
        )
        .expect("custom param should generate binding");
        let second = generate_call_param_binding(
            1,
            &ScriptMethodParam {
                name: "enabled".to_string(),
                ty: "bool".to_string(),
            },
        )
        .expect("bool param should generate binding");

        assert!(first.contains("match params.first()"));
        assert!(!first.contains("params.get(0)"));
        assert!(second.contains("match params.get(1)"));
    }

    #[test]
    fn bare_module_has_no_exported_ctor() {
        let source = r#"
    pub const SCALE: f32 = 2.0;

    pub enum Team {
    Red,
    Blue,
    }

    pub fn mix(a: f32, b: f32) -> f32 {
    (a + b) * SCALE
    }
    "#;

        let transpiled = transpile_frontend_script(source, "res://scripts/util.rs");
        assert!(
            !transpiled_exports_script_ctor(&transpiled),
            "bare modules should not register as script constructors"
        );
    }

    #[test]
    fn generated_scripts_lib_exports_v2_abi_descriptor() {
        let root = unique_temp_dir("perro_compiler_script_abi_descriptor");
        let src = root.join("src");
        write_scripts_lib(
            &src,
            &["player.rs".to_string()],
            &["player.rs".to_string()],
            "res://",
        )
        .expect("write scripts lib");
        let generated = std::fs::read_to_string(src.join("lib.rs")).expect("read scripts lib");

        assert!(generated.contains("perro_script_abi_descriptor_v2"));
        assert!(generated.contains("ScriptAbiDescriptor::v2()"));
        assert!(generated.contains("-> *const ScriptAbiDescriptorHeader"));
        assert!(generated.contains("#[cfg(feature = \"dynamic-scripts\")]"));
        assert!(generated.contains("DYNAMIC_SCRIPT_REGISTRY"));
        assert!(generated.contains("perro_create_script_dynamic as DynamicScriptConstructor"));

        std::fs::remove_dir_all(root).expect("remove script ABI fixture");
    }
}

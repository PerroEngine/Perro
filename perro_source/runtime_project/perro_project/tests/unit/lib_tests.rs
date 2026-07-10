use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn parse_project_toml_reads_aspect_ratio() {
    let landscape = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"
"#;

    let parsed = parse_project_toml(landscape).expect("failed to parse project.toml");
    assert_eq!(parsed.virtual_width, 1920);
    assert_eq!(parsed.virtual_height, 1080);

    let portrait = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "9:16"
"#;

    let parsed = parse_project_toml(portrait).expect("failed to parse project.toml");
    assert_eq!(parsed.virtual_width, 1080);
    assert_eq!(parsed.virtual_height, 1920);
}

#[test]
fn parse_project_toml_defaults_to_wide_aspect_canvas() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
vsync = true
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    assert_eq!(parsed.virtual_width, 1920);
    assert_eq!(parsed.virtual_height, 1080);
    assert!(parsed.vsync);
}

#[test]
fn parse_project_toml_rejects_removed_virtual_resolution() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "9:16"
virtual_resolution = "1280x720"
"#;

    let err = parse_project_toml(toml).expect_err("expected parse failure");
    assert!(err.to_string().contains("graphics.virtual_resolution"));
}

#[test]
fn parse_project_toml_rejects_removed_split_virtual_dimensions() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_width = 1920
virtual_height = 1080
"#;

    let err = parse_project_toml(toml).expect_err("expected parse failure");
    assert!(err.to_string().contains("graphics.virtual_width"));
}

#[test]
fn parse_project_toml_reads_frame_rate_cap() {
    let fps = parse_project_toml(
        r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[runtime]
frame_rate_cap = 144
"#,
    )
    .expect("fps cap");
    assert_eq!(fps.frame_rate_cap, FrameRateCap::Fps(144.0));

    let refresh = parse_project_toml(
        r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[runtime]
frame_rate_cap = "refresh_rate"
"#,
    )
    .expect("refresh cap");
    assert_eq!(refresh.frame_rate_cap, FrameRateCap::RefreshRate);
}

#[test]
fn parse_project_toml_reads_vsync_and_msaa() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"
vsync = true
msaa = false
meshlets = true
dev_meshlets = true
release_meshlets = false
meshlet_debug_view = true
occlusion_culling = "cpu"
particle_sim_default = "gpu"
texture_filter = "nearest"
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    assert!(parsed.vsync);
    assert!(!parsed.msaa);
    assert!(parsed.meshlets);
    assert!(parsed.dev_meshlets);
    assert!(!parsed.release_meshlets);
    assert!(parsed.meshlet_debug_view);
    assert_eq!(parsed.occlusion_culling, OcclusionCulling::Cpu);
    assert_eq!(parsed.particle_sim_default, ParticleSimDefault::GpuCompute);
    assert_eq!(
        parsed.texture_filter,
        perro_structs::TextureFilterMode::Nearest
    );
    assert_eq!(parsed.physics_gravity, -9.81);
    assert_eq!(parsed.physics_coef, 1.0);
    assert!(parsed.localization.is_none());
}

#[test]
fn parse_project_toml_ui_pixel_snapping_defaults_true() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    assert!(parsed.rendering.ui.pixel_snapping);
}

#[test]
fn parse_project_toml_reads_ui_pixel_snapping() {
    let enabled = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[rendering.ui]
pixel_snapping = true
"#;
    let disabled = enabled.replace("pixel_snapping = true", "pixel_snapping = false");

    let parsed = parse_project_toml(enabled).expect("enabled pixel snap");
    assert!(parsed.rendering.ui.pixel_snapping);

    let parsed = parse_project_toml(&disabled).expect("disabled pixel snap");
    assert!(!parsed.rendering.ui.pixel_snapping);
}

#[test]
fn parse_project_toml_reads_physics_config() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[physics]
gravity = -4.905
coef = 0.5
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    assert_eq!(parsed.physics_gravity, -4.905);
    assert_eq!(parsed.physics_coef, 0.5);
}

#[test]
fn parse_project_toml_reads_steam_config() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[steam]
enabled = true
app_id = 123456
input = "metadata"
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    assert!(parsed.steam.enabled);
    assert_eq!(parsed.steam.app_id, Some(123456));
    assert_eq!(parsed.steam.input_mode, SteamInputMode::Metadata);
}

#[test]
fn parse_project_toml_rejects_enabled_steam_without_app_id() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[steam]
enabled = true
"#;

    let err = parse_project_toml(toml).expect_err("expected parse failure");
    assert!(matches!(err, ProjectError::MissingField("steam.app_id")));
}

#[test]
fn parse_project_toml_rejects_invalid_steam_app_id() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[steam]
enabled = true
app_id = "spacewar"
"#;

    let err = parse_project_toml(toml).expect_err("expected parse failure");
    assert!(matches!(err, ProjectError::InvalidField("steam.app_id", _)));
}

#[test]
fn parse_project_toml_rejects_non_res_path() {
    let toml = r#"
[project]
name = "Game"
main_scene = "./main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"
"#;

    let err = parse_project_toml(toml).expect_err("expected parse failure");
    assert!(matches!(
        err,
        ProjectError::InvalidField("project.main_scene", _)
    ));
}

#[test]
fn parse_project_toml_particle_sim_rejects_old_names() {
    let base = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"
"#;

    let gpu_compute = format!("{base}particle_sim_default = \"gpu_compute\"\n");
    let gpu_vertex = format!("{base}particle_sim_default = \"gpu_vertex\"\n");

    let err_gpu_compute =
        parse_project_toml(&gpu_compute).expect_err("gpu_compute should be rejected");
    let err_gpu_vertex =
        parse_project_toml(&gpu_vertex).expect_err("gpu_vertex should be rejected");

    assert!(matches!(
        err_gpu_compute,
        ProjectError::InvalidField("graphics.particle_sim_default", _)
    ));
    assert!(matches!(
        err_gpu_vertex,
        ProjectError::InvalidField("graphics.particle_sim_default", _)
    ));
}

#[test]
#[cfg(target_os = "windows")]
fn resolve_local_path_maps_slash_to_local_root() {
    let root = PathBuf::from("D:/workspace");
    assert_eq!(
        resolve_local_path("/games/demo", &root),
        PathBuf::from("D:/workspace").join("games").join("demo")
    );
    assert_eq!(resolve_local_path("/", &root), root);
}

#[test]
#[cfg(not(target_os = "windows"))]
fn resolve_local_path_keeps_unix_absolute_path() {
    let root = PathBuf::from("/workspace");
    assert_eq!(
        resolve_local_path("/games/demo", &root),
        PathBuf::from("/games/demo")
    );
    assert_eq!(resolve_local_path("/", &root), PathBuf::from("/"));
}

#[test]
fn resolve_local_path_supports_local_scheme() {
    let root = PathBuf::from("D:/workspace");
    assert_eq!(
        resolve_local_path("local://games/demo", &root),
        PathBuf::from("D:/workspace").join("games").join("demo")
    );
}

#[test]
fn crate_name_from_project_name_normalizes() {
    assert_eq!(crate_name_from_project_name("My Project!"), "my_project");
    assert_eq!(crate_name_from_project_name("123"), "_123");
}

#[test]
fn parse_project_toml_reads_localization_config() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[localization]
default_locale = "JA"
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    let localization = parsed
        .localization
        .as_ref()
        .expect("localization should be present");
    assert_eq!(localization.source_csv, "");
    assert_eq!(localization.key_column, "key");
    assert_eq!(localization.default_locale, "ja");
}

#[test]
fn load_project_toml_detects_sibling_localization_csv() {
    let root = unique_temp_dir("perro_localization_sibling");
    ensure_project_layout(&root).expect("layout");
    fs::write(
        root.join("project.toml"),
        r#"[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[localization]
default_locale = "ES"
"#,
    )
    .expect("write project.toml");
    fs::write(
        root.join("locale.csv"),
        "key,en,es\nmenu.start,Start,Iniciar\n",
    )
    .expect("write locale.csv");

    let parsed = load_project_toml(&root).expect("failed to load project.toml");
    let localization = parsed
        .localization
        .as_ref()
        .expect("localization should be present");
    assert_eq!(localization.source_csv, "locale.csv");
    assert_eq!(localization.key_column, "key");
    assert_eq!(localization.default_locale, "es");

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn load_project_toml_uses_en_when_sibling_csv_has_no_localization_table() {
    let root = unique_temp_dir("perro_localization_implicit_default");
    ensure_project_layout(&root).expect("layout");
    fs::write(
        root.join("project.toml"),
        r#"[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"
"#,
    )
    .expect("write project.toml");
    fs::write(
        root.join("translations.csv"),
        "key,en,es\nmenu.start,Start,Iniciar\n",
    )
    .expect("write translations.csv");

    let parsed = load_project_toml(&root).expect("failed to load project.toml");
    let localization = parsed
        .localization
        .as_ref()
        .expect("localization should be present");
    assert_eq!(localization.source_csv, "translations.csv");
    assert_eq!(localization.default_locale, "en");

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn load_project_toml_rejects_localization_table_without_sibling_csv() {
    let root = unique_temp_dir("perro_localization_missing_sibling");
    ensure_project_layout(&root).expect("layout");
    fs::write(
        root.join("project.toml"),
        r#"[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[localization]
default_locale = "en"
"#,
    )
    .expect("write project.toml");

    let err = load_project_toml(&root).expect_err("expected missing csv failure");
    assert!(matches!(err, ProjectError::InvalidField("localization", _)));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn parse_project_toml_reads_export_metadata() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[metadata]
description = "Arcade test game"
company = "Perro Lab"
version = "1.2.3"
copyright = "Copyright (c) 2026 Perro Lab"
trademark = "Perro Lab"

[graphics]
aspect_ratio = "16:9"
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    assert_eq!(
        parsed.metadata.description.as_deref(),
        Some("Arcade test game")
    );
    assert_eq!(parsed.metadata.company.as_deref(), Some("Perro Lab"));
    assert_eq!(parsed.metadata.version.as_deref(), Some("1.2.3"));
    assert_eq!(
        parsed.metadata.copyright.as_deref(),
        Some("Copyright (c) 2026 Perro Lab")
    );
    assert_eq!(parsed.metadata.trademark.as_deref(), Some("Perro Lab"));
}

#[test]
fn parse_project_toml_reads_web_metadata() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[web]
title = "Game Site"
description = "Ship fast"
keywords = ["rust", "engine", "web"]

[graphics]
aspect_ratio = "16:9"
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    assert_eq!(parsed.web.title.as_deref(), Some("Game Site"));
    assert_eq!(parsed.web.description.as_deref(), Some("Ship fast"));
    assert_eq!(parsed.web.keywords, vec!["rust", "engine", "web"]);
}

#[test]
fn parse_routes_toml_reads_routes() {
    let parsed = parse_routes_toml(
        r#"
[[route]]
href = "/"
name = "home"
scene = "res://routes/home.scn"

[[route]]
href = "docs/"
name = "docs"
scene = "res://routes/docs.scn"
title = "Docs"
description = "API docs"
keywords = ["docs", "api"]
"#,
    )
    .expect("parse routes");

    assert_eq!(parsed.routes.len(), 2);
    assert_eq!(parsed.routes[0].href, "/");
    assert_eq!(parsed.routes[1].href, "/docs");
    assert_eq!(parsed.routes[1].title.as_deref(), Some("Docs"));
    assert_eq!(parsed.routes[1].description.as_deref(), Some("API docs"));
    assert_eq!(parsed.routes[1].keywords, vec!["docs", "api"]);
}

#[test]
fn load_routes_toml_defaults_to_main_scene() {
    let root = unique_temp_dir("perro_routes_default");
    ensure_project_layout(&root).expect("layout");
    fs::write(
        root.join("project.toml"),
        r#"[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"
"#,
    )
    .expect("write project");

    let project = load_project_toml(&root).expect("load project");
    let routes = load_routes_toml(&root, &project).expect("load routes");
    assert_eq!(routes.routes.len(), 1);
    assert_eq!(routes.routes[0].href, "/");
    assert_eq!(routes.routes[0].scene, "res://main.scn");

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn parse_routes_toml_rejects_bad_scene() {
    let err = parse_routes_toml(
        r#"
[[route]]
href = "/"
name = "home"
scene = "./bad.scn"
"#,
    )
    .expect_err("bad scene");
    assert!(matches!(err, ProjectError::InvalidField("route.scene", _)));
}

#[test]
fn project_paths_reject_parent_and_platform_escape_components() {
    for scene in [
        "res://../outside.scn",
        "res://dir\\outside.scn",
        "res://C:/outside.scn",
    ] {
        let project = format!(
            r#"[project]
name = "Game"
main_scene = {scene:?}
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"
"#
        );
        let err = parse_project_toml(&project).expect_err("unsafe project path");
        assert!(matches!(
            err,
            ProjectError::InvalidField("project.main_scene", _)
        ));
    }
}

#[test]
fn parse_routes_toml_rejects_path_escape() {
    for (href, scene) in [
        ("/../outside", "res://main.scn"),
        ("/dir\\outside", "res://main.scn"),
        ("/safe", "res://../outside.scn"),
    ] {
        let routes = format!(
            r#"[[route]]
href = "{href}"
name = "bad"
scene = "{scene}"
"#
        );
        assert!(parse_routes_toml(&routes).is_err());
    }
}

#[test]
fn normalize_route_href_trims_extra_bits() {
    assert_eq!(normalize_route_href("/"), "/");
    assert_eq!(normalize_route_href("/docs/"), "/docs");
    assert_eq!(normalize_route_href("docs"), "/docs");
    assert_eq!(normalize_route_href("/docs/index.html"), "/docs");
}

#[test]
fn parse_input_map_toml_reads_key_mouse_gamepad_and_joycon() {
    let parsed = parse_input_map_toml(
        r#"
[jump]
keys = ["KeySpace", "KeyUp"]
mouse = ["Left"]
gamepad = ["Bottom"]
joycon = ["Bottom"]
"#,
    )
    .expect("parse input map");
    let action = parsed.action("jump").expect("jump action");

    assert_eq!(action.bindings.len(), 5);
    assert!(
        action
            .bindings
            .contains(&perro_input_api::InputBinding::Key(
                perro_input_api::KeyCode::Space
            ))
    );
    assert!(
        action
            .bindings
            .contains(&perro_input_api::InputBinding::Key(
                perro_input_api::KeyCode::ArrowUp
            ))
    );
    assert!(
        action
            .bindings
            .contains(&perro_input_api::InputBinding::Mouse(
                perro_input_api::MouseButton::Left
            ))
    );
    assert!(
        action
            .bindings
            .contains(&perro_input_api::InputBinding::Gamepad(
                perro_input_api::GamepadButton::Bottom
            ))
    );
    assert!(
        action
            .bindings
            .contains(&perro_input_api::InputBinding::JoyCon(
                perro_input_api::JoyConButton::Bottom
            ))
    );
}

#[test]
fn parse_input_map_toml_rejects_unknown_binding() {
    let err = parse_input_map_toml(
        r#"
[jump]
keys = ["Nope"]
"#,
    )
    .expect_err("unknown key");

    assert!(matches!(err, ProjectError::InvalidField("input_map", _)));
}

#[test]
fn load_project_toml_reads_sibling_input_map() {
    let root = unique_temp_dir("perro_input_map_sibling");
    ensure_project_layout(&root).expect("layout");
    fs::write(
        root.join("project.toml"),
        r#"[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"
"#,
    )
    .expect("write project");
    fs::write(
        root.join("input_map.toml"),
        "[jump]\nkeys = [\"KeySpace\"]\n",
    )
    .expect("write input map");

    let project = load_project_toml(&root).expect("load project");
    assert!(project.input_map.action("jump").is_some());

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn load_input_map_toml_missing_returns_empty() {
    let root = unique_temp_dir("perro_input_map_missing");
    ensure_project_layout(&root).expect("layout");

    let input_map = load_input_map_toml(&root).expect("load missing input map");
    assert!(input_map.is_empty());

    fs::remove_dir_all(&root).expect("cleanup");
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{nanos}_{}", std::process::id()))
}

fn manifest_dep_has_path(manifest_src: &str, dep: &str) -> bool {
    let value = toml::Value::Table(manifest_src.parse::<toml::Table>().expect("parse manifest"));
    value
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .and_then(|deps| deps.get(dep))
        .and_then(toml::Value::as_table)
        .and_then(|spec| spec.get("path"))
        .and_then(toml::Value::as_str)
        .is_some()
}

#[test]
fn ensure_source_overrides_merges_deps_toml_into_scripts_manifest() {
    let root = unique_temp_dir("perro_deps_merge");
    ensure_project_layout(&root).expect("layout");
    ensure_project_scaffold(&root, "Deps Merge").expect("scaffold");

    fs::write(
        root.join("deps.toml"),
        r#"[dependencies]
serde = { version = "1", features = ["derive"] }
"#,
    )
    .expect("write deps.toml");

    ensure_source_overrides(&root).expect("overrides");

    let scripts_manifest =
        fs::read_to_string(root.join(".perro").join("scripts").join("Cargo.toml"))
            .expect("read scripts manifest");
    assert!(manifest_dep_has_path(&scripts_manifest, "perro_api"));
    assert!(manifest_dep_has_path(&scripts_manifest, "perro_runtime"));
    assert!(scripts_manifest.contains("serde"));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn ensure_source_overrides_keeps_steamworks_behind_perro_api_when_steam_enabled() {
    let root = unique_temp_dir("perro_steam_enabled_deps");
    ensure_project_layout(&root).expect("layout");
    ensure_project_scaffold(&root, "Steam Enabled").expect("scaffold");
    fs::write(
        root.join("project.toml"),
        r#"[project]
name = "Steam Enabled"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[steam]
enabled = true
app_id = 480
"#,
    )
    .expect("write project.toml");

    ensure_source_overrides(&root).expect("overrides");

    let scripts_manifest =
        fs::read_to_string(root.join(".perro").join("scripts").join("Cargo.toml"))
            .expect("read scripts manifest");
    let project_manifest =
        fs::read_to_string(root.join(".perro").join("project").join("Cargo.toml"))
            .expect("read project manifest");
    assert!(manifest_dep_has_path(&scripts_manifest, "perro_api"));
    assert!(!scripts_manifest.contains("\nperro_steamworks = \"0.1.0\""));
    assert!(project_manifest.contains("\"scripts/steamworks\""));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn ensure_source_overrides_repairs_project_steamworks_script_feature() {
    let root = unique_temp_dir("perro_steam_project_feature_repair");
    ensure_project_layout(&root).expect("layout");
    ensure_project_scaffold(&root, "Steam Feature Repair").expect("scaffold");
    let project_manifest_path = root.join(".perro").join("project").join("Cargo.toml");
    let project_manifest = fs::read_to_string(&project_manifest_path).expect("read manifest");
    fs::write(
        &project_manifest_path,
        project_manifest.replace(", \"scripts/steamworks\"", ""),
    )
    .expect("write manifest");

    ensure_source_overrides(&root).expect("overrides");

    let project_manifest = fs::read_to_string(project_manifest_path).expect("read manifest");
    assert!(project_manifest.contains("\"scripts/steamworks\""));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn ensure_source_overrides_removes_steamworks_when_steam_disabled() {
    let root = unique_temp_dir("perro_steam_disabled_deps");
    ensure_project_layout(&root).expect("layout");
    ensure_project_scaffold(&root, "Steam Disabled").expect("scaffold");
    fs::write(
        root.join("project.toml"),
        r#"[project]
name = "Steam Disabled"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[steam]
enabled = true
app_id = 480
"#,
    )
    .expect("write enabled project.toml");
    ensure_source_overrides(&root).expect("enabled overrides");
    fs::write(
        root.join("project.toml"),
        r#"[project]
name = "Steam Disabled"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[steam]
enabled = false
app_id = 480
"#,
    )
    .expect("write disabled project.toml");

    ensure_source_overrides(&root).expect("disabled overrides");

    let scripts_manifest =
        fs::read_to_string(root.join(".perro").join("scripts").join("Cargo.toml"))
            .expect("read scripts manifest");
    assert!(!scripts_manifest.contains("\nperro_steamworks = \"0.1.0\""));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn ensure_source_overrides_regenerates_project_build_script() {
    let root = unique_temp_dir("perro_stale_build_metadata");
    ensure_project_layout(&root).expect("layout");
    ensure_project_scaffold(&root, "Metadata Repair").expect("scaffold");

    let build_script = root.join(".perro").join("project").join("build.rs");
    fs::write(
        &build_script,
        r#"fn main() {
    println!("stale build script");
    println!("cargo:rustc-check-cfg=cfg(perro_no_console)");
}
"#,
    )
    .expect("write stale build script");

    ensure_source_overrides(&root).expect("overrides");

    let repaired = fs::read_to_string(&build_script).expect("read repaired build script");
    assert!(!repaired.contains("stale build script"));
    assert!(repaired.contains("apply_windows_metadata"));
    assert!(repaired.contains("FileDescription"));
    assert!(repaired.contains("ProductName"));
    assert!(repaired.contains("LegalCopyright"));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn ensure_source_overrides_ignores_perro_api_override_in_deps_toml() {
    let root = unique_temp_dir("perro_deps_ignore_perro_api");
    ensure_project_layout(&root).expect("layout");
    ensure_project_scaffold(&root, "Deps Ignore").expect("scaffold");

    fs::write(
        root.join("deps.toml"),
        r#"[dependencies]
perro_api = "9.9.9"
rand = "0.9"
"#,
    )
    .expect("write deps.toml");

    ensure_source_overrides(&root).expect("overrides");

    let scripts_manifest =
        fs::read_to_string(root.join(".perro").join("scripts").join("Cargo.toml"))
            .expect("read scripts manifest");
    assert!(manifest_dep_has_path(&scripts_manifest, "perro_api"));
    assert!(manifest_dep_has_path(&scripts_manifest, "perro_runtime"));
    assert!(scripts_manifest.contains("rand = \"0.9\""));
    assert!(!scripts_manifest.contains("perro_api = \"9.9.9\""));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn ensure_source_overrides_removes_deps_not_present_in_deps_toml() {
    let root = unique_temp_dir("perro_deps_remove");
    ensure_project_layout(&root).expect("layout");
    ensure_project_scaffold(&root, "Deps Remove").expect("scaffold");

    fs::write(
        root.join("deps.toml"),
        r#"[dependencies]
rand = "0.9"
"#,
    )
    .expect("write deps.toml");
    ensure_source_overrides(&root).expect("overrides first");

    fs::write(root.join("deps.toml"), "[dependencies]\n").expect("rewrite deps.toml");
    ensure_source_overrides(&root).expect("overrides second");

    let scripts_manifest =
        fs::read_to_string(root.join(".perro").join("scripts").join("Cargo.toml"))
            .expect("read scripts manifest");
    assert!(manifest_dep_has_path(&scripts_manifest, "perro_api"));
    assert!(manifest_dep_has_path(&scripts_manifest, "perro_runtime"));
    assert!(!scripts_manifest.contains("rand = \"0.9\""));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn ensure_source_overrides_repairs_dev_runner_features() {
    let root = unique_temp_dir("perro_dev_runner_features");
    ensure_project_layout(&root).expect("layout");
    ensure_project_scaffold(&root, "Dev Runner Features").expect("scaffold");

    let manifest = root.join(".perro").join("dev_runner").join("Cargo.toml");
    let mut src = fs::read_to_string(&manifest).expect("read dev runner manifest");
    src = src
        .replace("timings = [\"perro_app/fps\"]\n", "")
        .replace("ui_profile = [\"perro_app/ui_profile\"]\n", "")
        .replace("mem_profile = [\"perro_app/mem_profile\"]\n", "")
        .replace(
            "\n[profile.dev.package.perro_physics]\nopt-level = 3\ndebug-assertions = false\noverflow-checks = false\n",
            "\n",
        );
    fs::write(&manifest, src).expect("write stale dev runner manifest");

    ensure_source_overrides(&root).expect("overrides");

    let repaired = fs::read_to_string(&manifest).expect("read repaired dev runner manifest");
    assert!(repaired.contains("timings = [\"perro_app/fps\"]"));
    assert!(repaired.contains("profile = [\"perro_app/profile\"]"));
    assert!(repaired.contains("ui_profile = [\"perro_app/ui_profile\"]"));
    assert!(repaired.contains("mem_profile = [\"perro_app/mem_profile\"]"));
    assert!(repaired.contains("build = \"build.rs\""));
    assert!(repaired.contains("winresource = \"0.1.20\""));
    assert!(repaired.contains("toml = \"0.8.23\""));
    assert!(repaired.contains("[target.'cfg(target_os = \"windows\")'.build-dependencies.image]"));
    assert!(repaired.contains("version = \"0.25.9\""));
    assert!(repaired.contains("resvg = \"0.47.0\""));
    assert!(repaired.contains("[profile.dev.package.perro_physics]"));
    assert!(repaired.contains("debug-assertions = false"));
    assert!(repaired.contains("overflow-checks = false"));

    let before = fs::metadata(&manifest)
        .and_then(|meta| meta.modified())
        .expect("dev runner manifest modified time before no-op");
    ensure_source_overrides(&root).expect("overrides no-op");
    let after = fs::metadata(&manifest)
        .and_then(|meta| meta.modified())
        .expect("dev runner manifest modified time after no-op");
    assert_eq!(before, after);

    let build_rs = fs::read_to_string(root.join(".perro").join("dev_runner").join("build.rs"))
        .expect("read dev runner build script");
    assert!(build_rs.contains("embed_windows_icon"));
    assert!(build_rs.contains("decode_svg_icon"));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn ensure_source_overrides_repairs_dev_runner_main() {
    let root = unique_temp_dir("perro_dev_runner_main");
    ensure_project_layout(&root).expect("layout");
    ensure_project_scaffold(&root, "Dev Runner Main").expect("scaffold");

    let main_rs = root
        .join(".perro")
        .join("dev_runner")
        .join("src")
        .join("main.rs");
    fs::write(
        &main_rs,
        r#"use perro_app::{entry, winit_runner::AppExitKind};
use perro_project::resolve_local_path;
use std::{env, path::PathBuf, process};

fn main() {
    let root = PathBuf::from(".");
    let fallback_name = "Perro Project".to_string();
    match entry::run_dev_project_from_path(&root, &fallback_name) {
        Ok(result) => match result.kind {
            AppExitKind::WindowClose => println!("perro exit: window close"),
            AppExitKind::EventLoopExit => println!("perro exit: event loop exit"),
        },
        Err(_) => process::exit(1),
    }
}
"#,
    )
    .expect("write stale dev runner main");

    ensure_source_overrides(&root).expect("overrides");
    let repaired = fs::read_to_string(&main_rs).expect("read repaired dev runner main");
    assert!(repaired.contains("run_dev_project_from_path"));
    assert!(repaired.contains("parse_flag_value"));
    assert!(!repaired.contains("threaded"));
    assert!(!repaired.contains("PERRO_THREADED_RENDER"));

    let before = fs::metadata(&main_rs)
        .and_then(|meta| meta.modified())
        .expect("main modified time before no-op");
    ensure_source_overrides(&root).expect("overrides no-op");
    let after = fs::metadata(&main_rs)
        .and_then(|meta| meta.modified())
        .expect("main modified time after no-op");
    assert_eq!(before, after);

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn ensure_source_overrides_recreates_missing_scripts_manifest() {
    let root = unique_temp_dir("perro_restore_scripts_manifest");
    ensure_project_layout(&root).expect("layout");
    ensure_project_scaffold(&root, "Restore Scripts Manifest").expect("scaffold");

    let manifest = root.join(".perro").join("scripts").join("Cargo.toml");
    let scripts_src = root.join(".perro").join("scripts").join("src");
    fs::remove_file(&manifest).expect("rm scripts manifest");
    fs::remove_dir_all(&scripts_src).expect("rm scripts src");

    ensure_source_overrides(&root).expect("overrides");

    let repaired = fs::read_to_string(&manifest).expect("read repaired scripts manifest");
    assert!(repaired.contains("name = \"scripts\""));
    assert!(repaired.contains("crate-type = [\"cdylib\", \"rlib\"]"));
    assert!(repaired.contains("dynamic-scripts = []"));
    assert!(manifest_dep_has_path(&repaired, "perro_api"));
    assert!(manifest_dep_has_path(&repaired, "perro_runtime"));
    let repaired_lib =
        fs::read_to_string(scripts_src.join("lib.rs")).expect("read repaired scripts lib");
    assert!(repaired_lib.contains("SCRIPT_REGISTRY"));
    assert!(repaired_lib.contains("perro_scripts_init"));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn scaffold_project_release_strip_only_targets_project_package() {
    let root = unique_temp_dir("perro_release_strip_project_only");
    ensure_project_layout(&root).expect("layout");
    ensure_project_scaffold(&root, "Strip Scope").expect("scaffold");

    let project_manifest =
        fs::read_to_string(root.join(".perro").join("project").join("Cargo.toml"))
            .expect("read project manifest");
    assert!(project_manifest.contains("[profile.release]\n"));
    assert!(project_manifest.contains("opt-level = 3"));
    assert!(project_manifest.contains("lto = \"fat\""));
    assert!(project_manifest.contains("codegen-units = 1"));
    assert!(project_manifest.contains("panic = \"abort\""));
    assert!(project_manifest.contains("strip = \"none\""));
    assert!(project_manifest.contains("[profile.release.package.strip_scope]"));
    assert!(project_manifest.contains("strip = \"symbols\""));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn parse_project_toml_rejects_unbound_audio_bounces() {
    let toml = format!(
        r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
aspect_ratio = "16:9"

[audio.propagation_2d]
max_bounces = {}
"#,
        MAX_AUDIO_PROPAGATION_BOUNCES + 1
    );

    let err = parse_project_toml(&toml).expect_err("bounce cap");
    assert!(err.to_string().contains("max_bounces must be <="), "{err}");
}

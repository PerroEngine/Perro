use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn parse_project_toml_reads_virtual_resolution_string() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_resolution = "1280x720"
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    assert_eq!(parsed.name, "Game");
    assert_eq!(parsed.main_scene, "res://main.scn");
    assert_eq!(parsed.icon, "res://icon.png");
    assert_eq!(parsed.virtual_width, 1280);
    assert_eq!(parsed.virtual_height, 720);
    assert!(!parsed.vsync);
    assert!(parsed.msaa);
    assert!(!parsed.meshlets);
    assert!(!parsed.dev_meshlets);
    assert!(parsed.release_meshlets);
    assert!(!parsed.meshlet_debug_view);
    assert_eq!(parsed.occlusion_culling, OcclusionCulling::Gpu);
    assert_eq!(parsed.particle_sim_default, ParticleSimDefault::Cpu);
    assert_eq!(parsed.physics_gravity, -9.81);
    assert_eq!(parsed.physics_coef, 1.0);
    assert!(parsed.localization.is_none());
    assert_eq!(parsed.steam, SteamConfig::default());
}

#[test]
fn parse_project_toml_reads_split_virtual_dimensions() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_width = 1920
virtual_height = 1080
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    assert_eq!(parsed.virtual_width, 1920);
    assert_eq!(parsed.virtual_height, 1080);
    assert!(!parsed.vsync);
    assert!(parsed.msaa);
    assert!(!parsed.meshlets);
    assert!(!parsed.dev_meshlets);
    assert!(parsed.release_meshlets);
    assert!(!parsed.meshlet_debug_view);
    assert_eq!(parsed.occlusion_culling, OcclusionCulling::Gpu);
    assert_eq!(parsed.particle_sim_default, ParticleSimDefault::Cpu);
    assert_eq!(parsed.physics_gravity, -9.81);
    assert_eq!(parsed.physics_coef, 1.0);
    assert!(parsed.localization.is_none());
}

#[test]
fn parse_project_toml_reads_vsync_and_msaa() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_resolution = "1920x1080"
vsync = true
msaa = false
meshlets = true
dev_meshlets = true
release_meshlets = false
meshlet_debug_view = true
occlusion_culling = "cpu"
particle_sim_default = "gpu"
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
    assert_eq!(parsed.physics_gravity, -9.81);
    assert_eq!(parsed.physics_coef, 1.0);
    assert!(parsed.localization.is_none());
}

#[test]
fn parse_project_toml_reads_physics_config() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_resolution = "1920x1080"

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
virtual_resolution = "1920x1080"

[steam]
enabled = true
app_id = 123456
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    assert!(parsed.steam.enabled);
    assert_eq!(parsed.steam.app_id, Some(123456));
}

#[test]
fn parse_project_toml_rejects_enabled_steam_without_app_id() {
    let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_resolution = "1920x1080"

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
virtual_resolution = "1920x1080"

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
virtual_resolution = "1920x1080"
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
virtual_resolution = "1920x1080"
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
virtual_resolution = "1920x1080"

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
virtual_resolution = "1920x1080"

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
virtual_resolution = "1920x1080"
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
virtual_resolution = "1920x1080"

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
virtual_resolution = "1920x1080"
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

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{nanos}_{}", std::process::id()))
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
    assert!(scripts_manifest.contains("perro_api = \"0.1.0\""));
    assert!(scripts_manifest.contains("perro_runtime = \"0.1.0\""));
    assert!(scripts_manifest.contains("serde"));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn ensure_source_overrides_adds_steamworks_when_steam_enabled() {
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
virtual_resolution = "1920x1080"

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
    assert!(scripts_manifest.contains("perro_steamworks = \"0.1.0\""));
    assert!(
        scripts_manifest.contains("perro_steamworks = { path =")
            || scripts_manifest.contains("perro_steamworks = {path =")
    );

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
virtual_resolution = "1920x1080"

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
virtual_resolution = "1920x1080"

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
    assert!(!scripts_manifest.contains("perro_steamworks = \"0.1.0\""));

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
    assert!(scripts_manifest.contains("perro_api = \"0.1.0\""));
    assert!(scripts_manifest.contains("perro_runtime = \"0.1.0\""));
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
    assert!(scripts_manifest.contains("perro_api = \"0.1.0\""));
    assert!(scripts_manifest.contains("perro_runtime = \"0.1.0\""));
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
        .replace("ui_profile = [\"perro_app/ui_profile\"]\n", "")
        .replace("mem_profile = [\"perro_app/mem_profile\"]\n", "");
    fs::write(&manifest, src).expect("write stale dev runner manifest");

    ensure_source_overrides(&root).expect("overrides");

    let repaired = fs::read_to_string(&manifest).expect("read repaired dev runner manifest");
    assert!(repaired.contains("profile = [\"perro_app/profile\"]"));
    assert!(repaired.contains("ui_profile = [\"perro_app/ui_profile\"]"));
    assert!(repaired.contains("mem_profile = [\"perro_app/mem_profile\"]"));

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
    assert!(project_manifest.contains("strip = \"none\""));
    assert!(project_manifest.contains("[profile.release.package.strip_scope]"));
    assert!(project_manifest.contains("strip = \"symbols\""));

    fs::remove_dir_all(&root).expect("cleanup");
}

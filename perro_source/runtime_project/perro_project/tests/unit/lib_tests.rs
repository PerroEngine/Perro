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
source = "res://loc/game.csv"
key = "key"
default_locale = "JA"
"#;

    let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
    let localization = parsed
        .localization
        .as_ref()
        .expect("localization should be present");
    assert_eq!(localization.source_csv, "res://loc/game.csv");
    assert_eq!(localization.key_column, "key");
    assert_eq!(localization.default_locale, "ja");
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

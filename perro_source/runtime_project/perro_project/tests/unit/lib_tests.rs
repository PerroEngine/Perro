use super::*;

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
fn resolve_local_path_maps_slash_to_local_root() {
    let root = PathBuf::from("D:/workspace");
    assert_eq!(
        resolve_local_path("/games/demo", &root),
        PathBuf::from("D:/workspace").join("games").join("demo")
    );
    assert_eq!(resolve_local_path("/", &root), root);
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

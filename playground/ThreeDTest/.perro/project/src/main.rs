#[path = "static/mod.rs"]
mod static_assets;

static ASSETS_BRK: &[u8] = include_bytes!("../embedded/assets.brk");

fn project_root() -> std::path::PathBuf {
if let Ok(exe) = std::env::current_exe() {
if let Some(exe_dir) = exe.parent() {
for dir in exe_dir.ancestors() {
if dir.join("project.toml").exists() {
return dir.to_path_buf();
}
}
return exe_dir.to_path_buf();
}
}
let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
if root.join("project.toml").exists() {
return root.canonicalize().unwrap_or(root);
}
std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

fn main() {
let root = project_root();
perro_app::entry::run_static_embedded_project(perro_app::entry::StaticEmbeddedProject {
project_root: &root,
project_name: "ThreeDTest",
main_scene: "res://main.scn",
icon: "res://icon.png",
virtual_width: 1920,
virtual_height: 1080,
assets_brk: ASSETS_BRK,
scene_lookup: static_assets::scenes::lookup_scene,
material_lookup: static_assets::materials::lookup_material,
mesh_lookup: static_assets::meshes::lookup_mesh,
texture_lookup: static_assets::textures::lookup_texture,
static_script_registry: Some(scripts::SCRIPT_REGISTRY),
}).expect("failed to run embedded static project");
}

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
perro_app::entry::run_static_embedded_project(
&root,
"TwoDTest",
"TwoDTest",
"res://main.scn",
"res://icon.png",
1920,
1080,
ASSETS_BRK,
static_assets::scenes::lookup_scene,
static_assets::materials::lookup_material,
static_assets::textures::lookup_texture,
Some(scripts::SCRIPT_REGISTRY),
).expect("failed to run embedded static project");
}

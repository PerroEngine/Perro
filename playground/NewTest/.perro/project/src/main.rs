#[path = "static/mod.rs"]
mod static_assets;

static PERRO_ASSETS: &[u8] = include_bytes!("../embedded/assets.perro");

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
        project_name: "NewTest",
        main_scene: "res://main.scn",
        icon: "res://icon.png",
        virtual_width: 1920,
        virtual_height: 1080,
        vsync: false,
        msaa: true,
        meshlets: false,
        dev_meshlets: false,
        release_meshlets: true,
        meshlet_debug_view: false,
        occlusion_culling: perro_app::entry::OcclusionCulling::Gpu,
        particle_sim_default: perro_app::entry::ParticleSimDefault::Cpu,
        perro_assets: PERRO_ASSETS,
        scene_lookup: static_assets::scenes::lookup_scene,
        material_lookup: static_assets::materials::lookup_material,
        particle_lookup: static_assets::particles::lookup_particle,
        mesh_lookup: static_assets::meshes::lookup_mesh,
        texture_lookup: static_assets::textures::lookup_texture,
        audio_lookup: static_assets::audios::lookup_audio,
        static_script_registry: Some(scripts::SCRIPT_REGISTRY),
    }).expect("failed to run embedded static project");
}

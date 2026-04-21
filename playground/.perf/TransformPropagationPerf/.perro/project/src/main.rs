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
          project: perro_app::entry::StaticEmbeddedProjectInfo {
              project_root: &root,
              project_name: "TransformPropagationPerf",
              main_scene_hash: 7300106721993353294u64,
              icon_hash: 6859512821849760879u64,
              startup_splash_hash: 6859512821849760879u64,
              virtual_width: 1920,
              virtual_height: 1080,
          },
          graphics: perro_app::entry::StaticEmbeddedGraphicsConfig {
              vsync: false,
              msaa: true,
              meshlets: false,
              dev_meshlets: false,
              release_meshlets: true,
              meshlet_debug_view: false,
              occlusion_culling: perro_app::entry::OcclusionCulling::Gpu,
              particle_sim_default: perro_app::entry::ParticleSimDefault::Cpu,
          },
          runtime: perro_app::entry::StaticEmbeddedRuntimeConfig {
              target_fixed_update: Some(60.0),
          },
          localization: perro_app::entry::StaticEmbeddedLocalizationConfig {
              source_csv_hash: None,
              key_column: "key",
              default_locale: "en",
          },
          assets: perro_app::entry::StaticEmbeddedAssetsConfig {
              perro_assets: PERRO_ASSETS,
              scene_lookup: static_assets::scenes::lookup_scene,
              localization_lookup: static_assets::localizations::lookup_localized_string,
              material_lookup: static_assets::materials::lookup_material,
              particle_lookup: static_assets::particles::lookup_particle,
              animation_lookup: static_assets::animations::lookup_animation,
              mesh_lookup: static_assets::meshes::lookup_mesh,
              skeleton_lookup: static_assets::skeletons::lookup_skeleton,
              texture_lookup: static_assets::textures::lookup_texture,
              shader_lookup: static_assets::shaders::lookup_shader,
              audio_lookup: static_assets::audios::lookup_audio,
              static_script_registry: Some(scripts::SCRIPT_REGISTRY),
          },
      })
      .expect("failed to run embedded static project");
  }

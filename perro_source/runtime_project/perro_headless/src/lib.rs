use perro_runtime::{ProviderMode, Runtime, RuntimeProject, WindowRequest};
use perro_scripting::ScriptConstructor;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub use perro_runtime::{FrameRateCap, OcclusionCulling, ParticleSimDefault};

pub type StaticScriptRegistry =
    &'static [(u64, ScriptConstructor<perro_runtime::RuntimeScriptApi>)];

pub fn run_dev_project_from_path(
    project_root: &Path,
    default_name: &str,
) -> Result<(), perro_runtime::ProjectLoadError> {
    let mut project =
        RuntimeProject::from_project_dir_with_default_name(project_root, default_name)?;
    init_steam_server(&mut project);
    run_runtime(Runtime::from_project(project, ProviderMode::Dynamic));
    Ok(())
}

pub fn run_static_embedded_project(
    input: StaticEmbeddedProject<'_>,
) -> Result<(), std::convert::Infallible> {
    #[cfg(feature = "steamworks")]
    if input.steam.enabled
        && let Some(app_id) = input.steam.app_id.map(|get| get())
    {
        let config = perro_steamworks::game_server::GameServerConfig::from_env(
            app_id,
            input.project.project_name,
            input.metadata.version,
        );
        if let Err(err) = perro_steamworks::runtime::init_game_server(config) {
            eprintln!("[headless][steam][warn] game-server init fail: {err}");
        }
    }
    let config = perro_runtime::StaticProjectConfig::new(
        input.project.project_name,
        input.project.main_scene_hash,
        input.project.icon_hash,
        input.project.startup_splash_hash,
        input.project.virtual_width,
        input.project.virtual_height,
    )
    .with_target_fixed_update(input.runtime.target_fixed_update)
    .with_frame_rate_cap(input.runtime.frame_rate_cap)
    .with_physics_gravity(input.runtime.physics_gravity)
    .with_physics_coef(input.runtime.physics_coef)
    .with_particle_sim_default(ParticleSimDefault::Cpu)
    .with_metadata(
        input.metadata.description,
        input.metadata.company,
        input.metadata.version,
        input.metadata.copyright,
        input.metadata.trademark,
    )
    .with_localization(input.localization.default_locale)
    .with_steam(false, None)
    .with_steam_input_mode(input.steam.input_mode);
    let project = RuntimeProject::from_static(config, input.project.project_root.to_path_buf())
        .with_routes(routes(&input.routes))
        .with_input_map(input_map(&input.input))
        .with_static_scene_lookup(input.assets.scene_lookup)
        .with_static_localization_lookup(input.assets.localization_lookup)
        .with_static_material_lookup(input.assets.material_lookup)
        .with_static_ui_style_lookup(input.assets.ui_style_lookup)
        .with_static_tileset_lookup(input.assets.tileset_lookup)
        .with_static_particle_lookup(input.assets.particle_lookup)
        .with_static_animation_lookup(input.assets.animation_lookup)
        .with_static_animation_tree_lookup(input.assets.animation_tree_lookup)
        .with_static_csv_lookup(input.assets.csv_lookup)
        .with_static_collision_trimesh_lookup(input.assets.collision_trimesh_lookup)
        .with_static_navmesh_lookup(input.assets.navmesh_lookup)
        .with_static_skeleton_lookup(input.assets.skeleton_lookup)
        .with_static_audio_lookup(input.assets.audio_lookup)
        .with_perro_assets_bytes(input.assets.perro_assets);
    run_runtime(Runtime::from_project_with_script_registry(
        project,
        ProviderMode::Static,
        input.assets.static_script_registry,
    ));
    Ok(())
}

fn init_steam_server(project: &mut RuntimeProject) {
    #[cfg(feature = "steamworks")]
    if project.config.steam.enabled
        && let Some(app_id) = project.config.steam.app_id
    {
        let config = perro_steamworks::game_server::GameServerConfig::from_env(
            app_id,
            &project.config.name,
            project.config.metadata.version.as_deref(),
        );
        if let Err(err) = perro_steamworks::runtime::init_game_server(config) {
            eprintln!("[headless][steam][warn] game-server init fail: {err}");
        }
        project.config.steam.enabled = false;
    }
    #[cfg(not(feature = "steamworks"))]
    let _ = project;
}

fn run_runtime(mut runtime: Runtime) {
    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    let _ = ctrlc::set_handler(move || signal.store(false, Ordering::SeqCst));
    let fixed_delta = runtime
        .project()
        .and_then(|project| project.config.target_fixed_update)
        .filter(|fps| *fps > 0.0)
        .map(|fps| 1.0 / fps)
        .unwrap_or(1.0 / 60.0);
    let step = Duration::from_secs_f32(fixed_delta);
    let mut last = Instant::now();
    let mut accumulator = Duration::ZERO;
    let mut requests = Vec::new();
    while running.load(Ordering::SeqCst) {
        let frame_start = Instant::now();
        let delta = frame_start
            .duration_since(last)
            .min(Duration::from_millis(250));
        last = frame_start;
        accumulator += delta;
        runtime.update(delta.as_secs_f32());
        while accumulator >= step {
            runtime.fixed_update(fixed_delta);
            accumulator -= step;
        }
        runtime.drain_window_requests(&mut requests);
        if requests
            .iter()
            .any(|request| matches!(request, WindowRequest::CloseApp))
        {
            break;
        }
        requests.clear();
        if let Some(rest) = step.checked_sub(frame_start.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}

fn routes(input: &StaticEmbeddedRoutesConfig<'_>) -> perro_runtime::ProjectRoutesConfig {
    perro_runtime::ProjectRoutesConfig {
        routes: input
            .routes
            .iter()
            .map(|route| perro_runtime::ProjectRoute {
                href: route.href.to_string(),
                name: route.name.to_string(),
                scene: route.scene_hash.to_string(),
                title: None,
                description: None,
                keywords: Vec::new(),
            })
            .collect(),
    }
}

fn input_map(input: &StaticEmbeddedInputMapConfig<'_>) -> perro_input_api::InputMap {
    perro_input_api::InputMap::from_actions(
        input
            .actions
            .iter()
            .map(|action| {
                let bindings = action
                    .keys
                    .iter()
                    .copied()
                    .map(perro_input_api::InputBinding::Key)
                    .chain(
                        action
                            .mouse
                            .iter()
                            .copied()
                            .map(perro_input_api::InputBinding::Mouse),
                    )
                    .chain(
                        action
                            .gamepad
                            .iter()
                            .copied()
                            .map(perro_input_api::InputBinding::Gamepad),
                    )
                    .chain(
                        action
                            .joycon
                            .iter()
                            .copied()
                            .map(perro_input_api::InputBinding::JoyCon),
                    )
                    .collect();
                perro_input_api::InputAction::new(action.name, bindings)
            })
            .collect(),
    )
}

pub struct StaticEmbeddedProject<'a> {
    pub project: StaticEmbeddedProjectInfo<'a>,
    pub routes: StaticEmbeddedRoutesConfig<'a>,
    pub input: StaticEmbeddedInputMapConfig<'a>,
    pub graphics: StaticEmbeddedGraphicsConfig,
    pub runtime: StaticEmbeddedRuntimeConfig,
    pub metadata: StaticEmbeddedMetadataConfig,
    pub localization: StaticEmbeddedLocalizationConfig,
    pub steam: StaticEmbeddedSteamConfig,
    pub assets: StaticEmbeddedAssetsConfig,
}
pub struct StaticEmbeddedProjectInfo<'a> {
    pub project_root: &'a Path,
    pub project_name: &'static str,
    pub main_scene_hash: u64,
    pub icon_hash: u64,
    pub startup_splash_hash: u64,
    pub virtual_width: u32,
    pub virtual_height: u32,
}
#[derive(Clone, Copy)]
pub struct StaticEmbeddedRoute {
    pub href: &'static str,
    pub name: &'static str,
    pub scene_hash: u64,
}
pub struct StaticEmbeddedRoutesConfig<'a> {
    pub routes: &'a [StaticEmbeddedRoute],
}
#[derive(Clone, Copy)]
pub struct StaticEmbeddedInputAction {
    pub name: &'static str,
    pub keys: &'static [perro_input_api::KeyCode],
    pub mouse: &'static [perro_input_api::MouseButton],
    pub gamepad: &'static [perro_input_api::GamepadButton],
    pub joycon: &'static [perro_input_api::JoyConButton],
}
pub struct StaticEmbeddedInputMapConfig<'a> {
    pub actions: &'a [StaticEmbeddedInputAction],
}
pub struct StaticEmbeddedGraphicsConfig {
    pub vsync: bool,
    pub msaa: bool,
    pub ssao: perro_runtime::SsaoQuality,
    pub meshlets: bool,
    pub dev_meshlets: bool,
    pub release_meshlets: bool,
    pub meshlet_debug_view: bool,
    pub occlusion_culling: OcclusionCulling,
    pub particle_sim_default: ParticleSimDefault,
    pub ui_pixel_snapping: bool,
}
pub struct StaticEmbeddedRuntimeConfig {
    pub target_fixed_update: Option<f32>,
    pub frame_rate_cap: FrameRateCap,
    pub physics_gravity: f32,
    pub physics_coef: f32,
}
pub struct StaticEmbeddedMetadataConfig {
    pub description: Option<&'static str>,
    pub company: Option<&'static str>,
    pub version: Option<&'static str>,
    pub copyright: Option<&'static str>,
    pub trademark: Option<&'static str>,
}
pub struct StaticEmbeddedLocalizationConfig {
    pub default_locale: &'static str,
}
pub struct StaticEmbeddedSteamConfig {
    pub enabled: bool,
    pub app_id: Option<fn() -> u32>,
    pub input_mode: perro_runtime::SteamInputMode,
}
pub struct StaticEmbeddedAssetsConfig {
    pub perro_assets: &'static [u8],
    pub scene_lookup: perro_runtime::StaticSceneLookup,
    pub localization_lookup: perro_runtime::StaticLocalizationLookup,
    pub material_lookup: perro_runtime::StaticMaterialLookup,
    pub ui_style_lookup: perro_runtime::StaticUiStyleLookup,
    pub tileset_lookup: perro_runtime::StaticTilesetLookup,
    pub particle_lookup: perro_runtime::StaticParticleLookup,
    pub animation_lookup: perro_runtime::StaticAnimationLookup,
    pub animation_tree_lookup: perro_runtime::StaticAnimationTreeLookup,
    pub csv_lookup: perro_runtime::StaticCsvLookup,
    pub mesh_lookup: perro_runtime::StaticBytesLookup,
    pub collision_trimesh_lookup: perro_runtime::StaticBytesLookup,
    pub navmesh_lookup: perro_runtime::StaticBytesLookup,
    pub skeleton_lookup: perro_runtime::StaticSkeletonLookup,
    pub texture_lookup: perro_runtime::StaticBytesLookup,
    pub shader_lookup: perro_runtime::StaticShaderLookup,
    pub audio_lookup: perro_runtime::StaticAudioLookup,
    pub static_script_registry: Option<StaticScriptRegistry>,
}

use crate::App;
#[cfg(not(target_arch = "wasm32"))]
use crate::winit_runner::image_helpers::{preload_project_images, spawn_preload_project_images};
use crate::winit_runner::{AppExitError, AppExitResult, WinitRunner};
use perro_graphics::{
    GraphicsBackend, OcclusionCullingMode, PerroGraphics, SsaoQuality as GraphicsSsaoQuality,
};
pub use perro_runtime::{FrameRateCap, OcclusionCulling, ParticleSimDefault};
use perro_runtime::{ProjectLoadError, ProviderMode, Runtime, RuntimeProject, WindowRequest};
use perro_scripting::ScriptConstructor;
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_os = "android")]
pub use winit::platform::android::activity::AndroidApp;

type StaticScriptRegistry = &'static [(u64, ScriptConstructor<perro_runtime::RuntimeScriptApi>)];

#[cfg(target_os = "windows")]
fn clear_steam_fossilize_application_filter(steam_enabled: bool) {
    if steam_enabled && std::env::var_os("FOSSILIZE_APPLICATION_INFO_FILTER_PATH").is_some() {
        // Steam's Windows Fossilize layer may ship a filter newer than the loaded layer.
        // A parse failure already falls back to unfiltered recording, so skip only the filter.
        // SAFETY: Windows synchronizes process-environment access.
        unsafe { std::env::remove_var("FOSSILIZE_APPLICATION_INFO_FILTER_PATH") };
    }
}

#[cfg(not(target_os = "windows"))]
fn clear_steam_fossilize_application_filter(_steam_enabled: bool) {}

#[derive(Debug)]
pub enum RunProjectError {
    Load(ProjectLoadError),
    Exit(AppExitError),
}

impl std::fmt::Display for RunProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load(err) => write!(f, "{err}"),
            Self::Exit(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for RunProjectError {}

impl From<ProjectLoadError> for RunProjectError {
    fn from(value: ProjectLoadError) -> Self {
        Self::Load(value)
    }
}

impl From<AppExitError> for RunProjectError {
    fn from(value: AppExitError) -> Self {
        Self::Exit(value)
    }
}

pub fn create_runtime_from_project(
    project: RuntimeProject,
    provider_mode: ProviderMode,
) -> Runtime {
    Runtime::from_project(project, provider_mode)
}

fn static_embedded_routes(
    input: &StaticEmbeddedRoutesConfig<'_>,
) -> perro_runtime::ProjectRoutesConfig {
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

fn static_embedded_input_map(
    input: &StaticEmbeddedInputMapConfig<'_>,
) -> perro_input_api::InputMap {
    let actions = input
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
        .collect();
    perro_input_api::InputMap::from_actions(actions)
}

pub fn create_dev_runtime(project: RuntimeProject) -> Runtime {
    create_runtime_from_project(project, ProviderMode::Dynamic)
}

pub fn create_static_runtime(project: RuntimeProject) -> Runtime {
    create_runtime_from_project(project, ProviderMode::Static)
}

pub fn create_app_from_project<B: GraphicsBackend>(
    graphics: B,
    project: RuntimeProject,
    provider_mode: ProviderMode,
) -> App<B> {
    App::new(
        create_runtime_from_project(project, provider_mode),
        graphics,
    )
}

pub fn create_dev_app<B: GraphicsBackend>(graphics: B, project: RuntimeProject) -> App<B> {
    create_app_from_project(graphics, project, ProviderMode::Dynamic)
}

pub fn create_static_app<B: GraphicsBackend>(graphics: B, project: RuntimeProject) -> App<B> {
    create_app_from_project(graphics, project, ProviderMode::Static)
}

fn graphics_from_project_config(
    config: &perro_runtime::RuntimeProjectConfig,
    release_mode: bool,
) -> PerroGraphics {
    let occlusion_culling = effective_occlusion_culling(config.occlusion_culling);
    PerroGraphics::new()
        .with_vsync(config.vsync)
        .with_msaa(effective_msaa(config.msaa))
        .with_ssao(graphics_ssao(config.ssao))
        .with_meshlets_enabled(config.meshlets)
        .with_dev_meshlets(!release_mode && config.dev_meshlets)
        .with_meshlet_debug_view(config.meshlet_debug_view)
        .with_texture_filter(config.texture_filter)
        .with_occlusion_culling(match occlusion_culling {
            OcclusionCulling::Cpu => OcclusionCullingMode::Cpu,
            OcclusionCulling::Gpu => OcclusionCullingMode::Gpu,
            OcclusionCulling::Off => OcclusionCullingMode::Off,
        })
}

#[cfg(not(target_arch = "wasm32"))]
fn effective_occlusion_culling(mode: OcclusionCulling) -> OcclusionCulling {
    mode
}

#[cfg(target_arch = "wasm32")]
fn effective_occlusion_culling(_: OcclusionCulling) -> OcclusionCulling {
    OcclusionCulling::Off
}

#[cfg(not(target_arch = "wasm32"))]
fn effective_msaa(enabled: bool) -> bool {
    enabled
}

#[cfg(target_arch = "wasm32")]
fn effective_msaa(_: bool) -> bool {
    false
}

fn graphics_ssao(quality: perro_runtime::SsaoQuality) -> GraphicsSsaoQuality {
    match quality {
        perro_runtime::SsaoQuality::Off => GraphicsSsaoQuality::Off,
        perro_runtime::SsaoQuality::Low => GraphicsSsaoQuality::Low,
        perro_runtime::SsaoQuality::Medium => GraphicsSsaoQuality::Medium,
        perro_runtime::SsaoQuality::High => GraphicsSsaoQuality::High,
        perro_runtime::SsaoQuality::Ultra => GraphicsSsaoQuality::Ultra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_keeps_occlusion_mode() {
        assert_eq!(
            effective_occlusion_culling(OcclusionCulling::Gpu),
            OcclusionCulling::Gpu
        );
        assert_eq!(
            effective_occlusion_culling(OcclusionCulling::Cpu),
            OcclusionCulling::Cpu
        );
        assert!(effective_msaa(true));
        assert!(!effective_msaa(false));
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn wasm_forces_occlusion_off() {
        assert_eq!(
            effective_occlusion_culling(OcclusionCulling::Gpu),
            OcclusionCulling::Off
        );
        assert_eq!(
            effective_occlusion_culling(OcclusionCulling::Cpu),
            OcclusionCulling::Off
        );
        assert!(!effective_msaa(true));
        assert!(!effective_msaa(false));
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_dev_project_from_path(
    project_root: &Path,
    default_name: &str,
) -> Result<AppExitResult, RunProjectError> {
    eprintln!(
        "perro dev runner: load project {}",
        project_root.to_string_lossy()
    );
    let project = RuntimeProject::from_project_dir_with_default_name(project_root, default_name)?;
    clear_steam_fossilize_application_filter(project.config.steam.enabled);
    let _ = perro_web::init_router();
    let preload = spawn_preload_project_images(project.clone());
    eprintln!("perro dev runner: init graphics");
    let window_title = project.config.name.clone();
    let graphics = graphics_from_project_config(&project.config, false);
    eprintln!("perro dev runner: init runtime");
    let app = create_dev_app(graphics, project);
    let fixed = app
        .runtime
        .project()
        .and_then(|p| p.config.target_fixed_update);
    let preloaded_images = preload
        .join()
        .unwrap_or_else(|_| preload_project_images(app.runtime.project()));
    eprintln!("perro dev runner: enter event loop");
    WinitRunner::new()
        .run_with_timestep_and_preload(app, &window_title, fixed, Some(preloaded_images))
        .map_err(RunProjectError::from)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_headless_dev_project_from_path(
    project_root: &Path,
    default_name: &str,
) -> Result<(), RunProjectError> {
    let project = RuntimeProject::from_project_dir_with_default_name(project_root, default_name)?;
    run_headless_runtime(create_dev_runtime(project));
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn run_headless_runtime(mut runtime: Runtime) {
    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    let _ = ctrlc::set_handler(move || signal.store(false, Ordering::SeqCst));
    let fixed_step = runtime
        .project()
        .and_then(|project| project.config.target_fixed_update)
        .filter(|fps| *fps > 0.0)
        .map(|fps| 1.0 / fps)
        .unwrap_or(1.0 / 60.0);
    let step = Duration::from_secs_f32(fixed_step);
    let mut last = Instant::now();
    let mut accumulator = Duration::ZERO;
    let mut requests = Vec::new();
    while running.load(Ordering::SeqCst) {
        let frame_start = Instant::now();
        let delta = frame_start.duration_since(last);
        last = frame_start;
        accumulator += delta.min(Duration::from_millis(250));
        runtime.update(delta.as_secs_f32());
        while accumulator >= step {
            runtime.fixed_update(fixed_step);
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

pub fn run_static_project_from_path(
    project_root: &Path,
    default_name: &str,
) -> Result<AppExitResult, RunProjectError> {
    let project = RuntimeProject::from_project_dir_with_default_name(project_root, default_name)?;
    clear_steam_fossilize_application_filter(project.config.steam.enabled);
    let _ = perro_web::init_router();
    let window_title = project.config.name.clone();
    let graphics = graphics_from_project_config(&project.config, true);
    let app = create_static_app(graphics, project);
    let fixed = app
        .runtime
        .project()
        .and_then(|p| p.config.target_fixed_update);
    WinitRunner::new()
        .run_with_timestep(app, &window_title, fixed)
        .map_err(RunProjectError::from)
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
    pub mesh_lookup: perro_graphics::StaticMeshLookup,
    pub collision_trimesh_lookup: perro_runtime::StaticBytesLookup,
    pub navmesh_lookup: perro_runtime::StaticBytesLookup,
    pub skeleton_lookup: perro_runtime::StaticSkeletonLookup,
    pub texture_lookup: perro_graphics::StaticTextureLookup,
    pub shader_lookup: perro_graphics::StaticShaderLookup,
    pub audio_lookup: perro_runtime::StaticAudioLookup,
    pub static_script_registry: Option<StaticScriptRegistry>,
}

pub fn run_static_embedded_project(
    input: StaticEmbeddedProject<'_>,
) -> Result<AppExitResult, RunProjectError> {
    clear_steam_fossilize_application_filter(input.steam.enabled);
    let _ = perro_web::init_router();
    let mut static_config = perro_runtime::StaticProjectConfig::new(
        input.project.project_name,
        input.project.main_scene_hash,
        input.project.icon_hash,
        input.project.startup_splash_hash,
        input.project.virtual_width,
        input.project.virtual_height,
    )
    .with_vsync(input.graphics.vsync)
    .with_target_fixed_update(input.runtime.target_fixed_update)
    .with_frame_rate_cap(input.runtime.frame_rate_cap)
    .with_physics_gravity(input.runtime.physics_gravity)
    .with_physics_coef(input.runtime.physics_coef)
    .with_msaa(input.graphics.msaa)
    .with_ssao(input.graphics.ssao)
    .with_meshlets(input.graphics.meshlets)
    .with_dev_meshlets(input.graphics.dev_meshlets)
    .with_release_meshlets(input.graphics.release_meshlets)
    .with_meshlet_debug_view(input.graphics.meshlet_debug_view)
    .with_occlusion_culling(input.graphics.occlusion_culling)
    .with_particle_sim_default(input.graphics.particle_sim_default)
    .with_ui_pixel_snapping(input.graphics.ui_pixel_snapping)
    .with_metadata(
        input.metadata.description,
        input.metadata.company,
        input.metadata.version,
        input.metadata.copyright,
        input.metadata.trademark,
    );
    static_config = static_config.with_localization(input.localization.default_locale);
    static_config = static_config
        .with_steam(input.steam.enabled, input.steam.app_id.map(|f| f()))
        .with_steam_input_mode(input.steam.input_mode);
    let mut project =
        RuntimeProject::from_static(static_config, input.project.project_root.to_path_buf())
            .with_routes(static_embedded_routes(&input.routes))
            .with_input_map(static_embedded_input_map(&input.input));

    project = project
        .with_static_scene_lookup(input.assets.scene_lookup)
        .with_static_localization_lookup(input.assets.localization_lookup)
        .with_static_material_lookup(input.assets.material_lookup)
        .with_static_ui_style_lookup(input.assets.ui_style_lookup)
        .with_static_tileset_lookup(input.assets.tileset_lookup)
        .with_static_particle_lookup(input.assets.particle_lookup)
        .with_static_animation_lookup(input.assets.animation_lookup)
        .with_static_animation_tree_lookup(input.assets.animation_tree_lookup)
        .with_static_csv_lookup(input.assets.csv_lookup)
        .with_static_mesh_lookup(input.assets.mesh_lookup)
        .with_static_collision_trimesh_lookup(input.assets.collision_trimesh_lookup)
        .with_static_navmesh_lookup(input.assets.navmesh_lookup)
        .with_static_skeleton_lookup(input.assets.skeleton_lookup)
        .with_static_audio_lookup(input.assets.audio_lookup)
        .with_static_texture_lookup(input.assets.texture_lookup)
        .with_static_shader_lookup(input.assets.shader_lookup)
        .with_static_icon_lookup(input.assets.texture_lookup)
        .with_perro_assets_bytes(input.assets.perro_assets);

    let window_title = project.config.name.clone();
    let graphics = graphics_from_project_config(&project.config, true)
        .with_static_mesh_lookup(input.assets.mesh_lookup)
        .with_static_texture_lookup(input.assets.texture_lookup)
        .with_static_shader_lookup(input.assets.shader_lookup);
    let runtime = Runtime::from_project_with_script_registry(
        project,
        ProviderMode::Static,
        input.assets.static_script_registry,
    );
    let app = App::new(runtime, graphics);
    let fixed = app
        .runtime
        .project()
        .and_then(|p| p.config.target_fixed_update);
    WinitRunner::new()
        .run_with_timestep(app, &window_title, fixed)
        .map_err(RunProjectError::from)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_static_embedded_project_headless(input: StaticEmbeddedProject<'_>) {
    let mut static_config = perro_runtime::StaticProjectConfig::new(
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
    .with_particle_sim_default(input.graphics.particle_sim_default)
    .with_metadata(
        input.metadata.description,
        input.metadata.company,
        input.metadata.version,
        input.metadata.copyright,
        input.metadata.trademark,
    );
    static_config = static_config.with_localization(input.localization.default_locale);
    static_config = static_config
        .with_steam(input.steam.enabled, input.steam.app_id.map(|f| f()))
        .with_steam_input_mode(input.steam.input_mode);
    let project =
        RuntimeProject::from_static(static_config, input.project.project_root.to_path_buf())
            .with_routes(static_embedded_routes(&input.routes))
            .with_input_map(static_embedded_input_map(&input.input))
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
    let runtime = Runtime::from_project_with_script_registry(
        project,
        ProviderMode::Static,
        input.assets.static_script_registry,
    );
    run_headless_runtime(runtime);
}

#[cfg(target_os = "android")]
pub fn run_static_embedded_project_android(
    android_app: AndroidApp,
    input: StaticEmbeddedProject<'_>,
) -> Result<AppExitResult, RunProjectError> {
    let _ = perro_web::init_router();
    let mut static_config = perro_runtime::StaticProjectConfig::new(
        input.project.project_name,
        input.project.main_scene_hash,
        input.project.icon_hash,
        input.project.startup_splash_hash,
        input.project.virtual_width,
        input.project.virtual_height,
    )
    .with_vsync(input.graphics.vsync)
    .with_target_fixed_update(input.runtime.target_fixed_update)
    .with_frame_rate_cap(input.runtime.frame_rate_cap)
    .with_physics_gravity(input.runtime.physics_gravity)
    .with_physics_coef(input.runtime.physics_coef)
    .with_msaa(input.graphics.msaa)
    .with_ssao(input.graphics.ssao)
    .with_meshlets(input.graphics.meshlets)
    .with_dev_meshlets(input.graphics.dev_meshlets)
    .with_release_meshlets(input.graphics.release_meshlets)
    .with_meshlet_debug_view(input.graphics.meshlet_debug_view)
    .with_occlusion_culling(input.graphics.occlusion_culling)
    .with_particle_sim_default(input.graphics.particle_sim_default)
    .with_ui_pixel_snapping(input.graphics.ui_pixel_snapping)
    .with_metadata(
        input.metadata.description,
        input.metadata.company,
        input.metadata.version,
        input.metadata.copyright,
        input.metadata.trademark,
    );
    static_config = static_config.with_localization(input.localization.default_locale);
    static_config = static_config
        .with_steam(input.steam.enabled, input.steam.app_id.map(|f| f()))
        .with_steam_input_mode(input.steam.input_mode);
    let mut project =
        RuntimeProject::from_static(static_config, input.project.project_root.to_path_buf())
            .with_routes(static_embedded_routes(&input.routes))
            .with_input_map(static_embedded_input_map(&input.input));

    project = project
        .with_static_scene_lookup(input.assets.scene_lookup)
        .with_static_localization_lookup(input.assets.localization_lookup)
        .with_static_material_lookup(input.assets.material_lookup)
        .with_static_ui_style_lookup(input.assets.ui_style_lookup)
        .with_static_tileset_lookup(input.assets.tileset_lookup)
        .with_static_particle_lookup(input.assets.particle_lookup)
        .with_static_animation_lookup(input.assets.animation_lookup)
        .with_static_animation_tree_lookup(input.assets.animation_tree_lookup)
        .with_static_csv_lookup(input.assets.csv_lookup)
        .with_static_mesh_lookup(input.assets.mesh_lookup)
        .with_static_collision_trimesh_lookup(input.assets.collision_trimesh_lookup)
        .with_static_navmesh_lookup(input.assets.navmesh_lookup)
        .with_static_skeleton_lookup(input.assets.skeleton_lookup)
        .with_static_audio_lookup(input.assets.audio_lookup)
        .with_static_texture_lookup(input.assets.texture_lookup)
        .with_static_shader_lookup(input.assets.shader_lookup)
        .with_static_icon_lookup(input.assets.texture_lookup)
        .with_perro_assets_bytes(input.assets.perro_assets);

    let window_title = project.config.name.clone();
    let graphics = graphics_from_project_config(&project.config, true)
        .with_static_mesh_lookup(input.assets.mesh_lookup)
        .with_static_texture_lookup(input.assets.texture_lookup)
        .with_static_shader_lookup(input.assets.shader_lookup);
    let runtime = Runtime::from_project_with_script_registry(
        project,
        ProviderMode::Static,
        input.assets.static_script_registry,
    );
    let app = App::new(runtime, graphics);
    let fixed = app
        .runtime
        .project()
        .and_then(|p| p.config.target_fixed_update);
    WinitRunner::new()
        .run_with_timestep_android(app, &window_title, fixed, android_app)
        .map_err(RunProjectError::from)
}

#[cfg(target_arch = "wasm32")]
pub fn run_static_embedded_project_web(input: StaticEmbeddedProject<'_>) -> Result<(), JsValue> {
    fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
        if let Some(msg) = payload.downcast_ref::<String>() {
            return msg.clone();
        }
        if let Some(msg) = payload.downcast_ref::<&'static str>() {
            return (*msg).to_string();
        }
        "unknown panic".to_string()
    }

    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = perro_web::init_router();
        let mut static_config = perro_runtime::StaticProjectConfig::new(
            input.project.project_name,
            input.project.main_scene_hash,
            input.project.icon_hash,
            input.project.startup_splash_hash,
            input.project.virtual_width,
            input.project.virtual_height,
        )
        .with_vsync(input.graphics.vsync)
        .with_target_fixed_update(input.runtime.target_fixed_update)
        .with_frame_rate_cap(input.runtime.frame_rate_cap)
        .with_physics_gravity(input.runtime.physics_gravity)
        .with_physics_coef(input.runtime.physics_coef)
        .with_msaa(input.graphics.msaa)
        .with_ssao(input.graphics.ssao)
        .with_meshlets(input.graphics.meshlets)
        .with_dev_meshlets(input.graphics.dev_meshlets)
        .with_release_meshlets(input.graphics.release_meshlets)
        .with_meshlet_debug_view(input.graphics.meshlet_debug_view)
        .with_occlusion_culling(input.graphics.occlusion_culling)
        .with_particle_sim_default(input.graphics.particle_sim_default)
        .with_ui_pixel_snapping(input.graphics.ui_pixel_snapping)
        .with_metadata(
            input.metadata.description,
            input.metadata.company,
            input.metadata.version,
            input.metadata.copyright,
            input.metadata.trademark,
        );
        static_config = static_config.with_localization(input.localization.default_locale);
        static_config = static_config
            .with_steam(input.steam.enabled, input.steam.app_id.map(|f| f()))
            .with_steam_input_mode(input.steam.input_mode);
        let mut project =
            RuntimeProject::from_static(static_config, input.project.project_root.to_path_buf())
                .with_routes(static_embedded_routes(&input.routes))
                .with_input_map(static_embedded_input_map(&input.input));

        project = project
            .with_static_scene_lookup(input.assets.scene_lookup)
            .with_static_localization_lookup(input.assets.localization_lookup)
            .with_static_material_lookup(input.assets.material_lookup)
            .with_static_ui_style_lookup(input.assets.ui_style_lookup)
            .with_static_tileset_lookup(input.assets.tileset_lookup)
            .with_static_particle_lookup(input.assets.particle_lookup)
            .with_static_animation_lookup(input.assets.animation_lookup)
            .with_static_animation_tree_lookup(input.assets.animation_tree_lookup)
            .with_static_csv_lookup(input.assets.csv_lookup)
            .with_static_mesh_lookup(input.assets.mesh_lookup)
            .with_static_collision_trimesh_lookup(input.assets.collision_trimesh_lookup)
            .with_static_navmesh_lookup(input.assets.navmesh_lookup)
            .with_static_skeleton_lookup(input.assets.skeleton_lookup)
            .with_static_audio_lookup(input.assets.audio_lookup)
            .with_static_texture_lookup(input.assets.texture_lookup)
            .with_static_shader_lookup(input.assets.shader_lookup)
            .with_static_icon_lookup(input.assets.texture_lookup)
            .with_perro_assets_bytes(input.assets.perro_assets);

        let window_title = project.config.name.clone();
        let graphics = graphics_from_project_config(&project.config, true)
            .with_static_mesh_lookup(input.assets.mesh_lookup)
            .with_static_texture_lookup(input.assets.texture_lookup)
            .with_static_shader_lookup(input.assets.shader_lookup);
        let runtime = Runtime::from_project_with_script_registry(
            project,
            ProviderMode::Static,
            input.assets.static_script_registry,
        );
        let app = App::new(runtime, graphics);
        let fixed = app
            .runtime
            .project()
            .and_then(|p| p.config.target_fixed_update);
        WinitRunner::new()
            .run_with_timestep(app, &window_title, fixed)
            .map(|_| ())
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }))
    .map_err(|payload| JsValue::from_str(&panic_payload_to_string(payload)))?
}

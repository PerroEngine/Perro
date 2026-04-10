use crate::App;
use crate::winit_runner::WinitRunner;
use perro_graphics::{GraphicsBackend, OcclusionCullingMode, PerroGraphics};
pub use perro_runtime::{OcclusionCulling, ParticleSimDefault};
use perro_runtime::{ProjectLoadError, ProviderMode, Runtime, RuntimeProject};
use perro_scripting::ScriptConstructor;
use std::path::Path;

type StaticScriptRegistry = &'static [(
    &'static str,
    ScriptConstructor<Runtime, perro_runtime::RuntimeResourceApi, perro_runtime::RuntimeInputApi>,
)];

pub fn create_runtime_from_project(
    project: RuntimeProject,
    provider_mode: ProviderMode,
) -> Runtime {
    Runtime::from_project(project, provider_mode)
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
    PerroGraphics::new()
        .with_vsync(config.vsync)
        .with_msaa(config.msaa)
        .with_meshlets_enabled(config.meshlets)
        .with_dev_meshlets(!release_mode && config.dev_meshlets)
        .with_meshlet_debug_view(config.meshlet_debug_view)
        .with_occlusion_culling(match config.occlusion_culling {
            OcclusionCulling::Cpu => OcclusionCullingMode::Cpu,
            OcclusionCulling::Gpu => OcclusionCullingMode::Gpu,
            OcclusionCulling::Off => OcclusionCullingMode::Off,
        })
}

pub fn run_dev_project_from_path(
    project_root: &Path,
    default_name: &str,
) -> Result<(), ProjectLoadError> {
    let project = RuntimeProject::from_project_dir_with_default_name(project_root, default_name)?;
    let window_title = project.config.name.clone();
    let graphics = graphics_from_project_config(&project.config, false);
    let app = create_dev_app(graphics, project);
    let fps_cap = app
        .runtime
        .project()
        .and_then(|p| p.config.target_fps)
        .unwrap_or(0.0);
    let fixed = app
        .runtime
        .project()
        .and_then(|p| p.config.target_fixed_update);
    WinitRunner::new().run_with_fps_cap_and_timestep(app, &window_title, fps_cap, fixed);
    Ok(())
}

pub fn run_static_project_from_path(
    project_root: &Path,
    default_name: &str,
) -> Result<(), ProjectLoadError> {
    let project = RuntimeProject::from_project_dir_with_default_name(project_root, default_name)?;
    let window_title = project.config.name.clone();
    let graphics = graphics_from_project_config(&project.config, true);
    let app = create_static_app(graphics, project);
    let fps_cap = app
        .runtime
        .project()
        .and_then(|p| p.config.target_fps)
        .unwrap_or(0.0);
    let fixed = app
        .runtime
        .project()
        .and_then(|p| p.config.target_fixed_update);
    WinitRunner::new().run_with_fps_cap_and_timestep(app, &window_title, fps_cap, fixed);
    Ok(())
}

pub struct StaticEmbeddedProject<'a> {
    pub project: StaticEmbeddedProjectInfo<'a>,
    pub graphics: StaticEmbeddedGraphicsConfig,
    pub runtime: StaticEmbeddedRuntimeConfig,
    pub localization: StaticEmbeddedLocalizationConfig,
    pub assets: StaticEmbeddedAssetsConfig,
}

pub struct StaticEmbeddedProjectInfo<'a> {
    pub project_root: &'a Path,
    pub project_name: &'static str,
    pub main_scene: &'static str,
    pub icon: &'static str,
    pub startup_splash: &'static str,
    pub virtual_width: u32,
    pub virtual_height: u32,
}

pub struct StaticEmbeddedGraphicsConfig {
    pub vsync: bool,
    pub msaa: bool,
    pub meshlets: bool,
    pub dev_meshlets: bool,
    pub release_meshlets: bool,
    pub meshlet_debug_view: bool,
    pub occlusion_culling: OcclusionCulling,
    pub particle_sim_default: ParticleSimDefault,
}

pub struct StaticEmbeddedRuntimeConfig {
    pub target_fps: Option<f32>,
    pub target_fixed_update: Option<f32>,
}

pub struct StaticEmbeddedLocalizationConfig {
    pub source_csv: Option<&'static str>,
    pub key_column: &'static str,
    pub default_locale: &'static str,
}

pub struct StaticEmbeddedAssetsConfig {
    pub perro_assets: &'static [u8],
    pub scene_lookup: perro_runtime::StaticSceneLookup,
    pub localization_lookup: perro_runtime::StaticLocalizationLookup,
    pub material_lookup: perro_runtime::StaticMaterialLookup,
    pub terrain_lookup: perro_runtime::StaticTerrainLookup,
    pub particle_lookup: perro_runtime::StaticParticleLookup,
    pub animation_lookup: perro_runtime::StaticAnimationLookup,
    pub mesh_lookup: perro_graphics::StaticMeshLookup,
    pub skeleton_lookup: perro_runtime::StaticSkeletonLookup,
    pub texture_lookup: perro_graphics::StaticTextureLookup,
    pub shader_lookup: perro_graphics::StaticShaderLookup,
    pub audio_lookup: perro_runtime::StaticAudioLookup,
    pub static_script_registry: Option<StaticScriptRegistry>,
}

pub fn run_static_embedded_project(
    input: StaticEmbeddedProject<'_>,
) -> Result<(), ProjectLoadError> {
    let mut static_config = perro_runtime::StaticProjectConfig::new(
        input.project.project_name,
        input.project.main_scene,
        input.project.icon,
        input.project.startup_splash,
        input.project.virtual_width,
        input.project.virtual_height,
    )
    .with_vsync(input.graphics.vsync)
    .with_target_fps(input.runtime.target_fps)
    .with_target_fixed_update(input.runtime.target_fixed_update)
    .with_msaa(input.graphics.msaa)
    .with_meshlets(input.graphics.meshlets)
    .with_dev_meshlets(input.graphics.dev_meshlets)
    .with_release_meshlets(input.graphics.release_meshlets)
    .with_meshlet_debug_view(input.graphics.meshlet_debug_view)
    .with_occlusion_culling(input.graphics.occlusion_culling)
    .with_particle_sim_default(input.graphics.particle_sim_default);
    if let Some(source_csv) = input.localization.source_csv {
        static_config = static_config.with_localization(
            source_csv,
            input.localization.key_column,
            input.localization.default_locale,
        );
    }
    let mut project =
        RuntimeProject::from_static(static_config, input.project.project_root.to_path_buf());

    project = project
        .with_static_scene_lookup(input.assets.scene_lookup)
        .with_static_localization_lookup(input.assets.localization_lookup)
        .with_static_material_lookup(input.assets.material_lookup)
        .with_static_terrain_lookup(input.assets.terrain_lookup)
        .with_static_particle_lookup(input.assets.particle_lookup)
        .with_static_animation_lookup(input.assets.animation_lookup)
        .with_static_skeleton_lookup(input.assets.skeleton_lookup)
        .with_static_audio_lookup(input.assets.audio_lookup)
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
    let fps_cap = app
        .runtime
        .project()
        .and_then(|p| p.config.target_fps)
        .unwrap_or(0.0);
    let fixed = app
        .runtime
        .project()
        .and_then(|p| p.config.target_fixed_update);
    WinitRunner::new().run_with_fps_cap_and_timestep(app, &window_title, fps_cap, fixed);
    Ok(())
}

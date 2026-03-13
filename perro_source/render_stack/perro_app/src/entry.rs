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
    WinitRunner::new().run(app, &window_title);
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
    WinitRunner::new().run(app, &window_title);
    Ok(())
}

pub struct StaticEmbeddedProject<'a> {
    pub project_root: &'a Path,
    pub project_name: &'static str,
    pub main_scene: &'static str,
    pub icon: &'static str,
    pub virtual_width: u32,
    pub virtual_height: u32,
    pub vsync: bool,
    pub msaa: bool,
    pub meshlets: bool,
    pub dev_meshlets: bool,
    pub release_meshlets: bool,
    pub meshlet_debug_view: bool,
    pub occlusion_culling: OcclusionCulling,
    pub particle_sim_default: ParticleSimDefault,
    pub perro_assets: &'static [u8],
    pub scene_lookup: perro_runtime::StaticSceneLookup,
    pub material_lookup: perro_runtime::StaticMaterialLookup,
    pub particle_lookup: perro_runtime::StaticParticleLookup,
    pub mesh_lookup: perro_graphics::StaticMeshLookup,
    pub texture_lookup: perro_graphics::StaticTextureLookup,
    pub static_script_registry: Option<StaticScriptRegistry>,
}

pub fn run_static_embedded_project(
    input: StaticEmbeddedProject<'_>,
) -> Result<(), ProjectLoadError> {
    let static_config = perro_runtime::StaticProjectConfig::new(
        input.project_name,
        input.main_scene,
        input.icon,
        input.virtual_width,
        input.virtual_height,
    )
    .with_vsync(input.vsync)
    .with_msaa(input.msaa)
    .with_meshlets(input.meshlets)
    .with_dev_meshlets(input.dev_meshlets)
    .with_release_meshlets(input.release_meshlets)
    .with_meshlet_debug_view(input.meshlet_debug_view)
    .with_occlusion_culling(input.occlusion_culling)
    .with_particle_sim_default(input.particle_sim_default);
    let mut project = RuntimeProject::from_static(static_config, input.project_root.to_path_buf());

    project = project
        .with_static_scene_lookup(input.scene_lookup)
        .with_static_material_lookup(input.material_lookup)
        .with_static_particle_lookup(input.particle_lookup)
        .with_perro_assets_bytes(input.perro_assets);

    let window_title = project.config.name.clone();
    let graphics = graphics_from_project_config(&project.config, true)
        .with_static_mesh_lookup(input.mesh_lookup)
        .with_static_texture_lookup(input.texture_lookup);
    let runtime = Runtime::from_project_with_script_registry(
        project,
        ProviderMode::Static,
        input.static_script_registry,
    );
    let app = App::new(runtime, graphics);
    WinitRunner::new().run(app, &window_title);
    Ok(())
}

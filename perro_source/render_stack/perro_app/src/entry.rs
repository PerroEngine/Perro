use crate::App;
use crate::winit_runner::WinitRunner;
use perro_graphics::{GraphicsBackend, PerroGraphics};
use perro_runtime::{ProjectLoadError, ProviderMode, Runtime, RuntimeProject};
use perro_scripting::ScriptConstructor;
use std::path::Path;

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

fn graphics_from_project_config(config: &perro_runtime::RuntimeProjectConfig) -> PerroGraphics {
    PerroGraphics::new()
        .with_vsync(config.vsync)
        .with_msaa(config.msaa)
}

pub fn run_dev_project_from_path(
    project_root: &Path,
    default_name: &str,
) -> Result<(), ProjectLoadError> {
    let project = RuntimeProject::from_project_dir_with_default_name(project_root, default_name)?;
    let window_title = project.config.name.clone();
    let graphics = graphics_from_project_config(&project.config);
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
    let graphics = graphics_from_project_config(&project.config);
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
    pub assets_brk: &'static [u8],
    pub scene_lookup: perro_runtime::StaticSceneLookup,
    pub material_lookup: perro_runtime::StaticMaterialLookup,
    pub mesh_lookup: perro_graphics::StaticMeshLookup,
    pub texture_lookup: perro_graphics::StaticTextureLookup,
    pub static_script_registry: Option<
        &'static [(
            &'static str,
            ScriptConstructor<
                Runtime,
                perro_runtime::RuntimeResourceApi,
                perro_runtime::RuntimeInputApi,
            >,
        )],
    >,
}

pub fn run_static_embedded_project(
    input: StaticEmbeddedProject<'_>,
) -> Result<(), ProjectLoadError> {
    let mut project = RuntimeProject::from_static(
        perro_runtime::StaticProjectConfig::new(
            input.project_name,
            input.main_scene,
            input.icon,
            input.virtual_width,
            input.virtual_height,
        ),
        input.project_root.to_path_buf(),
    );

    project = project
        .with_static_scene_lookup(input.scene_lookup)
        .with_static_material_lookup(input.material_lookup)
        .with_brk_bytes(input.assets_brk);

    let window_title = project.config.name.clone();
    let graphics = graphics_from_project_config(&project.config)
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

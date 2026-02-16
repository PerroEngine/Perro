use crate::App;
use crate::winit_runner::WinitRunner;
use perro_graphics::{GraphicsBackend, PerroGraphics};
use perro_runtime::{ProjectLoadError, ProviderMode, Runtime, RuntimeProject};
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

pub fn run_dev_project_from_path(
    project_root: &Path,
    default_name: &str,
) -> Result<(), ProjectLoadError> {
    let project = RuntimeProject::from_project_dir_with_default_name(project_root, default_name)?;
    let window_title = project.config.name.clone();
    let graphics = PerroGraphics::new();
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
    let graphics = PerroGraphics::new();
    let app = create_static_app(graphics, project);
    WinitRunner::new().run(app, &window_title);
    Ok(())
}

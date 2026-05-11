fn static_generation_error(
    kind: &str,
    err: perro_static_pipeline::StaticPipelineError,
) -> CompilerError {
    CompilerError::SceneParse(format!("{kind} static generation failed: {err}"))
}

fn join_static_generation(
    kind: &str,
    handle: thread::ScopedJoinHandle<'_, Result<(), perro_static_pipeline::StaticPipelineError>>,
    first_error: &mut Option<CompilerError>,
) {
    let result = match handle.join() {
        Ok(result) => result.map_err(|err| static_generation_error(kind, err)),
        Err(_) => Err(CompilerError::SceneParse(format!(
            "{kind} static generation panicked"
        ))),
    };
    if let Err(err) = result
        && first_error.is_none()
    {
        *first_error = Some(err);
    }
}

fn generate_project_static_modules(
    project_root: &Path,
    cfg: &perro_project::ProjectConfig,
) -> Result<(), CompilerError> {
    thread::scope(|scope| {
        let tasks = [
            (
                "collision trimesh",
                scope.spawn(|| {
                    perro_static_pipeline::generate_static_collision_trimeshes(project_root)
                }),
            ),
            (
                "scene",
                scope.spawn(|| perro_static_pipeline::generate_static_scenes(project_root)),
            ),
            (
                "material",
                scope.spawn(|| perro_static_pipeline::generate_static_materials(project_root)),
            ),
            (
                "ui style",
                scope.spawn(|| perro_static_pipeline::generate_static_ui_styles(project_root)),
            ),
            (
                "tileset",
                scope.spawn(|| perro_static_pipeline::generate_static_tilesets(project_root)),
            ),
            (
                "particle",
                scope.spawn(|| perro_static_pipeline::generate_static_particles(project_root)),
            ),
            (
                "animation",
                scope.spawn(|| perro_static_pipeline::generate_static_animations(project_root)),
            ),
            (
                "animation tree",
                scope
                    .spawn(|| perro_static_pipeline::generate_static_animation_trees(project_root)),
            ),
            (
                "mesh",
                scope.spawn(|| {
                    perro_static_pipeline::generate_static_meshes(
                        project_root,
                        cfg.meshlets && cfg.release_meshlets,
                    )
                }),
            ),
            (
                "skeleton",
                scope.spawn(|| perro_static_pipeline::generate_static_skeletons(project_root)),
            ),
            (
                "texture",
                scope.spawn(|| perro_static_pipeline::generate_static_textures(project_root)),
            ),
            (
                "shader",
                scope.spawn(|| perro_static_pipeline::generate_static_shaders(project_root)),
            ),
            (
                "audio",
                scope.spawn(|| perro_static_pipeline::generate_static_audios(project_root)),
            ),
            (
                "localization",
                scope.spawn(|| {
                    perro_static_pipeline::generate_static_localizations(project_root, cfg)
                }),
            ),
        ];
        let mut first_error = None;
        for (kind, handle) in tasks {
            join_static_generation(kind, handle, &mut first_error);
        }
        first_error.map_or(Ok(()), Err)
    })
}

fn generate_dlc_static_modules(
    project_root: &Path,
    bake_meshlets: bool,
) -> Result<(), CompilerError> {
    thread::scope(|scope| {
        let tasks = [
            (
                "collision trimesh",
                scope.spawn(|| {
                    perro_static_pipeline::generate_static_collision_trimeshes(project_root)
                }),
            ),
            (
                "scene",
                scope.spawn(|| perro_static_pipeline::generate_static_scenes(project_root)),
            ),
            (
                "material",
                scope.spawn(|| perro_static_pipeline::generate_static_materials(project_root)),
            ),
            (
                "ui style",
                scope.spawn(|| perro_static_pipeline::generate_static_ui_styles(project_root)),
            ),
            (
                "tileset",
                scope.spawn(|| perro_static_pipeline::generate_static_tilesets(project_root)),
            ),
            (
                "particle",
                scope.spawn(|| perro_static_pipeline::generate_static_particles(project_root)),
            ),
            (
                "animation",
                scope.spawn(|| perro_static_pipeline::generate_static_animations(project_root)),
            ),
            (
                "animation tree",
                scope
                    .spawn(|| perro_static_pipeline::generate_static_animation_trees(project_root)),
            ),
            (
                "mesh",
                scope.spawn(|| {
                    perro_static_pipeline::generate_static_meshes(project_root, bake_meshlets)
                }),
            ),
            (
                "skeleton",
                scope.spawn(|| perro_static_pipeline::generate_static_skeletons(project_root)),
            ),
            (
                "texture",
                scope.spawn(|| perro_static_pipeline::generate_static_textures(project_root)),
            ),
            (
                "shader",
                scope.spawn(|| perro_static_pipeline::generate_static_shaders(project_root)),
            ),
            (
                "audio",
                scope.spawn(|| perro_static_pipeline::generate_static_audios(project_root)),
            ),
            (
                "localization",
                scope.spawn(|| perro_static_pipeline::generate_empty_localizations(project_root)),
            ),
        ];
        let mut first_error = None;
        for (kind, handle) in tasks {
            join_static_generation(kind, handle, &mut first_error);
        }
        first_error.map_or(Ok(()), Err)
    })
}

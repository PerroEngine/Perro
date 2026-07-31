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
        macro_rules! spawn_generation {
            ($kind:literal, $call:expr) => {
                (
                    $kind,
                    scope.spawn(|| {
                        let started = Instant::now();
                        eprintln!("  [assets] {}: start", $kind);
                        let result = $call;
                        let status = if result.is_ok() { "done" } else { "fail" };
                        eprintln!(
                            "  [assets] {}: {status} ({:.2?})",
                            $kind,
                            started.elapsed()
                        );
                        result
                    }),
                )
            };
        }
        let tasks = [
            spawn_generation!(
                "collision trimesh",
                perro_static_pipeline::generate_static_collision_trimeshes(project_root)
            ),
            spawn_generation!(
                "scene",
                perro_static_pipeline::generate_static_scenes(project_root)
            ),
            spawn_generation!(
                "material",
                perro_static_pipeline::generate_static_materials(project_root)
            ),
            spawn_generation!(
                "ui style",
                perro_static_pipeline::generate_static_ui_styles(project_root)
            ),
            spawn_generation!(
                "tileset",
                perro_static_pipeline::generate_static_tilesets(project_root)
            ),
            spawn_generation!(
                "particle",
                perro_static_pipeline::generate_static_particles(project_root)
            ),
            spawn_generation!(
                "animation",
                perro_static_pipeline::generate_static_animations(project_root)
            ),
            spawn_generation!(
                "animation tree",
                perro_static_pipeline::generate_static_animation_trees(project_root)
            ),
            spawn_generation!(
                "mesh",
                perro_static_pipeline::generate_static_meshes(
                    project_root,
                    cfg.meshlets && cfg.release_meshlets,
                )
            ),
            spawn_generation!(
                "navmesh",
                perro_static_pipeline::generate_static_navmeshes(project_root)
            ),
            spawn_generation!(
                "skeleton",
                perro_static_pipeline::generate_static_skeletons(project_root)
            ),
            spawn_generation!(
                "texture",
                perro_static_pipeline::generate_static_textures(project_root)
            ),
            spawn_generation!(
                "font",
                perro_static_pipeline::generate_static_fonts(project_root)
            ),
            spawn_generation!(
                "shader",
                perro_static_pipeline::generate_static_shaders(project_root)
            ),
            spawn_generation!(
                "audio",
                perro_static_pipeline::generate_static_audios(project_root)
            ),
            spawn_generation!(
                "csv",
                perro_static_pipeline::generate_static_csvs(project_root)
            ),
            spawn_generation!(
                "localization",
                perro_static_pipeline::generate_static_localizations(project_root, cfg)
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
    macro_rules! generate {
        ($kind:literal, $call:expr) => {
            $call.map_err(|err| static_generation_error($kind, err))?
        };
    }

    // DLC path overrides are thread-local. Keep every generator on this thread
    // so each one writes into the pack crate with the dlc:// asset prefix.
    generate!(
        "collision trimesh",
        perro_static_pipeline::generate_static_collision_trimeshes(project_root)
    );
    generate!(
        "scene",
        perro_static_pipeline::generate_static_scenes(project_root)
    );
    generate!(
        "material",
        perro_static_pipeline::generate_static_materials(project_root)
    );
    generate!(
        "ui style",
        perro_static_pipeline::generate_static_ui_styles(project_root)
    );
    generate!(
        "tileset",
        perro_static_pipeline::generate_static_tilesets(project_root)
    );
    generate!(
        "particle",
        perro_static_pipeline::generate_static_particles(project_root)
    );
    generate!(
        "animation",
        perro_static_pipeline::generate_static_animations(project_root)
    );
    generate!(
        "animation tree",
        perro_static_pipeline::generate_static_animation_trees(project_root)
    );
    generate!(
        "mesh",
        perro_static_pipeline::generate_static_meshes(project_root, bake_meshlets)
    );
    generate!(
        "navmesh",
        perro_static_pipeline::generate_static_navmeshes(project_root)
    );
    generate!(
        "skeleton",
        perro_static_pipeline::generate_static_skeletons(project_root)
    );
    generate!(
        "texture",
        perro_static_pipeline::generate_static_textures(project_root)
    );
    generate!(
        "font",
        perro_static_pipeline::generate_static_fonts(project_root)
    );
    generate!(
        "shader",
        perro_static_pipeline::generate_static_shaders(project_root)
    );
    generate!(
        "audio",
        perro_static_pipeline::generate_static_audios(project_root)
    );
    generate!(
        "csv",
        perro_static_pipeline::generate_static_csvs(project_root)
    );
    generate!(
        "localization",
        perro_static_pipeline::generate_empty_localizations(project_root)
    );
    Ok(())
}

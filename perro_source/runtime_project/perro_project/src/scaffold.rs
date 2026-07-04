pub fn create_new_project(project_root: &Path, project_name: &str) -> Result<(), ProjectError> {
    if project_root.exists() {
        return Err(ProjectError::AlreadyExists(project_root.to_path_buf()));
    }
    ensure_project_layout(project_root)?;
    ensure_project_toml(project_root, project_name)?;
    ensure_project_scaffold(project_root, project_name)?;
    ensure_source_overrides(project_root)?;
    Ok(())
}

pub fn resolve_local_path(input: &str, local_root: &Path) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("local://") {
        let rel = stripped.trim_start_matches('/');
        if rel.is_empty() {
            return local_root.to_path_buf();
        }
        return local_root.join(rel);
    }

    #[cfg(not(target_os = "windows"))]
    if input.starts_with('/') {
        return PathBuf::from(input);
    }

    if input.starts_with('/') {
        let rel = input.trim_start_matches('/');
        if rel.is_empty() {
            return local_root.to_path_buf();
        }
        return local_root.join(rel);
    }

    PathBuf::from(input)
}

pub fn bootstrap_project(
    project_root: &Path,
    default_name: &str,
) -> Result<ProjectConfig, ProjectError> {
    ensure_project_layout(project_root)?;
    ensure_project_toml(project_root, default_name)?;
    let config = load_project_toml(project_root)?;
    ensure_project_scaffold(project_root, &config.name)?;
    ensure_source_overrides(project_root)?;
    Ok(config)
}

pub fn ensure_project_layout(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root)?;
    fs::create_dir_all(root.join("res"))?;
    fs::create_dir_all(root.join(".perro"))?;
    Ok(())
}

pub fn ensure_project_scaffold(root: &Path, project_name: &str) -> std::io::Result<()> {
    let res_dir = root.join("res");
    let res_scripts_dir = res_dir.join("scripts");

    fs::create_dir_all(&res_dir)?;
    fs::create_dir_all(&res_scripts_dir)?;

    write_if_missing(root.join(".gitignore"), &default_gitignore())?;
    write_if_missing(root.join("deps.toml"), &default_deps_toml())?;
    write_if_missing(root.join("input_map.toml"), &default_input_map_toml())?;
    write_if_missing(
        root.join("README.md"),
        &default_project_readme_md(project_name),
    )?;
    write_if_missing(res_dir.join("main.scn"), &default_main_scene())?;
    write_if_missing(
        res_scripts_dir.join("script.rs"),
        &default_script_empty_rs(),
    )?;

    ensure_build_crates_scaffold(root, project_name)
}

/// Creates the generated `.perro` build crates (project, scripts, dev_runner)
/// without touching user-facing files (`res/`, `project.toml`, `deps.toml`, ...).
/// `.perro` is gitignored, so builds from a fresh checkout must be able to
/// recreate it.
pub fn ensure_build_crates_scaffold(root: &Path, project_name: &str) -> std::io::Result<()> {
    let perro_dir = root.join(".perro");
    let project_crate = perro_dir.join("project");
    let scripts_crate = perro_dir.join("scripts");
    let dev_runner_crate = perro_dir.join("dev_runner");
    let project_cargo_config = project_crate.join(".cargo");
    let scripts_cargo_config = scripts_crate.join(".cargo");
    let project_src = project_crate.join("src");
    let project_static_src = project_src.join("static");
    let project_embedded = project_crate.join("embedded");
    let scripts_src = scripts_crate.join("src");
    let dev_runner_src = dev_runner_crate.join("src");

    fs::create_dir_all(&project_src)?;
    fs::create_dir_all(&project_static_src)?;
    fs::create_dir_all(&project_embedded)?;
    fs::create_dir_all(&project_cargo_config)?;
    fs::create_dir_all(&scripts_src)?;
    fs::create_dir_all(&scripts_cargo_config)?;
    fs::create_dir_all(&dev_runner_src)?;

    let crate_name = crate_name_from_project_name(project_name);
    write_if_missing(
        project_crate.join("Cargo.toml"),
        &default_project_crate_toml(&crate_name),
    )?;
    write_if_missing(project_crate.join("build.rs"), &default_project_build_rs())?;
    write_if_missing(
        scripts_crate.join("Cargo.toml"),
        &default_scripts_crate_toml(),
    )?;
    write_if_missing(
        project_cargo_config.join("config.toml"),
        &default_project_cargo_config_toml(),
    )?;
    write_if_missing(
        scripts_cargo_config.join("config.toml"),
        &default_scripts_cargo_config_toml(),
    )?;
    write_if_missing(
        dev_runner_crate.join("Cargo.toml"),
        &default_dev_runner_crate_toml(),
    )?;
    write_if_missing(dev_runner_crate.join("build.rs"), &default_project_build_rs())?;
    write_if_missing(
        project_src.join("main.rs"),
        &default_project_main_rs(project_name),
    )?;
    write_if_missing(project_static_src.join("mod.rs"), &default_static_mod_rs())?;
    write_if_missing(
        project_static_src.join("scenes.rs"),
        &default_static_scenes_rs(),
    )?;
    write_if_missing(
        project_static_src.join("materials.rs"),
        &default_static_materials_rs(),
    )?;
    write_if_missing(
        project_static_src.join("ui_styles.rs"),
        &default_static_ui_styles_rs(),
    )?;
    write_if_missing(
        project_static_src.join("tilesets.rs"),
        &default_static_tilesets_rs(),
    )?;
    write_if_missing(
        project_static_src.join("particles.rs"),
        &default_static_particles_rs(),
    )?;
    write_if_missing(
        project_static_src.join("animations.rs"),
        &default_static_animations_rs(),
    )?;
    write_if_missing(
        project_static_src.join("animation_trees.rs"),
        &default_static_animation_trees_rs(),
    )?;
    write_if_missing(
        project_static_src.join("textures.rs"),
        &default_static_textures_rs(),
    )?;
    write_if_missing(
        project_static_src.join("shaders.rs"),
        &default_static_shaders_rs(),
    )?;
    write_if_missing(
        project_static_src.join("meshes.rs"),
        &default_static_meshes_rs(),
    )?;
    write_if_missing(
        project_static_src.join("collision_trimeshes.rs"),
        &default_static_collision_trimeshes_rs(),
    )?;
    write_if_missing(
        project_static_src.join("skeletons.rs"),
        &default_static_skeletons_rs(),
    )?;
    write_if_missing(
        project_static_src.join("audios.rs"),
        &default_static_audios_rs(),
    )?;
    write_if_missing(
        project_static_src.join("localizations.rs"),
        &default_static_localizations_rs(),
    )?;
    write_if_missing(project_embedded.join("assets.perro"), "")?;
    write_if_missing(scripts_src.join("lib.rs"), &default_scripts_lib_rs())?;
    write_if_missing(
        dev_runner_src.join("main.rs"),
        &default_dev_runner_main_rs(),
    )?;
    Ok(())
}

pub fn ensure_project_toml(root: &Path, default_name: &str) -> std::io::Result<()> {
    let project_toml = root.join("project.toml");
    if project_toml.exists() {
        return Ok(());
    }
    fs::write(project_toml, default_project_toml(default_name))
}

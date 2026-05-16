#[derive(Clone, Copy, Debug)]
pub struct ProjectBuildOptions {
    pub profile: bool,
    pub console: bool,
    pub release: bool,
    pub target: ProjectBuildTarget,
    pub web_output_dir: WebOutputDir,
}

impl ProjectBuildOptions {
    pub fn new(profile: bool, console: bool) -> Self {
        Self {
            profile,
            console,
            release: true,
            target: ProjectBuildTarget::Native,
            web_output_dir: WebOutputDir::Build,
        }
    }

    pub fn with_target(mut self, target: ProjectBuildTarget) -> Self {
        self.target = target;
        self
    }

    pub fn with_release(mut self, release: bool) -> Self {
        self.release = release;
        self
    }

    pub fn with_web_output_dir(mut self, output_dir: WebOutputDir) -> Self {
        self.web_output_dir = output_dir;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectBuildTarget {
    Native,
    Web,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebOutputDir {
    Build,
    Dev,
}

pub fn compile_project_bundle(
    project_root: &Path,
    options: ProjectBuildOptions,
) -> Result<(), CompilerError> {
    ensure_source_overrides(project_root)?;
    let cfg = load_project_toml(project_root)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    reset_embedded_dir(project_root)?;
    let _ = sync_scripts(project_root)?;
    generate_project_static_modules(project_root, &cfg)?;
    perro_static_pipeline::write_static_mod_rs(project_root)
        .map_err(|err| CompilerError::SceneParse(format!("static mod generation failed: {err}")))?;
    generate_embedded_entry_files(project_root)?;
    generate_perro_assets(project_root)?;
    build_project_crate(project_root, options, cfg.steam.enabled)?;
    Ok(())
}

fn generate_perro_assets(project_root: &Path) -> Result<(), CompilerError> {
    let embedded_dir = project_root.join(".perro").join("project").join("embedded");
    fs::create_dir_all(&embedded_dir)?;
    let output = embedded_dir.join("assets.perro");
    let res_dir = project_root.join("res");
    build_perro_assets_archive(&output, &res_dir, project_root, &[])?;
    Ok(())
}

fn reset_embedded_dir(project_root: &Path) -> Result<(), CompilerError> {
    let embedded_dir = project_root.join(".perro").join("project").join("embedded");
    if embedded_dir.exists() {
        fs::remove_dir_all(&embedded_dir)?;
    }
    fs::create_dir_all(&embedded_dir)?;
    Ok(())
}

fn build_project_crate(
    project_root: &Path,
    options: ProjectBuildOptions,
    steam_enabled: bool,
) -> Result<(), CompilerError> {
    let project_crate = project_root.join(".perro").join("project");
    let target_dir = project_root.join("target");
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .env("CARGO_TARGET_DIR", &target_dir)
        .current_dir(&project_crate);
    if options.release {
        cmd.arg("--release");
    }
    if options.target == ProjectBuildTarget::Web {
        cmd.arg("--lib")
            .arg("--target")
            .arg("wasm32-unknown-unknown");
        cmd.env(
            "RUSTFLAGS",
            append_rustflag(
                env::var_os("RUSTFLAGS"),
                "--cfg getrandom_backend=\"wasm_js\"",
            ),
        );
    }
    if options.target == ProjectBuildTarget::Native && !options.console {
        cmd.env(
            "RUSTFLAGS",
            append_rustflag(env::var_os("RUSTFLAGS"), "--cfg perro_no_console"),
        );
    }
    let mut features = Vec::new();
    if options.profile {
        features.push("profile");
    }
    if steam_enabled {
        features.push("steamworks");
    }
    if !features.is_empty() {
        cmd.arg("--features").arg(features.join(","));
    }
    let status = cmd.status()?;

    if !status.success() {
        return Err(CompilerError::CargoFailed(status.code().unwrap_or(-1)));
    }
    match options.target {
        ProjectBuildTarget::Native => export_project_binary(project_root, &target_dir)?,
        ProjectBuildTarget::Web => export_project_web_bundle(project_root, &target_dir, options)?,
    }
    Ok(())
}

fn append_rustflag(existing: Option<std::ffi::OsString>, flag: &str) -> std::ffi::OsString {
    let mut out = existing.unwrap_or_default();
    if !out.is_empty() {
        out.push(" ");
    }
    out.push(flag);
    out
}

fn export_project_binary(project_root: &Path, target_dir: &Path) -> Result<(), CompilerError> {
    let package_bin_name = read_project_binary_name(project_root)?;
    let output_bin_name = read_project_output_binary_name(project_root, &package_bin_name)?;
    let built_bin = target_dir
        .join("release")
        .join(platform_binary_name(&package_bin_name));
    if !built_bin.exists() {
        return Err(CompilerError::SceneParse(format!(
            "project binary not found after build: {}",
            built_bin.display()
        )));
    }

    let output_dir = project_root.join(".output");
    fs::create_dir_all(&output_dir)?;
    let copied_bin = output_dir.join(platform_binary_name(&package_bin_name));
    let output_bin = output_dir.join(platform_binary_name(&output_bin_name));
    fs::copy(&built_bin, &copied_bin)?;
    rename_exported_binary(&copied_bin, &output_bin)?;
    println!("exported project binary: {}", output_bin.display());
    Ok(())
}

fn rename_exported_binary(source: &Path, dest: &Path) -> Result<(), CompilerError> {
    if source == dest {
        return Ok(());
    }

    let source_str = source.to_string_lossy();
    let dest_str = dest.to_string_lossy();
    let case_only_rename =
        cfg!(target_os = "windows") && source_str.eq_ignore_ascii_case(&dest_str);

    if case_only_rename {
        return rename_exported_binary_via_temp(source, dest);
    }

    if dest.exists() {
        fs::remove_file(dest)?;
    }

    match fs::rename(source, dest) {
        Ok(()) => Ok(()),
        Err(err) => Err(CompilerError::Io(err)),
    }
}

fn rename_exported_binary_via_temp(source: &Path, dest: &Path) -> Result<(), CompilerError> {
    let Some(parent) = source.parent() else {
        return Err(CompilerError::SceneParse(format!(
            "failed to rename export: source has no parent: {}",
            source.display()
        )));
    };
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
    let mut tmp = parent.join(if ext.is_empty() {
        "__perro_export_tmp__".to_string()
    } else {
        format!("__perro_export_tmp__.{ext}")
    });
    let mut idx = 0usize;
    while tmp.exists() {
        idx += 1;
        tmp = parent.join(if ext.is_empty() {
            format!("__perro_export_tmp__{idx}")
        } else {
            format!("__perro_export_tmp__{idx}.{ext}")
        });
    }
    fs::rename(source, &tmp)?;
    if dest.exists() {
        fs::remove_file(dest)?;
    }
    fs::rename(tmp, dest)?;
    Ok(())
}

fn platform_binary_name(bin_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{bin_name}.exe")
    } else {
        bin_name.to_string()
    }
}

fn read_project_binary_name(project_root: &Path) -> Result<String, CompilerError> {
    let manifest_path = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let source = fs::read_to_string(&manifest_path)?;
    let mut in_package = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package || !trimmed.starts_with("name") {
            continue;
        }
        let Some((_, raw_value)) = trimmed.split_once('=') else {
            continue;
        };
        let value = raw_value.trim().trim_matches('"');
        if !value.is_empty() {
            return Ok(value.to_string());
        }
    }

    Err(CompilerError::SceneParse(format!(
        "failed to resolve package.name from {}",
        manifest_path.display()
    )))
}

fn read_project_output_binary_name(
    project_root: &Path,
    fallback_name: &str,
) -> Result<String, CompilerError> {
    let config = load_project_toml(project_root)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    let sanitized = sanitize_output_binary_name(&config.name);
    if sanitized.is_empty() {
        Ok(fallback_name.to_string())
    } else {
        Ok(sanitized)
    }
}

fn sanitize_output_binary_name(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.trim().chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid || c.is_control() {
            out.push('_');
        } else {
            out.push(c);
        }
    }

    out.trim_matches([' ', '.']).to_string()
}

fn ensure_project_dependency_line(
    project_root: &Path,
    crate_name: &str,
    dependency_line: &str,
) -> Result<(), CompilerError> {
    let manifest_path = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let mut src = fs::read_to_string(&manifest_path)?;

    // Only treat entries inside [dependencies] as satisfying this check.
    let mut in_dependencies = false;
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_dependencies = trimmed == "[dependencies]";
            continue;
        }
        if !in_dependencies {
            continue;
        }
        if trimmed.starts_with(&format!("{crate_name} "))
            || trimmed.starts_with(&format!("{crate_name}="))
        {
            return Ok(());
        }
    }

    if let Some(idx) = src.find("[dependencies]") {
        let insert_pos = src[idx..]
            .find('\n')
            .map(|off| idx + off + 1)
            .unwrap_or(src.len());
        src.insert_str(insert_pos, &format!("{dependency_line}\n"));
        fs::write(manifest_path, src)?;
    }
    Ok(())
}

fn generate_embedded_entry_files(project_root: &Path) -> Result<(), CompilerError> {
    let cfg = load_project_toml(project_root)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    let project_src = project_root.join(".perro").join("project").join("src");
    fs::create_dir_all(project_src.join("static"))?;
    ensure_project_dependency_line(project_root, "perro_scene", "perro_scene = \"0.1.0\"")?;
    ensure_project_dependency_line(
        project_root,
        "perro_render_bridge",
        "perro_render_bridge = \"0.1.0\"",
    )?;
    ensure_project_dependency_line(project_root, "perro_runtime", "perro_runtime = \"0.1.0\"")?;
    ensure_project_dependency_line(project_root, "perro_ids", "perro_ids = \"0.1.0\"")?;
    ensure_project_dependency_line(project_root, "perro_csv", "perro_csv = \"0.1.0\"")?;
    ensure_project_dependency_line(
        project_root,
        "perro_animation",
        "perro_animation = \"0.1.0\"",
    )?;
    ensure_project_dependency_line(project_root, "perro_structs", "perro_structs = \"0.1.0\"")?;
    perro_project::ensure_source_overrides(project_root)?;

    let embedded_block = format!(
        "let root = project_root();\n\
perro_app::entry::run_static_embedded_project(perro_app::entry::StaticEmbeddedProject {{\n\
  project: perro_app::entry::StaticEmbeddedProjectInfo {{\n\
        project_root: &root,\n\
        project_name: \"{name}\",\n\
        main_scene_hash: {main_scene_hash}u64,\n\
        icon_hash: {icon_hash}u64,\n\
        startup_splash_hash: {startup_splash_hash}u64,\n\
        virtual_width: {w},\n\
        virtual_height: {h},\n\
  }},\n\
  graphics: perro_app::entry::StaticEmbeddedGraphicsConfig {{\n\
        vsync: {vsync},\n\
        msaa: {msaa},\n\
        meshlets: {meshlets},\n\
        dev_meshlets: {dev_meshlets},\n\
        release_meshlets: {release_meshlets},\n\
        meshlet_debug_view: {meshlet_debug_view},\n\
        occlusion_culling: {occlusion_culling},\n\
        particle_sim_default: {particle_sim_default},\n\
  }},\n\
  runtime: perro_app::entry::StaticEmbeddedRuntimeConfig {{\n\
        target_fixed_update: {target_fixed_update},\n\
        physics_gravity: {physics_gravity},\n\
        physics_coef: {physics_coef},\n\
  }},\n\
  metadata: perro_app::entry::StaticEmbeddedMetadataConfig {{\n\
        description: {metadata_description},\n\
        company: {metadata_company},\n\
        version: {metadata_version},\n\
        copyright: {metadata_copyright},\n\
        trademark: {metadata_trademark},\n\
  }},\n\
  localization: perro_app::entry::StaticEmbeddedLocalizationConfig {{\n\
        default_locale: {localization_default_locale},\n\
  }},\n\
  steam: perro_app::entry::StaticEmbeddedSteamConfig {{\n\
        enabled: {steam_enabled},\n\
        app_id: {steam_app_id},\n\
  }},\n\
  assets: perro_app::entry::StaticEmbeddedAssetsConfig {{\n\
        perro_assets: PERRO_ASSETS,\n\
        scene_lookup: static_assets::scenes::lookup_scene,\n\
        localization_lookup: static_assets::localizations::lookup_localized_string,\n\
        material_lookup: static_assets::materials::lookup_material,\n\
        ui_style_lookup: static_assets::ui_styles::lookup_ui_style,\n\
        tileset_lookup: static_assets::tilesets::lookup_tileset,\n\
        particle_lookup: static_assets::particles::lookup_particle,\n\
        animation_lookup: static_assets::animations::lookup_animation,\n\
        animation_tree_lookup: static_assets::animation_trees::lookup_animation_tree,\n\
        csv_lookup: static_assets::csvs::lookup_csv,\n\
        mesh_lookup: static_assets::meshes::lookup_mesh,\n\
        collision_trimesh_lookup: static_assets::collision_trimeshes::lookup_collision_trimesh,\n\
        skeleton_lookup: static_assets::skeletons::lookup_skeleton,\n\
        texture_lookup: static_assets::textures::lookup_texture,\n\
        shader_lookup: static_assets::shaders::lookup_shader,\n\
        audio_lookup: static_assets::audios::lookup_audio,\n\
        static_script_registry: Some(scripts::SCRIPT_REGISTRY),\n\
  }},\n\
}})\n\
.expect(\"failed to run embedded static project\");",
        name = escape_str(&cfg.name),
        main_scene_hash = perro_ids::string_to_u64(&cfg.main_scene),
        icon_hash = perro_ids::string_to_u64(&cfg.icon),
        startup_splash_hash = perro_ids::string_to_u64(&cfg.startup_splash),
        w = cfg.virtual_width,
        h = cfg.virtual_height,
        vsync = cfg.vsync,
        msaa = cfg.msaa,
        meshlets = cfg.meshlets,
        dev_meshlets = cfg.dev_meshlets,
        release_meshlets = cfg.release_meshlets,
        meshlet_debug_view = cfg.meshlet_debug_view,
        occlusion_culling = emit_occlusion_culling_expr(cfg.occlusion_culling),
        particle_sim_default = emit_particle_sim_default_expr(cfg.particle_sim_default),
        target_fixed_update = emit_optional_f32(cfg.target_fixed_update),
        physics_gravity = emit_f32(cfg.physics_gravity),
        physics_coef = emit_f32(cfg.physics_coef),
        metadata_description = emit_optional_static_str(cfg.metadata.description.as_deref()),
        metadata_company = emit_optional_static_str(cfg.metadata.company.as_deref()),
        metadata_version = emit_optional_static_str(cfg.metadata.version.as_deref()),
        metadata_copyright = emit_optional_static_str(cfg.metadata.copyright.as_deref()),
        metadata_trademark = emit_optional_static_str(cfg.metadata.trademark.as_deref()),
        localization_default_locale = emit_static_str(
            cfg.localization
                .as_ref()
                .map(|loc| loc.default_locale.as_str())
                .unwrap_or("en"),
        ),
        steam_enabled = cfg.steam.enabled,
        steam_app_id = emit_optional_u32(cfg.steam.app_id),
    );
    let embedded_block = indent_block(&embedded_block, 2);
    let embedded_web_block = format!(
        "let root = project_root();\n\
perro_app::entry::run_static_embedded_project_web(perro_app::entry::StaticEmbeddedProject {{\n\
  project: perro_app::entry::StaticEmbeddedProjectInfo {{\n\
        project_root: &root,\n\
        project_name: \"{name}\",\n\
        main_scene_hash: {main_scene_hash}u64,\n\
        icon_hash: {icon_hash}u64,\n\
        startup_splash_hash: {startup_splash_hash}u64,\n\
        virtual_width: {w},\n\
        virtual_height: {h},\n\
  }},\n\
  graphics: perro_app::entry::StaticEmbeddedGraphicsConfig {{\n\
        vsync: {vsync},\n\
        msaa: {msaa},\n\
        meshlets: {meshlets},\n\
        dev_meshlets: {dev_meshlets},\n\
        release_meshlets: {release_meshlets},\n\
        meshlet_debug_view: {meshlet_debug_view},\n\
        occlusion_culling: {occlusion_culling},\n\
        particle_sim_default: {particle_sim_default},\n\
  }},\n\
  runtime: perro_app::entry::StaticEmbeddedRuntimeConfig {{\n\
        target_fixed_update: {target_fixed_update},\n\
        physics_gravity: {physics_gravity},\n\
        physics_coef: {physics_coef},\n\
  }},\n\
  metadata: perro_app::entry::StaticEmbeddedMetadataConfig {{\n\
        description: {metadata_description},\n\
        company: {metadata_company},\n\
        version: {metadata_version},\n\
        copyright: {metadata_copyright},\n\
        trademark: {metadata_trademark},\n\
  }},\n\
  localization: perro_app::entry::StaticEmbeddedLocalizationConfig {{\n\
        default_locale: {localization_default_locale},\n\
  }},\n\
  steam: perro_app::entry::StaticEmbeddedSteamConfig {{\n\
        enabled: {steam_enabled},\n\
        app_id: {steam_app_id},\n\
  }},\n\
  assets: perro_app::entry::StaticEmbeddedAssetsConfig {{\n\
        perro_assets: PERRO_ASSETS,\n\
        scene_lookup: static_assets::scenes::lookup_scene,\n\
        localization_lookup: static_assets::localizations::lookup_localized_string,\n\
        material_lookup: static_assets::materials::lookup_material,\n\
        ui_style_lookup: static_assets::ui_styles::lookup_ui_style,\n\
        tileset_lookup: static_assets::tilesets::lookup_tileset,\n\
        particle_lookup: static_assets::particles::lookup_particle,\n\
        animation_lookup: static_assets::animations::lookup_animation,\n\
        animation_tree_lookup: static_assets::animation_trees::lookup_animation_tree,\n\
        csv_lookup: static_assets::csvs::lookup_csv,\n\
        mesh_lookup: static_assets::meshes::lookup_mesh,\n\
        collision_trimesh_lookup: static_assets::collision_trimeshes::lookup_collision_trimesh,\n\
        skeleton_lookup: static_assets::skeletons::lookup_skeleton,\n\
        texture_lookup: static_assets::textures::lookup_texture,\n\
        shader_lookup: static_assets::shaders::lookup_shader,\n\
        audio_lookup: static_assets::audios::lookup_audio,\n\
        static_script_registry: Some(scripts::SCRIPT_REGISTRY),\n\
  }},\n\
}})",
        name = escape_str(&cfg.name),
        main_scene_hash = perro_ids::string_to_u64(&cfg.main_scene),
        icon_hash = perro_ids::string_to_u64(&cfg.icon),
        startup_splash_hash = perro_ids::string_to_u64(&cfg.startup_splash),
        w = cfg.virtual_width,
        h = cfg.virtual_height,
        vsync = cfg.vsync,
        msaa = cfg.msaa,
        meshlets = cfg.meshlets,
        dev_meshlets = cfg.dev_meshlets,
        release_meshlets = cfg.release_meshlets,
        meshlet_debug_view = cfg.meshlet_debug_view,
        occlusion_culling = emit_occlusion_culling_expr(cfg.occlusion_culling),
        particle_sim_default = emit_particle_sim_default_expr(cfg.particle_sim_default),
        target_fixed_update = emit_optional_f32(cfg.target_fixed_update),
        physics_gravity = emit_f32(cfg.physics_gravity),
        physics_coef = emit_f32(cfg.physics_coef),
        metadata_description = emit_optional_static_str(cfg.metadata.description.as_deref()),
        metadata_company = emit_optional_static_str(cfg.metadata.company.as_deref()),
        metadata_version = emit_optional_static_str(cfg.metadata.version.as_deref()),
        metadata_copyright = emit_optional_static_str(cfg.metadata.copyright.as_deref()),
        metadata_trademark = emit_optional_static_str(cfg.metadata.trademark.as_deref()),
        localization_default_locale = emit_static_str(
            cfg.localization
                .as_ref()
                .map(|loc| loc.default_locale.as_str())
                .unwrap_or("en"),
        ),
        steam_enabled = cfg.steam.enabled,
        steam_app_id = emit_optional_u32(cfg.steam.app_id),
    );
    let embedded_web_block = indent_block(&embedded_web_block, 4);

    let lib_src = format!(
        "#![cfg_attr(all(perro_no_console, target_os = \"windows\"), windows_subsystem = \"windows\")]\n\n\
#[path = \"static/mod.rs\"]\n\
mod static_assets;\n\n\
static PERRO_ASSETS: &[u8] = include_bytes!(\"../embedded/assets.perro\");\n\n\
#[used]\n\
#[unsafe(no_mangle)]\n\
pub static PERRO_ENGINE_DETECT: [u8; 89] =\n\
    *b\"PERRO_ENGINE_DETECT:v1;engine=Perro Engine;format=.perro;site=https://www.perroengine.com\";\n\n\
pub fn keep_perro_engine_marker() {{\n\
    unsafe {{\n\
        std::hint::black_box(std::ptr::read_volatile(PERRO_ENGINE_DETECT.as_ptr()));\n\
        std::hint::black_box(std::ptr::read_volatile(\n\
            PERRO_ENGINE_DETECT.as_ptr().add(PERRO_ENGINE_DETECT.len() - 1),\n\
        ));\n\
    }}\n\
}}\n\n\
#[cfg(not(target_arch = \"wasm32\"))]\n\
pub fn project_root() -> std::path::PathBuf {{\n\
    if let Ok(exe) = std::env::current_exe() {{\n\
        if let Some(exe_dir) = exe.parent() {{\n\
            for dir in exe_dir.ancestors() {{\n\
                if dir.join(\"project.toml\").exists() {{\n\
                    return dir.to_path_buf();\n\
                }}\n\
            }}\n\
            return exe_dir.to_path_buf();\n\
        }}\n\
    }}\n\
    let root = std::path::PathBuf::from(env!(\"CARGO_MANIFEST_DIR\")).join(\"..\").join(\"..\");\n\
    if root.join(\"project.toml\").exists() {{\n\
        return root.canonicalize().unwrap_or(root);\n\
    }}\n\
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(\".\"))\n\
}}\n\n\
#[cfg(target_arch = \"wasm32\")]\n\
pub fn project_root() -> std::path::PathBuf {{\n\
    std::path::PathBuf::from(\".\")\n\
}}\n\n\
#[cfg(not(target_arch = \"wasm32\"))]\n\
pub fn run_native() {{\n\
{embedded_block}\n\
}}\n\n\
#[cfg(target_arch = \"wasm32\")]\n\
#[wasm_bindgen::prelude::wasm_bindgen(start)]\n\
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {{\n\
    console_error_panic_hook::set_once();\n\
{embedded_web_block}\n\
}}\n",
        embedded_block = embedded_block,
        embedded_web_block = embedded_web_block,
    );
    let main_src = "fn main() {\n  important_project::keep_perro_engine_marker();\n  important_project::run_native();\n}\n";
    let crate_name = read_project_binary_name(project_root)?;
    let main_src = main_src.replace("important_project", &crate_name);
    fs::write(project_src.join("lib.rs"), lib_src)?;
    fs::write(project_src.join("main.rs"), main_src)?;
    Ok(())
}

fn export_project_web_bundle(
    project_root: &Path,
    target_dir: &Path,
    options: ProjectBuildOptions,
) -> Result<(), CompilerError> {
    let package_name = read_project_binary_name(project_root)?;
    let project_cfg = load_project_toml(project_root)
        .map_err(|err| CompilerError::SceneParse(format!("failed to load project.toml: {err}")))?;
    let profile_dir = if options.release { "release" } else { "debug" };
    let built_wasm = target_dir
        .join("wasm32-unknown-unknown")
        .join(profile_dir)
        .join(format!("{package_name}.wasm"));
    if !built_wasm.exists() {
        return Err(CompilerError::SceneParse(format!(
            "project wasm not found after build: {}",
            built_wasm.display()
        )));
    }

    let output_dir = match options.web_output_dir {
        WebOutputDir::Build => project_root.join(".output").join("web"),
        WebOutputDir::Dev => project_root.join(".output").join("web-dev"),
    };
    if output_dir.exists() {
        fs::remove_dir_all(&output_dir)?;
    }
    fs::create_dir_all(&output_dir)?;

    let bindgen_status = Command::new("wasm-bindgen")
        .arg("--target")
        .arg("web")
        .arg("--no-typescript")
        .arg("--out-dir")
        .arg(&output_dir)
        .arg("--out-name")
        .arg("app")
        .arg(&built_wasm)
        .status()
        .map_err(|err| {
            CompilerError::SceneParse(format!(
                "failed to run wasm-bindgen for {}: {err}. install via `cargo install wasm-bindgen-cli`",
                built_wasm.display()
            ))
        })?;
    if !bindgen_status.success() {
        return Err(CompilerError::SceneParse(format!(
            "wasm-bindgen failed with exit code {:?}",
            bindgen_status.code()
        )));
    }

    fs::write(output_dir.join("boot.js"), web_boot_js())?;
    fs::write(
        output_dir.join("index.html"),
        web_index_html(&project_cfg.name),
    )?;
    println!("exported web bundle: {}", output_dir.display());
    Ok(())
}

fn web_boot_js() -> &'static str {
    "import init from './app.js';\n\
\n\
const boot = document.getElementById('boot');\n\
const setBoot = (text, kind = 'info') => {\n\
  if (!boot) return;\n\
  boot.textContent = text;\n\
  boot.dataset.kind = kind;\n\
};\n\
\n\
const hideBoot = () => {\n\
  if (!boot) return;\n\
  boot.dataset.state = 'done';\n\
  window.setTimeout(() => boot.remove(), 400);\n\
};\n\
\n\
const obs = new MutationObserver(() => {\n\
  if (document.querySelector('canvas')) {\n\
    hideBoot();\n\
    obs.disconnect();\n\
  }\n\
});\n\
obs.observe(document.body, { childList: true, subtree: true });\n\
\n\
setBoot('loading wasm...');\n\
\n\
try {\n\
  await init();\n\
  setBoot('starting render...');\n\
  if (document.querySelector('canvas')) {\n\
    hideBoot();\n\
    obs.disconnect();\n\
  }\n\
} catch (err) {\n\
  console.error('perro web boot fail', err);\n\
  const msg = err instanceof Error ? err.message : String(err);\n\
  setBoot(`boot fail: ${msg}`, 'error');\n\
  obs.disconnect();\n\
}\n"
}

fn web_index_html(project_name: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n<style>\n:root{{color-scheme:dark}}html,body{{margin:0;height:100%;background:#111;color:#eee;font-family:system-ui,sans-serif}}body{{display:flex;flex-direction:column;overflow:hidden}}canvas{{display:block;width:100vw;height:100vh;outline:none}}#boot{{position:fixed;left:12px;top:12px;max-width:min(480px,calc(100vw - 24px));padding:8px 10px;background:rgba(0,0,0,.65);border:1px solid rgba(255,255,255,.12);border-radius:8px;font-size:13px;line-height:1.4;z-index:10;transition:opacity .2s ease}}#boot[data-kind='error']{{color:#ffb4b4;border-color:rgba(255,120,120,.35)}}#boot[data-state='done']{{opacity:0;pointer-events:none}}\n</style>\n</head>\n<body>\n<div id=\"boot\">{boot}</div>\n<script type=\"module\" src=\"./boot.js\"></script>\n</body>\n</html>\n",
        title = escape_html(project_name),
        boot = escape_html(&format!("{project_name} boot")),
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn normalize_generated_include_path(path: &str) -> String {
    let raw = if let Some(rest) = path.strip_prefix("\\\\?\\") {
        rest.to_string()
    } else {
        path.to_string()
    };
    raw.replace('\\', "/")
}

fn relative_include_path(generated_file: &Path, source_file: &Path) -> String {
    let from_dir = generated_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let from_abs = from_dir.canonicalize().unwrap_or(from_dir);
    let to_abs = source_file
        .canonicalize()
        .unwrap_or_else(|_| source_file.to_path_buf());

    let from_components: Vec<_> = from_abs.components().collect();
    let to_components: Vec<_> = to_abs.components().collect();

    let mut common = 0usize;
    let max_common = from_components.len().min(to_components.len());
    while common < max_common && from_components[common] == to_components[common] {
        common += 1;
    }

    if common == 0 {
        return normalize_generated_include_path(&to_abs.to_string_lossy());
    }

    let mut rel = PathBuf::new();
    for _ in common..from_components.len() {
        rel.push("..");
    }
    for comp in &to_components[common..] {
        rel.push(comp.as_os_str());
    }

    normalize_generated_include_path(&rel.to_string_lossy())
}

fn emit_occlusion_culling_expr(mode: perro_project::OcclusionCulling) -> &'static str {
    match mode {
        perro_project::OcclusionCulling::Cpu => "perro_app::entry::OcclusionCulling::Cpu",
        perro_project::OcclusionCulling::Gpu => "perro_app::entry::OcclusionCulling::Gpu",
        perro_project::OcclusionCulling::Off => "perro_app::entry::OcclusionCulling::Off",
    }
}

fn emit_particle_sim_default_expr(mode: perro_project::ParticleSimDefault) -> &'static str {
    match mode {
        perro_project::ParticleSimDefault::Cpu => "perro_app::entry::ParticleSimDefault::Cpu",
        perro_project::ParticleSimDefault::GpuVertex => {
            "perro_app::entry::ParticleSimDefault::GpuVertex"
        }
        perro_project::ParticleSimDefault::GpuCompute => {
            "perro_app::entry::ParticleSimDefault::GpuCompute"
        }
    }
}

fn emit_optional_f32(value: Option<f32>) -> String {
    match value {
        Some(v) if v.is_finite() => format!("Some({}f32)", v),
        _ => "None".to_string(),
    }
}

fn emit_optional_u32(value: Option<u32>) -> String {
    match value {
        Some(v) => format!("Some({v}u32)"),
        None => "None".to_string(),
    }
}

fn emit_f32(value: f32) -> String {
    if value.is_finite() {
        format!("{value}f32")
    } else {
        "0.0f32".to_string()
    }
}

fn emit_optional_static_str(value: Option<&str>) -> String {
    match value {
        Some(v) => format!("Some({})", emit_static_str(v)),
        None => "None".to_string(),
    }
}

fn emit_static_str(value: &str) -> String {
    format!("\"{}\"", escape_str(value))
}

fn indent_block(src: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    src.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{pad}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

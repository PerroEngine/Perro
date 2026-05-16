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
    let routes = perro_project::load_routes_toml(project_root, &cfg)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load routes.toml: {e}")))?;
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
  routes: perro_app::entry::StaticEmbeddedRoutesConfig {{\n\
        routes: {routes_block},\n\
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
        routes_block = emit_static_routes_block(&routes),
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
  routes: perro_app::entry::StaticEmbeddedRoutesConfig {{\n\
        routes: {routes_block},\n\
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
        routes_block = emit_static_routes_block(&routes),
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
    let routes = perro_project::load_routes_toml(project_root, &project_cfg)
        .map_err(|err| CompilerError::SceneParse(format!("failed to load routes.toml: {err}")))?;
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
    emit_web_route_html_files(project_root, &output_dir, &project_cfg, &routes)?;
    println!("exported web bundle: {}", output_dir.display());
    Ok(())
}

fn web_boot_js() -> &'static str {
    "import init from './app.js';\n\
\n\
const boot = document.getElementById('boot');\n\
const staticPage = document.getElementById('perro-static-page');\n\
const shellCache = new Map();\n\
const parser = new DOMParser();\n\
const setBoot = (text, kind = 'info') => {\n\
  if (!boot) return;\n\
  boot.textContent = text;\n\
  boot.dataset.kind = kind;\n\
};\n\
\n\
const appReady = () => document.body.dataset.perroApp === 'ready';\n\
\n\
const splitHref = (href) => {\n\
  const url = new URL(href, window.location.href);\n\
  let path = url.pathname || '/';\n\
  if (path.length > '/index.html'.length && path.endsWith('/index.html')) {\n\
    path = path.slice(0, -'/index.html'.length);\n\
  }\n\
  while (path.length > 1 && path.endsWith('/')) {\n\
    path = path.slice(0, -1);\n\
  }\n\
  if (!path.startsWith('/')) {\n\
    path = `/${path}`;\n\
  }\n\
  return {\n\
    path,\n\
    historyHref: `${path}${url.search}${url.hash}`,\n\
    documentHref: path === '/' ? '/index.html' : `${path}/index.html`,\n\
  };\n\
};\n\
\n\
const syncHead = (doc) => {\n\
  if (doc.title) {\n\
    document.title = doc.title;\n\
  }\n\
  for (const name of ['description', 'keywords']) {\n\
    const next = doc.head.querySelector(`meta[name=\"${name}\"]`);\n\
    const current = document.head.querySelector(`meta[name=\"${name}\"]`);\n\
    if (next && current) {\n\
      current.setAttribute('content', next.getAttribute('content') || '');\n\
    } else if (next && !current) {\n\
      document.head.appendChild(next.cloneNode(true));\n\
    } else if (!next && current) {\n\
      current.remove();\n\
    }\n\
  }\n\
  const nextIcon = doc.head.querySelector('link[rel=\"icon\"]');\n\
  const currentIcon = document.head.querySelector('link[rel=\"icon\"]');\n\
  if (nextIcon && currentIcon) {\n\
    currentIcon.setAttribute('href', nextIcon.getAttribute('href') || '');\n\
  }\n\
};\n\
\n\
const fetchShellDoc = async (href) => {\n\
  const parts = splitHref(href);\n\
  let pending = shellCache.get(parts.path);\n\
  if (!pending) {\n\
    pending = fetch(parts.documentHref, { credentials: 'same-origin' }).then((resp) => {\n\
      if (!resp.ok) {\n\
        throw new Error(`route fetch fail: ${resp.status}`);\n\
      }\n\
      return resp.text();\n\
    });\n\
    shellCache.set(parts.path, pending);\n\
  }\n\
  const text = await pending;\n\
  return { parts, doc: parser.parseFromString(text, 'text/html') };\n\
};\n\
\n\
const applyShellDoc = (doc) => {\n\
  if (!staticPage) return;\n\
  const nextStatic = doc.getElementById('perro-static-page');\n\
  if (!nextStatic) return;\n\
  staticPage.innerHTML = nextStatic.innerHTML;\n\
  syncHead(doc);\n\
};\n\
\n\
const navShell = async (href, pushHistory) => {\n\
  if (appReady()) return;\n\
  const { parts, doc } = await fetchShellDoc(href);\n\
  applyShellDoc(doc);\n\
  if (pushHistory) {\n\
    window.history.pushState(null, '', parts.historyHref);\n\
  }\n\
};\n\
\n\
const hideBoot = () => {\n\
  if (!boot) return;\n\
  boot.dataset.state = 'done';\n\
  document.body.dataset.perroApp = 'ready';\n\
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
document.addEventListener('click', (event) => {\n\
  if (appReady()) return;\n\
  if (event.defaultPrevented || event.button !== 0) return;\n\
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;\n\
  const anchor = event.target instanceof Element\n\
    ? event.target.closest('#perro-static-page a[href]')\n\
    : null;\n\
  if (!(anchor instanceof HTMLAnchorElement)) return;\n\
  if (anchor.target && anchor.target !== '_self') return;\n\
  const url = new URL(anchor.href, window.location.href);\n\
  if (url.origin !== window.location.origin) return;\n\
  event.preventDefault();\n\
  setBoot('loading route...');\n\
  navShell(url.href, true).catch((err) => {\n\
    console.error('perro route shell fail', err);\n\
    window.location.href = url.href;\n\
  });\n\
});\n\
\n\
const prefetchShell = (target) => {\n\
  if (appReady()) return;\n\
  const anchor = target instanceof Element\n\
    ? target.closest('#perro-static-page a[href]')\n\
    : null;\n\
  if (!(anchor instanceof HTMLAnchorElement)) return;\n\
  const url = new URL(anchor.href, window.location.href);\n\
  if (url.origin !== window.location.origin) return;\n\
  fetchShellDoc(url.href).catch(() => {});\n\
};\n\
\n\
document.addEventListener('pointerover', (event) => prefetchShell(event.target), { passive: true });\n\
document.addEventListener('focusin', (event) => prefetchShell(event.target));\n\
window.addEventListener('popstate', () => {\n\
  if (appReady()) return;\n\
  setBoot('loading route...');\n\
  navShell(window.location.href, false).catch((err) => {\n\
    console.error('perro route shell fail', err);\n\
    window.location.reload();\n\
  });\n\
});\n\
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
  document.body.dataset.perroApp = 'boot-fail';\n\
  setBoot(`boot fail: ${msg}`, 'error');\n\
  obs.disconnect();\n\
}\n"
}

struct StaticWebPage {
    title: String,
    description: Option<String>,
    keywords: Vec<String>,
    icon_href: String,
    boot_href: String,
    app_href: String,
    wasm_href: String,
    body_html: String,
    boot_label: String,
}

fn emit_web_route_html_files(
    project_root: &Path,
    output_dir: &Path,
    project_cfg: &perro_project::ProjectConfig,
    routes: &perro_project::ProjectRoutesConfig,
) -> Result<(), CompilerError> {
    let icon_output = copy_res_asset_into_web_output(project_root, output_dir, &project_cfg.icon)?;
    for route in &routes.routes {
        let html_path = web_route_html_path(output_dir, &route.href);
        if let Some(parent) = html_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body_html = render_route_scene_html(project_root, output_dir, &html_path, &route.scene)?;
        let title = route
            .title
            .clone()
            .or_else(|| {
                (route.href == "/")
                    .then(|| project_cfg.web.title.clone())
                    .flatten()
            })
            .unwrap_or_else(|| {
                if route.href == "/" {
                    project_cfg
                        .web
                        .title
                        .clone()
                        .unwrap_or_else(|| project_cfg.name.clone())
                } else {
                    let site_title = project_cfg
                        .web
                        .title
                        .clone()
                        .unwrap_or_else(|| project_cfg.name.clone());
                    format!("{} | {site_title}", route.name)
                }
            });
        let description = route
            .description
            .clone()
            .or_else(|| project_cfg.web.description.clone());
        let keywords = merge_web_keywords(&project_cfg.web.keywords, &route.keywords);
        let page = StaticWebPage {
            title: title.clone(),
            description,
            keywords,
            icon_href: relative_output_href(&html_path, &icon_output),
            boot_href: relative_output_href(&html_path, &output_dir.join("boot.js")),
            app_href: relative_output_href(&html_path, &output_dir.join("app.js")),
            wasm_href: relative_output_href(&html_path, &output_dir.join("app_bg.wasm")),
            body_html,
            boot_label: format!("{title} boot"),
        };
        fs::write(&html_path, web_index_html(&page))?;
    }
    Ok(())
}

fn web_route_html_path(output_dir: &Path, href: &str) -> PathBuf {
    let href = perro_project::normalize_route_href(href);
    if href == "/" {
        return output_dir.join("index.html");
    }
    output_dir
        .join(href.trim_start_matches('/'))
        .join("index.html")
}

fn copy_res_asset_into_web_output(
    project_root: &Path,
    output_dir: &Path,
    res_path: &str,
) -> Result<PathBuf, CompilerError> {
    let rel = res_path.trim().strip_prefix("res://").ok_or_else(|| {
        CompilerError::SceneParse(format!("expected res:// path for web asset, got `{res_path}`"))
    })?;
    let source = project_root.join("res").join(res_rel_to_path(rel));
    if !source.exists() {
        return Err(CompilerError::SceneParse(format!(
            "web asset not found: {}",
            source.display()
        )));
    }
    let target = output_dir.join("assets").join(res_rel_to_path(rel));
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&source, &target)?;
    Ok(target)
}

fn render_route_scene_html(
    project_root: &Path,
    output_dir: &Path,
    html_path: &Path,
    scene_path: &str,
) -> Result<String, CompilerError> {
    let rel = scene_path.trim().strip_prefix("res://").ok_or_else(|| {
        CompilerError::SceneParse(format!("expected res:// scene path, got `{scene_path}`"))
    })?;
    let scene_file = project_root.join("res").join(res_rel_to_path(rel));
    let scene_src = fs::read_to_string(&scene_file)?;
    let scene = std::panic::catch_unwind(|| perro_scene::Parser::new(&scene_src).parse_scene())
        .map_err(|_| {
            CompilerError::SceneParse(format!(
                "failed to parse scene for static web html: {}",
                scene_file.display()
            ))
        })?;
    let mut html = String::new();
    if let Some(root) = scene.root {
        html.push_str(&render_scene_entry_html(
            project_root,
            output_dir,
            html_path,
            &scene,
            root,
        )?);
    } else {
        for entry in scene.nodes.iter().filter(|entry| entry.parent.is_none()) {
            html.push_str(&render_scene_entry_html(
                project_root,
                output_dir,
                html_path,
                &scene,
                entry.key,
            )?);
        }
    }
    if html.trim().is_empty() {
        html.push_str("<main class=\"perro-static-page__content\"></main>");
    }
    Ok(html)
}

fn render_scene_entry_html(
    project_root: &Path,
    output_dir: &Path,
    html_path: &Path,
    scene: &perro_scene::Scene,
    key: perro_scene::SceneKey,
) -> Result<String, CompilerError> {
    let Some(entry) = scene.nodes.get(key.as_usize()) else {
        return Ok(String::new());
    };
    if scene_field_bool(&entry.data, "visible") == Some(false) {
        return Ok(String::new());
    }
    let children_html = render_scene_children_html(project_root, output_dir, html_path, scene, entry)?;
    let name_attr = entry
        .name
        .as_deref()
        .map(escape_html_attr)
        .map(|value| format!(" data-perro-name=\"{value}\""))
        .unwrap_or_default();
    let node_attr = format!(
        " class=\"perro-node perro-node--{}\" data-perro-node=\"{}\"{}",
        escape_html_attr(entry.data.ty.as_ref()),
        escape_html_attr(entry.data.ty.as_ref()),
        name_attr
    );
    match entry.data.ty.as_ref() {
        "UiLabel" => {
            let text = scene_field_str(&entry.data, "text")
                .map(decode_scene_text_literal)
                .map(normalize_static_html_text)
                .unwrap_or_default();
            Ok(format!("<p{node_attr}>{}</p>", escape_html(&text)))
        }
        "UiTextBox" | "UiTextBlock" => {
            let text = scene_field_str(&entry.data, "text")
                .map(decode_scene_text_literal)
                .or_else(|| scene_field_str(&entry.data, "placeholder").map(decode_scene_text_literal))
                .map(normalize_static_html_text)
                .unwrap_or_default();
            Ok(format!("<p{node_attr}>{}</p>", escape_html(&text)))
        }
        "UiButton" => {
            let inner = if children_html.trim().is_empty() {
                let fallback = entry
                    .name
                    .as_deref()
                    .map(str::to_string)
                    .unwrap_or_else(|| "link".to_string());
                escape_html(&fallback)
            } else {
                children_html
            };
            if let Some(href) = extract_button_href(&entry.data) {
                Ok(format!(
                    "<a href=\"{}\"{node_attr}>{inner}</a>",
                    escape_html_attr(&href)
                ))
            } else {
                Ok(format!("<button type=\"button\"{node_attr}>{inner}</button>"))
            }
        }
        "UiImage" | "UiAnimatedImage" => {
            if let Some(texture) = extract_ui_image_source(&entry.data) {
                let copied = copy_res_asset_into_web_output(project_root, output_dir, &texture)?;
                let src = relative_output_href(html_path, &copied);
                let alt = entry.name.as_deref().unwrap_or("");
                Ok(format!(
                    "<img src=\"{}\" alt=\"{}\"{node_attr}>",
                    escape_html_attr(&src),
                    escape_html_attr(alt)
                ))
            } else {
                Ok(children_html)
            }
        }
        ty if is_static_web_container(ty) => {
            let tag = static_web_container_tag(entry);
            Ok(format!("<{tag}{node_attr}>{children_html}</{tag}>"))
        }
        _ => Ok(children_html),
    }
}

fn render_scene_children_html(
    project_root: &Path,
    output_dir: &Path,
    html_path: &Path,
    scene: &perro_scene::Scene,
    entry: &perro_scene::SceneNodeEntry,
) -> Result<String, CompilerError> {
    let mut out = String::new();
    let child_keys: Vec<_> = if entry.children.is_empty() {
        scene.nodes
            .iter()
            .filter(|candidate| candidate.parent == Some(entry.key))
            .map(|candidate| candidate.key)
            .collect()
    } else {
        entry.children.iter().copied().collect()
    };
    for child in child_keys {
        out.push_str(&render_scene_entry_html(
            project_root,
            output_dir,
            html_path,
            scene,
            child,
        )?);
    }
    Ok(out)
}

fn is_static_web_container(ty: &str) -> bool {
    matches!(
        ty,
        "UiBox"
            | "UiPanel"
            | "UiLayout"
            | "UiHLayout"
            | "UiHBox"
            | "UiVLayout"
            | "UiVBox"
            | "UiGrid"
            | "UiScrollContainer"
            | "UiScroll"
            | "UiTreeList"
    )
}

fn static_web_container_tag(entry: &perro_scene::SceneNodeEntry) -> &'static str {
    let name = entry.name.as_deref().unwrap_or("").to_ascii_lowercase();
    if name.contains("nav") {
        "nav"
    } else if name.contains("header") {
        "header"
    } else if name.contains("footer") {
        "footer"
    } else if name.contains("section") || name.contains("hero") {
        "section"
    } else {
        "div"
    }
}

fn scene_field_bool(data: &perro_scene::SceneNodeData, field: &str) -> Option<bool> {
    scene_field_value(data, field)?.as_bool()
}

fn scene_field_str<'a>(data: &'a perro_scene::SceneNodeData, field: &str) -> Option<&'a str> {
    scene_field_value(data, field)?.as_str()
}

fn scene_field_value<'a>(
    data: &'a perro_scene::SceneNodeData,
    field: &str,
) -> Option<&'a perro_scene::SceneValue> {
    let mut found = data.base_ref().and_then(|base| scene_field_value(base, field));
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            found = Some(value);
        }
    }
    found
}

fn extract_button_href(data: &perro_scene::SceneNodeData) -> Option<String> {
    let perro_scene::SceneValue::Object(fields) = scene_field_value(data, "web")? else {
        return None;
    };
    fields.iter().find_map(|(name, value)| {
        (name.as_ref() == "href")
            .then(|| value.as_str().map(perro_project::normalize_route_href))
            .flatten()
    })
}

fn extract_ui_image_source(data: &perro_scene::SceneNodeData) -> Option<String> {
    for field in ["texture", "image", "source", "src"] {
        if let Some(value) = scene_field_str(data, field)
            && value.starts_with("res://")
        {
            return Some(value.to_string());
        }
    }
    None
}

fn decode_scene_text_literal(text: &str) -> String {
    if let Some(stripped) = text.strip_prefix("%%loc:") {
        return decode_text_escapes(&format!("%loc:{stripped}"));
    }
    if let Some(raw) = text.strip_prefix("%loc:") {
        let raw = raw.trim().trim_matches('"').trim();
        return raw.to_string();
    }
    decode_text_escapes(text)
}

fn normalize_static_html_text(text: String) -> String {
    text.replace("\\n", " ")
        .replace("\\r", " ")
        .replace("\\t", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_text_escapes(text: &str) -> String {
    if !text.contains('\\') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn merge_web_keywords(global: &[String], route: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for keyword in global.iter().chain(route.iter()) {
        let trimmed = keyword.trim();
        if trimmed.is_empty() || out.iter().any(|existing: &String| existing == trimmed) {
            continue;
        }
        out.push(trimmed.to_string());
    }
    out
}

fn relative_output_href(from_html: &Path, to: &Path) -> String {
    relative_include_path(from_html, to).replace('\\', "/")
}

fn res_rel_to_path(rel: &str) -> PathBuf {
    rel.split('/')
        .filter(|part| !part.is_empty())
        .collect::<PathBuf>()
}

fn web_index_html(page: &StaticWebPage) -> String {
    let description = page
        .description
        .as_deref()
        .map(|value| format!("<meta name=\"description\" content=\"{}\">\n", escape_html_attr(value)))
        .unwrap_or_default();
    let keywords = if page.keywords.is_empty() {
        String::new()
    } else {
        format!(
            "<meta name=\"keywords\" content=\"{}\">\n",
            escape_html_attr(&page.keywords.join(", "))
        )
    };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n<link rel=\"icon\" href=\"{icon}\">\n<link rel=\"modulepreload\" href=\"{app_href}\">\n<link rel=\"preload\" href=\"{wasm_href}\" as=\"fetch\" type=\"application/wasm\" crossorigin>\n{description}{keywords}<style>\n:root{{color-scheme:dark}}html,body{{margin:0;min-height:100%;background:radial-gradient(circle at top,#182233 0%,#0b0d12 55%,#07090d 100%);color:#dce7f9;font-family:Inter,Segoe UI,system-ui,sans-serif}}body{{display:flex;flex-direction:column}}#perro-static-page{{width:min(1240px,calc(100vw - 32px));margin:0 auto;padding:18px 0 36px}}#perro-static-page *{{box-sizing:border-box}}#perro-static-page a{{color:inherit}}#perro-static-page img{{max-width:100%;height:auto;display:block}}#perro-static-page nav,#perro-static-page header,#perro-static-page section,#perro-static-page footer,#perro-static-page div{{width:100%}}#perro-static-page .perro-node--UiHLayout{{display:flex;gap:18px;align-items:stretch;flex-wrap:wrap}}#perro-static-page .perro-node--UiVLayout,#perro-static-page .perro-node--UiScrollContainer{{display:grid;gap:18px}}#perro-static-page .perro-node--UiPanel,#perro-static-page .perro-node--UiScrollContainer{{padding:18px;border:1px solid #334158;border-radius:18px;background:rgba(16,21,30,.92);box-shadow:0 18px 60px rgba(0,0,0,.28)}}#perro-static-page nav{{display:flex;gap:14px;align-items:center;flex-wrap:wrap;padding:16px 18px;border:1px solid #334158;border-radius:18px;background:rgba(16,21,30,.92);box-shadow:0 18px 60px rgba(0,0,0,.28)}}#perro-static-page nav .perro-node--UiButton{{background:#1a2230;color:#dce7f9;border-color:#4a5f81;box-shadow:none}}#perro-static-page nav .perro-node--UiButton:first-child{{background:transparent;border-color:transparent;color:#fff4d4;padding-inline:10px}}#perro-static-page footer{{padding:18px;border:1px solid #334158;border-radius:18px;background:rgba(16,21,30,.92)}}#perro-static-page .perro-node--UiLabel{{margin:0;line-height:1.45;color:inherit}}#perro-static-page a.perro-node--UiButton,#perro-static-page button.perro-node--UiButton{{display:inline-flex;align-items:center;justify-content:center;gap:8px;min-height:46px;padding:12px 18px;border:1px solid #f7d891;border-radius:14px;background:#e4b85b;color:#201406;font-weight:700;text-decoration:none;transition:transform .18s ease,background .18s ease,border-color .18s ease}}#perro-static-page a.perro-node--UiButton:hover,#perro-static-page button.perro-node--UiButton:hover{{transform:translateY(-1px);background:#f0c96d}}#perro-static-page p[data-perro-name*='title'],#perro-static-page p[data-perro-name*='hero']{{font-size:clamp(1.8rem,4vw,3.4rem);line-height:1.05;color:#fff7e0;font-weight:800;letter-spacing:-.04em}}#perro-static-page p[data-perro-name*='text'],#perro-static-page p[data-perro-name*='copy']{{color:#c8d4e8}}body[data-perro-app='ready'] #perro-static-page{{display:none}}canvas{{display:block;width:100vw;height:100vh;outline:none}}#boot{{position:fixed;left:12px;top:12px;max-width:min(480px,calc(100vw - 24px));padding:8px 10px;background:rgba(0,0,0,.78);border:1px solid rgba(255,255,255,.12);border-radius:8px;font-size:13px;line-height:1.4;z-index:10;transition:opacity .2s ease;opacity:0;pointer-events:none}}#boot[data-kind='error']{{opacity:1;pointer-events:auto;color:#ffb4b4;border-color:rgba(255,120,120,.35)}}#boot[data-state='done']{{opacity:0;pointer-events:none}}@media (max-width: 760px){{#perro-static-page{{width:calc(100vw - 24px);padding:12px 0 28px}}#perro-static-page .perro-node--UiHLayout{{gap:12px}}#perro-static-page nav{{gap:10px;padding:14px}}#perro-static-page .perro-node--UiPanel,#perro-static-page .perro-node--UiScrollContainer,#perro-static-page footer{{padding:14px}}#perro-static-page a.perro-node--UiButton,#perro-static-page button.perro-node--UiButton{{width:100%}}}}\n</style>\n</head>\n<body>\n<main id=\"perro-static-page\">{body}</main>\n<div id=\"boot\">{boot}</div>\n<script type=\"module\" src=\"{boot_href}\"></script>\n</body>\n</html>\n",
        title = escape_html(&page.title),
        icon = escape_html_attr(&page.icon_href),
        app_href = escape_html_attr(&page.app_href),
        wasm_href = escape_html_attr(&page.wasm_href),
        description = description,
        keywords = keywords,
        body = page.body_html,
        boot = escape_html(&page.boot_label),
        boot_href = escape_html_attr(&page.boot_href),
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_html_attr(s: &str) -> String {
    escape_html(s).replace('"', "&quot;").replace('\'', "&#39;")
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

fn emit_static_routes_block(routes: &perro_project::ProjectRoutesConfig) -> String {
    let mut out = String::from("&[");
    for route in &routes.routes {
        out.push_str("\n            perro_app::entry::StaticEmbeddedRoute { ");
        out.push_str(&format!(
            "href: {}, name: {}, scene_hash: {}u64 }},",
            emit_static_str(&route.href),
            emit_static_str(&route.name),
            perro_ids::parse_hashed_source_uri(&route.scene)
                .unwrap_or_else(|| perro_ids::string_to_u64(&route.scene))
        ));
    }
    if !routes.routes.is_empty() {
        out.push_str("\n        ");
    }
    out.push(']');
    out
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

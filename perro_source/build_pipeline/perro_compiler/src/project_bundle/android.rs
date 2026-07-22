use super::*;

pub(super) fn export_project_android_bundle(
    project_root: &Path,
    built_apk: &Path,
) -> Result<(), CompilerError> {
    let output_name =
        read_project_output_binary_name(project_root, &read_project_package_name(project_root)?)?;
    if !built_apk.is_file() {
        return Err(CompilerError::SceneParse(format!(
            "android apk not found after build: {}",
            built_apk.display()
        )));
    }
    if !built_apk
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("apk"))
    {
        return Err(CompilerError::SceneParse(format!(
            "android build artifact is not an apk: {}",
            built_apk.display()
        )));
    }

    let output_dir = project_root.join(".output").join("android");
    fs::create_dir_all(&output_dir)?;
    let output_apk = output_dir.join(format!("{output_name}.apk"));
    fs::copy(built_apk, &output_apk)?;
    println!("exported android apk: {}", output_apk.display());
    Ok(())
}

pub(super) fn rename_exported_binary(source: &Path, dest: &Path) -> Result<(), CompilerError> {
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

pub(super) fn rename_exported_binary_via_temp(source: &Path, dest: &Path) -> Result<(), CompilerError> {
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

pub(super) fn platform_binary_name(bin_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{bin_name}.exe")
    } else {
        bin_name.to_string()
    }
}

pub(super) fn read_project_package_name(project_root: &Path) -> Result<String, CompilerError> {
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

pub(super) fn read_project_library_name(
    project_root: &Path,
    fallback_name: &str,
) -> Result<String, CompilerError> {
    let manifest_path = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let source = fs::read_to_string(&manifest_path)?;
    let mut in_lib = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_lib = trimmed == "[lib]";
            continue;
        }
        if !in_lib || !trimmed.starts_with("name") {
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
    Ok(fallback_name.to_string())
}

pub(super) fn read_project_output_binary_name(
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

pub(super) fn sanitize_output_binary_name(input: &str) -> String {
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

pub(super) fn ensure_project_dependency_line(
    project_root: &Path,
    crate_name: &str,
    dependency_line: &str,
) -> Result<(), CompilerError> {
    let manifest_path = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let mut src = fs::read_to_string(&manifest_path)?;

    let dotted_dependency = format!("[dependencies.{crate_name}]");
    let mut in_dependencies = false;
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed == dotted_dependency {
            return Ok(());
        }
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
        write_string_if_changed(&manifest_path, &src)?;
    } else if let Some(idx) = src.find("[dependencies.") {
        src.insert_str(idx, &format!("[dependencies]\n{dependency_line}\n\n"));
        write_string_if_changed(&manifest_path, &src)?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn generate_embedded_entry_files(project_root: &Path) -> Result<(), CompilerError> {
    generate_embedded_entry_files_with_options(project_root, ProjectBuildOptions::new(false, false))
}

// Shared `assets:` block for the generated entry source. Every field of
// `perro_app::entry::StaticEmbeddedAssetsConfig` must appear here exactly
// once; all three entry targets (native/web/android) splice this same block.
pub(super) const STATIC_EMBEDDED_ASSETS_BLOCK: &str = "  assets: perro_app::entry::StaticEmbeddedAssetsConfig {\n\
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
        navmesh_lookup: static_assets::navmeshes::lookup_navmesh,\n\
        skeleton_lookup: static_assets::skeletons::lookup_skeleton,\n\
        texture_lookup: static_assets::textures::lookup_texture,\n\
        font_lookup: static_assets::fonts::lookup_font,\n\
        shader_lookup: static_assets::shaders::lookup_shader,\n\
        audio_lookup: static_assets::audios::lookup_audio,\n\
        static_script_registry: Some(scripts::SCRIPT_REGISTRY),\n\
  },\n";

pub(super) fn generate_embedded_entry_files_with_options(
    project_root: &Path,
    options: ProjectBuildOptions,
) -> Result<(), CompilerError> {
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
    ensure_project_dependency_line(
        project_root,
        "perro_input_api",
        "perro_input_api = \"0.1.0\"",
    )?;
    ensure_project_dependency_line(project_root, "perro_ids", "perro_ids = \"0.1.0\"")?;
    ensure_project_dependency_line(project_root, "perro_csv", "perro_csv = \"0.1.0\"")?;
    ensure_project_dependency_line(
        project_root,
        "perro_animation",
        "perro_animation = \"0.1.0\"",
    )?;
    ensure_project_dependency_line(project_root, "perro_structs", "perro_structs = \"0.1.0\"")?;
    perro_project::ensure_source_overrides(project_root)?;

    let native_entry = "run_static_embedded_project";
    let mut embedded_block = format!(
        "let root = project_root();\n\
perro_app::entry::{native_entry}(perro_app::entry::StaticEmbeddedProject {{\n\
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
  input: perro_app::entry::StaticEmbeddedInputMapConfig {{\n\
        actions: {input_map_block},\n\
  }},\n\
  graphics: perro_app::entry::StaticEmbeddedGraphicsConfig {{\n\
        vsync: {vsync},\n\
        hdr: {hdr},\n\
        msaa: {msaa},\n\
        ssao: {ssao},\n\
        meshlets: {meshlets},\n\
        dev_meshlets: {dev_meshlets},\n\
        release_meshlets: {release_meshlets},\n\
        meshlet_debug_view: {meshlet_debug_view},\n\
        occlusion_culling: {occlusion_culling},\n\
        particle_sim_default: {particle_sim_default},\n\
        ui_pixel_snapping: {ui_pixel_snapping},\n\
        default_font: \"{default_font}\",\n\
  }},\n\
  runtime: perro_app::entry::StaticEmbeddedRuntimeConfig {{\n\
        target_fixed_update: {target_fixed_update},\n\
        frame_rate_cap: {frame_rate_cap},\n\
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
        input_mode: {steam_input_mode},\n\
  }},\n\
{assets_block}\
}})\n\
.expect(\"failed to run embedded static project\");",
        name = escape_str(&cfg.name),
        main_scene_hash = perro_ids::string_to_u64(&cfg.main_scene),
        icon_hash = perro_ids::string_to_u64(&cfg.icon),
        startup_splash_hash = perro_ids::string_to_u64(&cfg.startup_splash),
        w = cfg.virtual_width,
        h = cfg.virtual_height,
        routes_block = emit_static_routes_block(&routes),
        input_map_block = emit_static_input_map_block(&cfg.input_map),
        assets_block = STATIC_EMBEDDED_ASSETS_BLOCK,
        vsync = cfg.vsync,
        hdr = emit_hdr_expr(cfg.hdr),
        msaa = cfg.msaa,
        ssao = emit_ssao_expr(cfg.ssao),
        meshlets = cfg.meshlets,
        dev_meshlets = cfg.dev_meshlets,
        release_meshlets = cfg.release_meshlets,
        meshlet_debug_view = cfg.meshlet_debug_view,
        occlusion_culling = emit_occlusion_culling_expr(cfg.occlusion_culling),
        particle_sim_default = emit_particle_sim_default_expr(cfg.particle_sim_default),
        ui_pixel_snapping = cfg.rendering.ui.pixel_snapping,
        default_font = escape_str(&cfg.rendering.default_font),
        target_fixed_update = emit_optional_f32(cfg.target_fixed_update),
        frame_rate_cap = emit_frame_rate_cap_expr(cfg.frame_rate_cap),
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
        steam_app_id = emit_optional_steam_app_id_fn(cfg.steam.app_id),
        steam_input_mode = emit_steam_input_mode(cfg.steam.input_mode),
    );
    if options.headless {
        embedded_block = embedded_block.replace("perro_app::entry", "perro_headless");
    }
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
  input: perro_app::entry::StaticEmbeddedInputMapConfig {{\n\
        actions: {input_map_block},\n\
  }},\n\
  graphics: perro_app::entry::StaticEmbeddedGraphicsConfig {{\n\
        vsync: {vsync},\n\
        hdr: {hdr},\n\
        msaa: {msaa},\n\
        ssao: {ssao},\n\
        meshlets: {meshlets},\n\
        dev_meshlets: {dev_meshlets},\n\
        release_meshlets: {release_meshlets},\n\
        meshlet_debug_view: {meshlet_debug_view},\n\
        occlusion_culling: {occlusion_culling},\n\
        particle_sim_default: {particle_sim_default},\n\
        ui_pixel_snapping: {ui_pixel_snapping},\n\
        default_font: \"{default_font}\",\n\
  }},\n\
  runtime: perro_app::entry::StaticEmbeddedRuntimeConfig {{\n\
        target_fixed_update: {target_fixed_update},\n\
        frame_rate_cap: {frame_rate_cap},\n\
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
        input_mode: {steam_input_mode},\n\
  }},\n\
{assets_block}\
}})",
        name = escape_str(&cfg.name),
        main_scene_hash = perro_ids::string_to_u64(&cfg.main_scene),
        icon_hash = perro_ids::string_to_u64(&cfg.icon),
        startup_splash_hash = perro_ids::string_to_u64(&cfg.startup_splash),
        w = cfg.virtual_width,
        h = cfg.virtual_height,
        routes_block = emit_static_routes_block(&routes),
        input_map_block = emit_static_input_map_block(&cfg.input_map),
        assets_block = STATIC_EMBEDDED_ASSETS_BLOCK,
        vsync = cfg.vsync,
        hdr = emit_hdr_expr(cfg.hdr),
        msaa = cfg.msaa,
        ssao = emit_ssao_expr(cfg.ssao),
        meshlets = cfg.meshlets,
        dev_meshlets = cfg.dev_meshlets,
        release_meshlets = cfg.release_meshlets,
        meshlet_debug_view = cfg.meshlet_debug_view,
        occlusion_culling = emit_occlusion_culling_expr(cfg.occlusion_culling),
        particle_sim_default = emit_particle_sim_default_expr(cfg.particle_sim_default),
        ui_pixel_snapping = cfg.rendering.ui.pixel_snapping,
        default_font = escape_str(&cfg.rendering.default_font),
        target_fixed_update = emit_optional_f32(cfg.target_fixed_update),
        frame_rate_cap = emit_frame_rate_cap_expr(cfg.frame_rate_cap),
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
        steam_app_id = emit_optional_steam_app_id_fn(cfg.steam.app_id),
        steam_input_mode = emit_steam_input_mode(cfg.steam.input_mode),
    );
    let embedded_android_block = format!(
        "let root = project_root();\n\
perro_app::entry::run_static_embedded_project_android(app, perro_app::entry::StaticEmbeddedProject {{\n\
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
  input: perro_app::entry::StaticEmbeddedInputMapConfig {{\n\
        actions: {input_map_block},\n\
  }},\n\
  graphics: perro_app::entry::StaticEmbeddedGraphicsConfig {{\n\
        vsync: {vsync},\n\
        hdr: {hdr},\n\
        msaa: {msaa},\n\
        ssao: {ssao},\n\
        meshlets: {meshlets},\n\
        dev_meshlets: {dev_meshlets},\n\
        release_meshlets: {release_meshlets},\n\
        meshlet_debug_view: {meshlet_debug_view},\n\
        occlusion_culling: {occlusion_culling},\n\
        particle_sim_default: {particle_sim_default},\n\
        ui_pixel_snapping: {ui_pixel_snapping},\n\
        default_font: \"{default_font}\",\n\
  }},\n\
  runtime: perro_app::entry::StaticEmbeddedRuntimeConfig {{\n\
        target_fixed_update: {target_fixed_update},\n\
        frame_rate_cap: {frame_rate_cap},\n\
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
        input_mode: {steam_input_mode},\n\
  }},\n\
{assets_block}\
}})\n\
.expect(\"failed to run embedded static project on android\");",
        name = escape_str(&cfg.name),
        main_scene_hash = perro_ids::string_to_u64(&cfg.main_scene),
        icon_hash = perro_ids::string_to_u64(&cfg.icon),
        startup_splash_hash = perro_ids::string_to_u64(&cfg.startup_splash),
        w = cfg.virtual_width,
        h = cfg.virtual_height,
        routes_block = emit_static_routes_block(&routes),
        input_map_block = emit_static_input_map_block(&cfg.input_map),
        assets_block = STATIC_EMBEDDED_ASSETS_BLOCK,
        vsync = cfg.vsync,
        hdr = emit_hdr_expr(cfg.hdr),
        msaa = cfg.msaa,
        ssao = emit_ssao_expr(cfg.ssao),
        meshlets = cfg.meshlets,
        dev_meshlets = cfg.dev_meshlets,
        release_meshlets = cfg.release_meshlets,
        meshlet_debug_view = cfg.meshlet_debug_view,
        occlusion_culling = emit_occlusion_culling_expr(cfg.occlusion_culling),
        particle_sim_default = emit_particle_sim_default_expr(cfg.particle_sim_default),
        ui_pixel_snapping = cfg.rendering.ui.pixel_snapping,
        default_font = escape_str(&cfg.rendering.default_font),
        target_fixed_update = emit_optional_f32(cfg.target_fixed_update),
        frame_rate_cap = emit_frame_rate_cap_expr(cfg.frame_rate_cap),
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
        steam_app_id = emit_optional_steam_app_id_fn(cfg.steam.app_id),
        steam_input_mode = emit_steam_input_mode(cfg.steam.input_mode),
    );
    let embedded_android_block = indent_block(&embedded_android_block, 2);
    let embedded_web_block = indent_block(&embedded_web_block, 4);
    let steam_app_id_fn_block = emit_static_steam_app_id_fn(cfg.steam.app_id, &cfg.name);

    let shared_src = format!(
        "#![allow(dead_code)]\n\n\
#[path = \"static/mod.rs\"]\n\
mod static_assets;\n\n\
pub(super) static PERRO_ASSETS: &[u8] = include_bytes!(\"../embedded/assets.perro\");\n\n\
{steam_app_id_fn_block}\
#[used]\n\
#[unsafe(no_mangle)]\n\
pub static PERRO_ENGINE_DETECT: [u8; 89] =\n\
    *b\"PERRO_ENGINE_DETECT:v1;engine=Perro Engine;format=.perro;site=https://www.perroengine.com\";\n\n\
pub fn keep_perro_engine_marker() {{\n\
    // SAFETY: Reads stay within static marker bounds and use valid static pointers.\n\
    unsafe {{\n\
        std::hint::black_box(std::ptr::read_volatile(PERRO_ENGINE_DETECT.as_ptr()));\n\
        std::hint::black_box(std::ptr::read_volatile(\n\
            PERRO_ENGINE_DETECT.as_ptr().add(PERRO_ENGINE_DETECT.len() - 1),\n\
        ));\n\
    }}\n\
}}\n\n\
#[cfg(any(target_os = \"android\", target_arch = \"wasm32\"))]\n\
pub fn project_root() -> std::path::PathBuf {{\n\
    std::path::PathBuf::from(\".\")\n\
}}\n\n\
#[cfg(all(not(target_os = \"android\"), not(target_arch = \"wasm32\")))]\n\
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
#[cfg(all(not(target_os = \"android\"), not(target_arch = \"wasm32\")))]\n\
pub fn run_native() {{\n\
{embedded_block}\n\
}}\n\n\
#[cfg(target_os = \"android\")]\n\
pub fn run_android(app: perro_app::entry::AndroidApp) {{\n\
{embedded_android_block}\n\
}}\n\n\
#[cfg(target_arch = \"wasm32\")]\n\
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {{\n\
    console_error_panic_hook::set_once();\n\
{embedded_web_block}\n\
}}\n",
        embedded_block = embedded_block,
        embedded_android_block = embedded_android_block,
        embedded_web_block = embedded_web_block,
        steam_app_id_fn_block = steam_app_id_fn_block,
    );
    let lib_src = "#![cfg_attr(all(perro_no_console, target_os = \"windows\"), windows_subsystem = \"windows\")]\n\n#[path = \"entry_shared.rs\"]\nmod entry_shared;\n\npub use entry_shared::*;\n\n#[cfg(target_os = \"android\")]\n#[unsafe(no_mangle)]\npub fn android_main(app: perro_app::entry::AndroidApp) {\n    keep_perro_engine_marker();\n    run_android(app);\n}\n\n#[cfg(target_arch = \"wasm32\")]\n#[wasm_bindgen::prelude::wasm_bindgen(start)]\npub fn run_web_entry() -> Result<(), wasm_bindgen::JsValue> {\n    keep_perro_engine_marker();\n    run_web()\n}\n";
    let main_src = "#![cfg_attr(all(perro_no_console, target_os = \"windows\"), windows_subsystem = \"windows\")]\n\n#[path = \"entry_shared.rs\"]\nmod entry_shared;\n\n#[cfg(all(not(target_os = \"android\"), not(target_arch = \"wasm32\")))]\nfn main() {\n  entry_shared::keep_perro_engine_marker();\n  entry_shared::run_native();\n}\n";
    write_string_if_changed(&project_src.join("entry_shared.rs"), &shared_src)?;
    write_string_if_changed(&project_src.join("lib.rs"), lib_src)?;
    write_string_if_changed(&project_src.join("main.rs"), main_src)?;
    Ok(())
}

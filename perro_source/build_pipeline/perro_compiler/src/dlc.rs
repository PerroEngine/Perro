fn write_dlc_manifest(
    path: &Path,
    dlc_name: &str,
    script_lib_rel: &str,
    pack_lib_rel: &str,
) -> Result<(), CompilerError> {
    let body = format!(
        "name = \"{dlc_name}\"\nversion = \"0.1.0\"\nrequired_perro = \"0.1.0\"\nrequired_project = \"*\"\nscript_lib = \"{script_lib_rel}\"\npack_lib = \"{pack_lib_rel}\"\n"
    );
    write_string_if_changed(path, &body)?;
    Ok(())
}

fn write_dlc_pack_manifest(
    _project_root: &Path,
    crate_name: &str,
    pack_dir: &Path,
) -> Result<(), CompilerError> {
    fs::create_dir_all(pack_dir.join("src"))?;
    let engine_root = engine_root_dir();
    let perro_api_path = normalize_toml_path(
        &engine_root
            .join("perro_source")
            .join("api_modules")
            .join("perro_api"),
    );
    let perro_scene_path = normalize_toml_path(
        &engine_root
            .join("perro_source")
            .join("runtime_project")
            .join("perro_scene"),
    );
    let perro_render_bridge_path = normalize_toml_path(
        &engine_root
            .join("perro_source")
            .join("render_stack")
            .join("perro_render_bridge"),
    );
    let perro_animation_path = normalize_toml_path(
        &engine_root
            .join("perro_source")
            .join("core")
            .join("perro_animation"),
    );
    let perro_structs_path = normalize_toml_path(
        &engine_root
            .join("perro_source")
            .join("core")
            .join("perro_structs"),
    );
    let manifest = format!(
        "[workspace]\n\n[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n\n[dependencies]\nperro_api = {{ path = \"{perro_api_path}\" }}\nperro_scene = {{ path = \"{perro_scene_path}\" }}\nperro_render_bridge = {{ path = \"{perro_render_bridge_path}\" }}\nperro_animation = {{ path = \"{perro_animation_path}\" }}\nperro_structs = {{ path = \"{perro_structs_path}\" }}\n"
    );
    let mut manifest = manifest;
    manifest.push_str(&build_patch_crates_io_block(&engine_root));
    write_string_if_changed(&pack_dir.join("Cargo.toml"), &manifest)?;
    write_dlc_internal_crate_gitignore(pack_dir)?;
    Ok(())
}

fn write_dlc_internal_crate_gitignore(crate_root: &Path) -> Result<(), CompilerError> {
    write_string_if_changed(
        &crate_root.join(".gitignore"),
        &default_dlc_internal_gitignore(),
    )?;
    Ok(())
}

fn default_dlc_internal_gitignore() -> String {
    "target/\nsrc/\nembedded/\nCargo.lock\nscripts.dll\nlibscripts.so\nlibscripts.dylib\npack.dll\nlibpack.so\nlibpack.dylib\n".to_string()
}

fn write_dlc_pack_lib(
    _project_root: &Path,
    _dlc_name: &str,
    _dlc_root: &Path,
    pack_dir: &Path,
) -> Result<(), CompilerError> {
    let mut src = String::new();
    src.push_str("#[path = \"static/mod.rs\"]\n");
    src.push_str("pub mod static_assets;\n\n");
    src.push_str("use perro_animation::AnimationClip;\n");
    src.push_str("use perro_render_bridge::{Material3D, ParticleProfile3D};\n");
    src.push_str("use perro_scene::Scene;\n\n");
    src.push_str("pub struct DlcPackEntry {\n");
    src.push_str("    pub hash: u64,\n");
    src.push_str("    pub path: &'static str,\n");
    src.push_str("    pub data: &'static [u8],\n");
    src.push_str("}\n\n");
    src.push_str("#[repr(C)]\n");
    src.push_str("pub struct PerroDlcStaticLookupApi {\n");
    src.push_str("    pub scene_lookup: extern \"C\" fn(u64) -> *const Scene,\n");
    src.push_str("    pub material_lookup: extern \"C\" fn(u64) -> *const Material3D,\n");
    src.push_str("    pub particle_lookup: extern \"C\" fn(u64) -> *const ParticleProfile3D,\n");
    src.push_str("    pub animation_lookup: extern \"C\" fn(u64) -> *const AnimationClip,\n");
    src.push_str(
        "    pub mesh_lookup: extern \"C\" fn(u64, *mut *const u8, *mut usize) -> bool,\n",
    );
    src.push_str("    pub collision_trimesh_lookup: extern \"C\" fn(u64, *mut *const u8, *mut usize) -> bool,\n");
    src.push_str(
        "    pub skeleton_lookup: extern \"C\" fn(u64, *mut *const u8, *mut usize) -> bool,\n",
    );
    src.push_str(
        "    pub texture_lookup: extern \"C\" fn(u64, *mut *const u8, *mut usize) -> bool,\n",
    );
    src.push_str(
        "    pub shader_lookup: extern \"C\" fn(u64, *mut *const u8, *mut usize) -> bool,\n",
    );
    src.push_str(
        "    pub audio_lookup: extern \"C\" fn(u64, *mut *const u8, *mut usize) -> bool,\n",
    );
    src.push_str("    pub assets_ptr: extern \"C\" fn() -> *const u8,\n");
    src.push_str("    pub assets_len: extern \"C\" fn() -> usize,\n");
    src.push_str("}\n\n");
    src.push_str("pub static PERRO_DLC_STATIC_LOOKUP_API: PerroDlcStaticLookupApi = PerroDlcStaticLookupApi {\n");
    src.push_str("    scene_lookup: perro_dlc_pack_lookup_scene,\n");
    src.push_str("    material_lookup: perro_dlc_pack_lookup_material,\n");
    src.push_str("    particle_lookup: perro_dlc_pack_lookup_particle,\n");
    src.push_str("    animation_lookup: perro_dlc_pack_lookup_animation,\n");
    src.push_str("    mesh_lookup: perro_dlc_pack_lookup_mesh,\n");
    src.push_str("    collision_trimesh_lookup: perro_dlc_pack_lookup_collision_trimesh,\n");
    src.push_str("    skeleton_lookup: perro_dlc_pack_lookup_skeleton,\n");
    src.push_str("    texture_lookup: perro_dlc_pack_lookup_texture,\n");
    src.push_str("    shader_lookup: perro_dlc_pack_lookup_shader,\n");
    src.push_str("    audio_lookup: perro_dlc_pack_lookup_audio,\n");
    src.push_str("    assets_ptr: perro_dlc_pack_assets_ptr,\n");
    src.push_str("    assets_len: perro_dlc_pack_assets_len,\n");
    src.push_str("};\n\n");
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_static_lookup_api() -> *const PerroDlcStaticLookupApi {\n    &PERRO_DLC_STATIC_LOOKUP_API\n}\n\n",
    );
    src.push_str(
        "static DLC_PACK_ASSETS_PERRO: &[u8] = include_bytes!(\"../embedded/assets.perro\");\n\n",
    );
    src.push_str(
        "fn write_bytes_out(bytes: &'static [u8], data_out: *mut *const u8, len_out: *mut usize) -> bool {\n",
    );
    src.push_str(
        "    if data_out.is_null() || len_out.is_null() {\n        return false;\n    }\n",
    );
    src.push_str("    unsafe {\n        *data_out = bytes.as_ptr();\n        *len_out = bytes.len();\n    }\n");
    src.push_str("    true\n}\n\n");
    src.push_str(
        "fn write_str_out(text: &'static str, data_out: *mut *const u8, len_out: *mut usize) -> bool {\n",
    );
    src.push_str("    write_bytes_out(text.as_bytes(), data_out, len_out)\n}\n\n");
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_assets_ptr() -> *const u8 {\n    DLC_PACK_ASSETS_PERRO.as_ptr()\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_assets_len() -> usize {\n    DLC_PACK_ASSETS_PERRO.len()\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_lookup_scene(path_hash: u64) -> *const Scene {\n    static_assets::scenes::lookup_scene(path_hash) as *const Scene\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_lookup_material(path_hash: u64) -> *const Material3D {\n    static_assets::materials::lookup_material(path_hash) as *const Material3D\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_lookup_particle(path_hash: u64) -> *const ParticleProfile3D {\n    static_assets::particles::lookup_particle(path_hash) as *const ParticleProfile3D\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_lookup_animation(path_hash: u64) -> *const AnimationClip {\n    static_assets::animations::lookup_animation(path_hash) as *const AnimationClip\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_lookup_mesh(path_hash: u64, data_out: *mut *const u8, len_out: *mut usize) -> bool {\n    write_bytes_out(static_assets::meshes::lookup_mesh(path_hash), data_out, len_out)\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_lookup_collision_trimesh(path_hash: u64, data_out: *mut *const u8, len_out: *mut usize) -> bool {\n    write_bytes_out(static_assets::collision_trimeshes::lookup_collision_trimesh(path_hash), data_out, len_out)\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_lookup_skeleton(path_hash: u64, data_out: *mut *const u8, len_out: *mut usize) -> bool {\n    write_bytes_out(static_assets::skeletons::lookup_skeleton(path_hash), data_out, len_out)\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_lookup_texture(path_hash: u64, data_out: *mut *const u8, len_out: *mut usize) -> bool {\n    write_bytes_out(static_assets::textures::lookup_texture(path_hash), data_out, len_out)\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_lookup_audio(path_hash: u64, data_out: *mut *const u8, len_out: *mut usize) -> bool {\n    write_bytes_out(static_assets::audios::lookup_audio(path_hash), data_out, len_out)\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_lookup_shader(path_hash: u64, data_out: *mut *const u8, len_out: *mut usize) -> bool {\n    write_str_out(static_assets::shaders::lookup_shader(path_hash), data_out, len_out)\n}\n\n",
    );
    src.push_str("pub fn perro_dlc_pack_lookup_typed(path_hash: u64) -> Option<&'static [u8]> {\n");
    src.push_str("    let bytes = static_assets::textures::lookup_texture(path_hash);\n    if !bytes.is_empty() {\n        return Some(bytes);\n    }\n");
    src.push_str("    let bytes = static_assets::meshes::lookup_mesh(path_hash);\n    if !bytes.is_empty() {\n        return Some(bytes);\n    }\n");
    src.push_str("    let bytes = static_assets::collision_trimeshes::lookup_collision_trimesh(path_hash);\n    if !bytes.is_empty() {\n        return Some(bytes);\n    }\n");
    src.push_str("    let bytes = static_assets::skeletons::lookup_skeleton(path_hash);\n    if !bytes.is_empty() {\n        return Some(bytes);\n    }\n");
    src.push_str("    let bytes = static_assets::audios::lookup_audio(path_hash);\n    if !bytes.is_empty() {\n        return Some(bytes);\n    }\n");
    src.push_str("    let shader = static_assets::shaders::lookup_shader(path_hash);\n    if !shader.is_empty() {\n        return Some(shader.as_bytes());\n    }\n");
    src.push_str("    None\n}\n\n");
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_lookup(path_hash: u64, data_out: *mut *const u8, len_out: *mut usize) -> bool {\n",
    );
    src.push_str(
        "    if data_out.is_null() || len_out.is_null() {\n        return false;\n    }\n",
    );
    src.push_str(
        "    let Some(bytes) = perro_dlc_pack_lookup_typed(path_hash) else {\n        return false;\n    };\n    unsafe {\n        *data_out = bytes.as_ptr();\n        *len_out = bytes.len();\n    }\n    true\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_has(path_hash: u64) -> bool {\n    perro_dlc_pack_lookup_typed(path_hash).is_some()\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_registry_len() -> usize {\n    0\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_registry_get(index: usize, path_hash_out: *mut u64, path_out: *mut *const u8, path_len_out: *mut usize, data_out: *mut *const u8, data_len_out: *mut usize) -> bool {\n",
    );
    src.push_str(
        "    if path_hash_out.is_null() || path_out.is_null() || path_len_out.is_null() || data_out.is_null() || data_len_out.is_null() {\n        return false;\n    }\n",
    );
    src.push_str("    let _ = index;\n    false\n}\n");

    write_string_if_changed(&pack_dir.join("src").join("lib.rs"), &src)?;
    Ok(())
}

fn resolve_compiled_dylib(
    project_root: &Path,
    dylib_name: &str,
    dylib_prefix: &str,
) -> Result<PathBuf, CompilerError> {
    let profiles = ["release", "debug"];
    let mut scanned = Vec::<String>::new();
    let mut candidates = Vec::<(std::time::SystemTime, PathBuf)>::new();
    for profile in profiles {
        let profile_dir = project_root.join("target").join(profile);
        let primary = profile_dir.join(dylib_name);
        scanned.push(primary.display().to_string());
        if primary.exists()
            && let Ok(meta) = fs::metadata(&primary)
            && let Ok(modified) = meta.modified()
        {
            candidates.push((modified, primary.clone()));
        }

        let deps_dir = profile_dir.join("deps");
        scanned.push(deps_dir.display().to_string());
        if deps_dir.exists() {
            for entry in fs::read_dir(&deps_dir)? {
                let entry = entry?;
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                    continue;
                };
                if name.starts_with(dylib_prefix)
                    && name.ends_with(scripts_dylib_suffix())
                    && let Ok(meta) = fs::metadata(&path)
                    && let Ok(modified) = meta.modified()
                {
                    candidates.push((modified, path));
                }
            }
        }
    }
    if let Some((_, path)) = candidates.into_iter().max_by_key(|(modified, _)| *modified) {
        return Ok(path);
    }
    Err(CompilerError::SceneParse(format!(
        "scripts dylib not found. scanned: {}",
        scanned.join(", ")
    )))
}

fn default_scripts_lib_rs() -> String {
    r#"use perro_runtime::RuntimeScriptApi;
use perro_api::scripting::ScriptConstructor;

pub static SCRIPT_REGISTRY: &[(u64, ScriptConstructor<RuntimeScriptApi>)] = &[];

#[unsafe(no_mangle)]
pub extern "C" fn perro_scripts_init() {}
"#
    .to_string()
}

#[cfg(target_os = "windows")]
fn scripts_dylib_name() -> &'static str {
    "scripts.dll"
}

#[cfg(target_os = "linux")]
fn scripts_dylib_name() -> &'static str {
    "libscripts.so"
}

#[cfg(target_os = "macos")]
fn scripts_dylib_name() -> &'static str {
    "libscripts.dylib"
}

#[cfg(target_os = "windows")]
fn runtime_pack_dylib_name() -> &'static str {
    "pack.dll"
}

#[cfg(target_os = "linux")]
fn runtime_pack_dylib_name() -> &'static str {
    "libpack.so"
}

#[cfg(target_os = "macos")]
fn runtime_pack_dylib_name() -> &'static str {
    "libpack.dylib"
}

#[cfg(target_os = "windows")]
fn dylib_name_for_crate(crate_name: &str) -> String {
    format!("{crate_name}.dll")
}

#[cfg(target_os = "linux")]
fn dylib_name_for_crate(crate_name: &str) -> String {
    format!("lib{crate_name}.so")
}

#[cfg(target_os = "macos")]
fn dylib_name_for_crate(crate_name: &str) -> String {
    format!("lib{crate_name}.dylib")
}

#[cfg(target_os = "windows")]
fn dylib_prefix_for_crate(crate_name: &str) -> String {
    format!("{crate_name}-")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn dylib_prefix_for_crate(crate_name: &str) -> String {
    format!("lib{crate_name}-")
}

#[cfg(target_os = "windows")]
fn scripts_dylib_suffix() -> &'static str {
    ".dll"
}

#[cfg(target_os = "linux")]
fn scripts_dylib_suffix() -> &'static str {
    ".so"
}

#[cfg(target_os = "macos")]
fn scripts_dylib_suffix() -> &'static str {
    ".dylib"
}

fn generate_dlc_static_assets(
    project_root: &Path,
    dlc_name: &str,
    dlc_root: &Path,
    pack_dir: &Path,
) -> Result<(), CompilerError> {
    let static_root = pack_dir.join("src").join("static");
    let embedded_root = pack_dir.join("embedded");
    if static_root.exists() {
        fs::remove_dir_all(&static_root)?;
    }
    if embedded_root.exists() {
        fs::remove_dir_all(&embedded_root)?;
    }
    fs::create_dir_all(&static_root)?;
    fs::create_dir_all(&embedded_root)?;

    let overrides = perro_static_pipeline::StaticPipelineOverrides {
        res_dir: dlc_root.to_path_buf(),
        static_dir: static_root.clone(),
        embedded_dir: embedded_root.clone(),
        asset_prefix: format!("dlc://{dlc_name}/"),
    };
    perro_static_pipeline::set_static_pipeline_overrides(Some(overrides));
    let bake_result = (|| -> Result<(), CompilerError> {
        let cfg = load_project_toml(project_root)
            .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
        generate_dlc_static_modules(project_root, cfg.meshlets)?;
        perro_static_pipeline::write_static_mod_rs(project_root)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
        build_perro_assets_archive(
            &embedded_root.join("assets.perro"),
            dlc_root,
            project_root,
            &[],
        )?;
        Ok(())
    })();
    perro_static_pipeline::set_static_pipeline_overrides(None);
    bake_result
}

pub fn compile_dlc_bundle(project_root: &Path, dlc_name: &str) -> Result<PathBuf, CompilerError> {
    validate_dlc_name(dlc_name)?;
    ensure_source_overrides(project_root)?;
    let dlc_root = project_root.join("dlcs").join(dlc_name);
    if !dlc_root.exists() {
        return Err(CompilerError::SceneParse(format!(
            "dlc source not found: {}",
            dlc_root.display()
        )));
    }

    let generated_root = project_root.join(".perro").join("dlc").join(dlc_name);
    let scripts_crate = generated_root.join("scripts");
    let scripts_src = scripts_crate.join("src");
    let pack_dir = generated_root.join("pack");
    fs::create_dir_all(&scripts_src)?;
    fs::create_dir_all(&pack_dir)?;

    let crate_slug = sanitize_crate_slug(dlc_name);
    let crate_name = format!("scripts_{crate_slug}");
    write_dlc_scripts_manifest(project_root, &crate_name, &scripts_crate)?;
    write_string_if_changed(&scripts_src.join("lib.rs"), &default_scripts_lib_rs())?;

    let _ = sync_dlc_scripts(project_root, dlc_name)?;
    compile_dlc_package_crate(project_root, &scripts_crate)?;
    let output_dylib_name = scripts_dylib_name();
    let dylib = resolve_compiled_dylib(
        project_root,
        &dylib_name_for_crate(&crate_name),
        &dylib_prefix_for_crate(&crate_name),
    )?;
    let staged_dylib = scripts_crate.join(output_dylib_name);
    fs::copy(&dylib, &staged_dylib)?;

    let pack_crate_name = format!("pack_{}", sanitize_crate_slug(dlc_name));
    write_dlc_pack_manifest(project_root, &pack_crate_name, &pack_dir)?;
    generate_dlc_static_assets(project_root, dlc_name, &dlc_root, &pack_dir)?;
    write_dlc_pack_lib(project_root, dlc_name, &dlc_root, &pack_dir)?;
    compile_dlc_package_crate(project_root, &pack_dir)?;
    let pack_dylib_name = runtime_pack_dylib_name();
    let built_pack_dylib = resolve_compiled_dylib(
        project_root,
        &dylib_name_for_crate(&pack_crate_name),
        &dylib_prefix_for_crate(&pack_crate_name),
    )?;
    let staged_pack_dylib = pack_dir.join(pack_dylib_name);
    fs::copy(&built_pack_dylib, &staged_pack_dylib)?;

    let package_root = project_root.join(".output").join("dlc");
    fs::create_dir_all(&package_root)?;
    let staging = package_root.join(format!("{dlc_name}.dlc.staging"));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(staging.join("scripts"))?;
    fs::create_dir_all(staging.join("pack"))?;
    fs::copy(
        &staged_dylib,
        staging.join("scripts").join(output_dylib_name),
    )?;
    fs::copy(
        &staged_pack_dylib,
        staging.join("pack").join(pack_dylib_name),
    )?;
    write_dlc_manifest(
        &staging.join("manifest.toml"),
        dlc_name,
        &format!("scripts/{output_dylib_name}"),
        &format!("pack/{pack_dylib_name}"),
    )?;

    let mut archive_entries = Vec::<(String, PathBuf)>::new();
    archive_entries.push(("manifest.toml".to_string(), staging.join("manifest.toml")));
    archive_entries.push((
        format!("scripts/{output_dylib_name}"),
        staging.join("scripts").join(output_dylib_name),
    ));
    archive_entries.push((
        format!("pack/{pack_dylib_name}"),
        staging.join("pack").join(pack_dylib_name),
    ));

    let mut rel_files = Vec::<String>::new();
    walk_dir(&dlc_root, &mut |path| {
        if path.is_dir() {
            return Ok(());
        }
        let rel = path.strip_prefix(&dlc_root).unwrap();
        let rel_norm = rel.to_string_lossy().replace('\\', "/");
        if rel_norm.ends_with(".rs") {
            return Ok(());
        }
        rel_files.push(rel_norm);
        Ok(())
    })?;
    rel_files.sort();
    for rel in rel_files {
        archive_entries.push((format!("res/{rel}"), dlc_root.join(rel.replace('/', "\\"))));
    }

    let package_file = package_root.join(format!("{dlc_name}.dlc"));
    if package_file.exists() {
        fs::remove_file(&package_file)?;
    }
    build_compressed_perro_archive_from_entries(&package_file, &archive_entries)?;
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    Ok(package_file)
}

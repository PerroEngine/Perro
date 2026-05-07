use perro_assets::{build_compressed_perro_archive_from_entries, build_perro_assets_archive};
use perro_io::walkdir::walk_dir;
use perro_project::{ensure_source_overrides, load_project_toml};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt::{Display, Formatter},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug)]
pub enum CompilerError {
    Io(std::io::Error),
    CargoFailed(i32),
    SceneParse(String),
}

impl Display for CompilerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::CargoFailed(code) => write!(f, "cargo build failed with exit code {code}"),
            Self::SceneParse(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CompilerError {}

impl From<std::io::Error> for CompilerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptsBuildProfile {
    Debug,
    Release,
}

fn validate_dlc_name(dlc_name: &str) -> Result<(), CompilerError> {
    if dlc_name.eq_ignore_ascii_case("self") {
        return Err(CompilerError::SceneParse(
            "dlc name `self` is reserved".to_string(),
        ));
    }
    Ok(())
}

pub fn sync_scripts(project_root: &Path) -> Result<Vec<String>, CompilerError> {
    let res_dir = project_root.join("res");
    let scripts_src = project_root.join(".perro").join("scripts").join("src");
    sync_scripts_from_source(&res_dir, &scripts_src, "res://")
}

pub fn sync_dlc_scripts(project_root: &Path, dlc_name: &str) -> Result<Vec<String>, CompilerError> {
    validate_dlc_name(dlc_name)?;
    let dlc_root = project_root.join("dlcs").join(dlc_name);
    let scripts_src = project_root
        .join(".perro")
        .join("dlc")
        .join(dlc_name)
        .join("scripts")
        .join("src");
    let prefix = format!("dlc://{dlc_name}/");
    sync_scripts_from_source(&dlc_root, &scripts_src, &prefix)
}

fn sync_scripts_from_source(
    source_dir: &Path,
    scripts_src: &Path,
    script_path_prefix: &str,
) -> Result<Vec<String>, CompilerError> {
    fs::create_dir_all(scripts_src)?;

    let mut copied = Vec::<String>::new();
    let mut registrable = Vec::<String>::new();
    let mut generated_rel_paths = HashSet::<String>::new();
    if source_dir.exists() {
        walk_dir(source_dir, &mut |path| {
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                return Ok(());
            }
            let rel = path.strip_prefix(source_dir).unwrap();
            let rel_norm = rel.to_string_lossy().replace('\\', "/");
            let generated_rel = generated_script_rel(&rel_norm);
            let dst = scripts_src.join(&generated_rel);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            let source = fs::read_to_string(path)?;
            let source_include = relative_include_path(&dst, path);
            let transformed = transpile_frontend_script(&source, &source_include);
            if transpiled_exports_script_ctor(&transformed) {
                registrable.push(rel_norm.clone());
            }
            write_string_if_changed(&dst, &transformed)?;
            generated_rel_paths.insert(generated_rel.clone());
            copied.push(rel_norm);
            Ok(())
        })?;
    }

    copied.sort();
    registrable.sort();
    let _ = remove_stale_generated_scripts(scripts_src, &generated_rel_paths)?;
    let _ = write_scripts_lib(scripts_src, &copied, &registrable, script_path_prefix)?;
    Ok(copied)
}

pub fn compile_scripts(project_root: &Path) -> Result<Vec<String>, CompilerError> {
    compile_scripts_with_profile(project_root, ScriptsBuildProfile::Release)
}

pub fn compile_scripts_with_profile(
    project_root: &Path,
    profile: ScriptsBuildProfile,
) -> Result<Vec<String>, CompilerError> {
    ensure_source_overrides(project_root)?;
    let copied = sync_scripts(project_root)?;
    let scripts_crate = project_root.join(".perro").join("scripts");
    let target_dir = project_root.join("target");

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(scripts_crate);
    if profile == ScriptsBuildProfile::Release {
        cmd.arg("--release");
        apply_fast_release_dylib_profile(&mut cmd);
    }
    let status = cmd.status()?;

    if !status.success() {
        return Err(CompilerError::CargoFailed(status.code().unwrap_or(-1)));
    }
    compile_all_dlc_scripts_with_profile(project_root, profile)?;

    Ok(copied)
}

fn compile_all_dlc_scripts_with_profile(
    project_root: &Path,
    profile: ScriptsBuildProfile,
) -> Result<(), CompilerError> {
    let dlcs_root = project_root.join("dlcs");
    if !dlcs_root.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(&dlcs_root)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(dlc_name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        let crate_slug = sanitize_crate_slug(dlc_name);
        let crate_name = format!("scripts_{crate_slug}");
        let scripts_crate = project_root
            .join(".perro")
            .join("dlc")
            .join(dlc_name)
            .join("scripts");
        let scripts_src = scripts_crate.join("src");
        fs::create_dir_all(&scripts_src)?;
        write_dlc_scripts_manifest(project_root, &crate_name, &scripts_crate)?;
        write_string_if_changed(&scripts_src.join("lib.rs"), &default_scripts_lib_rs())?;
        let _ = sync_dlc_scripts(project_root, dlc_name)?;
        compile_scripts_crate(project_root, &scripts_crate, profile)?;
        let dylib = resolve_compiled_dylib(
            project_root,
            &dylib_name_for_crate(&crate_name),
            &dylib_prefix_for_crate(&crate_name),
        )?;
        fs::copy(dylib, scripts_crate.join(scripts_dylib_name()))?;
    }
    Ok(())
}

fn compile_scripts_crate(
    project_root: &Path,
    scripts_crate: &Path,
    profile: ScriptsBuildProfile,
) -> Result<(), CompilerError> {
    let target_dir = project_root.join("target");
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(scripts_crate);
    if profile == ScriptsBuildProfile::Release {
        cmd.arg("--release");
        apply_fast_release_dylib_profile(&mut cmd);
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(CompilerError::CargoFailed(status.code().unwrap_or(-1)));
    }
    Ok(())
}

fn compile_dlc_package_crate(
    project_root: &Path,
    scripts_crate: &Path,
) -> Result<(), CompilerError> {
    let target_dir = project_root.join("target");
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(scripts_crate);
    apply_dlc_release_dylib_profile(&mut cmd);
    let status = cmd.status()?;
    if !status.success() {
        return Err(CompilerError::CargoFailed(status.code().unwrap_or(-1)));
    }
    Ok(())
}

fn apply_fast_release_dylib_profile(cmd: &mut Command) {
    cmd.env("CARGO_PROFILE_RELEASE_INCREMENTAL", "true")
        .env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "64");
}

fn apply_dlc_release_dylib_profile(cmd: &mut Command) {
    cmd.env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "3")
        .env("CARGO_PROFILE_RELEASE_LTO", "fat")
        .env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "1")
        .env("CARGO_PROFILE_RELEASE_PANIC", "abort")
        .env("CARGO_PROFILE_RELEASE_INCREMENTAL", "false");
}

fn write_dlc_scripts_manifest(
    project_root: &Path,
    crate_name: &str,
    scripts_crate: &Path,
) -> Result<(), CompilerError> {
    fs::create_dir_all(scripts_crate.join("src"))?;
    let engine_root = engine_root_dir();
    let perro_api_path = normalize_toml_path(
        &engine_root
            .join("perro_source")
            .join("api_modules")
            .join("perro_api"),
    );
    let perro_runtime_path = normalize_toml_path(
        &engine_root
            .join("perro_source")
            .join("runtime_project")
            .join("perro_runtime"),
    );
    let mut manifest = format!(
        "[workspace]\n\n[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n\n[dependencies]\nperro_api = {{ path = \"{perro_api_path}\" }}\nperro_runtime = {{ path = \"{perro_runtime_path}\" }}\n"
    );
    let extra_deps = read_extra_script_deps(project_root)?;
    if !extra_deps.is_empty() {
        for line in extra_deps {
            manifest.push_str(&line);
            manifest.push('\n');
        }
    }
    manifest.push_str(&build_patch_crates_io_block(&engine_root));
    let manifest_path = scripts_crate.join("Cargo.toml");
    write_string_if_changed(&manifest_path, &manifest)?;
    Ok(())
}

fn engine_root_dir() -> PathBuf {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..");
    raw.canonicalize().unwrap_or(raw)
}

fn normalize_toml_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = raw.strip_prefix("\\\\?\\").unwrap_or(raw.as_ref());
    stripped.replace('\\', "/")
}

fn build_patch_crates_io_block(engine_root: &Path) -> String {
    const CRATES: &[(&str, &str)] = &[
        ("perro_animation", "perro_source/core/perro_animation"),
        ("perro_nodes", "perro_source/core/perro_nodes"),
        ("perro_ui", "perro_source/core/perro_ui"),
        ("perro_structs", "perro_source/core/perro_structs"),
        ("perro_ids", "perro_source/core/perro_ids"),
        ("perro_variant", "perro_source/core/perro_variant"),
        (
            "perro_particle_math",
            "perro_source/core/perro_particle_math",
        ),
        (
            "perro_runtime",
            "perro_source/runtime_project/perro_runtime",
        ),
        (
            "perro_internal_updates",
            "perro_source/runtime_project/perro_internal_updates",
        ),
        ("perro_scene", "perro_source/runtime_project/perro_scene"),
        (
            "perro_runtime_context",
            "perro_source/api_modules/perro_runtime_context",
        ),
        (
            "perro_resource_context",
            "perro_source/api_modules/perro_resource_context",
        ),
        ("perro_api", "perro_source/api_modules/perro_api"),
        ("perro_modules", "perro_source/api_modules/perro_modules"),
        ("perro_input", "perro_source/api_modules/perro_input"),
        (
            "perro_render_bridge",
            "perro_source/render_stack/perro_render_bridge",
        ),
        ("perro_graphics", "perro_source/render_stack/perro_graphics"),
        ("perro_meshlets", "perro_source/render_stack/perro_meshlets"),
        ("perro_app", "perro_source/render_stack/perro_app"),
        (
            "perro_scripting",
            "perro_source/script_stack/perro_scripting",
        ),
        (
            "perro_scripting_macros",
            "perro_source/script_stack/perro_scripting_macros",
        ),
        (
            "perro_compiler",
            "perro_source/build_pipeline/perro_compiler",
        ),
        (
            "perro_static_pipeline",
            "perro_source/build_pipeline/perro_static_pipeline",
        ),
        ("perro_io", "perro_source/io_stack/perro_io"),
        ("perro_assets", "perro_source/io_stack/perro_assets"),
        ("perro_bark", "perro_source/audio_stack/perro_bark"),
        (
            "perro_project",
            "perro_source/runtime_project/perro_project",
        ),
        ("perro_cli", "perro_source/devtools/perro_cli"),
        ("perro_dev_runner", "perro_source/devtools/perro_dev_runner"),
    ];
    let mut out = String::new();
    out.push_str("\n[patch.crates-io]\n");
    for (name, rel) in CRATES {
        let full = normalize_toml_path(&engine_root.join(rel));
        out.push_str(&format!("{name} = {{ path = \"{full}\" }}\n"));
    }
    out
}

fn read_extra_script_deps(project_root: &Path) -> Result<Vec<String>, CompilerError> {
    let deps_toml = project_root.join("deps.toml");
    if !deps_toml.exists() {
        return Ok(Vec::new());
    }
    let src = fs::read_to_string(&deps_toml)?;
    let parsed = src.parse::<toml::Value>().map_err(|err| {
        CompilerError::SceneParse(format!("failed to parse {}: {err}", deps_toml.display()))
    })?;
    let Some(table) = parsed.get("dependencies").and_then(toml::Value::as_table) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (name, spec) in table {
        if name == "perro_api" || name == "perro_runtime" {
            continue;
        }
        if let Some(v) = spec.as_str() {
            out.push(format!("{name} = \"{v}\""));
        } else {
            out.push(format!("{name} = {}", spec));
        }
    }
    Ok(out)
}

fn sanitize_crate_slug(input: &str) -> String {
    let mut out = String::new();
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "dlc".to_string()
    } else {
        trimmed.to_string()
    }
}

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
    src.push_str(
        "pub fn perro_dlc_pack_lookup_typed(_path_hash: u64) -> Option<&'static [u8]> {\n    None\n}\n\n",
    );
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
    r#"use perro_runtime::{Runtime, RuntimeInputApi, RuntimeResourceApi};
use perro_api::scripting::{ScriptAPI, ScriptConstructor};

pub struct RuntimeScriptApi;
impl ScriptAPI for RuntimeScriptApi {
    type RT = Runtime;
    type RS = RuntimeResourceApi;
    type IP = RuntimeInputApi;
}
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
        perro_static_pipeline::generate_static_collision_trimeshes(project_root)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
        perro_static_pipeline::generate_static_scenes(project_root)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
        perro_static_pipeline::generate_static_materials(project_root)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
        perro_static_pipeline::generate_static_particles(project_root)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
        perro_static_pipeline::generate_static_animations(project_root)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
        perro_static_pipeline::generate_static_meshes(project_root, cfg.meshlets)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
        perro_static_pipeline::generate_static_skeletons(project_root)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
        perro_static_pipeline::generate_static_textures(project_root)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
        perro_static_pipeline::generate_static_shaders(project_root)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
        perro_static_pipeline::generate_static_audios(project_root)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
        perro_static_pipeline::generate_empty_localizations(project_root)
            .map_err(|e| CompilerError::SceneParse(e.to_string()))?;
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

pub fn compile_project_bundle(project_root: &Path, profile: bool) -> Result<(), CompilerError> {
    ensure_source_overrides(project_root)?;
    let cfg = load_project_toml(project_root)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    reset_embedded_dir(project_root)?;
    let _ = sync_scripts(project_root)?;
    perro_static_pipeline::generate_static_collision_trimeshes(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("collision trimesh static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_scenes(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("scene static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_materials(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("material static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_particles(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("particle static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_animations(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("animation static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_meshes(
        project_root,
        cfg.meshlets && cfg.release_meshlets,
    )
    .map_err(|err| CompilerError::SceneParse(format!("mesh static generation failed: {err}")))?;
    perro_static_pipeline::generate_static_skeletons(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("skeleton static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_textures(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("texture static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_shaders(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("shader static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_audios(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("audio static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_localizations(project_root, &cfg).map_err(|err| {
        CompilerError::SceneParse(format!("localization static generation failed: {err}"))
    })?;
    perro_static_pipeline::write_static_mod_rs(project_root)
        .map_err(|err| CompilerError::SceneParse(format!("static mod generation failed: {err}")))?;
    generate_embedded_main(project_root)?;
    generate_perro_assets(project_root, &cfg)?;
    build_project_crate(project_root, profile)?;
    Ok(())
}

fn generate_perro_assets(
    project_root: &Path,
    cfg: &perro_project::ProjectConfig,
) -> Result<(), CompilerError> {
    let embedded_dir = project_root.join(".perro").join("project").join("embedded");
    fs::create_dir_all(&embedded_dir)?;
    let output = embedded_dir.join("assets.perro");
    let res_dir = project_root.join("res");
    let mut skip_rel_paths = Vec::<String>::new();
    if let Some(localization) = cfg.localization.as_ref()
        && let Some(rel) = localization.source_csv.strip_prefix("res://")
    {
        skip_rel_paths.push(rel.replace('\\', "/"));
    }
    build_perro_assets_archive(&output, &res_dir, project_root, &skip_rel_paths)?;
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

fn build_project_crate(project_root: &Path, profile: bool) -> Result<(), CompilerError> {
    let project_crate = project_root.join(".perro").join("project");
    let target_dir = project_root.join("target");
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .env("CARGO_TARGET_DIR", &target_dir)
        .current_dir(project_crate);
    if profile {
        cmd.arg("--features").arg("profile");
    }
    let status = cmd.status()?;

    if !status.success() {
        return Err(CompilerError::CargoFailed(status.code().unwrap_or(-1)));
    }
    export_project_binary(project_root, &target_dir)?;
    Ok(())
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

fn generate_embedded_main(project_root: &Path) -> Result<(), CompilerError> {
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
    ensure_project_dependency_line(
        project_root,
        "perro_animation",
        "perro_animation = \"0.1.0\"",
    )?;
    ensure_project_dependency_line(project_root, "perro_structs", "perro_structs = \"0.1.0\"")?;

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
  localization: perro_app::entry::StaticEmbeddedLocalizationConfig {{\n\
        source_csv_hash: {localization_source_csv_hash},\n\
        key_column: {localization_key_column},\n\
        default_locale: {localization_default_locale},\n\
  }},\n\
  assets: perro_app::entry::StaticEmbeddedAssetsConfig {{\n\
        perro_assets: PERRO_ASSETS,\n\
        scene_lookup: static_assets::scenes::lookup_scene,\n\
        localization_lookup: static_assets::localizations::lookup_localized_string,\n\
        material_lookup: static_assets::materials::lookup_material,\n\
        particle_lookup: static_assets::particles::lookup_particle,\n\
        animation_lookup: static_assets::animations::lookup_animation,\n\
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
        localization_source_csv_hash = emit_optional_u64(
            cfg.localization
                .as_ref()
                .map(|loc| perro_ids::string_to_u64(&loc.source_csv)),
        ),
        localization_key_column = emit_static_str(
            cfg.localization
                .as_ref()
                .map(|loc| loc.key_column.as_str())
                .unwrap_or("key"),
        ),
        localization_default_locale = emit_static_str(
            cfg.localization
                .as_ref()
                .map(|loc| loc.default_locale.as_str())
                .unwrap_or("en"),
        ),
    );
    let embedded_block = indent_block(&embedded_block, 2);

    let main_src = format!(
        "#[path = \"static/mod.rs\"]\n\
  mod static_assets;\n\n\
static PERRO_ASSETS: &[u8] = include_bytes!(\"../embedded/assets.perro\");\n\n\
fn project_root() -> std::path::PathBuf {{\n\
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
  fn main() {{\n\
{embedded_block}\n\
  }}\n",
        embedded_block = embedded_block,
    );
    fs::write(project_src.join("main.rs"), main_src)?;
    Ok(())
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

fn emit_f32(value: f32) -> String {
    if value.is_finite() {
        format!("{value}f32")
    } else {
        "0.0f32".to_string()
    }
}

fn emit_optional_u64(value: Option<u64>) -> String {
    match value {
        Some(v) => format!("Some({v}u64)"),
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

fn write_scripts_lib(
    scripts_src: &Path,
    copied: &[String],
    registrable: &[String],
    script_path_prefix: &str,
) -> Result<bool, CompilerError> {
    let mut out = String::new();
    out.push_str("#![allow(unused_imports, unused_variables, dead_code)]\n");
    out.push_str("// AUTO-GENERATED by Perro Compiler. Do not edit by hand.\n\n");
    out.push_str("use perro_runtime::{Runtime, RuntimeInputApi, RuntimeResourceApi};\n");
    out.push_str("use perro_api::scripting::{ScriptAPI, ScriptConstructor};\n\n");

    for rel in copied {
        let module = module_name_from_rel(rel);
        let generated_rel = generated_script_rel(rel);
        out.push_str(&format!("#[path = \"{generated_rel}\"]\n"));
        out.push_str(&format!("pub mod {module};\n\n"));
    }

    if !copied.is_empty() {
        let mut short_name_counts = BTreeMap::<String, usize>::new();
        for rel in copied {
            let short = module_short_name_from_rel(rel);
            *short_name_counts.entry(short).or_insert(0) += 1;
        }

        for rel in copied {
            let module = module_name_from_rel(rel);
            let short = module_short_name_from_rel(rel);
            if short == module {
                continue;
            }
            if short_name_counts.get(&short).copied() == Some(1) {
                out.push_str(&format!("pub use {module} as {short};\n"));
            }
        }
        out.push('\n');

        out.push_str("pub mod script_modules {\n");
        for rel in copied {
            let module = module_name_from_rel(rel);
            out.push_str(&format!("    pub use super::{module};\n"));
            out.push_str(&format!("    pub use super::{module}::*;\n"));
        }
        for rel in copied {
            let module = module_name_from_rel(rel);
            let short = module_short_name_from_rel(rel);
            if short == module {
                continue;
            }
            if short_name_counts.get(&short).copied() == Some(1) {
                out.push_str(&format!("    pub use super::{module} as {short};\n"));
            }
        }
        out.push_str("}\n\n");
    }

    let mut script_entries = Vec::<(String, u64)>::with_capacity(registrable.len());
    let mut hashes = HashMap::<u64, String>::new();
    for rel in registrable {
        let module = module_name_from_rel(rel);
        let path = format!("{script_path_prefix}{rel}");
        let hash = perro_ids::string_to_u64(&path);
        if let Some(prev) = hashes.insert(hash, path.clone()) {
            return Err(CompilerError::SceneParse(format!(
                "script hash collision: `{prev}` + `{path}` => {hash}"
            )));
        }
        script_entries.push((module, hash));
    }

    out.push_str(
        "pub struct RuntimeScriptApi;\n\
impl ScriptAPI for RuntimeScriptApi {\n\
    type RT = Runtime;\n\
    type RS = RuntimeResourceApi;\n\
    type IP = RuntimeInputApi;\n\
}\n\
pub static SCRIPT_REGISTRY: &[(u64, ScriptConstructor<RuntimeScriptApi>)] = &[\n",
    );
    for (module, hash) in &script_entries {
        out.push_str(&format!(
            "    ({hash}u64, {module}::perro_create_script as ScriptConstructor<RuntimeScriptApi>),\n"
        ));
    }
    out.push_str("];\n");
    out.push_str(
        "\n#[unsafe(no_mangle)]\n\
/// # Safety\n\
/// Caller pass valid UTF-8 ptr+len pairs for `root_ptr`/`root_len` + `name_ptr`/`name_len`.\n\
/// Pointers stay valid for full call + null ptr disallow.\n\
pub unsafe extern \"C\" fn perro_scripts_set_project_root(\n\
    root_ptr: *const u8,\n\
    root_len: usize,\n\
    name_ptr: *const u8,\n\
    name_len: usize,\n\
) -> bool {\n\
    if root_ptr.is_null() || name_ptr.is_null() {\n\
        return false;\n\
    }\n\
    let root_bytes = unsafe { std::slice::from_raw_parts(root_ptr, root_len) };\n\
    let name_bytes = unsafe { std::slice::from_raw_parts(name_ptr, name_len) };\n\
    let Ok(root) = std::str::from_utf8(root_bytes) else {\n\
        return false;\n\
    };\n\
    let Ok(name) = std::str::from_utf8(name_bytes) else {\n\
        return false;\n\
    };\n\
    perro_api::modules::file::set_project_root_disk(root, name);\n\
    true\n\
}\n",
    );
    out.push_str(
        "\n#[unsafe(no_mangle)]\n\
pub extern \"C\" fn perro_script_registry_len() -> usize {\n\
    SCRIPT_REGISTRY.len()\n\
}\n",
    );
    out.push_str(
        "\n#[allow(improper_ctypes_definitions)]\n\
#[unsafe(no_mangle)]\n\
/// # Safety\n\
/// Caller pass non-null writable ptrs for `path_hash_out` + `ctor_out`.\n\
/// Output ptrs stay valid for full call.\n\
pub unsafe extern \"C\" fn perro_script_registry_get(\n\
    index: usize,\n\
    path_hash_out: *mut u64,\n\
    ctor_out: *mut ScriptConstructor<RuntimeScriptApi>,\n\
) -> bool {\n\
    if path_hash_out.is_null() || ctor_out.is_null() {\n\
        return false;\n\
    }\n\
    let Some((path_hash, ctor)) = SCRIPT_REGISTRY.get(index) else {\n\
        return false;\n\
    };\n\
    unsafe {\n\
        *path_hash_out = *path_hash;\n\
        *ctor_out = *ctor;\n\
    }\n\
    true\n\
}\n",
    );

    write_string_if_changed(&scripts_src.join("lib.rs"), &out).map_err(CompilerError::Io)
}

fn write_string_if_changed(path: &Path, content: &str) -> std::io::Result<bool> {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == content
    {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(true)
}

fn remove_stale_generated_scripts(
    scripts_src: &Path,
    generated_rel_paths: &HashSet<String>,
) -> Result<bool, CompilerError> {
    if !scripts_src.exists() {
        return Ok(false);
    }

    let mut stale = Vec::<PathBuf>::new();
    walk_dir(scripts_src, &mut |path| {
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            return Ok(());
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            return Ok(());
        };
        if !name.ends_with(".gen.rs") {
            return Ok(());
        }
        let Ok(rel) = path.strip_prefix(scripts_src) else {
            return Ok(());
        };
        let rel_norm = rel.to_string_lossy().replace('\\', "/");
        if !generated_rel_paths.contains(&rel_norm) {
            stale.push(path.to_path_buf());
        }
        Ok(())
    })?;

    let stale_len = stale.len();
    for path in stale {
        fs::remove_file(path)?;
    }

    Ok(stale_len > 0)
}

fn transpile_frontend_script(source: &str, source_include: &str) -> String {
    let debug_methods = methods_debug_enabled();
    let source = ensure_script_allows(source);
    let source_include = escape_str(&normalize_generated_include_path(source_include));
    if source.contains("impl ScriptBehavior") {
        return format!("include!(\"{source_include}\");\n");
    }
    let stripped_source = strip_transpiler_attributes(&source);

    let state_ty = match parse_marked_struct_name(&source, "@State")
        .or_else(|| parse_attributed_struct_name(&source, "state"))
    {
        Some(v) => v,
        None => return format!("include!(\"{source_include}\");\n"),
    };

    let script_ty = parse_marked_struct_name(&source, "@Script")
        .or_else(|| parse_attributed_struct_name(&source, "script"))
        .or_else(|| parse_named_struct(&stripped_source, "Script"))
        .unwrap_or_else(|| "Script".to_string());
    let script_ctor_expr = if is_unit_struct(&stripped_source, &script_ty) {
        script_ty.clone()
    } else {
        format!("<{script_ty} as Default>::default()")
    };

    let has_init = has_nonempty_lifecycle_method(&source, "on_init");
    let has_start = has_nonempty_lifecycle_method(&source, "on_all_init");
    let has_update = has_nonempty_lifecycle_method(&source, "on_update");
    let has_fixed = has_nonempty_lifecycle_method(&source, "on_fixed_update");
    let has_removal = has_nonempty_lifecycle_method(&source, "on_removal");
    let user_methods = parse_inherent_methods(&source, &script_ty);
    if debug_methods {
        let method_names = user_methods
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "[perro][methods] source={} script_ty={} methods_found={} [{}]",
            source_include,
            script_ty,
            user_methods.len(),
            method_names
        );
        if user_methods.is_empty() && source.contains("methods!(") {
            eprintln!(
                "[perro][methods][warn] methods! macro exists but zero methods were parsed for source={}",
                source_include
            );
        }
    }
    let state_fields = parse_struct_fields(&source, &state_ty);
    let exposed_fields = supported_fields(&state_fields);
    let attributed_fields = supported_attributed_fields(&state_fields);

    let mut flags = String::from("ScriptFlags::NONE");
    if has_init {
        flags.push_str(" | ScriptFlags::HAS_INIT");
    }
    if has_start {
        flags.push_str(" | ScriptFlags::HAS_ALL_INIT");
    }
    if has_update {
        flags.push_str(" | ScriptFlags::HAS_UPDATE");
    }
    if has_fixed {
        flags.push_str(" | ScriptFlags::HAS_FIXED_UPDATE");
    }
    if has_removal {
        flags.push_str(" | ScriptFlags::HAS_REMOVAL");
    }

    let member_consts = generate_member_consts(&exposed_fields, &user_methods);
    let get_var_body = generate_get_var_body(&state_ty, &exposed_fields);
    let set_var_match_fn = generate_set_var_match_fn(&state_ty, &exposed_fields);
    let set_var_body = generate_set_var_body(&state_ty, &exposed_fields);
    let apply_scene_injected_vars_body =
        generate_apply_scene_injected_vars_body(&state_ty, &exposed_fields);
    let call_method_body = generate_call_method_body(&user_methods);
    let attr_of_body = generate_attributes_of_body(&attributed_fields, &user_methods);
    let members_with_body = generate_members_with_body(&attributed_fields, &user_methods);
    let has_attr_body = generate_has_attribute_body(&attributed_fields, &user_methods);

    format!(
        r#"include!("{source_include}");

// ---- AUTO-GENERATED by Perro Compiler ----
{member_consts}
{set_var_match_fn}

impl<API: ScriptAPI + ?Sized> ScriptBehavior<API> for {script_ty} {{
    fn script_flags(&self) -> ScriptFlags {{
        ScriptFlags::new({flags})
    }}

    fn create_state(&self) -> Box<dyn std::any::Any> {{
        Box::new(<{state_ty} as Default>::default())
    }}

    fn get_var(&self, state: &dyn std::any::Any, var: ScriptMemberID) -> Variant {{
{get_var_body}
    }}

    fn set_var(&self, state: &mut dyn std::any::Any, var: ScriptMemberID, value: &Variant) {{
{set_var_body}
    }}

    fn apply_scene_injected_vars(&self, state: &mut dyn std::any::Any, vars: &[(ScriptMemberID, Variant)]) {{
{apply_scene_injected_vars_body}
    }}

    fn call_method(
        &self,
        method: ScriptMemberID,
        ctx: &mut ScriptContext<'_, API>,
        params: &[Variant],
    ) -> Variant {{
{call_method_body}
    }}

    fn attributes_of(
        &self,
        member: &str,
    ) -> &'static [Attribute] {{
{attr_of_body}
    }}

    fn members_with(
        &self,
        attribute: &str,
    ) -> &'static [Member] {{
{members_with_body}
    }}

    fn has_attribute(
        &self,
        member: &str,
        attribute: &str,
    ) -> bool {{
{has_attr_body}
    }}
}}

#[allow(improper_ctypes_definitions)]
pub(crate) extern "C" fn perro_create_script() -> *mut dyn ScriptBehavior<crate::RuntimeScriptApi> {{
    let script: Box<dyn ScriptBehavior<crate::RuntimeScriptApi>> =
        Box::new({script_ctor_expr});
    Box::into_raw(script)
}}
"#
    )
}

fn transpiled_exports_script_ctor(transpiled: &str) -> bool {
    transpiled.contains("fn perro_create_script(")
}

fn ensure_script_allows(source: &str) -> String {
    if source.contains("#![allow(unused_imports")
        || source.contains("#![allow(unused_variables")
        || source.contains("#![allow(dead_code")
    {
        return source.to_string();
    }
    format!("#![allow(unused_imports, unused_variables, dead_code)]\n{source}")
}

fn strip_transpiler_attributes(source: &str) -> String {
    let mut out = String::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if is_transpiler_attr_line(trimmed) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn parse_marked_struct_name(source: &str, marker: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    for i in 0..lines.len() {
        let l = lines[i].trim();
        if !(l == format!("///{marker}") || l == format!("//{marker}")) {
            continue;
        }
        for next in lines.iter().skip(i + 1) {
            let n = next.trim();
            if n.is_empty() {
                continue;
            }
            if let Some(name) = parse_struct_name(n) {
                return Some(name);
            }
        }
    }
    None
}

fn has_nonempty_lifecycle_method(source: &str, method_name: &str) -> bool {
    let needle = format!("fn {method_name}(");
    let mut search_from = 0usize;

    while search_from < source.len() {
        let Some(rel) = source[search_from..].find(&needle) else {
            break;
        };
        let fn_start = search_from + rel;
        let Some(body) = extract_method_body_from_fn_start(source, fn_start) else {
            search_from = fn_start + needle.len();
            continue;
        };

        if method_signature_looks_like_lifecycle(source, fn_start, body.start)
            && block_has_non_comment_tokens(&source[body.start + 1..body.end])
        {
            return true;
        }

        search_from = body.end + 1;
    }

    false
}

fn extract_method_body_from_fn_start(
    source: &str,
    fn_start: usize,
) -> Option<std::ops::Range<usize>> {
    let after_fn = &source[fn_start..];
    let sig_open_rel = after_fn.find('(')?;
    let sig_open = fn_start + sig_open_rel;
    let sig_close = find_matching_delim(source, sig_open, '(', ')')?;
    let body_start_rel = source[sig_close + 1..].find('{')?;
    let body_start = sig_close + 1 + body_start_rel;
    let body_end = find_matching_delim(source, body_start, '{', '}')?;
    Some(body_start..body_end)
}

fn method_signature_looks_like_lifecycle(source: &str, fn_start: usize, body_start: usize) -> bool {
    let sig = &source[fn_start..body_start];
    sig.contains("&self") && sig.contains("ScriptContext")
}

fn block_has_non_comment_tokens(block: &str) -> bool {
    let bytes = block.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        if b == b'/' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b'/' {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if next == b'*' {
                i += 2;
                let mut depth = 1_i32;
                while i + 1 < bytes.len() && depth > 0 {
                    if bytes[i] == b'/' && bytes[i + 1] == b'*' {
                        depth += 1;
                        i += 2;
                    } else if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        depth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                continue;
            }
        }

        return true;
    }

    false
}

fn parse_named_struct(source: &str, expected: &str) -> Option<String> {
    for line in source.lines() {
        if let Some(name) = parse_struct_name(line.trim())
            && name == expected
        {
            return Some(name);
        }
    }
    None
}

fn parse_struct_name(line: &str) -> Option<String> {
    let line = line.trim_start_matches("pub ").trim_start();
    if !line.starts_with("struct ") {
        return None;
    }
    let rest = line.trim_start_matches("struct ").trim_start();
    let mut name = String::new();
    for c in rest.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name.push(c);
        } else {
            break;
        }
    }
    if name.is_empty() { None } else { Some(name) }
}

fn is_unit_struct(source: &str, struct_name: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim();
        let line = line.trim_start_matches("pub ").trim_start();
        line == format!("struct {struct_name};")
    })
}

#[derive(Clone, Debug)]
struct ScriptField {
    name: String,
    ty: String,
    attrs: Vec<String>,
}

fn parse_struct_fields(source: &str, struct_name: &str) -> Vec<ScriptField> {
    let lines: Vec<&str> = source.lines().collect();
    let mut struct_line = None;
    for (i, line) in lines.iter().enumerate() {
        if parse_struct_name(line.trim()) == Some(struct_name.to_string()) {
            struct_line = Some(i);
            break;
        }
    }
    let Some(start) = struct_line else {
        return Vec::new();
    };

    let mut fields = Vec::new();
    let mut depth = 0_i32;
    let mut opened = false;
    let mut i = start;
    let mut pending_attrs: Vec<String> = Vec::new();

    while i < lines.len() {
        let raw_line = lines[i];
        if let Some(attr) = parse_transpiler_attr_name(raw_line.trim()) {
            pending_attrs.push(attr);
            i += 1;
            continue;
        }
        let line = strip_line_comment(raw_line);
        if !opened {
            if let Some(pos) = line.find('{') {
                opened = true;
                depth = 1;
                let rest = &line[pos + 1..];
                if depth == 1
                    && let Some(mut field) = parse_field_line(rest)
                {
                    apply_field_attrs(&mut field, &pending_attrs);
                    pending_attrs.clear();
                    fields.push(field);
                }
                depth += brace_delta(rest);
                if depth <= 0 {
                    break;
                }
            }
            i += 1;
            continue;
        }

        if depth == 1
            && let Some(mut field) = parse_field_line(line)
        {
            apply_field_attrs(&mut field, &pending_attrs);
            pending_attrs.clear();
            fields.push(field);
        }
        depth += brace_delta(line);
        if depth <= 0 {
            break;
        }
        i += 1;
    }

    fields
}

fn strip_line_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

fn brace_delta(line: &str) -> i32 {
    let opens = line.chars().filter(|c| *c == '{').count() as i32;
    let closes = line.chars().filter(|c| *c == '}').count() as i32;
    opens - closes
}

fn parse_field_line(line: &str) -> Option<ScriptField> {
    let trimmed = line.trim().trim_end_matches(',').trim();
    if trimmed.is_empty()
        || trimmed.starts_with("#[")
        || trimmed.starts_with("///")
        || trimmed.starts_with("//")
    {
        return None;
    }

    let without_vis = if let Some(rest) = trimmed.strip_prefix("pub(") {
        let after = rest.split_once(')')?.1;
        after.trim()
    } else {
        trimmed.trim_start_matches("pub ").trim_start()
    };

    let (name, ty) = without_vis.split_once(':')?;
    let name = name.trim();
    let ty = ty.trim();
    if name.is_empty() || ty.is_empty() || !is_ident(name) {
        return None;
    }

    Some(ScriptField {
        name: name.to_string(),
        ty: ty.to_string(),
        attrs: Vec::new(),
    })
}

fn apply_field_attrs(field: &mut ScriptField, attrs: &[String]) {
    field.attrs = dedup_attrs(attrs);
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn normalize_type(ty: &str) -> String {
    ty.chars().filter(|c| !c.is_whitespace()).collect()
}

fn supported_fields(fields: &[ScriptField]) -> Vec<ScriptField> {
    fields.to_vec()
}

fn supported_attributed_fields(fields: &[ScriptField]) -> Vec<ScriptField> {
    fields
        .iter()
        .filter(|f| !f.attrs.is_empty())
        .cloned()
        .collect()
}

fn member_const_name(field_name: &str) -> String {
    let mut out = String::from("__PERRO_VAR_");
    for c in field_name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

fn method_const_name(method_name: &str) -> String {
    let mut out = String::from("__PERRO_METHOD_");
    for c in method_name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

#[derive(Clone, Debug)]
struct ScriptMethod {
    name: String,
    takes_raw_params: bool,
    params: Vec<ScriptMethodParam>,
    returns_variant: bool,
    attrs: Vec<String>,
}

#[derive(Clone, Debug)]
struct ScriptMethodParam {
    name: String,
    ty: String,
}

fn generate_member_consts(fields: &[ScriptField], methods: &[ScriptMethod]) -> String {
    if fields.is_empty() && methods.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for field in fields {
        let const_name = member_const_name(&field.name);
        out.push_str(&format!(
            "const {const_name}: ScriptMemberID = var!(\"{}\");\n",
            field.name
        ));
    }
    for method in methods {
        let const_name = method_const_name(&method.name);
        out.push_str(&format!(
            "const {const_name}: ScriptMemberID = func!(\"{}\");\n",
            method.name
        ));
    }
    out
}

fn generate_call_method_body(methods: &[ScriptMethod]) -> String {
    if methods.is_empty() {
        return "        let _ = (method, ctx, params);\n        Variant::Null".to_string();
    }

    let mut out = String::new();
    out.push_str("        match method {\n");
    for method in methods {
        let const_name = method_const_name(&method.name);
        let call = if method.takes_raw_params {
            format!("self.{}(ctx, params)", method.name)
        } else if method.params.is_empty() {
            format!("self.{}(ctx)", method.name)
        } else {
            let args = method
                .params
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("self.{}(ctx, {args})", method.name)
        };

        let mut prelude = String::new();
        let mut supported = true;
        if !method.takes_raw_params && !method.params.is_empty() {
            for (i, param) in method.params.iter().enumerate() {
                if let Some(binding) = generate_call_param_binding(i, param) {
                    prelude.push_str("                ");
                    prelude.push_str(&binding);
                    prelude.push('\n');
                } else {
                    supported = false;
                    break;
                }
            }
        }

        if !supported {
            out.push_str(&format!(
                "            {const_name} => {{\n                let _ = (ctx, params);\n                Variant::Null\n            }}\n"
            ));
            continue;
        }

        if method.returns_variant {
            out.push_str(&format!(
                "            {const_name} => {{\n{prelude}                {call}\n            }}\n"
            ));
        } else {
            out.push_str(&format!(
                "            {const_name} => {{\n{prelude}                {call};\n                Variant::Null\n            }}\n"
            ));
        }
    }
    out.push_str("            _ => Variant::Null,\n");
    out.push_str("        }");
    out
}

fn parse_inherent_methods(source: &str, struct_name: &str) -> Vec<ScriptMethod> {
    let lines: Vec<&str> = source.lines().collect();
    let mut methods = Vec::new();
    let mut i = 0usize;

    while i < lines.len() {
        let line = strip_line_comment(lines[i]).trim();
        if !line.starts_with("impl") {
            i += 1;
            continue;
        }

        if line.contains(" for ") || !line.contains(struct_name) {
            i += 1;
            continue;
        }

        let mut depth = brace_delta(line);
        let mut opened = line.contains('{');
        let mut pending_attrs: Vec<String> = Vec::new();
        i += 1;

        while i < lines.len() {
            let raw_line = lines[i];
            if opened
                && depth == 1
                && let Some(attr) = parse_transpiler_attr_name(raw_line.trim())
            {
                pending_attrs.push(attr);
                i += 1;
                continue;
            }
            let l = strip_line_comment(raw_line);
            if opened
                && depth == 1
                && let Some(mut method) = parse_script_method_signature(l.trim())
            {
                method.attrs = dedup_attrs(&pending_attrs);
                pending_attrs.clear();
                methods.push(method);
            }

            if !opened && l.contains('{') {
                opened = true;
            }
            depth += brace_delta(l);
            if opened && depth <= 0 {
                break;
            }
            i += 1;
        }
        i += 1;
    }

    methods.extend(parse_methods_macro_methods(source, struct_name));
    methods.sort_by(|a, b| a.name.cmp(&b.name));
    methods.dedup_by(|a, b| a.name == b.name);
    methods
}

fn parse_attributed_struct_name(source: &str, attribute_name: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    for i in 0..lines.len() {
        let l = lines[i].trim();
        if !is_attribute_line_named(l, attribute_name) {
            continue;
        }
        for next in lines.iter().skip(i + 1) {
            let n = next.trim();
            if n.is_empty() {
                continue;
            }
            if n.starts_with("#[") {
                continue;
            }
            if let Some(name) = parse_struct_name(n) {
                return Some(name);
            }
            break;
        }
    }
    None
}

fn is_attribute_line_named(line: &str, attribute_name: &str) -> bool {
    let Some(inner) = line.strip_prefix("#[").and_then(|v| v.strip_suffix(']')) else {
        return false;
    };
    let inner = inner.trim();
    if inner.eq_ignore_ascii_case(attribute_name) {
        return true;
    }
    if let Some(open) = inner.find('(') {
        let name = inner[..open].trim();
        return name.eq_ignore_ascii_case(attribute_name);
    }
    false
}

fn parse_methods_macro_methods(source: &str, struct_name: &str) -> Vec<ScriptMethod> {
    let mut methods = Vec::new();
    let needle = "methods!(";
    let mut search_from = 0usize;

    while search_from < source.len() {
        let Some(rel) = source[search_from..].find(needle) else {
            break;
        };
        let start = search_from + rel;
        let open_paren = start + "methods!".len();
        let Some(close_paren) = find_matching_delim(source, open_paren, '(', ')') else {
            break;
        };

        let inner = &source[open_paren + 1..close_paren];
        if let Some((target_name, body)) = parse_methods_macro_inner(inner)
            && target_name == struct_name
        {
            methods.extend(parse_methods_block_signatures(body));
        }

        search_from = close_paren + 1;
    }

    methods
}

fn find_matching_delim(source: &str, open_index: usize, open: char, close: char) -> Option<usize> {
    find_matching_delim_lexed(source, open_index, open, close)
}

fn parse_methods_macro_inner(inner: &str) -> Option<(String, &str)> {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with('{') {
        let body = extract_brace_block(trimmed)?;
        return Some(("Script".to_string(), body));
    }

    let mut target = String::new();
    for c in trimmed.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            target.push(c);
        } else {
            break;
        }
    }
    if target.is_empty() {
        return None;
    }

    let rest = trimmed[target.len()..].trim_start();
    if !rest.starts_with('{') {
        return None;
    }
    let body = extract_brace_block(rest)?;
    Some((target, body))
}

fn extract_brace_block(s: &str) -> Option<&str> {
    if !s.starts_with('{') {
        return None;
    }
    let end = find_matching_delim_lexed(s, 0, '{', '}')?;
    Some(&s[1..end])
}

fn find_matching_delim_lexed(
    source: &str,
    open_index: usize,
    open: char,
    close: char,
) -> Option<usize> {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Mode {
        Code,
        LineComment,
        BlockComment,
        String,
        RawString(usize),
    }

    let bytes = source.as_bytes();
    if open_index >= bytes.len() || bytes[open_index] != open as u8 {
        return None;
    }

    let mut mode = Mode::Code;
    let mut block_comment_depth: usize = 0;
    let mut depth = 0_i32;
    let mut i = open_index;
    let mut escaped = false;

    while i < bytes.len() {
        let b = bytes[i];
        match mode {
            Mode::Code => {
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    mode = Mode::LineComment;
                    i += 2;
                    continue;
                }
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                    mode = Mode::BlockComment;
                    block_comment_depth = 1;
                    i += 2;
                    continue;
                }
                if let Some((prefix_len, hashes)) = raw_string_start_at(bytes, i) {
                    mode = Mode::RawString(hashes);
                    i += prefix_len;
                    continue;
                }
                if b == b'"' {
                    mode = Mode::String;
                    escaped = false;
                    i += 1;
                    continue;
                }
                if b == open as u8 {
                    depth += 1;
                } else if b == close as u8 {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                i += 1;
            }
            Mode::LineComment => {
                if b == b'\n' {
                    mode = Mode::Code;
                }
                i += 1;
            }
            Mode::BlockComment => {
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                    block_comment_depth += 1;
                    i += 2;
                    continue;
                }
                if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    block_comment_depth = block_comment_depth.saturating_sub(1);
                    i += 2;
                    if block_comment_depth == 0 {
                        mode = Mode::Code;
                    }
                    continue;
                }
                i += 1;
            }
            Mode::String => {
                if escaped {
                    escaped = false;
                    i += 1;
                    continue;
                }
                if b == b'\\' {
                    escaped = true;
                    i += 1;
                    continue;
                }
                if b == b'"' {
                    mode = Mode::Code;
                }
                i += 1;
            }
            Mode::RawString(hashes) => {
                if b == b'"' {
                    let mut ok = true;
                    for j in 0..hashes {
                        if i + 1 + j >= bytes.len() || bytes[i + 1 + j] != b'#' {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        mode = Mode::Code;
                        i += 1 + hashes;
                        continue;
                    }
                }
                i += 1;
            }
        }
    }
    None
}

fn raw_string_start_at(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
    if i >= bytes.len() {
        return None;
    }

    let (start, prefix_len) = if bytes[i] == b'r' {
        (i, 1usize)
    } else if i + 1 < bytes.len()
        && ((bytes[i] == b'b' && bytes[i + 1] == b'r')
            || (bytes[i] == b'r' && bytes[i + 1] == b'b'))
    {
        (i + 1, 2usize)
    } else {
        return None;
    };

    let mut j = start + 1;
    let mut hashes = 0usize;
    while j < bytes.len() && bytes[j] == b'#' {
        hashes += 1;
        j += 1;
    }
    if j < bytes.len() && bytes[j] == b'"' {
        return Some((prefix_len + hashes + 1, hashes));
    }
    None
}

fn parse_methods_block_signatures(body: &str) -> Vec<ScriptMethod> {
    let mut methods = Vec::new();
    let mut depth = 0_i32;
    let mut pending_attrs: Vec<String> = Vec::new();
    let mut sig_buf: Option<String> = None;
    let mut sig_paren_depth: i32 = 0;
    let debug_methods = methods_debug_enabled();

    for line in body.lines() {
        if depth == 0
            && let Some(attr) = parse_transpiler_attr_name(line.trim())
        {
            pending_attrs.push(attr);
            continue;
        }
        let l = strip_line_comment(line);
        let trimmed = l.trim();

        if depth == 0 {
            if let Some(buf) = sig_buf.as_mut() {
                if !trimmed.is_empty() {
                    buf.push(' ');
                    buf.push_str(trimmed);
                }
                sig_paren_depth += paren_delta(trimmed);
                if sig_paren_depth <= 0 {
                    match parse_script_method_signature_detailed(buf.trim()) {
                        Ok(mut method) => {
                            method.attrs = dedup_attrs(&pending_attrs);
                            pending_attrs.clear();
                            methods.push(method);
                        }
                        Err(reason) => {
                            if debug_methods {
                                eprintln!(
                                    "[perro][methods][skip] {} | signature=`{}`",
                                    reason,
                                    buf.trim()
                                );
                            }
                        }
                    }
                    sig_buf = None;
                    sig_paren_depth = 0;
                }
            } else if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                sig_buf = Some(trimmed.to_string());
                sig_paren_depth = paren_delta(trimmed);
                if sig_paren_depth <= 0 {
                    match parse_script_method_signature_detailed(trimmed) {
                        Ok(mut method) => {
                            method.attrs = dedup_attrs(&pending_attrs);
                            pending_attrs.clear();
                            methods.push(method);
                        }
                        Err(reason) => {
                            if debug_methods {
                                eprintln!(
                                    "[perro][methods][skip] {} | signature=`{}`",
                                    reason, trimmed
                                );
                            }
                        }
                    }
                    sig_buf = None;
                    sig_paren_depth = 0;
                }
            } else if let Ok(mut method) = parse_script_method_signature_detailed(trimmed) {
                method.attrs = dedup_attrs(&pending_attrs);
                pending_attrs.clear();
                methods.push(method);
            }
        }

        depth += brace_delta(l);
    }

    methods
}

fn paren_delta(s: &str) -> i32 {
    let mut depth = 0_i32;
    for c in s.chars() {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
        }
    }
    depth
}

fn parse_script_method_signature(line: &str) -> Option<ScriptMethod> {
    parse_script_method_signature_detailed(line).ok()
}

fn parse_script_method_signature_detailed(line: &str) -> Result<ScriptMethod, String> {
    let line = line.trim_start_matches("pub ").trim_start();
    if !line.starts_with("fn ") {
        return Err("not a function signature".to_string());
    }

    let rest = line.trim_start_matches("fn ").trim_start();
    let mut name = String::new();
    for c in rest.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name.push(c);
        } else {
            break;
        }
    }
    if name.is_empty() {
        Err("missing function name".to_string())
    } else {
        let params_sig = extract_fn_params_segment(line)
            .ok_or_else(|| "could not extract function parameters".to_string())?;
        let mut takes_raw_params = false;
        let mut params = Vec::new();
        let mut has_self = false;
        let mut has_ctx = false;
        let mut consumed_ctx = false;

        for raw in split_top_level_commas(params_sig) {
            let token = raw.trim();
            if token.is_empty()
                || token == "&self"
                || token == "self"
                || token == "&mut self"
                || token == "mut self"
            {
                has_self = true;
                continue;
            }

            let Some((name_part, ty_part)) = token.split_once(':') else {
                continue;
            };
            let param_name = name_part.trim();
            let param_ty = ty_part.trim();

            let normalized = normalize_type(param_ty);
            if is_script_context_type(&normalized) && !consumed_ctx {
                consumed_ctx = true;
                has_ctx = true;
                continue;
            }

            let is_raw_params = param_name == "params"
                && (normalized == "&[Variant]" || normalized == "&[perro_api::variant::Variant]");
            if is_raw_params {
                takes_raw_params = true;
                continue;
            }

            params.push(ScriptMethodParam {
                name: param_name.to_string(),
                ty: param_ty.to_string(),
            });
        }

        if takes_raw_params && !params.is_empty() {
            return Err("`params: &[Variant]` cannot be mixed with typed params".to_string());
        }
        if !(has_self && has_ctx) {
            let mut missing = Vec::new();
            if !has_self {
                missing.push("&self");
            }
            if !has_ctx {
                missing.push("ctx: &mut ScriptContext<...> (or ScriptCtx<...>)");
            }
            return Err(format!(
                "missing required leading parameters: {}",
                missing.join(", ")
            ));
        }

        let returns_variant =
            line.contains("-> Variant") || line.contains("->perro_api::variant::Variant");
        Ok(ScriptMethod {
            name,
            takes_raw_params,
            params,
            returns_variant,
            attrs: Vec::new(),
        })
    }
}

fn methods_debug_enabled() -> bool {
    let Ok(v) = std::env::var("PERRO_DEBUG_METHODS") else {
        return false;
    };
    let normalized = v.trim().to_ascii_lowercase();
    !normalized.is_empty()
        && !matches!(
            normalized.as_str(),
            "0" | "false" | "off" | "no" | "n" | "disabled"
        )
}

fn is_script_context_type(ty: &str) -> bool {
    ty.starts_with("&mutScriptContext<")
        || ty == "&mutScriptContext"
        || ty.starts_with("&mutScriptCtx<")
        || ty == "&mutScriptCtx"
        || ty.starts_with("&mutperro_api::scripting::ScriptContext<")
        || ty == "&mutperro_api::scripting::ScriptContext"
        || ty.starts_with("&mutperro_api::scripting::ScriptCtx<")
        || ty == "&mutperro_api::scripting::ScriptCtx"
        || ty.starts_with("ScriptContext<")
        || ty == "ScriptContext"
        || ty.starts_with("ScriptCtx<")
        || ty == "ScriptCtx"
        || ty.starts_with("perro_api::scripting::ScriptContext<")
        || ty == "perro_api::scripting::ScriptContext"
        || ty.starts_with("perro_api::scripting::ScriptCtx<")
        || ty == "perro_api::scripting::ScriptCtx"
}

fn parse_transpiler_attr_name(line: &str) -> Option<String> {
    let line = line.trim();
    if let Some(comment) = line.strip_prefix("///").or_else(|| line.strip_prefix("//")) {
        let comment = comment.trim();
        let rest = comment
            .strip_prefix('@')
            .or_else(|| comment.strip_prefix('#'))?
            .trim();
        if rest.is_empty() {
            return None;
        }
        let mut name = String::new();
        for c in rest.chars() {
            if c.is_ascii_alphanumeric() || c == '_' {
                name.push(c);
            } else {
                break;
            }
        }
        return is_ident(&name).then(|| name.to_ascii_lowercase());
    }

    if line.starts_with("#[") {
        let inner = line.strip_prefix("#[")?.strip_suffix(']')?.trim();
        if inner.is_empty() {
            return None;
        }
        let name = inner.split('(').next()?.trim();
        if !is_ident(name) || is_rust_attribute_name(name) {
            return None;
        }
        return Some(name.to_ascii_lowercase());
    }

    let rest = line.strip_prefix('#')?;
    if rest.starts_with('[') || rest.starts_with('!') {
        return None;
    }
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    let mut name = String::new();
    for c in rest.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name.push(c);
        } else {
            break;
        }
    }
    is_ident(&name).then(|| name.to_ascii_lowercase())
}

fn is_rust_attribute_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    matches!(
        name.as_str(),
        "state"
            | "default"
            | "derive"
            | "allow"
            | "warn"
            | "deny"
            | "forbid"
            | "cfg"
            | "cfg_attr"
            | "doc"
            | "path"
            | "test"
            | "inline"
            | "cold"
            | "deprecated"
            | "must_use"
            | "repr"
            | "non_exhaustive"
            | "no_mangle"
            | "unsafe"
    )
}

fn is_transpiler_attr_line(line: &str) -> bool {
    parse_transpiler_attr_name(line).is_some()
}

fn dedup_attrs(attrs: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::<String>::new();
    for attr in attrs {
        if seen.insert(attr.clone()) {
            out.push(attr.clone());
        }
    }
    out
}

fn sanitize_const_suffix(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "X".to_string() } else { out }
}

fn extract_fn_params_segment(line: &str) -> Option<&str> {
    let start = line.find('(')?;
    let mut depth = 0_i32;
    let mut end = None;
    for (i, c) in line.char_indices().skip(start) {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                end = Some(i);
                break;
            }
        }
    }
    let end = end?;
    Some(&line[start + 1..end])
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth_angle = 0_i32;
    let mut depth_paren = 0_i32;
    let mut depth_bracket = 0_i32;
    let mut start = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            ',' if depth_angle == 0 && depth_paren == 0 && depth_bracket == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start <= s.len() {
        out.push(&s[start..]);
    }
    out
}

fn params_get_expr(index: usize) -> String {
    if index == 0 {
        "params.first()".to_string()
    } else {
        format!("params.get({index})")
    }
}

fn generate_call_param_binding(index: usize, param: &ScriptMethodParam) -> Option<String> {
    let ty = normalize_type(&param.ty);
    let name = &param.name;
    let param_ref = params_get_expr(index);
    let line = match ty.as_str() {
        "bool" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Bool(v)) => *v, Some(_) => return Variant::Null, None => false }};"
        ),
        "i8" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I8(v))) => *v, Some(_) => return Variant::Null, None => 0_i8 }};"
        ),
        "i16" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I16(v))) => *v, Some(_) => return Variant::Null, None => 0_i16 }};"
        ),
        "i32" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I32(v))) => *v, Some(_) => return Variant::Null, None => 0_i32 }};"
        ),
        "i64" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I64(v))) => *v, Some(_) => return Variant::Null, None => 0_i64 }};"
        ),
        "i128" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I128(v))) => *v, Some(_) => return Variant::Null, None => 0_i128 }};"
        ),
        "isize" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::I64(v))) => match isize::try_from(*v) {{ Ok(v) => v, Err(_) => return Variant::Null }}, Some(_) => return Variant::Null, None => 0_isize }};"
        ),
        "u8" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U8(v))) => *v, Some(_) => return Variant::Null, None => 0_u8 }};"
        ),
        "u16" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U16(v))) => *v, Some(_) => return Variant::Null, None => 0_u16 }};"
        ),
        "u32" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U32(v))) => *v, Some(_) => return Variant::Null, None => 0_u32 }};"
        ),
        "u64" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U64(v))) => *v, Some(_) => return Variant::Null, None => 0_u64 }};"
        ),
        "u128" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U128(v))) => *v, Some(_) => return Variant::Null, None => 0_u128 }};"
        ),
        "usize" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::U64(v))) => match usize::try_from(*v) {{ Ok(v) => v, Err(_) => return Variant::Null }}, Some(_) => return Variant::Null, None => 0_usize }};"
        ),
        "f32" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::F32(v))) => *v, Some(_) => return Variant::Null, None => 0.0_f32 }};"
        ),
        "f64" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::Number(perro_api::variant::Number::F64(v))) => *v, Some(_) => return Variant::Null, None => 0.0_f64 }};"
        ),
        "String" | "std::string::String" | "alloc::string::String" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::String(v)) => v.to_string(), Some(_) => return Variant::Null, None => String::new() }};"
        ),
        "&str" => format!(
            "let {name}: &str = match {param_ref} {{ Some(Variant::String(v)) => v.as_ref(), Some(_) => return Variant::Null, None => \"\" }};"
        ),
        "Arc<str>" | "std::sync::Arc<str>" | "alloc::sync::Arc<str>" => format!(
            "let {name} = match {param_ref} {{ Some(Variant::String(v)) => std::sync::Arc::<str>::clone(v), Some(_) => return Variant::Null, None => std::sync::Arc::<str>::from(\"\") }};"
        ),
        "NodeID" | "perro_api::ids::NodeID" => format!(
            "let {name} = match {param_ref} {{ Some(v) => match v.as_node() {{ Some(v) => v, None => return Variant::Null }}, None => perro_api::ids::NodeID::nil() }};"
        ),
        "TextureID" | "perro_api::ids::TextureID" => format!(
            "let {name} = match {param_ref} {{ Some(v) => match v.as_texture() {{ Some(v) => v, None => return Variant::Null }}, None => perro_api::ids::TextureID::nil() }};"
        ),
        "Variant" | "perro_api::variant::Variant" => format!(
            "let {name} = match {param_ref} {{ Some(v) => v.clone(), None => Variant::Null }};"
        ),
        _ => {
            if ty.starts_with('&') {
                return None;
            }
            format!(
                "let {name}: {raw_ty} = match {param_ref} {{ \
                    Some(v) => match v.parse::<{raw_ty}>() {{ Ok(v) => v, Err(_) => return Variant::Null }}, \
                    None => Default::default() \
                }};",
                raw_ty = param.ty.trim()
            )
        }
    };
    Some(line)
}

fn generate_get_var_body(state_ty: &str, fields: &[ScriptField]) -> String {
    if fields.is_empty() {
        return String::from("           Variant::Null");
    }

    let mut out = String::new();
    out.push_str(&format!(
        "        let state = unsafe {{ &*(state as *const dyn std::any::Any as *const {state_ty}) }};\n"
    ));
    out.push_str("        match var {\n");
    for field in fields {
        let const_name = member_const_name(&field.name);
        out.push_str(&format!(
            "            {const_name} => perro_api::variant::DeriveVariant::to_variant(&state.{}),\n",
            field.name
        ));
    }
    out.push_str("            _ => __perro_get_nested_var(state, var).unwrap_or(Variant::Null),\n");
    out.push_str("        }");
    out
}

fn generate_set_var_body(state_ty: &str, fields: &[ScriptField]) -> String {
    if fields.is_empty() {
        return String::from("");
    }

    let mut out = String::new();
    out.push_str(&format!(
        "        let state = unsafe {{ &mut *(state as *mut dyn std::any::Any as *mut {state_ty}) }};\n"
    ));
    out.push_str("        __perro_set_var_match(state, var, value);\n");
    out
}

fn generate_apply_scene_injected_vars_body(state_ty: &str, fields: &[ScriptField]) -> String {
    if fields.is_empty() {
        return String::from("");
    }

    let mut out = String::new();
    out.push_str(&format!(
        "        let state = unsafe {{ &mut *(state as *mut dyn std::any::Any as *mut {state_ty}) }};\n"
    ));
    out.push_str("        for (var, value) in vars {\n");
    out.push_str("            __perro_set_var_match(state, *var, value);\n");
    out.push_str("        }\n");
    out
}

fn generate_set_var_match_fn(state_ty: &str, fields: &[ScriptField]) -> String {
    if fields.is_empty() {
        return String::from(
            "fn __perro_set_var_match(_state: &mut (), _var: ScriptMemberID, _value: &Variant) {}",
        );
    }

    let mut out = String::new();
    out.push_str(&format!(
        "fn __perro_set_var_match(state: &mut {state_ty}, var: ScriptMemberID, value: &Variant) {{\n"
    ));
    out.push_str("        match var {\n");
    for field in fields {
        let const_name = member_const_name(&field.name);
        let ty = normalize_type(&field.ty);
        let assign_block = format!(
            "if let Ok(v) = value.parse::<{ty}>() {{\n                    state.{} = v;\n                }}",
            field.name
        );
        out.push_str(&format!(
            "            {const_name} => {{\n                {assign_block}\n            }}\n"
        ));
    }
    out.push_str("            _ => {\n");
    out.push_str("                __perro_set_nested_var(state, var, value);\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("}\n\n");

    out.push_str(
        "fn __perro_get_nested_by_hash(prefix: &str, value: &Variant, var: ScriptMemberID) -> Option<Variant> {\n",
    );
    out.push_str("    let obj = value.as_object()?;\n");
    out.push_str("    for (key, child) in obj {\n");
    out.push_str("        let full = if prefix.is_empty() {\n");
    out.push_str("            key.to_string()\n");
    out.push_str("        } else {\n");
    out.push_str("            format!(\"{prefix}.{}\", key.as_ref())\n");
    out.push_str("        };\n");
    out.push_str("        if ScriptMemberID::from_string(full.as_str()) == var {\n");
    out.push_str("            return Some(child.clone());\n");
    out.push_str("        }\n");
    out.push_str(
        "        if let Some(found) = __perro_get_nested_by_hash(full.as_str(), child, var) {\n",
    );
    out.push_str("            return Some(found);\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    None\n");
    out.push_str("}\n\n");

    out.push_str(
        "fn __perro_set_nested_by_hash(prefix: &str, value: &mut Variant, var: ScriptMemberID, new_value: &Variant) -> bool {\n",
    );
    out.push_str("    let Some(obj) = value.as_object_mut() else {\n");
    out.push_str("        return false;\n");
    out.push_str("    };\n");
    out.push_str("    for (key, child) in obj {\n");
    out.push_str("        let full = if prefix.is_empty() {\n");
    out.push_str("            key.to_string()\n");
    out.push_str("        } else {\n");
    out.push_str("            format!(\"{prefix}.{}\", key.as_ref())\n");
    out.push_str("        };\n");
    out.push_str("        if ScriptMemberID::from_string(full.as_str()) == var {\n");
    out.push_str("            *child = new_value.clone();\n");
    out.push_str("            return true;\n");
    out.push_str("        }\n");
    out.push_str("        if __perro_set_nested_by_hash(full.as_str(), child, var, new_value) {\n");
    out.push_str("            return true;\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    false\n");
    out.push_str("}\n\n");

    out.push_str(&format!(
        "fn __perro_get_nested_var(state: &{state_ty}, var: ScriptMemberID) -> Option<Variant> {{\n"
    ));
    for field in fields {
        out.push_str(&format!(
            "    {{\n        let nested_root = perro_api::variant::DeriveVariant::to_variant(&state.{});\n        if let Some(value) = __perro_get_nested_by_hash(\"{}\", &nested_root, var) {{\n            return Some(value);\n        }}\n    }}\n",
            field.name, field.name
        ));
    }
    out.push_str("    None\n");
    out.push_str("}\n\n");

    out.push_str(&format!(
        "fn __perro_set_nested_var(state: &mut {state_ty}, var: ScriptMemberID, value: &Variant) -> bool {{\n"
    ));
    for field in fields {
        let ty = normalize_type(&field.ty);
        out.push_str(&format!(
            "    {{\n        let mut nested_root = perro_api::variant::DeriveVariant::to_variant(&state.{});\n        if __perro_set_nested_by_hash(\"{}\", &mut nested_root, var, value) {{\n            if let Ok(decoded) = nested_root.parse::<{ty}>() {{\n                state.{} = decoded;\n            }}\n            return true;\n        }}\n    }}\n",
            field.name, field.name, field.name
        ));
    }
    out.push_str("    false\n}\n");
    out
}

fn collect_member_attributes(
    fields: &[ScriptField],
    methods: &[ScriptMethod],
) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::<String, Vec<String>>::new();
    for field in fields {
        out.insert(field.name.clone(), dedup_attrs(&field.attrs));
    }
    for method in methods {
        let attrs = dedup_attrs(&method.attrs);
        if attrs.is_empty() {
            continue;
        }
        out.insert(method.name.clone(), attrs);
    }
    out
}

fn generate_attributes_of_body(fields: &[ScriptField], methods: &[ScriptMethod]) -> String {
    let member_attrs = collect_member_attributes(fields, methods);
    if member_attrs.is_empty() {
        return "        &[]".to_string();
    }

    let mut unique_attrs = BTreeSet::<String>::new();
    for attrs in member_attrs.values() {
        for attr in attrs {
            unique_attrs.insert(attr.clone());
        }
    }

    let mut out = String::new();
    let mut attr_consts = BTreeMap::<String, String>::new();
    for attr in unique_attrs {
        let const_name = format!("__PERRO_ATTR_{}", sanitize_const_suffix(&attr));
        attr_consts.insert(attr.clone(), const_name.clone());
        out.push_str(&format!(
            "        const {const_name}: Attribute = attribute!(\"{attr}\");\n"
        ));
    }

    for (name, attrs) in &member_attrs {
        let const_name = format!("__PERRO_MEMBER_ATTRS_{}", sanitize_const_suffix(name));
        let values = attrs
            .iter()
            .filter_map(|a| attr_consts.get(a))
            .map(|const_name| const_name.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "        const {const_name}: &[Attribute] = &[{values}];\n"
        ));
    }

    out.push_str("        match member {\n");
    for name in member_attrs.keys() {
        let const_name = format!("__PERRO_MEMBER_ATTRS_{}", sanitize_const_suffix(name));
        out.push_str(&format!("            \"{name}\" => {const_name},\n"));
    }
    out.push_str("            _ => &[],\n");
    out.push_str("        }");
    out
}

fn generate_members_with_body(fields: &[ScriptField], methods: &[ScriptMethod]) -> String {
    let member_attrs = collect_member_attributes(fields, methods);
    if member_attrs.is_empty() {
        return "        &[]".to_string();
    }

    let mut by_attr: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (member, attrs) in &member_attrs {
        for attr in attrs {
            by_attr
                .entry(attr.clone())
                .or_default()
                .push(member.clone());
        }
    }

    let mut out = String::new();
    for (attr, members) in &by_attr {
        let const_name = format!("__PERRO_MEMBERS_WITH_{}", sanitize_const_suffix(attr));
        out.push_str(&format!("        const {const_name}: &[Member] = &[\n"));
        for member in members {
            out.push_str(&format!("            member!(\"{member}\"),\n"));
        }
        out.push_str("        ];\n");
    }
    out.push_str("        match attribute {\n");
    for attr in by_attr.keys() {
        let const_name = format!("__PERRO_MEMBERS_WITH_{}", sanitize_const_suffix(attr));
        out.push_str(&format!("            \"{attr}\" => {const_name},\n"));
    }
    out.push_str("            _ => &[],\n");
    out.push_str("        }");
    out
}

fn generate_has_attribute_body(fields: &[ScriptField], methods: &[ScriptMethod]) -> String {
    let member_attrs = collect_member_attributes(fields, methods);
    if member_attrs.is_empty() {
        return "        false".to_string();
    }

    let mut out = String::new();
    out.push_str("        match member {\n");
    for (member, attrs) in &member_attrs {
        if attrs.is_empty() {
            out.push_str(&format!("            \"{member}\" => false,\n"));
            continue;
        }
        out.push_str(&format!("            \"{member}\" => matches!(attribute, "));
        for (i, attr) in attrs.iter().enumerate() {
            if i > 0 {
                out.push_str(" | ");
            }
            out.push_str(&format!("\"{attr}\""));
        }
        out.push_str("),\n");
    }
    out.push_str("            _ => false,\n");
    out.push_str("        }");
    out
}

fn module_name_from_rel(rel: &str) -> String {
    let mut out = String::with_capacity(rel.len());
    for c in rel.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    let mut name = if trimmed.is_empty() {
        "script".to_string()
    } else {
        trimmed.to_string()
    };
    if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    name
}

fn module_short_name_from_rel(rel: &str) -> String {
    module_name_from_rel(rel.strip_suffix(".rs").unwrap_or(rel))
}

fn generated_script_rel(rel: &str) -> String {
    if let Some(base) = rel.strip_suffix(".rs") {
        format!("{base}.gen.rs")
    } else {
        format!("{rel}.gen.rs")
    }
}

#[allow(dead_code)]
fn rel_to_path(base: &Path, rel: &str) -> PathBuf {
    base.join(rel.replace('/', "\\"))
}

#[cfg(test)]
mod tests {
    use super::{
        ScriptMethodParam, generate_call_param_binding, module_name_from_rel,
        module_short_name_from_rel, transpile_frontend_script, transpiled_exports_script_ctor,
    };

    fn assert_methods_emitted(transpiled: &str, expected_method_names: &[&str]) {
        assert!(
            transpiled.contains("match method {"),
            "expected generated call_method match"
        );
        assert!(
            !transpiled.contains("let _ = (method, ctx, params);"),
            "unexpected empty call_method stub generated"
        );
        for method_name in expected_method_names {
            let const_name = format!("__PERRO_METHOD_{}", method_name.to_ascii_uppercase());
            assert!(
                transpiled.contains(&const_name),
                "missing method const for {method_name}"
            );
            let arm = format!("{const_name} =>");
            assert!(
                transpiled.contains(&arm),
                "missing call_method arm for {method_name}"
            );
        }
    }

    #[test]
    fn transpiles_controller_methods_into_call_method_arms() {
        let source = r#"
use perro_api::prelude::*;

#[State]
pub struct ArcherControllerState {
    #[default = false]
    pub enabled: bool,
}

lifecycle!({
    fn on_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
});

methods!({
    fn bind_agent(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _agent_id: NodeID,
    ) {}

    fn set_player_index(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _player_index: i32,
    ) {}

    fn set_turn_enabled(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _enabled: bool,
    ) {}
});
"#;

        let transpiled = transpile_frontend_script(source, "res://tests/controller.rs");
        assert_methods_emitted(
            &transpiled,
            &["bind_agent", "set_player_index", "set_turn_enabled"],
        );
    }

    #[test]
    fn transpiles_ai_methods_into_call_method_arms() {
        let source = r#"
use perro_api::prelude::*;

#[derive(Variant, Clone, Copy)]
pub struct AgentRef {
    pub agent_id: NodeID,
}

impl Default for AgentRef {
    fn default() -> Self {
        Self {
            agent_id: NodeID::nil(),
        }
    }
}

#[derive(Variant, Clone, Copy)]
pub struct AimPlan {
    pub has_plan: bool,
}

impl Default for AimPlan {
    fn default() -> Self {
        Self { has_plan: false }
    }
}

#[State]
pub struct ArcherAiBrainState {
    #[default = false]
    pub enabled: bool,
    #[default = AgentRef::default()]
    pub agent_ref: AgentRef,
    #[default = AimPlan::default()]
    pub plan: AimPlan,
}

lifecycle!({
    fn on_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
});

methods!({
    fn bind_agent(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _agent_id: NodeID,
    ) {}

    fn set_turn_enabled(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _enabled: bool,
    ) {}

    fn set_ai_skill(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _skill: f32,
    ) {}

    fn reset_plan(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
});
"#;

        let transpiled = transpile_frontend_script(source, "res://tests/ai.rs");
        assert_methods_emitted(
            &transpiled,
            &[
                "bind_agent",
                "set_turn_enabled",
                "set_ai_skill",
                "reset_plan",
            ],
        );
    }

    #[test]
    fn transpiles_methods_even_with_braces_in_strings_comments_and_raw_strings() {
        let source = r###"
use perro_api::prelude::*;

#[State]
pub struct WeirdState {
    #[default = false]
    pub enabled: bool,
}

lifecycle!({
    fn on_update(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        // comment with misleading delimiters: methods!({ fn nope( ) { } });
        let _a = "format-like braces {x} and parens (y) should not affect parser";
        let _b = r#"raw string with fake delimiters: methods!({ fn fake() {} })"#;
        let _c = br##"byte raw with nested hashes and braces { } ) ("##;
        let _d = "emoji \u{1F3F9} and braces {{{}}}";
    }
});

methods!({
    fn alpha(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}

    fn beta(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _enabled: bool,
    ) {}
});
"###;

        let transpiled = transpile_frontend_script(source, "res://tests/weird.rs");
        assert_methods_emitted(&transpiled, &["alpha", "beta"]);
    }

    #[test]
    fn typed_param_binding_uses_first_for_zero_index() {
        let first = generate_call_param_binding(
            0,
            &ScriptMethodParam {
                name: "sport".to_string(),
                ty: "Sport".to_string(),
            },
        )
        .expect("custom param should generate binding");
        let second = generate_call_param_binding(
            1,
            &ScriptMethodParam {
                name: "enabled".to_string(),
                ty: "bool".to_string(),
            },
        )
        .expect("bool param should generate binding");

        assert!(first.contains("match params.first()"));
        assert!(!first.contains("params.get(0)"));
        assert!(second.contains("match params.get(1)"));
    }

    #[test]
    fn bare_module_has_no_exported_ctor() {
        let source = r#"
pub const SCALE: f32 = 2.0;

pub enum Team {
    Red,
    Blue,
}

pub fn mix(a: f32, b: f32) -> f32 {
    (a + b) * SCALE
}
"#;

        let transpiled = transpile_frontend_script(source, "res://scripts/util.rs");
        assert!(
            !transpiled_exports_script_ctor(&transpiled),
            "bare modules should not register as script constructors"
        );
    }

    #[test]
    fn state_script_exports_ctor() {
        let source = r#"
use perro_api::prelude::*;

#[State]
pub struct StateOnly {
    #[default = 1.0]
    pub speed: f32,
}
"#;

        let transpiled = transpile_frontend_script(source, "res://scripts/state_only.rs");
        assert!(
            transpiled_exports_script_ctor(&transpiled),
            "state-backed scripts should register constructors"
        );
    }

    #[test]
    fn transpiled_state_includes_nested_var_helpers() {
        let source = r#"
use perro_api::prelude::*;

#[derive(Variant, Clone)]
pub struct Person {
    pub name: String,
}

#[State]
pub struct NestedState {
    #[default = Person { name: String::new() }]
    pub person: Person,
}
"#;

        let transpiled = transpile_frontend_script(source, "res://scripts/nested_state.rs");
        assert!(transpiled.contains("__perro_get_nested_var"));
        assert!(transpiled.contains("__perro_set_nested_var"));
        assert!(transpiled.contains("ScriptMemberID::from_string(full.as_str())"));
    }

    #[test]
    fn module_short_name_drops_rs_suffix() {
        assert_eq!(
            module_name_from_rel("scripts/personality_module.rs"),
            "scripts_personality_module_rs"
        );
        assert_eq!(
            module_short_name_from_rel("scripts/personality_module.rs"),
            "scripts_personality_module"
        );
    }
}

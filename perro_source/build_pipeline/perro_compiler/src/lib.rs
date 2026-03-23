use perro_assets::build_perro_assets_archive;
use perro_io::walkdir::walk_dir;
use perro_project::{ensure_source_overrides, load_project_toml};
use std::{
    collections::{BTreeMap, BTreeSet},
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

pub fn sync_scripts(project_root: &Path) -> Result<Vec<String>, CompilerError> {
    let res_dir = project_root.join("res");
    let scripts_src = project_root.join(".perro").join("scripts").join("src");

    if scripts_src.exists() {
        fs::remove_dir_all(&scripts_src)?;
    }
    fs::create_dir_all(&scripts_src)?;

    let mut copied = Vec::<String>::new();
    if res_dir.exists() {
        walk_dir(&res_dir, &mut |path| {
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                return Ok(());
            }
            let rel = path.strip_prefix(&res_dir).unwrap();
            let rel_norm = rel.to_string_lossy().replace('\\', "/");
            let generated_rel = generated_script_rel(&rel_norm);
            let dst = scripts_src.join(&generated_rel);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            let source = fs::read_to_string(path)?;
            let transformed = transpile_frontend_script(&source);
            fs::write(&dst, transformed)?;
            copied.push(rel_norm);
            Ok(())
        })?;
    }

    copied.sort();
    write_scripts_lib(&scripts_src, &res_dir, &copied)?;
    Ok(copied)
}

pub fn compile_scripts(project_root: &Path) -> Result<Vec<String>, CompilerError> {
    ensure_source_overrides(project_root)?;
    let copied = sync_scripts(project_root)?;
    let scripts_crate = project_root.join(".perro").join("scripts");
    let target_dir = project_root.join("target");

    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(scripts_crate)
        .status()?;

    if !status.success() {
        return Err(CompilerError::CargoFailed(status.code().unwrap_or(-1)));
    }

    Ok(copied)
}

pub fn compile_project_bundle(project_root: &Path, profile: bool) -> Result<(), CompilerError> {
    ensure_source_overrides(project_root)?;
    let cfg = load_project_toml(project_root)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    reset_embedded_dir(project_root)?;
    let _ = compile_scripts(project_root)?;
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
    perro_static_pipeline::write_static_mod_rs(project_root)
        .map_err(|err| CompilerError::SceneParse(format!("static mod generation failed: {err}")))?;
    generate_embedded_main(project_root)?;
    generate_perro_assets(project_root)?;
    build_project_crate(project_root, profile)?;
    Ok(())
}

fn generate_perro_assets(project_root: &Path) -> Result<(), CompilerError> {
    let embedded_dir = project_root.join(".perro").join("project").join("embedded");
    fs::create_dir_all(&embedded_dir)?;
    let output = embedded_dir.join("assets.perro");
    let res_dir = project_root.join("res");
    build_perro_assets_archive(&output, &res_dir, project_root)?;
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
    ensure_project_dependency_line(
        project_root,
        "perro_animation",
        "perro_animation = \"0.1.0\"",
    )?;
    ensure_project_dependency_line(
        project_root,
        "perro_structs",
        "perro_structs = \"0.1.0\"",
    )?;

    let embedded_block = format!(
        "let root = project_root();\n\
perro_app::entry::run_static_embedded_project(perro_app::entry::StaticEmbeddedProject {{\n\
  project: perro_app::entry::StaticEmbeddedProjectInfo {{\n\
        project_root: &root,\n\
        project_name: \"{name}\",\n\
        main_scene: \"{main_scene}\",\n\
        icon: \"{icon}\",\n\
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
        target_fps: {target_fps},\n\
        target_fixed_update: {target_fixed_update},\n\
  }},\n\
  assets: perro_app::entry::StaticEmbeddedAssetsConfig {{\n\
        perro_assets: PERRO_ASSETS,\n\
        scene_lookup: static_assets::scenes::lookup_scene,\n\
        material_lookup: static_assets::materials::lookup_material,\n\
        particle_lookup: static_assets::particles::lookup_particle,\n\
        animation_lookup: static_assets::animations::lookup_animation,\n\
        mesh_lookup: static_assets::meshes::lookup_mesh,\n\
        skeleton_lookup: static_assets::skeletons::lookup_skeleton,\n\
        texture_lookup: static_assets::textures::lookup_texture,\n\
        shader_lookup: static_assets::shaders::lookup_shader,\n\
        audio_lookup: static_assets::audios::lookup_audio,\n\
        static_script_registry: Some(scripts::SCRIPT_REGISTRY),\n\
  }},\n\
}})\n\
.expect(\"failed to run embedded static project\");",
        name = escape_str(&cfg.name),
        main_scene = escape_str(&cfg.main_scene),
        icon = escape_str(&cfg.icon),
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
        target_fps = emit_optional_f32(cfg.target_fps),
        target_fixed_update = emit_optional_f32(cfg.target_fixed_update),
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
    res_dir: &Path,
    copied: &[String],
) -> Result<(), CompilerError> {
    let mut out = String::new();
    out.push_str("#![allow(unused_imports, unused_variables, dead_code)]\n");
    out.push_str("// AUTO-GENERATED by Perro Compiler. Do not edit by hand.\n\n");
    out.push_str("use perro::runtime::{Runtime, RuntimeInputApi, RuntimeResourceApi};\n");
    out.push_str("use perro::scripting::ScriptConstructor;\n\n");

    for rel in copied {
        let module = module_name_from_rel(rel);
        let generated_rel = generated_script_rel(rel);
        let source_path = res_dir.join(rel.replace('/', "\\"));
        let source_abs = source_path
            .canonicalize()
            .unwrap_or(source_path)
            .to_string_lossy()
            .to_string();
        let source_abs = escape_str(&normalize_generated_include_path(&source_abs));
        out.push_str(&format!("#[path = \"{generated_rel}\"]\n"));
        out.push_str(&format!("pub mod {module};\n\n"));
        out.push_str("#[cfg(rust_analyzer)]\n");
        out.push_str(&format!("mod __ra_{module} {{\n"));
        out.push_str(&format!("    include!(\"{source_abs}\");\n"));
        out.push_str("}\n\n");
    }

    out.push_str(
        "pub static SCRIPT_REGISTRY: &[(&str, ScriptConstructor<Runtime, RuntimeResourceApi, RuntimeInputApi>)] = &[\n",
    );
    for rel in copied {
        let module = module_name_from_rel(rel);
        out.push_str(&format!(
            "    (\"res://{rel}\", {module}::perro_create_script as ScriptConstructor<Runtime, RuntimeResourceApi, RuntimeInputApi>),\n"
        ));
    }
    out.push_str("];\n");
    out.push_str(
        "\n#[unsafe(no_mangle)]\n\
pub extern \"C\" fn perro_scripts_set_project_root(\n\
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
    perro::modules::file::set_project_root_disk(root, name);\n\
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
pub extern \"C\" fn perro_script_registry_get(\n\
    index: usize,\n\
    path_out: *mut *const u8,\n\
    len_out: *mut usize,\n\
    ctor_out: *mut ScriptConstructor<Runtime, RuntimeResourceApi, RuntimeInputApi>,\n\
) -> bool {\n\
    if path_out.is_null() || len_out.is_null() || ctor_out.is_null() {\n\
        return false;\n\
    }\n\
    let Some((path, ctor)) = SCRIPT_REGISTRY.get(index) else {\n\
        return false;\n\
    };\n\
    unsafe {\n\
        *path_out = path.as_ptr();\n\
        *len_out = path.len();\n\
        *ctor_out = *ctor;\n\
    }\n\
    true\n\
}\n",
    );

    fs::write(scripts_src.join("lib.rs"), out)?;
    Ok(())
}

fn transpile_frontend_script(source: &str) -> String {
    let source = ensure_script_allows(source);
    if source.contains("impl ScriptBehavior") {
        return source;
    }
    let stripped_source = strip_transpiler_attributes(&source);

    let state_ty = match parse_marked_struct_name(&source, "@State")
        .or_else(|| parse_attributed_struct_name(&source, "state"))
    {
        Some(v) => v,
        None => return source,
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
    let set_var_body = generate_set_var_body(&state_ty, &exposed_fields);
    let call_method_body = generate_call_method_body(&user_methods);
    let attr_of_body = generate_attributes_of_body(&attributed_fields, &user_methods);
    let members_with_body = generate_members_with_body(&attributed_fields, &user_methods);
    let has_attr_body = generate_has_attribute_body(&attributed_fields, &user_methods);

    format!(
        r#"{stripped_source}

// ---- AUTO-GENERATED by Perro Compiler ----
{member_consts}

impl<RT: RuntimeAPI + ?Sized, RS: perro::resource_context::api::ResourceAPI + ?Sized, IP: perro::input::InputAPI + ?Sized> ScriptBehavior<RT, RS, IP> for {script_ty} {{
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

    fn call_method(
        &self,
        method: ScriptMemberID,
        ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        ipt: &InputContext<'_, IP>,
        self_id: NodeID,
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
pub extern "C" fn perro_create_script() -> *mut dyn ScriptBehavior<perro::runtime::Runtime, perro::runtime::RuntimeResourceApi, perro::runtime::RuntimeInputApi> {{
    let script: Box<dyn ScriptBehavior<perro::runtime::Runtime, perro::runtime::RuntimeResourceApi, perro::runtime::RuntimeInputApi>> =
        Box::new({script_ctor_expr});
    Box::into_raw(script)
}}
"#
    )
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
    sig.contains("&self")
        && sig.contains("RuntimeContext")
        && sig.contains("ResourceContext")
        && sig.contains("InputContext")
        && sig.contains("NodeID")
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
struct StateField {
    name: String,
    ty: String,
    attrs: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldKind {
    Bool,
    SignedInt,
    UnsignedInt,
    Float32,
    Float64,
    String,
    ArcStr,
    NodeID,
    TextureID,
}

fn parse_struct_fields(source: &str, struct_name: &str) -> Vec<StateField> {
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

fn parse_field_line(line: &str) -> Option<StateField> {
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

    Some(StateField {
        name: name.to_string(),
        ty: ty.to_string(),
        attrs: Vec::new(),
    })
}

fn apply_field_attrs(field: &mut StateField, attrs: &[String]) {
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

fn field_kind(ty: &str) -> Option<FieldKind> {
    let ty = normalize_type(ty);
    match ty.as_str() {
        "bool" => Some(FieldKind::Bool),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => Some(FieldKind::SignedInt),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => Some(FieldKind::UnsignedInt),
        "f32" => Some(FieldKind::Float32),
        "f64" => Some(FieldKind::Float64),
        "String" | "std::string::String" | "alloc::string::String" => Some(FieldKind::String),
        "Arc<str>" | "std::sync::Arc<str>" | "alloc::sync::Arc<str>" => Some(FieldKind::ArcStr),
        "NodeID" | "perro::ids::NodeID" => Some(FieldKind::NodeID),
        "TextureID" | "perro::ids::TextureID" => Some(FieldKind::TextureID),
        _ => None,
    }
}

fn supported_fields(fields: &[StateField]) -> Vec<StateField> {
    fields
        .iter()
        .filter(|f| field_kind(&f.ty).is_some())
        .cloned()
        .collect()
}

fn supported_attributed_fields(fields: &[StateField]) -> Vec<StateField> {
    fields
        .iter()
        .filter(|f| field_kind(&f.ty).is_some() && !f.attrs.is_empty())
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

fn method_arity_const_name(method_name: &str) -> String {
    let mut out = String::from("__PERRO_METHOD_ARITY_");
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

fn generate_member_consts(fields: &[StateField], methods: &[ScriptMethod]) -> String {
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
        if !method.takes_raw_params {
            let arity_const_name = method_arity_const_name(&method.name);
            out.push_str(&format!(
                "const {arity_const_name}: usize = {};\n",
                method.params.len()
            ));
        }
    }
    out
}

fn generate_call_method_body(methods: &[ScriptMethod]) -> String {
    if methods.is_empty() {
        return "        let _ = (method, ctx, res, ipt, self_id, params);\n        Variant::Null"
            .to_string();
    }

    let mut out = String::new();
    out.push_str("        match method {\n");
    for method in methods {
        let const_name = method_const_name(&method.name);
        let arity_const_name = method_arity_const_name(&method.name);
        let call = if method.takes_raw_params {
            format!("self.{}(ctx, res, ipt, self_id, params)", method.name)
        } else if method.params.is_empty() {
            format!("self.{}(ctx, res, ipt, self_id)", method.name)
        } else {
            let args = method
                .params
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("self.{}(ctx, res, ipt, self_id, {args})", method.name)
        };

        let mut prelude = String::new();
        let mut supported = true;
        if !method.takes_raw_params {
            prelude.push_str(&format!(
                "                if params.len() != {arity_const_name} {{ return Variant::Null; }}\n"
            ));
        }
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
                "            {const_name} => {{\n                let _ = (ctx, res, ipt, self_id, params);\n                Variant::Null\n            }}\n"
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
    let mut depth = 0_i32;
    for (idx, c) in source.char_indices().skip(open_index) {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
    }
    None
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
    let mut depth = 0_i32;
    let mut end = None;
    for (i, c) in s.char_indices() {
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                end = Some(i);
                break;
            }
        }
    }
    let end = end?;
    Some(&s[1..end])
}

fn parse_methods_block_signatures(body: &str) -> Vec<ScriptMethod> {
    let mut methods = Vec::new();
    let mut depth = 0_i32;
    let mut pending_attrs: Vec<String> = Vec::new();
    let mut sig_buf: Option<String> = None;
    let mut sig_paren_depth: i32 = 0;

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
                    if let Some(mut method) = parse_script_method_signature(buf.trim()) {
                        method.attrs = dedup_attrs(&pending_attrs);
                        pending_attrs.clear();
                        methods.push(method);
                    }
                    sig_buf = None;
                    sig_paren_depth = 0;
                }
            } else if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                sig_buf = Some(trimmed.to_string());
                sig_paren_depth = paren_delta(trimmed);
                if sig_paren_depth <= 0 {
                    if let Some(mut method) = parse_script_method_signature(trimmed) {
                        method.attrs = dedup_attrs(&pending_attrs);
                        pending_attrs.clear();
                        methods.push(method);
                    }
                    sig_buf = None;
                    sig_paren_depth = 0;
                }
            } else if let Some(mut method) = parse_script_method_signature(trimmed) {
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
    let line = line.trim_start_matches("pub ").trim_start();
    if !line.starts_with("fn ") {
        return None;
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
        None
    } else {
        let params_sig = extract_fn_params_segment(line)?;
        let mut takes_raw_params = false;
        let mut params = Vec::new();
        let mut has_self = false;
        let mut has_ctx = false;
        let mut has_res = false;
        let mut has_ipt = false;
        let mut has_node_id = false;

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
            if is_runtime_context_type(&normalized) {
                has_ctx = true;
                continue;
            }
            if is_resource_context_type(&normalized) {
                has_res = true;
                continue;
            }
            if is_input_context_type(&normalized) {
                has_ipt = true;
                continue;
            }
            if is_node_id_type(&normalized) {
                has_node_id = true;
                continue;
            }

            let is_raw_params = param_name == "params"
                && (normalized == "&[Variant]" || normalized == "&[perro::variant::Variant]");
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
            return None;
        }
        if !(has_self && has_ctx && has_res && has_ipt && has_node_id) {
            return None;
        }

        let returns_variant =
            line.contains("-> Variant") || line.contains("->perro::variant::Variant");
        Some(ScriptMethod {
            name,
            takes_raw_params,
            params,
            returns_variant,
            attrs: Vec::new(),
        })
    }
}

fn is_runtime_context_type(ty: &str) -> bool {
    ty.starts_with("&mutRuntimeContext<")
        || ty == "&mutRuntimeContext"
        || ty.starts_with("&mutperro::runtime_context::RuntimeContext<")
        || ty == "&mutperro::runtime_context::RuntimeContext"
}

fn is_resource_context_type(ty: &str) -> bool {
    ty.starts_with("&ResourceContext<")
        || ty == "&ResourceContext"
        || ty.starts_with("&perro::resource_context::ResourceContext<")
        || ty == "&perro::resource_context::ResourceContext"
}

fn is_input_context_type(ty: &str) -> bool {
    ty.starts_with("&InputContext<")
        || ty == "&InputContext"
        || ty.starts_with("&perro::input::InputContext<")
        || ty == "&perro::input::InputContext"
}

fn is_node_id_type(ty: &str) -> bool {
    ty == "NodeID" || ty == "perro::ids::NodeID"
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

fn generate_call_param_binding(index: usize, param: &ScriptMethodParam) -> Option<String> {
    let ty = normalize_type(&param.ty);
    let name = &param.name;
    let line = match ty.as_str() {
        "bool" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Bool(v)) => *v, _ => return Variant::Null }};"
        ),
        "i8" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::I8(v))) => *v, _ => return Variant::Null }};"
        ),
        "i16" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::I16(v))) => *v, _ => return Variant::Null }};"
        ),
        "i32" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::I32(v))) => *v, _ => return Variant::Null }};"
        ),
        "i64" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::I64(v))) => *v, _ => return Variant::Null }};"
        ),
        "i128" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::I128(v))) => *v, _ => return Variant::Null }};"
        ),
        "isize" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::I64(v))) => match isize::try_from(*v) {{ Ok(v) => v, Err(_) => return Variant::Null }}, _ => return Variant::Null }};"
        ),
        "u8" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::U8(v))) => *v, _ => return Variant::Null }};"
        ),
        "u16" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::U16(v))) => *v, _ => return Variant::Null }};"
        ),
        "u32" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::U32(v))) => *v, _ => return Variant::Null }};"
        ),
        "u64" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::U64(v))) => *v, _ => return Variant::Null }};"
        ),
        "u128" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::U128(v))) => *v, _ => return Variant::Null }};"
        ),
        "usize" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::U64(v))) => match usize::try_from(*v) {{ Ok(v) => v, Err(_) => return Variant::Null }}, _ => return Variant::Null }};"
        ),
        "f32" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::F32(v))) => *v, _ => return Variant::Null }};"
        ),
        "f64" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro::variant::Number::F64(v))) => *v, _ => return Variant::Null }};"
        ),
        "String" | "std::string::String" | "alloc::string::String" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::String(v)) => v.to_string(), _ => return Variant::Null }};"
        ),
        "&str" => format!(
            "let {name}: &str = match params.get({index}) {{ Some(Variant::String(v)) => v.as_ref(), _ => return Variant::Null }};"
        ),
        "Arc<str>" | "std::sync::Arc<str>" | "alloc::sync::Arc<str>" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::String(v)) => std::sync::Arc::<str>::clone(v), _ => return Variant::Null }};"
        ),
        "NodeID" | "perro::ids::NodeID" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::NodeID(v)) => *v, _ => return Variant::Null }};"
        ),
        "TextureID" | "perro::ids::TextureID" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::TextureID(v)) => *v, _ => return Variant::Null }};"
        ),
        "Variant" | "perro::variant::Variant" => format!(
            "let {name} = match params.get({index}) {{ Some(v) => v.clone(), None => return Variant::Null }};"
        ),
        _ => return None,
    };
    Some(line)
}

fn generate_get_var_body(state_ty: &str, fields: &[StateField]) -> String {
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
        let access = match field_kind(&field.ty).unwrap() {
            FieldKind::String | FieldKind::ArcStr => format!("state.{}.clone()", field.name),
            _ => format!("state.{}", field.name),
        };
        out.push_str(&format!(
            "            {const_name} => Variant::from({access}),\n"
        ));
    }
    out.push_str("            _ => Variant::Null,\n");
    out.push_str("        }");
    out
}

fn generate_set_var_body(state_ty: &str, fields: &[StateField]) -> String {
    if fields.is_empty() {
        return String::from("");
    }

    let mut out = String::new();
    out.push_str(&format!(
        "        let state = unsafe {{ &mut *(state as *mut dyn std::any::Any as *mut {state_ty}) }};\n"
    ));
    out.push_str("        match var {\n");
    for field in fields {
        let const_name = member_const_name(&field.name);
        let ty = normalize_type(&field.ty);
        let assign_block = match field_kind(&field.ty).unwrap() {
            FieldKind::Bool => format!(
                "if let Some(v) = value.as_bool() {{\n                    state.{} = v;\n                }}",
                field.name
            ),
            FieldKind::SignedInt => exact_signed_assign_block(&ty, &field.name),
            FieldKind::UnsignedInt => exact_unsigned_assign_block(&ty, &field.name),
            FieldKind::Float32 => format!(
                "if let Some(v) = value.as_f32() {{\n                    state.{} = v;\n                }}",
                field.name
            ),
            FieldKind::Float64 => format!(
                "if let Some(v) = value.as_f64() {{\n                    state.{} = v;\n                }}",
                field.name
            ),
            FieldKind::String => format!(
                "if let Some(v) = value.as_str().map(|s| s.to_string()) {{\n                    state.{} = v;\n                }}",
                field.name
            ),
            FieldKind::ArcStr => format!(
                "if let Some(v) = value.as_str().map(std::sync::Arc::<str>::from) {{\n                    state.{} = v;\n                }}",
                field.name
            ),
            FieldKind::NodeID => format!(
                "if let Some(v) = value.as_node().or_else(|| value.as_number().and_then(|n| n.as_i64_lossy()).and_then(|n| u64::try_from(n).ok()).map(perro::ids::NodeID::from_u64)).or_else(|| value.as_str().and_then(|s| perro::ids::NodeID::parse_str(s).ok())) {{\n                    state.{} = v;\n                }}",
                field.name
            ),
            FieldKind::TextureID => format!(
                "if let Some(v) = value.as_texture().or_else(|| value.as_number().and_then(|n| n.as_i64_lossy()).and_then(|n| u64::try_from(n).ok()).map(perro::ids::TextureID::from_u64)).or_else(|| value.as_str().and_then(|s| perro::ids::TextureID::parse_str(s).ok())) {{\n                    state.{} = v;\n                }}",
                field.name
            ),
        };
        out.push_str(&format!(
            "            {const_name} => {{\n                {assign_block}\n            }}\n"
        ));
    }
    out.push_str("            _ => {}\n");
    out.push_str("        }\n");
    out
}

fn exact_signed_assign_block(ty: &str, field_name: &str) -> String {
    match ty {
        "i8" => format!(
            "if let Some(perro::variant::Number::I8(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "i16" => format!(
            "if let Some(perro::variant::Number::I16(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "i32" => format!(
            "if let Some(perro::variant::Number::I32(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "i64" => format!(
            "if let Some(perro::variant::Number::I64(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "i128" => format!(
            "if let Some(perro::variant::Number::I128(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        // Number has no isize variant; accept only I64 then checked cast.
        "isize" => format!(
            "if let Some(perro::variant::Number::I64(v)) = value.as_number() {{\n                    if let Ok(v) = <isize>::try_from(v) {{\n                        state.{field_name} = v;\n                    }}\n                }}"
        ),
        _ => String::new(),
    }
}

fn exact_unsigned_assign_block(ty: &str, field_name: &str) -> String {
    match ty {
        "u8" => format!(
            "if let Some(perro::variant::Number::U8(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "u16" => format!(
            "if let Some(perro::variant::Number::U16(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "u32" => format!(
            "if let Some(perro::variant::Number::U32(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "u64" => format!(
            "if let Some(perro::variant::Number::U64(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "u128" => format!(
            "if let Some(perro::variant::Number::U128(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        // Number has no usize variant; accept only U64 then checked cast.
        "usize" => format!(
            "if let Some(perro::variant::Number::U64(v)) = value.as_number() {{\n                    if let Ok(v) = <usize>::try_from(v) {{\n                        state.{field_name} = v;\n                    }}\n                }}"
        ),
        _ => String::new(),
    }
}

fn collect_member_attributes(
    fields: &[StateField],
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

fn generate_attributes_of_body(fields: &[StateField], methods: &[ScriptMethod]) -> String {
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

fn generate_members_with_body(fields: &[StateField], methods: &[ScriptMethod]) -> String {
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

fn generate_has_attribute_body(fields: &[StateField], methods: &[ScriptMethod]) -> String {
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

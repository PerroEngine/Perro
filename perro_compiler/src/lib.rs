use perro_io::brk::build_brk;
use perro_io::walkdir::walk_dir;
use perro_project::{ensure_source_overrides, load_project_toml};
use std::{
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
            let dst = scripts_src.join(rel);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            let source = fs::read_to_string(path)?;
            let transformed = transpile_frontend_script(&source);
            fs::write(&dst, transformed)?;
            copied.push(rel.to_string_lossy().replace('\\', "/"));
            Ok(())
        })?;
    }

    copied.sort();
    write_scripts_lib(&scripts_src, &copied)?;
    Ok(copied)
}

pub fn compile_scripts(project_root: &Path) -> Result<Vec<String>, CompilerError> {
    ensure_source_overrides(project_root)?;
    let copied = sync_scripts(project_root)?;
    let scripts_crate = project_root.join(".perro").join("scripts");
    let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target");

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

pub fn compile_project_bundle(project_root: &Path) -> Result<(), CompilerError> {
    ensure_source_overrides(project_root)?;
    let _ = compile_scripts(project_root)?;
    perro_static_pipeline::generate_static_scenes(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("scene static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_materials(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("material static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_meshes(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("mesh static generation failed: {err}"))
    })?;
    perro_static_pipeline::generate_static_textures(project_root).map_err(|err| {
        CompilerError::SceneParse(format!("texture static generation failed: {err}"))
    })?;
    perro_static_pipeline::write_static_mod_rs(project_root)
        .map_err(|err| CompilerError::SceneParse(format!("static mod generation failed: {err}")))?;
    generate_embedded_main(project_root)?;
    generate_assets_brk(project_root)?;
    build_project_crate(project_root)?;
    Ok(())
}

fn generate_assets_brk(project_root: &Path) -> Result<(), CompilerError> {
    let embedded_dir = project_root.join(".perro").join("project").join("embedded");
    fs::create_dir_all(&embedded_dir)?;
    let output = embedded_dir.join("assets.brk");
    let res_dir = project_root.join("res");
    build_brk(&output, &res_dir, project_root)?;
    Ok(())
}

fn build_project_crate(project_root: &Path) -> Result<(), CompilerError> {
    let project_crate = project_root.join(".perro").join("project");
    let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target");
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(project_crate)
        .status()?;

    if !status.success() {
        return Err(CompilerError::CargoFailed(status.code().unwrap_or(-1)));
    }
    Ok(())
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

    let main_src = format!(
        "#[path = \"static/mod.rs\"]\n\
mod static_assets;\n\n\
static ASSETS_BRK: &[u8] = include_bytes!(\"../embedded/assets.brk\");\n\n\
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
    let root = project_root();\n\
    perro_app::entry::run_static_embedded_project(\n\
        &root,\n\
        \"{default_name}\",\n\
        \"{name}\",\n\
        \"{main_scene}\",\n\
        \"{icon}\",\n\
        {w},\n\
        {h},\n\
        ASSETS_BRK,\n\
        static_assets::scenes::lookup_scene,\n\
        static_assets::materials::lookup_material,\n\
        static_assets::meshes::lookup_mesh,\n\
        static_assets::textures::lookup_texture,\n\
        Some(scripts::SCRIPT_REGISTRY),\n\
    ).expect(\"failed to run embedded static project\");\n\
}}\n",
        default_name = cfg.name,
        name = escape_str(&cfg.name),
        main_scene = escape_str(&cfg.main_scene),
        icon = escape_str(&cfg.icon),
        w = cfg.virtual_width,
        h = cfg.virtual_height
    );
    fs::write(project_src.join("main.rs"), main_src)?;
    Ok(())
}

fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_scripts_lib(scripts_src: &Path, copied: &[String]) -> Result<(), CompilerError> {
    let mut out = String::new();
    out.push_str("#![allow(unused_imports, unused_variables, dead_code)]\n");
    out.push_str("// AUTO-GENERATED by Perro Compiler. Do not edit by hand.\n\n");
    out.push_str("use perro_runtime::Runtime;\n");
    out.push_str("use perro_scripting::ScriptConstructor;\n\n");

    for rel in copied {
        let module = module_name_from_rel(rel);
        out.push_str(&format!("#[path = \"{rel}\"]\n"));
        out.push_str(&format!("pub mod {module};\n\n"));
    }

    out.push_str("pub static SCRIPT_REGISTRY: &[(&str, ScriptConstructor<Runtime>)] = &[\n");
    for rel in copied {
        let module = module_name_from_rel(rel);
        out.push_str(&format!(
            "    (\"res://{rel}\", {module}::perro_create_script as ScriptConstructor<Runtime>),\n"
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
    perro_modules::file::set_project_root_disk(root, name);\n\
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
    ctor_out: *mut ScriptConstructor<Runtime>,\n\
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

    let script_ty = parse_marked_struct_name(&stripped_source, "@Script")
        .or_else(|| parse_named_struct(&stripped_source, "Script"))
        .or_else(|| first_non_state_struct(&stripped_source, &state_ty))
        .unwrap_or_else(|| "Script".to_string());
    let script_ctor_expr = if is_unit_struct(&stripped_source, &script_ty) {
        script_ty.clone()
    } else {
        format!("<{script_ty} as Default>::default()")
    };

    let has_init = source.contains("fn on_init(");
    let has_start = source.contains("fn on_start(");
    let has_update = source.contains("fn on_update(");
    let has_fixed = source.contains("fn on_fixed_update(");
    let has_removed = source.contains("fn on_removed(");
    let user_methods = parse_inherent_methods(&stripped_source, &script_ty);
    let state_fields = parse_struct_fields(&source, &state_ty);
    let state_default_impl = generate_state_default_impl(
        &source,
        &state_ty,
        &state_fields,
    );
    let source_with_state_default =
        inject_default_impl_after_state_struct(&stripped_source, &state_ty, &state_default_impl);
    let exposed_fields = supported_fields(&state_fields);

    let mut flags = String::from("ScriptFlags::NONE");
    if has_init {
        flags.push_str(" | ScriptFlags::HAS_INIT");
    }
    if has_start {
        flags.push_str(" | ScriptFlags::HAS_START");
    }
    if has_update {
        flags.push_str(" | ScriptFlags::HAS_UPDATE");
    }
    if has_fixed {
        flags.push_str(" | ScriptFlags::HAS_FIXED_UPDATE");
    }
    if has_removed {
        flags.push_str(" | ScriptFlags::HAS_REMOVED");
    }

    let member_consts = generate_member_consts(&exposed_fields, &user_methods);
    let get_var_body = generate_get_var_body(&state_ty, &exposed_fields);
    let set_var_body = generate_set_var_body(&state_ty, &exposed_fields);
    let call_method_body = generate_call_method_body(&user_methods);
    let attr_of_body = generate_attributes_of_body(&exposed_fields);
    let members_with_body = generate_members_with_body(&exposed_fields);
    let has_attr_body = generate_has_attribute_body(&exposed_fields);

    format!(
        r#"{source_with_state_default}

// ---- AUTO-GENERATED by Perro Compiler ----
{member_consts}

impl<R: RuntimeAPI + ?Sized> ScriptBehavior<R> for {script_ty} {{
    fn script_flags(&self) -> ScriptFlags {{
        ScriptFlags::new({flags})
    }}

    fn create_state(&self) -> Box<dyn std::any::Any> {{
        Box::new(<{state_ty} as Default>::default())
    }}

    fn get_var(&self, state: &dyn std::any::Any, var_id: ScriptMemberID) -> Variant {{
{get_var_body}
    }}

    fn set_var(&self, state: &mut dyn std::any::Any, var_id: ScriptMemberID, value: &Variant) {{
{set_var_body}
    }}

    fn call_method(
        &self,
        method_id: ScriptMemberID,
        ctx: &mut RuntimeContext<'_, R>,
        self_id: NodeID,
        params: &[Variant],
    ) -> Variant {{
{call_method_body}
    }}

    fn attributes_of(&self, member: &str) -> &'static [&'static str] {{
{attr_of_body}
    }}

    fn members_with(&self, attribute: &str) -> &'static [&'static str] {{
{members_with_body}
    }}

    fn has_attribute(&self, member: &str, attribute: &str) -> bool {{
{has_attr_body}
    }}
}}

#[allow(improper_ctypes_definitions)]
pub extern "C" fn perro_create_script() -> *mut dyn ScriptBehavior<perro_runtime::Runtime> {{
    Box::into_raw(Box::new({script_ctor_expr}))
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
        let t = line.trim();
        if t == "#[state]" || t.starts_with("#[default") {
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

fn parse_named_struct(source: &str, expected: &str) -> Option<String> {
    for line in source.lines() {
        if let Some(name) = parse_struct_name(line.trim()) {
            if name == expected {
                return Some(name);
            }
        }
    }
    None
}

fn first_non_state_struct(source: &str, state_ty: &str) -> Option<String> {
    for line in source.lines() {
        if let Some(name) = parse_struct_name(line.trim()) {
            if name != state_ty {
                return Some(name);
            }
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
    default_expr: Option<String>,
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
    let mut pending_default_expr: Option<String> = None;
    let mut depth = 0_i32;
    let mut opened = false;
    let mut i = start;

    while i < lines.len() {
        let line = strip_line_comment(lines[i]);
        if !opened {
            if let Some(pos) = line.find('{') {
                opened = true;
                depth = 1;
                let rest = &line[pos + 1..];
                if depth == 1 {
                    if let Some(field) = parse_field_line(rest, pending_default_expr.take()) {
                        fields.push(field);
                    }
                }
                depth += brace_delta(rest);
                if depth <= 0 {
                    break;
                }
            }
            i += 1;
            continue;
        }

        if depth == 1 {
            if let Some(expr) = parse_default_attr_line(line) {
                pending_default_expr = Some(expr);
                i += 1;
                continue;
            }
            if let Some(field) = parse_field_line(line, pending_default_expr.take()) {
                fields.push(field);
            }
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

fn parse_field_line(line: &str, default_expr: Option<String>) -> Option<StateField> {
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
        default_expr,
    })
}

fn parse_default_attr_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("#[default") {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("#[default") {
        if let Some(eq_rest) = rest.trim().strip_prefix('=') {
            let expr = eq_rest.trim().trim_end_matches(']').trim().to_string();
            return (!expr.is_empty()).then_some(expr);
        }
    }

    if let Some(inner) = trimmed
        .strip_prefix("#[default(")
        .and_then(|v| v.strip_suffix(")]"))
    {
        let expr = inner.trim().to_string();
        return (!expr.is_empty()).then_some(expr);
    }

    None
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
        "NodeID" | "perro_ids::NodeID" => Some(FieldKind::NodeID),
        "TextureID" | "perro_ids::TextureID" => Some(FieldKind::TextureID),
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
            "const {const_name}: ScriptMemberID = smid!(\"{}\");\n",
            field.name
        ));
    }
    for method in methods {
        let const_name = method_const_name(&method.name);
        out.push_str(&format!(
            "const {const_name}: ScriptMemberID = smid!(\"{}\");\n",
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
        return "        let _ = (method_id, ctx, self_id, params);\n        Variant::Null".to_string();
    }

    let mut out = String::new();
    out.push_str("        match method_id {\n");
    for method in methods {
        let const_name = method_const_name(&method.name);
        let arity_const_name = method_arity_const_name(&method.name);
        let call = if method.takes_raw_params {
            format!("self.{}(ctx, self_id, params)", method.name)
        } else if method.params.is_empty() {
            format!("self.{}(ctx, self_id)", method.name)
        } else {
            let args = method
                .params
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("self.{}(ctx, self_id, {args})", method.name)
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
                "            {const_name} => {{\n                let _ = (ctx, self_id, params);\n                Variant::Null\n            }}\n"
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
        i += 1;

        while i < lines.len() {
            let l = strip_line_comment(lines[i]);
            if opened && depth == 1 {
                if let Some(method) = parse_script_method_signature(l.trim()) {
                    methods.push(method);
                }
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
    inner == attribute_name
        || inner
            .strip_prefix(attribute_name)
            .is_some_and(|rest| rest.starts_with('('))
}

fn generate_state_default_impl(source: &str, state_ty: &str, state_fields: &[StateField]) -> String {
    if has_explicit_default_impl(source, state_ty) {
        return String::new();
    }

    let has_inline_defaults = state_fields.iter().any(|f| f.default_expr.is_some());
    if state_struct_has_derive_default(source, state_ty) && !has_inline_defaults {
        return String::new();
    }

    let mut out = String::new();
    out.push_str(&format!("impl Default for {state_ty} {{\n"));
    out.push_str("    fn default() -> Self {\n");
    out.push_str("        Self {\n");
    for field in state_fields {
        if let Some(expr) = &field.default_expr {
            out.push_str(&format!("            {}: {},\n", field.name, expr));
        } else {
            out.push_str(&format!(
                "            {}: Default::default(),\n",
                field.name
            ));
        }
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

fn inject_default_impl_after_state_struct(
    source: &str,
    state_ty: &str,
    state_default_impl: &str,
) -> String {
    if state_default_impl.trim().is_empty() {
        return source.to_string();
    }

    let lines: Vec<&str> = source.lines().collect();
    let mut start = None;
    for (i, line) in lines.iter().enumerate() {
        if parse_struct_name(line.trim()) == Some(state_ty.to_string()) {
            start = Some(i);
            break;
        }
    }
    let Some(start_idx) = start else {
        return format!("{source}\n{state_default_impl}\n");
    };

    let mut end_idx = start_idx;
    let mut opened = false;
    let mut depth = 0_i32;
    for (i, line) in lines.iter().enumerate().skip(start_idx) {
        let l = strip_line_comment(line);
        if !opened {
            if l.contains('{') {
                opened = true;
            } else if l.trim_end().ends_with(';') {
                end_idx = i;
                break;
            }
        }
        if opened {
            depth += brace_delta(l);
            if depth <= 0 {
                end_idx = i;
                break;
            }
        }
    }

    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        out.push_str(line);
        out.push('\n');
        if i == end_idx {
            out.push('\n');
            out.push_str(state_default_impl);
            out.push('\n');
        }
    }
    out
}

fn has_explicit_default_impl(source: &str, state_ty: &str) -> bool {
    source.contains(&format!("impl Default for {state_ty}"))
        || source.contains(&format!("impl std::default::Default for {state_ty}"))
}

fn state_struct_has_derive_default(source: &str, state_ty: &str) -> bool {
    let lines: Vec<&str> = source.lines().collect();
    let mut struct_line = None;
    for (i, line) in lines.iter().enumerate() {
        if parse_struct_name(line.trim()) == Some(state_ty.to_string()) {
            struct_line = Some(i);
            break;
        }
    }
    let Some(mut i) = struct_line else {
        return false;
    };
    if i == 0 {
        return false;
    }
    i -= 1;
    loop {
        let t = lines[i].trim();
        if t.is_empty() {
            if i == 0 {
                break;
            }
            i -= 1;
            continue;
        }
        if t.starts_with("#[derive(") && t.contains("Default") {
            return true;
        }
        if t.starts_with("#[") || t.starts_with("///") || t.starts_with("//") {
            if i == 0 {
                break;
            }
            i -= 1;
            continue;
        }
        break;
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
        if let Some((target_name, body)) = parse_methods_macro_inner(inner) {
            if target_name == struct_name {
                methods.extend(parse_methods_block_signatures(body));
            }
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

    for line in body.lines() {
        let l = strip_line_comment(line);
        if depth == 0 {
            if let Some(method) = parse_script_method_signature(l.trim()) {
                methods.push(method);
            }
        }
        depth += brace_delta(l);
    }

    methods
}

fn parse_script_method_signature(line: &str) -> Option<ScriptMethod> {
    let line = line.trim_start_matches("pub ").trim_start();
    if !line.starts_with("fn ") {
        return None;
    }

    if !line.contains("&self") || !line.contains("ctx") || !line.contains("self_id") {
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

        for raw in split_top_level_commas(params_sig) {
            let token = raw.trim();
            if token.is_empty()
                || token == "&self"
                || token == "self"
                || token == "&mut self"
                || token == "mut self"
            {
                continue;
            }

            let Some((name_part, ty_part)) = token.split_once(':') else {
                continue;
            };
            let param_name = name_part.trim();
            let param_ty = ty_part.trim();
            if param_name == "ctx" || param_name == "self_id" {
                continue;
            }

            let normalized = normalize_type(param_ty);
            let is_raw_params = param_name == "params"
                && (normalized == "&[Variant]" || normalized == "&[perro_variant::Variant]");
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

        let returns_variant =
            line.contains("-> Variant") || line.contains("->perro_variant::Variant");
        Some(ScriptMethod {
            name,
            takes_raw_params,
            params,
            returns_variant,
        })
    }
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
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::I8(v))) => *v, _ => return Variant::Null }};"
        ),
        "i16" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::I16(v))) => *v, _ => return Variant::Null }};"
        ),
        "i32" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::I32(v))) => *v, _ => return Variant::Null }};"
        ),
        "i64" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::I64(v))) => *v, _ => return Variant::Null }};"
        ),
        "i128" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::I128(v))) => *v, _ => return Variant::Null }};"
        ),
        "isize" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::I64(v))) => match isize::try_from(*v) {{ Ok(v) => v, Err(_) => return Variant::Null }}, _ => return Variant::Null }};"
        ),
        "u8" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::U8(v))) => *v, _ => return Variant::Null }};"
        ),
        "u16" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::U16(v))) => *v, _ => return Variant::Null }};"
        ),
        "u32" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::U32(v))) => *v, _ => return Variant::Null }};"
        ),
        "u64" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::U64(v))) => *v, _ => return Variant::Null }};"
        ),
        "u128" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::U128(v))) => *v, _ => return Variant::Null }};"
        ),
        "usize" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::U64(v))) => match usize::try_from(*v) {{ Ok(v) => v, Err(_) => return Variant::Null }}, _ => return Variant::Null }};"
        ),
        "f32" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::F32(v))) => *v, _ => return Variant::Null }};"
        ),
        "f64" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::Number(perro_variant::Number::F64(v))) => *v, _ => return Variant::Null }};"
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
        "NodeID" | "perro_ids::NodeID" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::NodeID(v)) => *v, _ => return Variant::Null }};"
        ),
        "TextureID" | "perro_ids::TextureID" => format!(
            "let {name} = match params.get({index}) {{ Some(Variant::TextureID(v)) => *v, _ => return Variant::Null }};"
        ),
        "Variant" | "perro_variant::Variant" => format!(
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
    out.push_str("        match var_id {\n");
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
    out.push_str("        match var_id {\n");
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
                "if let Some(v) = value.as_node().or_else(|| value.as_number().and_then(|n| n.as_i64_lossy()).and_then(|n| u64::try_from(n).ok()).map(perro_ids::NodeID::from_u64)).or_else(|| value.as_str().and_then(|s| perro_ids::NodeID::parse_str(s).ok())) {{\n                    state.{} = v;\n                }}",
                field.name
            ),
            FieldKind::TextureID => format!(
                "if let Some(v) = value.as_texture().or_else(|| value.as_number().and_then(|n| n.as_i64_lossy()).and_then(|n| u64::try_from(n).ok()).map(perro_ids::TextureID::from_u64)).or_else(|| value.as_str().and_then(|s| perro_ids::TextureID::parse_str(s).ok())) {{\n                    state.{} = v;\n                }}",
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
            "if let Some(perro_variant::Number::I8(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "i16" => format!(
            "if let Some(perro_variant::Number::I16(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "i32" => format!(
            "if let Some(perro_variant::Number::I32(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "i64" => format!(
            "if let Some(perro_variant::Number::I64(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "i128" => format!(
            "if let Some(perro_variant::Number::I128(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        // Number has no isize variant; accept only I64 then checked cast.
        "isize" => format!(
            "if let Some(perro_variant::Number::I64(v)) = value.as_number() {{\n                    if let Ok(v) = <isize>::try_from(v) {{\n                        state.{field_name} = v;\n                    }}\n                }}"
        ),
        _ => String::new(),
    }
}

fn exact_unsigned_assign_block(ty: &str, field_name: &str) -> String {
    match ty {
        "u8" => format!(
            "if let Some(perro_variant::Number::U8(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "u16" => format!(
            "if let Some(perro_variant::Number::U16(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "u32" => format!(
            "if let Some(perro_variant::Number::U32(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "u64" => format!(
            "if let Some(perro_variant::Number::U64(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        "u128" => format!(
            "if let Some(perro_variant::Number::U128(v)) = value.as_number() {{\n                    state.{field_name} = v;\n                }}"
        ),
        // Number has no usize variant; accept only U64 then checked cast.
        "usize" => format!(
            "if let Some(perro_variant::Number::U64(v)) = value.as_number() {{\n                    if let Ok(v) = <usize>::try_from(v) {{\n                        state.{field_name} = v;\n                    }}\n                }}"
        ),
        _ => String::new(),
    }
}

fn generate_attributes_of_body(fields: &[StateField]) -> String {
    if fields.is_empty() {
        return "        &[]".to_string();
    }
    let mut out = String::new();
    out.push_str("        match member {\n");
    for field in fields {
        out.push_str(&format!(
            "            \"{}\" => &[\"export\"],\n",
            field.name
        ));
    }
    out.push_str("            _ => &[],\n");
    out.push_str("        }");
    out
}

fn generate_members_with_body(fields: &[StateField]) -> String {
    if fields.is_empty() {
        return "        &[]".to_string();
    }
    let mut out = String::new();
    out.push_str("        if attribute == \"export\" {\n");
    out.push_str("            return &[\n");
    for field in fields {
        out.push_str(&format!("                \"{}\",\n", field.name));
    }
    out.push_str("            ];\n");
    out.push_str("        }\n");
    out.push_str("        &[]");
    out
}

fn generate_has_attribute_body(fields: &[StateField]) -> String {
    if fields.is_empty() {
        return "        false".to_string();
    }
    let mut out = String::new();
    out.push_str("        if attribute != \"export\" {\n");
    out.push_str("            return false;\n");
    out.push_str("        }\n");
    out.push_str("        matches!(member, ");
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            out.push_str(" | ");
        }
        out.push_str(&format!("\"{}\"", field.name));
    }
    out.push(')');
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

#[allow(dead_code)]
fn rel_to_path(base: &Path, rel: &str) -> PathBuf {
    base.join(rel.replace('/', "\\"))
}

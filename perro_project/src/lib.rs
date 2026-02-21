use std::{
    collections::BTreeSet,
    fmt::{Display, Formatter},
    fs,
    path::{Path, PathBuf},
};
use toml::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticProjectConfig {
    pub name: &'static str,
    pub main_scene: &'static str,
    pub icon: &'static str,
    pub virtual_width: u32,
    pub virtual_height: u32,
}

impl StaticProjectConfig {
    pub const fn new(
        name: &'static str,
        main_scene: &'static str,
        icon: &'static str,
        virtual_width: u32,
        virtual_height: u32,
    ) -> Self {
        Self {
            name,
            main_scene,
            icon,
            virtual_width,
            virtual_height,
        }
    }

    pub fn to_runtime(self) -> ProjectConfig {
        ProjectConfig {
            name: self.name.to_string(),
            main_scene: self.main_scene.to_string(),
            icon: self.icon.to_string(),
            virtual_width: self.virtual_width,
            virtual_height: self.virtual_height,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfig {
    pub name: String,
    pub main_scene: String,
    pub icon: String,
    pub virtual_width: u32,
    pub virtual_height: u32,
}

impl ProjectConfig {
    pub fn default_for_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            main_scene: "res://main.scn".to_string(),
            icon: "res://icon.png".to_string(),
            virtual_width: 1920,
            virtual_height: 1080,
        }
    }
}

#[derive(Debug)]
pub enum ProjectError {
    Io(std::io::Error),
    ParseToml(toml::de::Error),
    MissingField(&'static str),
    InvalidField(&'static str, String),
    AlreadyExists(PathBuf),
}

impl Display for ProjectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::ParseToml(err) => write!(f, "{err}"),
            Self::MissingField(field) => write!(f, "missing required field `{field}`"),
            Self::InvalidField(field, reason) => write!(f, "invalid field `{field}`: {reason}"),
            Self::AlreadyExists(path) => {
                write!(f, "project directory already exists: {}", path.display())
            }
        }
    }
}

impl std::error::Error for ProjectError {}

impl From<std::io::Error> for ProjectError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for ProjectError {
    fn from(value: toml::de::Error) -> Self {
        Self::ParseToml(value)
    }
}

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
    let perro_dir = root.join(".perro");
    let project_crate = perro_dir.join("project");
    let scripts_crate = perro_dir.join("scripts");
    let project_src = project_crate.join("src");
    let project_static_src = project_src.join("static");
    let project_embedded = project_crate.join("embedded");
    let scripts_src = scripts_crate.join("src");

    fs::create_dir_all(&res_dir)?;
    fs::create_dir_all(&res_scripts_dir)?;
    fs::create_dir_all(&project_src)?;
    fs::create_dir_all(&project_static_src)?;
    fs::create_dir_all(&project_embedded)?;
    fs::create_dir_all(&scripts_src)?;

    let crate_name = crate_name_from_project_name(project_name);
    write_if_missing(root.join(".gitignore"), &default_gitignore())?;
    write_if_missing(res_dir.join("main.scn"), &default_main_scene())?;
    write_if_missing(
        res_scripts_dir.join("script.rs"),
        &default_script_example_rs(),
    )?;
    write_if_missing(
        project_crate.join("Cargo.toml"),
        &default_project_crate_toml(&crate_name),
    )?;
    write_if_missing(
        scripts_crate.join("Cargo.toml"),
        &default_scripts_crate_toml(),
    )?;
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
        project_static_src.join("textures.rs"),
        &default_static_textures_rs(),
    )?;
    write_if_missing(
        project_static_src.join("meshes.rs"),
        &default_static_meshes_rs(),
    )?;
    write_if_missing(project_embedded.join("assets.brk"), "")?;
    write_if_missing(scripts_src.join("lib.rs"), &default_scripts_lib_rs())?;

    Ok(())
}

pub fn ensure_project_toml(root: &Path, default_name: &str) -> std::io::Result<()> {
    let project_toml = root.join("project.toml");
    if project_toml.exists() {
        return Ok(());
    }
    fs::write(project_toml, default_project_toml(default_name))
}

pub fn default_project_toml(name: &str) -> String {
    format!(
        r#"[project]
name = "{name}"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_resolution = "1920x1080"
"#
    )
}

pub fn load_project_toml(root: &Path) -> Result<ProjectConfig, ProjectError> {
    let project_toml = fs::read_to_string(root.join("project.toml"))?;
    parse_project_toml(&project_toml)
}

pub fn parse_project_toml(contents: &str) -> Result<ProjectConfig, ProjectError> {
    let value: Value = contents.parse::<Value>()?;
    let project_table = value
        .get("project")
        .and_then(Value::as_table)
        .ok_or(ProjectError::MissingField("project"))?;

    let graphics_table = value
        .get("graphics")
        .and_then(Value::as_table)
        .ok_or(ProjectError::MissingField("graphics"))?;

    let name = project_table
        .get("name")
        .and_then(Value::as_str)
        .ok_or(ProjectError::MissingField("project.name"))?
        .to_string();

    let main_scene = project_table
        .get("main_scene")
        .and_then(Value::as_str)
        .ok_or(ProjectError::MissingField("project.main_scene"))?
        .to_string();
    validate_res_path("project.main_scene", &main_scene)?;

    let icon = project_table
        .get("icon")
        .and_then(Value::as_str)
        .unwrap_or("res://icon.png")
        .to_string();
    validate_res_path("project.icon", &icon)?;

    let (virtual_width, virtual_height) = if let Some(raw) = graphics_table
        .get("virtual_resolution")
        .and_then(Value::as_str)
    {
        parse_resolution(raw)?
    } else {
        let w = graphics_table
            .get("virtual_width")
            .and_then(Value::as_integer)
            .ok_or(ProjectError::MissingField("graphics.virtual_width"))?;
        let h = graphics_table
            .get("virtual_height")
            .and_then(Value::as_integer)
            .ok_or(ProjectError::MissingField("graphics.virtual_height"))?;
        (
            u32::try_from(w).map_err(|_| {
                ProjectError::InvalidField(
                    "graphics.virtual_width",
                    "must be a positive integer".to_string(),
                )
            })?,
            u32::try_from(h).map_err(|_| {
                ProjectError::InvalidField(
                    "graphics.virtual_height",
                    "must be a positive integer".to_string(),
                )
            })?,
        )
    };

    if virtual_width == 0 || virtual_height == 0 {
        return Err(ProjectError::InvalidField(
            "graphics.virtual_resolution",
            "resolution values must be greater than 0".to_string(),
        ));
    }

    Ok(ProjectConfig {
        name,
        main_scene,
        icon,
        virtual_width,
        virtual_height,
    })
}

fn validate_res_path(field: &'static str, path: &str) -> Result<(), ProjectError> {
    if path.starts_with("res://") {
        return Ok(());
    }
    Err(ProjectError::InvalidField(
        field,
        "must start with `res://`".to_string(),
    ))
}

fn parse_resolution(raw: &str) -> Result<(u32, u32), ProjectError> {
    let raw = raw.trim().to_ascii_lowercase();
    let (w, h) = raw.split_once('x').ok_or(ProjectError::InvalidField(
        "graphics.virtual_resolution",
        "expected format `WIDTHxHEIGHT`, for example `1920x1080`".to_string(),
    ))?;

    let width = w.parse::<u32>().map_err(|_| {
        ProjectError::InvalidField(
            "graphics.virtual_resolution",
            "invalid width component".to_string(),
        )
    })?;
    let height = h.parse::<u32>().map_err(|_| {
        ProjectError::InvalidField(
            "graphics.virtual_resolution",
            "invalid height component".to_string(),
        )
    })?;

    Ok((width, height))
}

fn write_if_missing(path: PathBuf, contents: &str) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, contents)
}

fn crate_name_from_project_name(project_name: &str) -> String {
    let mut out = String::with_capacity(project_name.len() + 8);
    for c in project_name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    let mut normalized = if trimmed.is_empty() {
        "perro_project".to_string()
    } else {
        trimmed.to_string()
    };
    if normalized
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
    {
        normalized.insert(0, '_');
    }
    normalized
}

fn default_main_scene() -> String {
    r#"@root = main

[main]

[Node2D]
    position = (0, 0)
[/Node2D]
[/main]
"#
    .to_string()
}

fn default_script_example_rs() -> String {
    r#"use perro_context::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;

// Script is authored against a node type. This default template uses Node2D.
type SelfNodeType = Node2D;

// State is data-only. Keeping state separate from behavior makes cross-calls memory safe
// and helps the runtime handle recursion/re-entrancy without borrowing issues.
#[state]
pub struct ExampleState {
    #[default = 5]
    count: i32,
}

const SPEED: f32 = 5.0;

lifecycle!({
    // Lifecycle methods are engine entry points. They are called by the runtime.
    // `ctx` is the main interface into the engine core to access runtime data/scripts and nodes.
    // `self_id` is the NodeID handle of the node this script is attached to.
    fn on_init(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        // local-state access using with_state!/with_state_mut!.
        let count = with_state!(ctx, ExampleState, self_id, |state| {
            state.count
        }).unwrap_or_default();
        log_info!(count);
    }

    fn on_update(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        let dt = delta_time!(ctx);
        // Regular Rust method calls are for internal methods.
        self.bump_count(ctx, self_id);

        // Local node mutation: use context + expected node type + node id + closure.
        // Here we mutate the attached node via `self_id`.
        mutate_node!(ctx, SelfNodeType, self_id, |node| {
            node.position.x += dt * SPEED;
        });

        // You can also pass another NodeID with another node type if that id maps
        // to that type at runtime.
        // Example:
        // mutate_node!(ctx, MeshInstance3D, enemy_id, |mesh| { mesh.scale.x += 1.0; });
        // If unsure, check node type first (for example with read_meta! + match).

        // call_method! can invoke methods through the script interface by member id.
        // Here we call our own script through self_id for demonstration.
        call_method!(ctx, self_id, smid!("test"), params![7123_i32, "bodsasb"]);
        set_var!(ctx, self_id, smid!("count"), 77_i32.into());
        let remote_count = get_var!(ctx, self_id, smid!("count"));
        log_info!(remote_count);
        // For local/internal behavior and local state, prefer direct methods plus
        // with_state!/with_state_mut! (for example self.bump_count(...)).
        // That is simpler and more performant than call_method!/get_var!/set_var!.

        // Typical NodeID lookup is runtime-dependent. NodeID is a handle, not the node value.
        // if let Some(enemy_id) = find_node!(ctx, "enemy") {
        //     // Cross-script call on another script instance:
        //     call_method!(ctx, enemy_id, smid!("test"), params![1_i32, "ping"]);
        //
        //     // Mutate enemy node directly if you know its runtime node type:
        //     mutate_node!(ctx, MeshInstance3D, enemy_id, |enemy| {
        //         enemy.scale.x += 0.1;
        //     });
        //
        //     // If type is uncertain, check metadata/type first, then branch/match.
        // }
    }

    fn on_fixed_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
});

methods!({
    // methods! defines callable behavior methods (local or cross-script via call_method!)...
    fn bump_count(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        //  Use `with_state_mut!` for mutable access to state
        with_state_mut!(ctx, ExampleState, self_id, |state| {
            state.count += 1;
        });
    }

    fn test(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID, param1: i32, msg: &str) {
        log_info!(param1);
        log_info!(msg);
        self.bump_count(ctx, self_id);
    }
});
"#
    .to_string()
}

fn default_gitignore() -> String {
    r#"target/
.perro/project/embedded/
.perro/project/src/static/
.perro/scripts/src/
"#
    .to_string()
}

fn default_project_crate_toml(crate_name: &str) -> String {
    format!(
        r#"[workspace]

[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2024"

[dependencies]
perro_app = "0.1.0"
perro_ids = "0.1.0"
perro_scripting = "0.1.0"
perro_context = "0.1.0"
perro_core = "0.1.0"
perro_scene = "0.1.0"
perro_render_bridge = "0.1.0"
scripts = {{ path = "../scripts" }}

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
incremental = true
debug = false
debug-assertions = false
overflow-checks = false
 "#
    )
}

fn default_scripts_crate_toml() -> String {
    format!(
        r#"[workspace]

[package]
name = "scripts"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
perro_ids = "0.1.0"
perro_scripting = "0.1.0"
perro_context = "0.1.0"
perro_core = "0.1.0"
perro_modules = "0.1.0"
perro_variant = "0.1.0"
perro_runtime = "0.1.0"

[profile.dev]
opt-level = 0
incremental = true
codegen-units = 256
lto = false
debug = false
strip = "none"
overflow-checks = false
panic = "abort"

[profile.dev.package."*"]
opt-level = 2
incremental = true
codegen-units = 64
debug = false
strip = "none"
overflow-checks = false
"#
    )
}

fn rel_path(from: &Path, to: &Path) -> String {
    let from_components: Vec<_> = from.components().collect();
    let to_components: Vec<_> = to.components().collect();
    let common = from_components
        .iter()
        .zip(to_components.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut out = PathBuf::new();
    for _ in common..from_components.len() {
        out.push("..");
    }
    for c in &to_components[common..] {
        out.push(c.as_os_str());
    }
    out.to_string_lossy().replace('\\', "/")
}

fn default_project_main_rs(project_name: &str) -> String {
    r#"#[path = "static/mod.rs"]
mod static_assets;

static ASSETS_BRK: &[u8] = include_bytes!("../embedded/assets.brk");

fn project_root() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for dir in exe_dir.ancestors() {
                if dir.join("project.toml").exists() {
                    return dir.to_path_buf();
                }
            }
            return exe_dir.to_path_buf();
        }
    }
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    if root.join("project.toml").exists() {
        return root.canonicalize().unwrap_or(root);
    }
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

fn main() {
    let root = project_root();
    perro_app::entry::run_static_embedded_project(
        &root,
        "__PROJECT_NAME__",
        "__PROJECT_NAME__",
        "res://main.scn",
        "res://icon.png",
        1920,
        1080,
        ASSETS_BRK,
        static_assets::scenes::lookup_scene,
        static_assets::materials::lookup_material,
        static_assets::meshes::lookup_mesh,
        static_assets::textures::lookup_texture,
        Some(scripts::SCRIPT_REGISTRY),
    ).expect("failed to run embedded static project");
}
"#
    .replace("__PROJECT_NAME__", project_name)
}

fn default_static_mod_rs() -> String {
    "pub mod scenes;\npub mod materials;\npub mod meshes;\npub mod textures;\n".to_string()
}

fn default_static_scenes_rs() -> String {
    r#"use perro_scene::StaticScene;

pub fn lookup_scene(_path: &str) -> Option<&'static StaticScene> {
    None
}
"#
    .to_string()
}

fn default_static_materials_rs() -> String {
    r#"use perro_render_bridge::Material3D;

pub fn lookup_material(_path: &str) -> Option<&'static Material3D> {
    None
}
"#
    .to_string()
}

fn default_static_textures_rs() -> String {
    r#"pub fn lookup_texture(_path: &str) -> Option<&'static [u8]> {
    None
}
"#
    .to_string()
}

fn default_static_meshes_rs() -> String {
    r#"#![allow(dead_code)]

pub fn lookup_mesh(_path: &str) -> Option<&'static [u8]> {
    None
}
"#
    .to_string()
}

fn default_scripts_lib_rs() -> String {
    r#"use perro_runtime::Runtime;
use perro_scripting::ScriptConstructor;

pub static SCRIPT_REGISTRY: &[(&str, ScriptConstructor<Runtime>)] = &[];

#[unsafe(no_mangle)]
pub extern "C" fn perro_scripts_init() {}
"#
    .to_string()
}

pub fn ensure_source_overrides(project_root: &Path) -> std::io::Result<()> {
    let project_manifest = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let scripts_manifest = project_root
        .join(".perro")
        .join("scripts")
        .join("Cargo.toml");
    ensure_scripts_manifest_deps(&scripts_manifest)?;
    ensure_patch_block_in_manifest(&project_manifest)?;
    ensure_patch_block_in_manifest(&scripts_manifest)?;
    Ok(())
}

fn ensure_scripts_manifest_deps(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let deps = root
        .entry("dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(deps_table) = deps.as_table_mut() else {
        return Ok(());
    };

    if deps_table.contains_key("perro_modules") {
        return Ok(());
    }

    deps_table.insert(
        "perro_modules".to_string(),
        Value::String("0.1.0".to_string()),
    );

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_patch_block_in_manifest(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let src = fs::read_to_string(path)?;
    let overrides = source_overrides_block_for_manifest(path, &src);
    let stripped = strip_patch_crates_io(&src);
    let mut out = stripped.trim_end().to_string();
    if !overrides.is_empty() {
        out.push_str("\n\n");
        out.push_str(&overrides);
        out.push('\n');
    }
    fs::write(path, out)
}

fn strip_patch_crates_io(src: &str) -> String {
    let mut out = String::new();
    let mut in_patch = false;

    for line in src.lines() {
        let trimmed = line.trim();
        let is_header = trimmed.starts_with('[') && trimmed.ends_with(']');
        if is_header && trimmed == "[patch.crates-io]" {
            in_patch = true;
            continue;
        }
        if in_patch && is_header {
            in_patch = false;
        }
        if !in_patch {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn source_overrides_block_for_manifest(manifest_path: &Path, manifest_src: &str) -> String {
    let engine_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."));
    let manifest_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .canonicalize()
        .unwrap_or_else(|_| {
            manifest_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        });

    let Some(mut crates) = direct_perro_deps_from_manifest(manifest_src) else {
        return String::new();
    };
    let mut visited = BTreeSet::new();
    collect_perro_deps_from_local_path_deps(manifest_path, manifest_src, &mut crates, &mut visited);
    expand_transitive_perro_deps(&engine_root, &mut crates);
    if crates.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    lines.push("[patch.crates-io]".to_string());
    for crate_name in crates {
        let path = rel_path(&manifest_dir, &engine_root.join(&crate_name));
        lines.push(format!("{crate_name} = {{ path = \"{path}\" }}"));
    }
    lines.join("\n")
}

fn collect_perro_deps_from_local_path_deps(
    manifest_path: &Path,
    manifest_src: &str,
    crates: &mut BTreeSet<String>,
    visited: &mut BTreeSet<PathBuf>,
) {
    let Some(manifest_dir) = manifest_path.parent() else {
        return;
    };
    for rel_path in local_path_dependencies_from_manifest(manifest_src) {
        let dep_manifest = manifest_dir.join(rel_path).join("Cargo.toml");
        let dep_manifest = dep_manifest.canonicalize().unwrap_or(dep_manifest);
        if !visited.insert(dep_manifest.clone()) {
            continue;
        }
        let Ok(dep_src) = fs::read_to_string(&dep_manifest) else {
            continue;
        };
        if let Some(extra) = direct_perro_deps_from_manifest(&dep_src) {
            crates.extend(extra);
        }
        collect_perro_deps_from_local_path_deps(&dep_manifest, &dep_src, crates, visited);
    }
}

fn direct_perro_deps_from_manifest(src: &str) -> Option<BTreeSet<String>> {
    let value: Value = src.parse::<Value>().ok()?;
    let mut out = BTreeSet::new();
    collect_perro_dep_keys(value.get("dependencies"), &mut out);
    collect_perro_dep_keys(value.get("build-dependencies"), &mut out);
    collect_perro_dep_keys(value.get("dev-dependencies"), &mut out);
    Some(out)
}

fn local_path_dependencies_from_manifest(src: &str) -> Vec<String> {
    let Ok(value) = src.parse::<Value>() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_local_path_deps(value.get("dependencies"), &mut out);
    collect_local_path_deps(value.get("build-dependencies"), &mut out);
    collect_local_path_deps(value.get("dev-dependencies"), &mut out);
    out
}

fn collect_perro_dep_keys(table: Option<&Value>, out: &mut BTreeSet<String>) {
    let Some(table) = table.and_then(Value::as_table) else {
        return;
    };
    for key in table.keys() {
        if key.starts_with("perro_") {
            out.insert(key.to_string());
        }
    }
}

fn collect_local_path_deps(table: Option<&Value>, out: &mut Vec<String>) {
    let Some(table) = table.and_then(Value::as_table) else {
        return;
    };
    for dep in table.values() {
        let Some(dep_table) = dep.as_table() else {
            continue;
        };
        let Some(path) = dep_table.get("path").and_then(Value::as_str) else {
            continue;
        };
        out.push(path.to_string());
    }
}

fn expand_transitive_perro_deps(engine_root: &Path, crates: &mut BTreeSet<String>) {
    let mut queue: Vec<String> = crates.iter().cloned().collect();
    while let Some(crate_name) = queue.pop() {
        let manifest = engine_root.join(&crate_name).join("Cargo.toml");
        let Ok(src) = fs::read_to_string(manifest) else {
            continue;
        };
        let Some(extra) = direct_perro_deps_from_manifest(&src) else {
            continue;
        };
        for dep in extra {
            if crates.insert(dep.clone()) {
                queue.push(dep);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_project_toml_reads_virtual_resolution_string() {
        let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_resolution = "1280x720"
"#;

        let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
        assert_eq!(parsed.name, "Game");
        assert_eq!(parsed.main_scene, "res://main.scn");
        assert_eq!(parsed.icon, "res://icon.png");
        assert_eq!(parsed.virtual_width, 1280);
        assert_eq!(parsed.virtual_height, 720);
    }

    #[test]
    fn parse_project_toml_reads_split_virtual_dimensions() {
        let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_width = 1920
virtual_height = 1080
"#;

        let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
        assert_eq!(parsed.virtual_width, 1920);
        assert_eq!(parsed.virtual_height, 1080);
    }

    #[test]
    fn parse_project_toml_rejects_non_res_path() {
        let toml = r#"
[project]
name = "Game"
main_scene = "./main.scn"
icon = "res://icon.png"

[graphics]
virtual_resolution = "1920x1080"
"#;

        let err = parse_project_toml(toml).expect_err("expected parse failure");
        assert!(matches!(
            err,
            ProjectError::InvalidField("project.main_scene", _)
        ));
    }

    #[test]
    fn resolve_local_path_maps_slash_to_local_root() {
        let root = PathBuf::from("D:/workspace");
        assert_eq!(
            resolve_local_path("/games/demo", &root),
            PathBuf::from("D:/workspace").join("games").join("demo")
        );
        assert_eq!(resolve_local_path("/", &root), root);
    }

    #[test]
    fn resolve_local_path_supports_local_scheme() {
        let root = PathBuf::from("D:/workspace");
        assert_eq!(
            resolve_local_path("local://games/demo", &root),
            PathBuf::from("D:/workspace").join("games").join("demo")
        );
    }

    #[test]
    fn crate_name_from_project_name_normalizes() {
        assert_eq!(crate_name_from_project_name("My Project!"), "my_project");
        assert_eq!(crate_name_from_project_name("123"), "_123");
    }
}

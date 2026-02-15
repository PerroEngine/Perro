use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_PROJECT_NAME: &str = "Perro Project";
const PERRO_CRATES: &[&str] = &[
    "perro_api",
    "perro_app",
    "perro_compiler",
    "perro_core",
    "perro_dev_runner",
    "perro_graphics",
    "perro_ids",
    "perro_io",
    "perro_modules",
    "perro_project",
    "perro_render_bridge",
    "perro_runtime",
    "perro_scene",
    "perro_scripting",
    "perro_variant",
];

fn main() {
    let args: Vec<String> = env::args().collect();
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let Some(command) = args.get(1).map(String::as_str) else {
        print_usage();
        std::process::exit(2);
    };

    let result = if command.starts_with('-') {
        legacy_flag_command(&args, &cwd)
    } else {
        match command {
            "new" => new_command(&args, &cwd),
            "build" => build_command(&args, &cwd),
            "dev" => dev_command(&args, &cwd),
            "run" => run_command(&args, &cwd),
            _ => {
                print_usage();
                Err(format!("unknown command `{command}`"))
            }
        }
    };

    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn legacy_flag_command(args: &[String], cwd: &Path) -> Result<(), String> {
    if args.iter().any(|a| a == "--dev") {
        return dev_command(args, cwd);
    }
    if args.iter().any(|a| a == "--scripts") {
        return build_command(args, cwd);
    }
    if args.iter().any(|a| a == "--build") {
        return build_command(args, cwd);
    }
    if args.iter().any(|a| a == "--run") {
        return run_command(args, cwd);
    }
    print_usage();
    Err(format!("unknown command `{}`", args[1]))
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  perro_cli new [--path <parent_dir>] [--name <project_name>]");
    eprintln!("  perro_cli build [--path <project_dir>]   # builds .perro/scripts");
    eprintln!("  perro_cli dev [--path <project_dir>] [--name <project_name>]");
    eprintln!("  perro_cli run [--path <project_dir>]     # alias for build scripts");
}

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    args.get(idx + 1).cloned()
}

fn resolve_local_path(input: &str, local_root: &Path) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("local://") {
        let rel = stripped.trim_start_matches('/');
        if rel.is_empty() {
            return local_root.to_path_buf();
        }
        return local_root.join(rel);
    }
    if input.starts_with('/') || input.starts_with('\\') {
        let rel = input.trim_start_matches('/').trim_start_matches('\\');
        if rel.is_empty() {
            return local_root.to_path_buf();
        }
        return local_root.join(rel);
    }
    PathBuf::from(input)
}

fn sanitize_project_dir_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "perro_project".to_string();
    }

    let mut out = String::with_capacity(trimmed.len());
    for c in trimmed.chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            out.push('_');
        } else {
            out.push(c);
        }
    }

    let collapsed = out.trim_matches('.');
    if collapsed.is_empty() {
        "perro_project".to_string()
    } else {
        collapsed.to_string()
    }
}

fn crate_name_from_project_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if c == '_' || c == '-' || c.is_whitespace() {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "perro_project".to_string()
    } else {
        out
    }
}

fn workspace_root() -> PathBuf {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    raw.canonicalize().unwrap_or(raw)
}

fn rel_path(from: &Path, to: &Path) -> PathBuf {
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
    out
}

fn patch_block(manifest_dir: &Path, engine_root: &Path) -> String {
    let mut lines = Vec::new();
    lines.push("[patch.crates-io]".to_string());
    for crate_name in PERRO_CRATES {
        let rel = rel_path(manifest_dir, &engine_root.join(crate_name));
        let rel_unix = sanitize_path_for_toml(&rel);
        lines.push(format!(r#"{crate_name} = {{ path = "{rel_unix}" }}"#));
    }
    lines.join("\n")
}

fn sanitize_path_for_toml(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace("//?/", "")
}

fn new_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let base_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_name =
        parse_flag_value(args, "--name").unwrap_or_else(|| DEFAULT_PROJECT_NAME.to_string());
    let project_dir = base_dir.join(sanitize_project_dir_name(&project_name));

    if project_dir.exists() {
        return Err(format!(
            "project directory already exists: {}",
            project_dir.display()
        ));
    }

    create_project_from_template(&project_dir, &project_name)?;
    println!(
        "created project `{}` at {}",
        project_name,
        project_dir.display()
    );
    Ok(())
}

fn build_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let crate_dir = project_dir.join(".perro").join("scripts");

    if !crate_dir.exists() {
        return Err(format!("missing scripts crate directory: {}", crate_dir.display()));
    }

    let target_dir = workspace_root().join("target");
    let status = Command::new("cargo")
        .arg("build")
        .env("CARGO_TARGET_DIR", &target_dir)
        .current_dir(&crate_dir)
        .status()
        .map_err(|err| format!("failed to run cargo build in {}: {err}", crate_dir.display()))?;

    if !status.success() {
        return Err(format!(
            "cargo build failed in {} with exit code {:?}",
            crate_dir.display(),
            status.code()
        ));
    }
    Ok(())
}

fn dev_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_name =
        parse_flag_value(args, "--name").unwrap_or_else(|| DEFAULT_PROJECT_NAME.to_string());
    let root = workspace_root();

    let status = Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("perro_dev_runner")
        .arg("--release")
        .arg("--")
        .arg("--path")
        .arg(project_dir.to_string_lossy().to_string())
        .arg("--name")
        .arg(project_name)
        .current_dir(&root)
        .status()
        .map_err(|err| format!("failed to run perro_dev_runner from {}: {err}", root.display()))?;

    if !status.success() {
        return Err(format!(
            "perro_dev_runner failed with exit code {:?}",
            status.code()
        ));
    }
    Ok(())
}

fn run_command(args: &[String], cwd: &Path) -> Result<(), String> {
    build_command(args, cwd)
}

fn create_project_from_template(project_root: &Path, project_name: &str) -> Result<(), String> {
    let res_dir = project_root.join("res");
    let res_scripts_dir = res_dir.join("scripts");
    let perro_dir = project_root.join(".perro");
    let project_crate = perro_dir.join("project");
    let scripts_crate = perro_dir.join("scripts");
    let project_src = project_crate.join("src");
    let scripts_src = scripts_crate.join("src");

    fs::create_dir_all(&project_src).map_err(|e| e.to_string())?;
    fs::create_dir_all(&scripts_src).map_err(|e| e.to_string())?;
    fs::create_dir_all(&res_scripts_dir).map_err(|e| e.to_string())?;

    let engine_root = workspace_root();
    let project_patch = patch_block(&project_crate, &engine_root);
    let scripts_patch = patch_block(&scripts_crate, &engine_root);
    let crate_name = crate_name_from_project_name(project_name);

    fs::write(project_root.join(".gitignore"), default_gitignore()).map_err(|e| e.to_string())?;
    fs::write(project_root.join("project.toml"), default_project_toml(project_name))
        .map_err(|e| e.to_string())?;
    fs::write(res_dir.join("main.scn"), default_main_scene()).map_err(|e| e.to_string())?;
    fs::write(
        res_scripts_dir.join("script.rs"),
        default_script_example_rs(),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        project_crate.join("Cargo.toml"),
        default_project_crate_toml(&crate_name, &project_patch),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        scripts_crate.join("Cargo.toml"),
        default_scripts_crate_toml(&scripts_patch),
    )
    .map_err(|e| e.to_string())?;
    fs::write(project_src.join("main.rs"), default_project_main_rs()).map_err(|e| e.to_string())?;
    fs::write(scripts_src.join("lib.rs"), default_scripts_lib_rs()).map_err(|e| e.to_string())?;

    Ok(())
}

fn default_project_toml(name: &str) -> String {
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

fn default_main_scene() -> &'static str {
    r#"[main]
name = "World"

[Node2D]
    position = (0, 0)
[/Node2D]
[/main]
"#
}

fn default_script_example_rs() -> &'static str {
    r#"use perro_api::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_scripting::prelude::*;

///@State
#[derive(Default)]
pub struct ExampleState {
    speed: f32,
}

///@Script
pub struct ExampleScript;

impl<R: RuntimeAPI + ?Sized> ScriptLifecycle<R> for ExampleScript {
    fn init(&self, api: &mut API<'_, R>, self_id: NodeID) {
        let _origin = Vector2::new(0.0, 0.0);
        let _ = api
            .Scripts()
            .with_state_mut::<ExampleState, _, _>(self_id, |state| {
                state.speed = 240.0;
            });
    }

    fn update(&self, api: &mut API<'_, R>, self_id: NodeID) {
        let dt = api.Time().get_delta();
        let _ = api
            .Scripts()
            .with_state_mut::<ExampleState, _, _>(self_id, |state| {
                state.speed += dt;
            });
    }

    fn fixed_update(&self, _api: &mut API<'_, R>, _self_id: NodeID) {}
}
"#
}

fn default_gitignore() -> &'static str {
    r#"target/
.perro/project/embedded_assets/
.perro/project/static_assets/
.perro/scripts/src/
"#
}

fn default_project_crate_toml(crate_name: &str, patch_block: &str) -> String {
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
perro_api = "0.1.0"
perro_core = "0.1.0"
scripts = {{ path = "../scripts" }}

{patch_block}
"#
    )
}

fn default_scripts_crate_toml(patch_block: &str) -> String {
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
perro_api = "0.1.0"
perro_core = "0.1.0"
perro_variant = "0.1.0"

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
opt-level = 3
incremental = true
codegen-units = 16
debug = false
strip = "none"
overflow-checks = false

{patch_block}
"#
    )
}

fn default_project_main_rs() -> &'static str {
    r#"use std::path::PathBuf;

fn project_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn main() {
    let root = project_root();
    perro_app::entry::run_static_project_from_path(&root, "Perro Project")
        .expect("failed to run project");
}
"#
}

fn default_scripts_lib_rs() -> &'static str {
    r#"#[no_mangle]
pub extern "C" fn perro_scripts_init() {}
"#
}

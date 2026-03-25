use perro_compiler::{compile_project_bundle, compile_scripts};
use perro_project::{create_new_project, default_script_empty_rs};
use serde_json::Value;
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_PROJECT_NAME: &str = "Perro Project";
const COLOR_RESET: &str = "\x1b[0m";
const COLOR_BLUE: &str = "\x1b[94m";
const COLOR_GREEN: &str = "\x1b[92m";
const COLOR_YELLOW: &str = "\x1b[93m";

fn log_step(label: &str) {
    println!("{COLOR_BLUE}🔧 {label}...{COLOR_RESET}");
}

fn log_done(label: &str) {
    println!("{COLOR_GREEN}✅ {label}{COLOR_RESET}");
}

fn log_note(label: &str) {
    println!("{COLOR_YELLOW}🚀 {label}{COLOR_RESET}");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let Some(command) = args.get(1).map(String::as_str) else {
        print_usage();
        std::process::exit(2);
    };

    let result = if command == "--help" || command == "-h" || command == "help" {
        print_usage();
        Ok(())
    } else {
        match command {
            "new" => new_command(&args, &cwd),
            "new_script" => new_script_command(&args, &cwd),
            "new_scene" => new_scene_command(&args, &cwd),
            "new_animation" => new_animation_command(&args, &cwd),
            "clean" => clean_command(&args, &cwd),
            "install" => install_command(&args),
            "check" => scripts_command(&args, &cwd),
            "build" => project_command(&args, &cwd),
            "dev" => dev_command(&args, &cwd),
            "flamegraph" => flamegraph_command(&args, &cwd),
            "format" => format_command(&args, &cwd),
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

fn print_usage() {
    eprintln!("Usage:");
    eprintln!(
        "  perro_cli check [--path <project_dir>]    # scripts-only compile (.perro/scripts)"
    );
    eprintln!(
        "  perro_cli build [--path <project_dir>] [--profile]    # full static project bundle + build"
    );
    eprintln!(
        "  perro_cli dev [--path <project_dir>] [--profile]      # build scripts + run dev runner"
    );
    eprintln!(
        "  perro_cli flamegraph [--path <project_dir>] [--profile] [--root]    # run cargo flamegraph for dev runner"
    );
    eprintln!("  perro_cli format [--path <project_dir>]   # rustfmt .rs under project res only");
    eprintln!("  perro_cli clean [--path <project_dir>]    # remove project target/");
    eprintln!(
        "  perro_cli install                          # add `perro` source-mode command (PowerShell)"
    );
    eprintln!("  perro_cli new [--path <parent_dir>] [--name <project_name>]");
    eprintln!(
        "  perro_cli new_script --name <script_name> [--path <project_dir>] [--res <res_subdir>]"
    );
    eprintln!(
        "  perro_cli new_scene --name <scene_name> [--path <project_dir>] [--res <res_subdir>] [--template 2D|3D]"
    );
    eprintln!(
        "  perro_cli new_animation --name <animation_name> [--path <project_dir>] [--res <res_subdir>]"
    );
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

fn find_project_root(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        if ancestor.join("project.toml").exists() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn sanitize_script_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("script name cannot be empty".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("script name must not include path separators".to_string());
    }
    let mut out = String::with_capacity(trimmed.len() + 3);
    for c in trimmed.chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let mut rendered = out.trim_matches('.').to_string();
    if rendered.is_empty() {
        return Err("script name must include at least one valid character".to_string());
    }
    if !rendered.ends_with(".rs") {
        rendered.push_str(".rs");
    }
    Ok(rendered)
}

fn sanitize_scene_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("scene name cannot be empty".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("scene name must not include path separators".to_string());
    }
    let mut out = String::with_capacity(trimmed.len() + 4);
    for c in trimmed.chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let mut rendered = out.trim_matches('.').to_string();
    if rendered.is_empty() {
        return Err("scene name must include at least one valid character".to_string());
    }
    if !rendered.ends_with(".scn") {
        rendered.push_str(".scn");
    }
    Ok(rendered)
}

fn sanitize_animation_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("animation name cannot be empty".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("animation name must not include path separators".to_string());
    }
    let mut out = String::with_capacity(trimmed.len() + 6);
    for c in trimmed.chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let mut rendered = out.trim_matches('.').to_string();
    if rendered.is_empty() {
        return Err("animation name must include at least one valid character".to_string());
    }
    if !rendered.ends_with(".panim") {
        rendered.push_str(".panim");
    }
    Ok(rendered)
}

fn resolve_res_subdir(input: &str, res_root: &Path) -> Result<PathBuf, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(res_root.to_path_buf());
    }
    let rel = if let Some(stripped) = trimmed.strip_prefix("res://") {
        stripped.trim_start_matches('/').trim_start_matches('\\')
    } else if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        trimmed.trim_start_matches('/').trim_start_matches('\\')
    } else {
        trimmed
    };

    let rel_path = PathBuf::from(rel);
    if rel_path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("res subdir cannot contain `..` segments".to_string());
    }
    Ok(res_root.join(rel_path))
}

fn write_new_file(path: &Path, contents: &str) -> Result<(), String> {
    if path.exists() {
        return Err(format!("file already exists: {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(path, contents)
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    Ok(())
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

fn workspace_root() -> PathBuf {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..");
    raw.canonicalize().unwrap_or(raw)
}

const PROFILE_SNIPPET_BEGIN: &str = "# >>> perro_cli source-mode >>>";
const PROFILE_SNIPPET_END: &str = "# <<< perro_cli source-mode <<<";

fn install_command(args: &[String]) -> Result<(), String> {
    if !cfg!(target_os = "windows") {
        return Err(
            "install currently supports Windows PowerShell profile setup only. Use the docs snippet manually for other shells."
                .to_string(),
        );
    }

    let explicit_profile = parse_flag_value(args, "--profile").map(PathBuf::from);
    let profile_paths = if let Some(path) = explicit_profile {
        vec![path]
    } else {
        default_powershell_profile_paths()
    };
    let workspace_manifest =
        normalize_powershell_path(&workspace_root().join("Cargo.toml")).replace('\\', "\\\\");
    let snippet = format!(
        "{PROFILE_SNIPPET_BEGIN}\n\
function perro {{\n\
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Args)\n\
    cargo run --manifest-path \"{workspace_manifest}\" -p perro_cli -- @Args\n\
}}\n\
{PROFILE_SNIPPET_END}\n"
    );

    for profile_path in &profile_paths {
        let parent = profile_path.parent().ok_or_else(|| {
            format!(
                "invalid profile path (no parent directory): {}",
                profile_path.display()
            )
        })?;
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;

        let existing = if profile_path.exists() {
            fs::read_to_string(profile_path)
                .map_err(|err| format!("failed to read {}: {err}", profile_path.display()))?
        } else {
            String::new()
        };

        let updated = replace_or_append_snippet(&existing, &snippet)?;
        fs::write(profile_path, updated)
            .map_err(|err| format!("failed to write {}: {err}", profile_path.display()))?;
        println!(
            "installed source-mode command `perro` into {}",
            profile_path.display()
        );
    }
    if let Some(primary) = profile_paths.first() {
        println!("restart PowerShell or run: . \"{}\"", primary.display());
    }
    Ok(())
}

fn default_powershell_profile_paths() -> Vec<PathBuf> {
    let user_profile = env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    let docs = PathBuf::from(user_profile).join("Documents");
    let ps7 = docs
        .join("PowerShell")
        .join("Microsoft.PowerShell_profile.ps1");
    let ps5 = docs
        .join("WindowsPowerShell")
        .join("Microsoft.PowerShell_profile.ps1");
    vec![ps7, ps5]
}

fn normalize_powershell_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if let Some(stripped) = raw.strip_prefix("\\\\?\\") {
        stripped.to_string()
    } else {
        raw.to_string()
    }
}

fn replace_or_append_snippet(existing: &str, snippet: &str) -> Result<String, String> {
    let start = existing.find(PROFILE_SNIPPET_BEGIN);
    let end = existing.find(PROFILE_SNIPPET_END);
    match (start, end) {
        (Some(s), Some(e)) if e >= s => {
            let after = e + PROFILE_SNIPPET_END.len();
            let mut out = String::new();
            out.push_str(&existing[..s]);
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(snippet);
            let tail = &existing[after..];
            if !tail.is_empty() {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(tail.trim_start_matches('\n'));
            }
            Ok(out)
        }
        (None, None) => {
            let mut out = existing.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(snippet);
            Ok(out)
        }
        _ => Err(
            "profile contains a partial perro_cli snippet; remove it and re-run install"
                .to_string(),
        ),
    }
}

fn new_command(args: &[String], cwd: &Path) -> Result<(), String> {
    if args
        .iter()
        .any(|a| a == "--build-scripts" || a == "--open" || a == "--no-open")
    {
        return Err("`perro new` only accepts --path and --name".to_string());
    }
    let base_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let base_dir = base_dir.canonicalize().unwrap_or(base_dir);
    let project_name =
        parse_flag_value(args, "--name").unwrap_or_else(|| DEFAULT_PROJECT_NAME.to_string());
    let project_dir = base_dir.join(sanitize_project_dir_name(&project_name));

    create_new_project(&project_dir, &project_name).map_err(|err| {
        format!(
            "failed to create project at {}: {err}",
            project_dir.display()
        )
    })?;
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    println!(
        "created project `{}` at {}",
        project_name,
        normalize_powershell_path(&project_dir)
    );
    maybe_open_project_in_new_window(&project_dir)?;
    Ok(())
}

fn new_script_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let Some(raw_name) = parse_flag_value(args, "--name") else {
        return Err("missing required flag `--name`".to_string());
    };
    let file_name = sanitize_script_file_name(&raw_name)?;

    let project_dir = if let Some(raw_project) = parse_flag_value(args, "--path") {
        resolve_local_path(&raw_project, cwd)
    } else {
        find_project_root(cwd).ok_or_else(|| {
            "could not find project.toml. Run from a project directory or pass --path <project_dir>."
                .to_string()
        })?
    };
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let res_root = project_dir.join("res");
    if !res_root.exists() {
        return Err(format!("res directory not found at {}", res_root.display()));
    }

    let target_dir = if let Some(raw_path) = parse_flag_value(args, "--res") {
        resolve_res_subdir(&raw_path, &res_root)?
    } else {
        res_root
    };

    let target_path = target_dir.join(file_name);
    write_new_file(&target_path, &default_script_empty_rs())?;
    println!(
        "created script at {}",
        normalize_powershell_path(&target_path)
    );
    maybe_open_file_in_editor(args, &target_path)?;
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;
    log_step("Building Scripts");
    compile_scripts(&project_dir)
        .map(|_| {
            log_done("Scripts Built");
        })
        .map_err(|err| {
            format!(
                "scripts pipeline failed for {}: {err}",
                project_dir.display()
            )
        })?;
    Ok(())
}

fn parse_scene_template(args: &[String]) -> Result<SceneTemplate, String> {
    let Some(raw) = parse_flag_value(args, "--template") else {
        return Ok(SceneTemplate::TwoD);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "2d" => Ok(SceneTemplate::TwoD),
        "3d" => Ok(SceneTemplate::ThreeD),
        _ => Err("invalid --template value. Use 2D or 3D.".to_string()),
    }
}

enum SceneTemplate {
    TwoD,
    ThreeD,
}

fn default_scene_2d() -> String {
    r#"@root = main

[main]

[Node2D]
    position = (0, 0)
[/Node2D]
[/main]
"#
    .to_string()
}

fn default_scene_3d() -> String {
    r#"@root = main

[main]

[Node3D]
    position = (0, 0, 0)
[/Node3D]
[/main]

[camera]
parent = @root

[Camera3D]
    active = true
    [Node3D]
        position = (0, 0, 8)
    [/Node3D]
[/Camera3D]
[/camera]

[ambient]
parent = @root

[AmbientLight3D]
    color = (1.0, 1.0, 1.0)
    intensity = 0.8
[/AmbientLight3D]
[/ambient]
"#
    .to_string()
}

fn new_scene_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let Some(raw_name) = parse_flag_value(args, "--name") else {
        return Err("missing required flag `--name`".to_string());
    };
    let file_name = sanitize_scene_file_name(&raw_name)?;
    let template = parse_scene_template(args)?;

    let project_dir = if let Some(raw_project) = parse_flag_value(args, "--path") {
        resolve_local_path(&raw_project, cwd)
    } else {
        find_project_root(cwd).ok_or_else(|| {
            "could not find project.toml. Run from a project directory or pass --path <project_dir>."
                .to_string()
        })?
    };
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let res_root = project_dir.join("res");
    if !res_root.exists() {
        return Err(format!("res directory not found at {}", res_root.display()));
    }

    let target_dir = if let Some(raw_path) = parse_flag_value(args, "--res") {
        resolve_res_subdir(&raw_path, &res_root)?
    } else {
        res_root
    };

    let target_path = target_dir.join(file_name);
    let contents = match template {
        SceneTemplate::TwoD => default_scene_2d(),
        SceneTemplate::ThreeD => default_scene_3d(),
    };
    write_new_file(&target_path, &contents)?;
    println!(
        "created scene at {}",
        normalize_powershell_path(&target_path)
    );
    maybe_open_file_in_editor(args, &target_path)?;
    Ok(())
}

fn default_animation_panim(animation_name: &str) -> String {
    r#"[Animation]
name = "__ANIMATION_NAME__"
fps = 60
default_interp = "interpolate"
default_ease = "linear"
[/Animation]

[Objects]
@Target = Node3D
[/Objects]

[Frame0]
@Target {
    position = (0, 0, 0)
}
[/Frame0]

[Frame30]
@Target {
    position = (2, 0, 0)
}
[/Frame30]
"#
    .replace("__ANIMATION_NAME__", animation_name)
}

fn new_animation_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let Some(raw_name) = parse_flag_value(args, "--name") else {
        return Err("missing required flag `--name`".to_string());
    };
    let file_name = sanitize_animation_file_name(&raw_name)?;

    let project_dir = if let Some(raw_project) = parse_flag_value(args, "--path") {
        resolve_local_path(&raw_project, cwd)
    } else {
        find_project_root(cwd).ok_or_else(|| {
            "could not find project.toml. Run from a project directory or pass --path <project_dir>."
                .to_string()
        })?
    };
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let res_root = project_dir.join("res");
    if !res_root.exists() {
        return Err(format!("res directory not found at {}", res_root.display()));
    }

    let target_dir = if let Some(raw_path) = parse_flag_value(args, "--res") {
        resolve_res_subdir(&raw_path, &res_root)?
    } else {
        res_root.join("animations")
    };

    let target_path = target_dir.join(file_name);
    let animation_name = target_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("NewAnimation");
    write_new_file(&target_path, &default_animation_panim(animation_name))?;
    println!(
        "created animation at {}",
        normalize_powershell_path(&target_path)
    );
    maybe_open_file_in_editor(args, &target_path)?;
    Ok(())
}

fn clean_command(args: &[String], _cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, _cwd))
        .or_else(|| find_project_root(_cwd))
        .ok_or_else(|| {
            "could not find project.toml. Run from a project directory or pass --path <project_dir>."
                .to_string()
        })?;
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    if !project_dir.join("project.toml").exists() {
        return Err(format!(
            "invalid --path `{}` for clean. Use project root (directory containing project.toml).",
            project_dir.display()
        ));
    }

    let target_dir = project_dir.join("target");
    if !target_dir.exists() {
        log_note("No project target/ directory to clean");
        return Ok(());
    }

    if let Ok(current_exe) = env::current_exe()
        && current_exe.starts_with(&target_dir)
    {
        return Err(
                "cannot clean while running from the project's target/. Use the installed `perro` command or run from another location."
                    .to_string(),
            );
    }

    log_step("Cleaning Project Target");
    fs::remove_dir_all(&target_dir)
        .map_err(|err| format!("failed to remove {}: {err}", target_dir.display()))?;
    log_done("Project Target Cleaned");
    Ok(())
}

fn maybe_open_file_in_editor(args: &[String], file_path: &Path) -> Result<(), String> {
    if args.iter().any(|a| a == "--no-open") {
        return Ok(());
    }
    let file_arg = normalize_powershell_path(file_path);
    let status = Command::new("code")
        .arg("-g")
        .arg(file_arg)
        .status()
        .map_err(|err| {
            format!(
                "failed to launch VS Code. Ensure the `code` command is available on PATH: {err}"
            )
        })?;
    if !status.success() {
        return Err(format!(
            "VS Code launch failed with exit code {:?}",
            status.code()
        ));
    }
    Ok(())
}

fn maybe_open_project_in_new_window(project_dir: &Path) -> Result<(), String> {
    let can_prompt = io::stdin().is_terminal();
    if !can_prompt {
        return Ok(());
    }
    let should_open = prompt_yes_no("Open the project in a new window? [y/N] ")?;
    if !should_open {
        return Ok(());
    }

    let readme = project_dir.join("README.md");
    let mut cmd = Command::new("code");
    cmd.arg("-n").arg(normalize_powershell_path(project_dir));
    if readme.exists() {
        cmd.arg(normalize_powershell_path(&readme));
    }
    let status = cmd.status().map_err(|err| {
        format!("failed to launch VS Code. Ensure the `code` command is available on PATH: {err}")
    })?;

    if !status.success() {
        return Err(format!(
            "VS Code launch failed with exit code {:?}",
            status.code()
        ));
    }
    Ok(())
}

fn prompt_yes_no(prompt: &str) -> Result<bool, String> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|err| format!("failed to flush prompt: {err}"))?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|err| format!("failed to read input: {err}"))?;
    let answer = input.trim().to_ascii_lowercase();
    Ok(matches!(answer.as_str(), "y" | "yes"))
}

fn update_workspace_vscode_linked_projects(
    workspace_root: &Path,
    project_dir: &Path,
) -> Result<(), String> {
    let settings_path = workspace_root.join(".vscode").join("settings.json");
    let mut json: Value = if settings_path.exists() {
        let raw = fs::read_to_string(&settings_path)
            .map_err(|err| format!("failed to read {}: {err}", settings_path.display()))?;
        serde_json::from_str(&raw)
            .map_err(|err| format!("failed to parse {} as JSON: {err}", settings_path.display()))?
    } else {
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
        Value::Object(Default::default())
    };
    let Some(root) = json.as_object_mut() else {
        return Err(format!(
            "expected {} to contain a JSON object",
            settings_path.display()
        ));
    };

    let workspace_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let entry = root
        .entry("rust-analyzer.linkedProjects".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = entry.as_array_mut() else {
        return Err(format!(
            "expected `rust-analyzer.linkedProjects` to be an array in {}",
            settings_path.display()
        ));
    };

    arr.retain(|v| {
        let Some(s) = v.as_str() else {
            return false;
        };
        let p = PathBuf::from(s);
        let full = if p.is_absolute() {
            p
        } else {
            workspace_root.join(p)
        };
        full.exists()
    });

    for rel in workspace_internal_project_manifests(&workspace_root, project_dir)? {
        if !arr.iter().any(|v| v.as_str() == Some(rel.as_str())) {
            arr.push(Value::String(rel));
        }
    }

    let vfs = root
        .entry("rust-analyzer.vfs.extraIncludes".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(vfs_arr) = vfs.as_array_mut() else {
        return Err(format!(
            "expected `rust-analyzer.vfs.extraIncludes` to be an array in {}",
            settings_path.display()
        ));
    };

    vfs_arr.retain(|v| {
        let Some(s) = v.as_str() else {
            return false;
        };
        let Some(path_part) = s.strip_prefix("${workspaceFolder}/") else {
            return true;
        };
        let trimmed = path_part.trim_end_matches('/').trim_end_matches('\\');
        workspace_root.join(trimmed).exists()
    });

    for vfs_entry in workspace_internal_project_vfs_entries(&workspace_root, project_dir)? {
        if !vfs_arr
            .iter()
            .any(|v| v.as_str() == Some(vfs_entry.as_str()))
        {
            vfs_arr.push(Value::String(vfs_entry));
        }
    }

    let rendered = serde_json::to_string_pretty(&json).map_err(|err| {
        format!(
            "failed to render {} as JSON: {err}",
            settings_path.display()
        )
    })?;
    fs::write(&settings_path, format!("{rendered}\n"))
        .map_err(|err| format!("failed to write {}: {err}", settings_path.display()))?;
    Ok(())
}

fn workspace_internal_project_roots(
    workspace_root: &Path,
    _project_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();

    let playground_root = workspace_root.join("playground");
    if playground_root.exists() {
        let entries = fs::read_dir(&playground_root).map_err(|err| {
            format!(
                "failed to read playground directory {}: {err}",
                playground_root.display()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|err| {
                format!(
                    "failed to read playground directory entry in {}: {err}",
                    playground_root.display()
                )
            })?;
            let path = entry.path();
            if !path.is_dir() || !path.join("project.toml").exists() {
                continue;
            }
            roots.push(path);
        }
    }

    roots.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    roots.dedup();
    Ok(roots)
}

fn workspace_internal_project_manifests(
    workspace_root: &Path,
    project_dir: &Path,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for root in workspace_internal_project_roots(workspace_root, project_dir)? {
        let scripts_manifest = root
            .join(".perro")
            .join("scripts")
            .join("Cargo.toml")
            .canonicalize()
            .unwrap_or_else(|_| root.join(".perro").join("scripts").join("Cargo.toml"));
        let Ok(rel) = scripts_manifest
            .strip_prefix(workspace_root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
        else {
            continue;
        };
        out.push(rel);
    }
    Ok(out)
}

fn workspace_internal_project_vfs_entries(
    workspace_root: &Path,
    project_dir: &Path,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for root in workspace_internal_project_roots(workspace_root, project_dir)? {
        let res_dir = root
            .join("res")
            .canonicalize()
            .unwrap_or_else(|_| root.join("res"));
        let Ok(rel_res) = res_dir
            .strip_prefix(workspace_root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
        else {
            continue;
        };
        out.push(format!("${{workspaceFolder}}/{rel_res}/"));
    }
    Ok(out)
}

fn update_project_vscode_linked_projects(project_dir: &Path) -> Result<(), String> {
    let settings_dir = project_dir.join(".vscode");
    fs::create_dir_all(&settings_dir)
        .map_err(|err| format!("failed to create {}: {err}", settings_dir.display()))?;

    let settings_path = settings_dir.join("settings.json");
    let mut json: Value = if settings_path.exists() {
        let raw = fs::read_to_string(&settings_path)
            .map_err(|err| format!("failed to read {}: {err}", settings_path.display()))?;
        serde_json::from_str(&raw)
            .map_err(|err| format!("failed to parse {} as JSON: {err}", settings_path.display()))?
    } else {
        Value::Object(Default::default())
    };

    let Some(root) = json.as_object_mut() else {
        return Err(format!(
            "expected {} to contain a JSON object",
            settings_path.display()
        ));
    };

    let linked_manifest = ".perro/scripts/Cargo.toml".to_string();
    let vfs_entry = "${workspaceFolder}/res/".to_string();

    let entry = root
        .entry("rust-analyzer.linkedProjects".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = entry.as_array_mut() else {
        return Err(format!(
            "expected `rust-analyzer.linkedProjects` to be an array in {}",
            settings_path.display()
        ));
    };
    if !arr
        .iter()
        .any(|v| v.as_str() == Some(linked_manifest.as_str()))
    {
        arr.push(Value::String(linked_manifest));
    }

    let vfs = root
        .entry("rust-analyzer.vfs.extraIncludes".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(vfs_arr) = vfs.as_array_mut() else {
        return Err(format!(
            "expected `rust-analyzer.vfs.extraIncludes` to be an array in {}",
            settings_path.display()
        ));
    };
    if !vfs_arr
        .iter()
        .any(|v| v.as_str() == Some(vfs_entry.as_str()))
    {
        vfs_arr.push(Value::String(vfs_entry));
    }

    let rendered = serde_json::to_string_pretty(&json).map_err(|err| {
        format!(
            "failed to render {} as JSON: {err}",
            settings_path.display()
        )
    })?;
    fs::write(&settings_path, format!("{rendered}\n"))
        .map_err(|err| format!("failed to write {}: {err}", settings_path.display()))?;
    Ok(())
}

fn scripts_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;
    log_step("Building Scripts");
    compile_scripts(&project_dir)
        .map(|_| {
            log_done("Scripts Built");
        })
        .map_err(|err| {
            format!(
                "scripts pipeline failed for {}: {err}",
                project_dir.display()
            )
        })
}

fn dev_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let profile = args.iter().any(|a| a == "--profile");
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    log_step("Building Scripts");
    compile_scripts(&project_dir).map_err(|err| {
        format!(
            "scripts pipeline failed for {}: {err}",
            project_dir.display()
        )
    })?;
    log_done("Scripts Built");

    let dev_runner_dir = project_dir.join(".perro").join("dev_runner");
    let target_dir = project_dir.join("target");
    log_step("Building Dev Runner");

    let mut build_cmd = Command::new("cargo");
    build_cmd
        .arg("build")
        .arg("--release")
        .env("CARGO_TARGET_DIR", &target_dir)
        .current_dir(&dev_runner_dir);
    if profile {
        build_cmd.arg("--features").arg("profile");
    }
    let build_status = build_cmd.status().map_err(|err| {
        format!(
            "failed to build project dev runner from {}: {err}",
            dev_runner_dir.display()
        )
    })?;

    if !build_status.success() {
        return Err(format!(
            "project dev runner build failed with exit code {:?}",
            build_status.code()
        ));
    }
    log_done("Dev Runner Built");

    let runner_path = if cfg!(target_os = "windows") {
        target_dir.join("release").join("perro_dev_runner.exe")
    } else {
        target_dir.join("release").join("perro_dev_runner")
    };
    log_note("Running Dev Runner");

    let run_status = Command::new(&runner_path)
        .arg("--path")
        .arg(project_dir.to_string_lossy().to_string())
        .current_dir(&project_dir)
        .status()
        .map_err(|err| {
            format!(
                "failed to launch project dev runner at {}: {err}",
                runner_path.display()
            )
        })?;

    if !run_status.success() {
        return Err(format!(
            "project dev runner failed with exit code {:?}",
            run_status.code()
        ));
    }
    log_done("Dev Runner Finished");
    Ok(())
}

fn flamegraph_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let profile = args.iter().any(|a| a == "--profile");
    let root = args.iter().any(|a| a == "--root");
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    log_step("Building Scripts");
    compile_scripts(&project_dir).map_err(|err| {
        format!(
            "scripts pipeline failed for {}: {err}",
            project_dir.display()
        )
    })?;
    log_done("Scripts Built");

    let dev_runner_dir = project_dir.join(".perro").join("dev_runner");
    let target_dir = project_dir.join("target");
    log_step("Running Flamegraph");

    let mut cmd = Command::new("cargo");
    cmd.arg("flamegraph")
        .arg("--release")
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("CARGO_PROFILE_RELEASE_DEBUG", "true")
        .current_dir(&dev_runner_dir);
    if root {
        cmd.arg("--root");
    }
    if profile {
        cmd.arg("--features").arg("profile");
    }
    cmd.arg("--")
        .arg("--path")
        .arg(project_dir.to_string_lossy().to_string());

    let status = cmd.status().map_err(|err| {
        format!(
            "failed to run cargo flamegraph from {}: {err}",
            dev_runner_dir.display()
        )
    })?;

    if !status.success() {
        return Err(format!(
            "cargo flamegraph failed with exit code {:?}",
            status.code()
        ));
    }

    log_done("Flamegraph Complete");
    Ok(())
}

fn format_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let base_path = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let base_path = base_path.canonicalize().unwrap_or(base_path);
    let res_dir = resolve_res_root_for_format(&base_path)?;
    let mut script_files = Vec::new();
    collect_rs_files_recursive(&res_dir, &mut script_files)?;

    if script_files.is_empty() {
        log_note("No .rs files found under res");
        return Ok(());
    }

    log_step("Formatting User Scripts");
    for file in &script_files {
        let status = Command::new("rustfmt")
            .arg(file)
            .status()
            .map_err(|err| format!("failed to run rustfmt for {}: {err}", file.display()))?;
        if !status.success() {
            return Err(format!(
                "rustfmt failed for {} with exit code {:?}",
                file.display(),
                status.code()
            ));
        }
    }
    log_done("User Scripts Formatted");
    Ok(())
}

fn resolve_res_root_for_format(path: &Path) -> Result<PathBuf, String> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    // `--path` must point at project root.
    if path.join("project.toml").exists() {
        return Ok(path.join("res"));
    }

    Err(format!(
        "invalid --path `{}` for format. Use project root (directory containing project.toml).",
        path.display()
    ))
}

fn collect_rs_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(dir)
        .map_err(|err| format!("failed to read directory {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry
            .map_err(|err| format!("failed to read directory entry in {}: {err}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files_recursive(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn project_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let profile = args.iter().any(|a| a == "--profile");
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;
    log_step("Building Project Bundle");
    compile_project_bundle(&project_dir, profile)
        .map(|_| {
            log_done("Project Bundle Built");
        })
        .map_err(|err| {
            format!(
                "project pipeline failed for {}: {err}",
                project_dir.display()
            )
        })
}

use perro_compiler::{
    ScriptsBuildProfile, compile_dlc_bundle, compile_project_bundle, compile_scripts_with_profile,
    sync_scripts,
};
use perro_project::{create_new_project, default_script_empty_rs, ensure_source_overrides};
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
            "new_dlc" => new_dlc_command(&args, &cwd),
            "new_script" => new_script_command(&args, &cwd),
            "new_scene" => new_scene_command(&args, &cwd),
            "new_animation" => new_animation_command(&args, &cwd),
            "clean" => clean_command(&args, &cwd),
            "install" => install_command(&args),
            "check" => scripts_command(&args, &cwd),
            "build" => project_command(&args, &cwd),
            "dlc" => dlc_command(&args, &cwd),
            "dev" => dev_command(&args, &cwd),
            "mem-profile" => mem_profile_command(&args, &cwd),
            "flamegraph" => flamegraph_command(&args, &cwd),
            "format" => format_command(&args, &cwd),
            "clippy" => clippy_command(&args, &cwd),
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
        "  perro_cli dlc --name <dlc_name> [--path <project_dir>] # build one runtime-loadable DLC package"
    );
    eprintln!(
        "  perro_cli dev [--path <project_dir>] [--profile] [--ui-profile] [--release] [--csv-profile [csv_name]]      # build scripts + run dev runner"
    );
    eprintln!(
        "  perro_cli mem-profile [--path <project_dir>] [--release] [--csv [csv_name]]    # run dev runner + process memory samples"
    );
    eprintln!(
        "  perro_cli flamegraph [--path <project_dir>] [--profile] [--root]    # run cargo flamegraph for dev runner (auto-installs tool if missing)"
    );
    eprintln!("  perro_cli format [--path <project_dir>]   # rustfmt .rs under project res only");
    eprintln!(
        "  perro_cli clippy [--path <project_dir>]   # cargo clippy for .rs under project res"
    );
    eprintln!("  perro_cli clean [--path <project_dir>]    # remove project target/");
    eprintln!(
        "  perro_cli install                          # add `perro` source-mode command in shell profile"
    );
    eprintln!("  perro_cli new [--path <parent_dir>] [--name <project_name>]");
    eprintln!("  perro_cli new_dlc --name <dlc_name> [--path <project_dir>]");
    eprintln!(
        "  perro_cli new_script --name <script_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>]"
    );
    eprintln!(
        "  perro_cli new_scene --name <scene_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--template 2D|3D]"
    );
    eprintln!(
        "  perro_cli new_animation --name <animation_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>]"
    );
}

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    args.get(idx + 1).cloned()
}

fn parse_optional_flag_value(args: &[String], flag: &str) -> Option<Option<String>> {
    let idx = args.iter().position(|a| a == flag)?;
    let next = args.get(idx + 1);
    if let Some(val) = next
        && !val.starts_with("--")
    {
        return Some(Some(val.clone()));
    }
    Some(None)
}

fn resolve_local_path(input: &str, local_root: &Path) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("local://") {
        let rel = stripped.trim_start_matches('/');
        if rel.is_empty() {
            return local_root.to_path_buf();
        }
        return local_root.join(rel);
    }
    #[cfg(not(target_os = "windows"))]
    if input.starts_with('/') {
        return PathBuf::from(input);
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

fn resolve_res_subdir(
    input: &str,
    res_root: &Path,
    allowed_prefix: &str,
) -> Result<PathBuf, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(res_root.to_path_buf());
    }

    let rel = if let Some(stripped) = trimmed.strip_prefix(allowed_prefix) {
        stripped.trim_start_matches('/').trim_start_matches('\\')
    } else if trimmed.contains("://") {
        return Err(format!(
            "path must use `{allowed_prefix}*` or `/...` style segments"
        ));
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

fn validate_dlc_name(raw: &str) -> Result<String, String> {
    let dlc = raw.trim();
    if dlc.is_empty() {
        return Err("dlc name cannot be empty".to_string());
    }
    if dlc.eq_ignore_ascii_case("self") {
        return Err("dlc name `self` is reserved".to_string());
    }
    if dlc.contains('/') || dlc.contains('\\') {
        return Err("dlc name must not include path separators".to_string());
    }
    if Path::new(dlc)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("dlc name must not include `..` segments".to_string());
    }
    Ok(dlc.to_string())
}

fn resolve_content_root(project_dir: &Path, args: &[String]) -> Result<PathBuf, String> {
    if let Some(raw_dlc) = parse_flag_value(args, "--dlc") {
        let dlc = validate_dlc_name(&raw_dlc)?;
        return Ok(project_dir.join("dlcs").join(dlc));
    }

    let res_root = project_dir.join("res");
    if !res_root.exists() {
        return Err(format!("res directory not found at {}", res_root.display()));
    }
    Ok(res_root)
}

fn resolve_content_prefix(args: &[String]) -> Result<String, String> {
    if let Some(raw_dlc) = parse_flag_value(args, "--dlc") {
        let dlc = validate_dlc_name(&raw_dlc)?;
        return Ok(format!("dlc://{dlc}/"));
    }
    Ok("res://".to_string())
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
    let explicit_profile = parse_flag_value(args, "--profile").map(PathBuf::from);

    if cfg!(target_os = "windows") {
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
        install_snippet_into_profiles(&profile_paths, &snippet)?;
        if let Some(primary) = profile_paths.first() {
            println!("restart PowerShell or run: . \"{}\"", primary.display());
        }
        return Ok(());
    }

    if cfg!(target_os = "linux") {
        let profile_paths = if let Some(path) = explicit_profile {
            vec![path]
        } else {
            default_posix_profile_paths()
        };
        let workspace_manifest = shell_single_quote_path(&workspace_root().join("Cargo.toml"));
        let snippet = format!(
            "{PROFILE_SNIPPET_BEGIN}\n\
perro() {{\n\
    cargo run --manifest-path {workspace_manifest} -p perro_cli -- \"$@\"\n\
}}\n\
{PROFILE_SNIPPET_END}\n"
        );
        install_snippet_into_profiles(&profile_paths, &snippet)?;
        if let Some(primary) = profile_paths.first() {
            println!("restart shell or run: . \"{}\"", primary.display());
        }
        return Ok(());
    }

    Err(
        "install currently supports Windows PowerShell + Linux POSIX shells only. Use docs snippet manually for this platform."
            .to_string(),
    )
}

fn install_snippet_into_profiles(profile_paths: &[PathBuf], snippet: &str) -> Result<(), String> {
    for profile_path in profile_paths {
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

        let updated = replace_or_append_snippet(&existing, snippet)?;
        fs::write(profile_path, updated)
            .map_err(|err| format!("failed to write {}: {err}", profile_path.display()))?;
        println!(
            "installed source-mode command `perro` into {}",
            profile_path.display()
        );
    }
    Ok(())
}

fn default_posix_profile_paths() -> Vec<PathBuf> {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let home = PathBuf::from(home);
    let mut paths = vec![
        home.join(".profile"),
        home.join(".bashrc"),
        home.join(".zshrc"),
    ];
    paths.sort();
    paths.dedup();
    paths
}

fn shell_single_quote_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let escaped = raw.replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

fn default_powershell_profile_paths() -> Vec<PathBuf> {
    let user_profile = env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    let docs = PathBuf::from(user_profile).join("Documents");
    let ps7_dir = docs.join("PowerShell");
    let ps5_dir = docs.join("WindowsPowerShell");

    // Install for current host + all hosts in both pwsh (7+) and Windows PowerShell (5.1).
    let mut paths = vec![
        ps7_dir.join("Microsoft.PowerShell_profile.ps1"),
        ps7_dir.join("profile.ps1"),
        ps5_dir.join("Microsoft.PowerShell_profile.ps1"),
        ps5_dir.join("profile.ps1"),
    ];
    paths.sort();
    paths.dedup();
    paths
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
    let mut project_name = parse_flag_value(args, "--name");
    let mut base_dir_input = parse_flag_value(args, "--path");

    if project_name.is_none() && base_dir_input.is_none() {
        if !io::stdin().is_terminal() {
            return Err(
                "`perro new` without --name/--path needs interactive terminal input".to_string(),
            );
        }
        let input_name = prompt_text(
            &format!("Project name [{DEFAULT_PROJECT_NAME}]: "),
            Some(DEFAULT_PROJECT_NAME),
        )?;
        let input_root = prompt_text(
            &format!("Project root folder [{}]: ", normalize_powershell_path(cwd)),
            Some(&normalize_powershell_path(cwd)),
        )?;
        project_name = Some(input_name);
        base_dir_input = Some(input_root);
    }

    let base_dir = base_dir_input
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let base_dir = base_dir.canonicalize().unwrap_or(base_dir);
    let project_name = project_name.unwrap_or_else(|| DEFAULT_PROJECT_NAME.to_string());
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

fn new_dlc_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let Some(raw_name) = parse_flag_value(args, "--name") else {
        return Err("missing required flag `--name`".to_string());
    };
    let dlc_name = validate_dlc_name(&raw_name)?;

    let project_dir = if let Some(raw_project) = parse_flag_value(args, "--path") {
        resolve_local_path(&raw_project, cwd)
    } else {
        find_project_root(cwd).ok_or_else(|| {
            "could not find project.toml. Run from a project directory or pass --path <project_dir>."
                .to_string()
        })?
    };
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    if !project_dir.join("project.toml").exists() {
        return Err(format!(
            "invalid --path `{}` for new_dlc. Use project root (directory containing project.toml).",
            project_dir.display()
        ));
    }

    let dlc_root = project_dir.join("dlcs").join(&dlc_name);
    if dlc_root.exists() {
        return Err(format!("dlc already exists: {}", dlc_root.display()));
    }

    fs::create_dir_all(dlc_root.join("scenes"))
        .map_err(|err| format!("failed to create dlc scenes dir: {err}"))?;
    fs::create_dir_all(dlc_root.join("scripts"))
        .map_err(|err| format!("failed to create dlc scripts dir: {err}"))?;
    fs::create_dir_all(dlc_root.join("materials"))
        .map_err(|err| format!("failed to create dlc materials dir: {err}"))?;
    fs::create_dir_all(dlc_root.join("meshes"))
        .map_err(|err| format!("failed to create dlc meshes dir: {err}"))?;

    let script_path = dlc_root.join("scripts").join("script.rs");
    write_new_file(&script_path, &default_script_empty_rs())?;

    let scene_path = dlc_root.join("scenes").join("main.scn");
    let scene = format!(
        "@root = main\n\n[main]\nscript = \"dlc://{dlc_name}/scripts/script.rs\"\n[Node2D]\n    position = (0, 0)\n[/Node2D]\n[/main]\n"
    );
    write_new_file(&scene_path, &scene)?;
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    println!(
        "created dlc `{}` at {}",
        dlc_name,
        normalize_powershell_path(&dlc_root)
    );
    println!("reference scene with: dlc://{}/scenes/main.scn", dlc_name);
    println!(
        "reference script with: dlc://{}/scripts/script.rs",
        dlc_name
    );
    maybe_open_file_in_editor(args, &scene_path)?;
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
    let content_root = resolve_content_root(&project_dir, args)?;
    let content_prefix = resolve_content_prefix(args)?;

    let target_dir = if let Some(raw_path) = parse_flag_value(args, "--res") {
        resolve_res_subdir(&raw_path, &content_root, &content_prefix)?
    } else {
        content_root
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
    compile_scripts_with_profile(&project_dir, ScriptsBuildProfile::Debug)
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
    let content_root = resolve_content_root(&project_dir, args)?;
    let content_prefix = resolve_content_prefix(args)?;

    let target_dir = if let Some(raw_path) = parse_flag_value(args, "--res") {
        resolve_res_subdir(&raw_path, &content_root, &content_prefix)?
    } else {
        content_root
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
    let content_root = resolve_content_root(&project_dir, args)?;
    let content_prefix = resolve_content_prefix(args)?;

    let target_dir = if let Some(raw_path) = parse_flag_value(args, "--res") {
        resolve_res_subdir(&raw_path, &content_root, &content_prefix)?
    } else {
        content_root.join("animations")
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

fn prompt_text(prompt: &str, default: Option<&str>) -> Result<String, String> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|err| format!("failed to flush prompt: {err}"))?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|err| format!("failed to read input: {err}"))?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(default.unwrap_or("").to_string());
    }
    Ok(trimmed.to_string())
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
    arr.retain(|v| {
        let Some(s) = v.as_str() else {
            return false;
        };
        if Path::new(s).is_absolute() {
            return Path::new(s).exists();
        }
        project_dir.join(s).exists()
    });
    for linked_manifest in project_internal_linked_manifests(project_dir)? {
        if !arr
            .iter()
            .any(|v| v.as_str() == Some(linked_manifest.as_str()))
        {
            arr.push(Value::String(linked_manifest));
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

fn project_internal_linked_manifests(project_dir: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::<String>::new();
    out.push(".perro/scripts/Cargo.toml".to_string());

    let dlc_root = project_dir.join(".perro").join("dlc");
    if dlc_root.exists() {
        let entries = fs::read_dir(&dlc_root)
            .map_err(|err| format!("failed to read {}: {err}", dlc_root.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| {
                format!(
                    "failed to read dlc generated entry in {}: {err}",
                    dlc_root.display()
                )
            })?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            out.push(format!(".perro/dlc/{name}/scripts/Cargo.toml"));
        }
    }

    out.sort();
    out.dedup();
    Ok(out)
}

fn scripts_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;
    log_step("Building Scripts");
    compile_scripts_with_profile(&project_dir, ScriptsBuildProfile::Debug)
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

fn dlc_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let Some(raw_dlc_name) = parse_flag_value(args, "--name") else {
        return Err("missing required flag `--name`".to_string());
    };
    let dlc_name = validate_dlc_name(&raw_dlc_name)?;
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    log_step("Building DLC");
    let package = compile_dlc_bundle(&project_dir, &dlc_name).map_err(|err| {
        format!(
            "dlc pipeline failed for {} ({}): {err}",
            project_dir.display(),
            dlc_name
        )
    })?;
    log_done(&format!("DLC Built ({})", package.display()));
    Ok(())
}

fn dev_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let profile_requested = args.iter().any(|a| a == "--profile");
    let ui_profile = args.iter().any(|a| a == "--ui-profile");
    let release = args.iter().any(|a| a == "--release");
    let csv_profile_name = parse_optional_flag_value(args, "--csv-profile")
        .map(|raw| PathBuf::from(raw.unwrap_or_else(|| "profiling.csv".to_string())));
    let profile = profile_requested || csv_profile_name.is_some();
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let profiling_dir = ensure_profiling_output_dir(&project_dir)?;
    let csv_profile_path = csv_profile_name.as_ref().map(|name| {
        profiling_dir.join(
            name.file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("profile_metrics.csv")),
        )
    });
    if let Some(csv_profile_path) = &csv_profile_path {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(csv_profile_path)
            .map_err(|err| {
                format!(
                    "failed to initialize profile csv {}: {err}",
                    csv_profile_path.display()
                )
            })?;
    }
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    log_step("Building Scripts");
    compile_scripts_with_profile(&project_dir, ScriptsBuildProfile::Debug).map_err(|err| {
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
    build_cmd.arg("build").env("CARGO_TARGET_DIR", &target_dir);
    if release {
        build_cmd.arg("--release");
    }
    build_cmd.current_dir(&dev_runner_dir);
    let mut features = Vec::new();
    if profile {
        features.push("profile");
    }
    if ui_profile {
        features.push("ui_profile");
    }
    if !features.is_empty() {
        build_cmd.arg("--features").arg(features.join(","));
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

    let profile_dir = if release { "release" } else { "debug" };
    let runner_path = if cfg!(target_os = "windows") {
        target_dir.join(profile_dir).join("perro_dev_runner.exe")
    } else {
        target_dir.join(profile_dir).join("perro_dev_runner")
    };
    log_note("Running Dev Runner");

    let mut run_cmd = Command::new(&runner_path);
    run_cmd
        .arg("--path")
        .arg(project_dir.to_string_lossy().to_string())
        .current_dir(&project_dir);
    if let Some(path) = &csv_profile_path {
        run_cmd.env("PERRO_PROFILE_CSV", path.to_string_lossy().to_string());
    }

    let run_status = run_cmd.status().map_err(|err| {
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

fn mem_profile_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let release = args.iter().any(|a| a == "--release");
    let csv_name = parse_optional_flag_value(args, "--csv")
        .map(|raw| PathBuf::from(raw.unwrap_or_else(|| "memory_profile.csv".to_string())));
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let profiling_dir = ensure_profiling_output_dir(&project_dir)?;
    let csv_path = profiling_dir.join(
        csv_name
            .as_ref()
            .and_then(|name| name.file_name())
            .unwrap_or_else(|| std::ffi::OsStr::new("memory_profile.csv")),
    );
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&csv_path)
        .map_err(|err| {
            format!(
                "failed to initialize memory profile csv {}: {err}",
                csv_path.display()
            )
        })?;
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    log_step("Building Scripts");
    compile_scripts_with_profile(&project_dir, ScriptsBuildProfile::Debug).map_err(|err| {
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
    build_cmd.arg("build").env("CARGO_TARGET_DIR", &target_dir);
    if release {
        build_cmd.arg("--release");
    }
    build_cmd.current_dir(&dev_runner_dir);
    build_cmd.arg("--features").arg("mem_profile");
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

    let profile_dir = if release { "release" } else { "debug" };
    let runner_path = if cfg!(target_os = "windows") {
        target_dir.join(profile_dir).join("perro_dev_runner.exe")
    } else {
        target_dir.join(profile_dir).join("perro_dev_runner")
    };
    log_note("Running Dev Runner");

    let mut run_cmd = Command::new(&runner_path);
    run_cmd
        .arg("--path")
        .arg(project_dir.to_string_lossy().to_string())
        .current_dir(&project_dir)
        .env("PERRO_MEM_PROFILE", "1")
        .env(
            "PERRO_MEM_PROFILE_CSV",
            csv_path.to_string_lossy().to_string(),
        );

    let run_status = run_cmd.status().map_err(|err| {
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
    if maybe_relaunch_flamegraph_as_admin(args)? {
        return Ok(());
    }

    let profile = args.iter().any(|a| a == "--profile");
    let root = args.iter().any(|a| a == "--root");
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let profiling_dir = ensure_profiling_output_dir(&project_dir)?;
    let flamegraph_output_path = profiling_dir.join("flamegraph.svg");
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    log_step("Building Scripts");
    compile_scripts_with_profile(&project_dir, ScriptsBuildProfile::Release).map_err(|err| {
        format!(
            "scripts pipeline failed for {}: {err}",
            project_dir.display()
        )
    })?;
    log_done("Scripts Built");

    let dev_runner_dir = project_dir.join(".perro").join("dev_runner");
    let target_dir = project_dir.join("target");
    ensure_cargo_flamegraph_installed()?;
    log_step("Running Flamegraph");

    let mut cmd = Command::new("cargo");
    cmd.arg("flamegraph")
        .arg("-o")
        .arg(flamegraph_output_path.to_string_lossy().to_string())
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
        let mut msg = format!("cargo flamegraph failed with exit code {:?}", status.code());
        if cfg!(target_os = "windows") {
            msg.push_str(
                "\nWindows note: cargo-flamegraph uses blondie + often needs elevated terminal.",
            );
            msg.push_str("\nIf output includes `NotAnAdmin`, rerun PowerShell as Administrator.");
            msg.push_str("\nFallback: run flamegraph in WSL/Linux for full perf support.");
        }
        return Err(msg);
    }

    log_done(&format!(
        "Flamegraph Complete ({})",
        flamegraph_output_path.display()
    ));
    Ok(())
}

fn ensure_profiling_output_dir(project_dir: &Path) -> Result<PathBuf, String> {
    let dir = project_dir.join(".output").join("profiling");
    fs::create_dir_all(&dir).map_err(|err| {
        format!(
            "failed to create profiling output dir {}: {err}",
            dir.display()
        )
    })?;
    Ok(dir)
}

fn maybe_relaunch_flamegraph_as_admin(args: &[String]) -> Result<bool, String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = args;
        Ok(false)
    }

    #[cfg(target_os = "windows")]
    {
        if is_windows_process_elevated()? || !io::stdin().is_terminal() {
            return Ok(false);
        }

        log_note("Windows flamegraph often needs Administrator permission (UAC).");
        let elevate =
            prompt_yes_no("Relaunch this flamegraph command as Administrator now? [y/N] ")?;
        if !elevate {
            return Ok(false);
        }

        relaunch_self_as_admin(args)?;
        Ok(true)
    }
}

#[cfg(target_os = "windows")]
fn is_windows_process_elevated() -> Result<bool, String> {
    let output = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg("[bool](([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator))")
        .output()
        .map_err(|err| format!("failed to check Administrator privilege: {err}"))?;

    if !output.status.success() {
        return Err(format!(
            "failed to check Administrator privilege; PowerShell exited with {:?}",
            output.status.code()
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase();
    match text.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("failed to parse Administrator privilege check output".to_string()),
    }
}

#[cfg(target_os = "windows")]
fn relaunch_self_as_admin(args: &[String]) -> Result<(), String> {
    let current_exe =
        env::current_exe().map_err(|err| format!("failed to locate current executable: {err}"))?;
    let exe = powershell_single_quoted(&current_exe.to_string_lossy());
    let forwarded = args
        .iter()
        .skip(1)
        .map(|arg| format!("'{}'", powershell_single_quoted(arg)))
        .collect::<Vec<_>>()
        .join(", ");
    let arg_list = format!("@({forwarded})");
    let script = format!(
        "$p = Start-Process -FilePath '{exe}' -ArgumentList {arg_list} -Verb RunAs -Wait -PassThru; exit $p.ExitCode"
    );

    let status = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(script)
        .status()
        .map_err(|err| format!("failed to relaunch elevated command: {err}"))?;

    if !status.success() {
        return Err(format!(
            "elevated flamegraph command failed with exit code {:?}",
            status.code()
        ));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn powershell_single_quoted(input: &str) -> String {
    input.replace('\'', "''")
}

fn ensure_cargo_flamegraph_installed() -> Result<(), String> {
    let check_status = Command::new("cargo")
        .arg("flamegraph")
        .arg("--version")
        .status();

    if let Ok(status) = check_status
        && status.success()
    {
        return Ok(());
    }

    log_note("cargo-flamegraph missing; installing via `cargo install flamegraph`");
    let install_status = Command::new("cargo")
        .arg("install")
        .arg("flamegraph")
        .status()
        .map_err(|err| format!("failed to run `cargo install flamegraph`: {err}"))?;

    if !install_status.success() {
        return Err(format!(
            "`cargo install flamegraph` failed with exit code {:?}",
            install_status.code()
        ));
    }

    log_done("cargo-flamegraph Installed");
    Ok(())
}

fn format_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let base_path = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let base_path = base_path.canonicalize().unwrap_or(base_path);
    let res_dir = resolve_project_res_root(&base_path, "format")?;
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

fn clippy_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let res_dir = resolve_project_res_root(&project_dir, "clippy")?;
    let mut script_files = Vec::new();
    collect_rs_files_recursive(&res_dir, &mut script_files)?;

    if script_files.is_empty() {
        log_note("No .rs files found under res");
        return Ok(());
    }

    log_step("Syncing User Scripts");
    ensure_source_overrides(&project_dir)
        .map_err(|err| format!("failed to refresh source overrides: {err}"))?;
    sync_scripts(&project_dir).map_err(|err| format!("failed to sync scripts: {err}"))?;
    log_done("User Scripts Synced");

    log_step("Running Clippy For User Scripts");
    let scripts_crate = project_dir.join(".perro").join("scripts");
    let target_dir = project_dir.join("target");
    let status = Command::new("cargo")
        .arg("clippy")
        .arg("--all-targets")
        .arg("--")
        .arg("-D")
        .arg("warnings")
        .arg("-A")
        .arg("clippy::not_unsafe_ptr_arg_deref")
        .arg("-A")
        .arg("clippy::too_many_arguments")
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(&scripts_crate)
        .status()
        .map_err(|err| {
            format!(
                "failed to run cargo clippy for {}: {err}",
                scripts_crate.display()
            )
        })?;
    if !status.success() {
        return Err(format!(
            "cargo clippy failed for {} with exit code {:?}",
            scripts_crate.display(),
            status.code()
        ));
    }
    log_done("User Scripts Clippy Clean");
    Ok(())
}

fn resolve_project_res_root(path: &Path, command: &str) -> Result<PathBuf, String> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    // `--path` must point at project root.
    if path.join("project.toml").exists() {
        return Ok(path.join("res"));
    }

    Err(format!(
        "invalid --path `{}` for {command}. Use project root (directory containing project.toml).",
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

use perro_compiler::{compile_project_bundle, compile_scripts};
use perro_project::create_new_project;
use serde_json::Value;
use std::env;
use std::fs;
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
            "install" => install_command(&args),
            "check" => scripts_command(&args, &cwd),
            "build" => project_command(&args, &cwd),
            "dev" => dev_command(&args, &cwd),
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
    eprintln!("  perro_cli build [--path <project_dir>]    # full static project bundle + build");
    eprintln!("  perro_cli dev [--path <project_dir>]      # build scripts + run dev runner");
    eprintln!("  perro_cli format [--path <project_dir>]   # rustfmt .rs under project res only");
    eprintln!(
        "  perro_cli install                          # add `perro` source-mode command (PowerShell)"
    );
    eprintln!("  perro_cli new [--path <parent_dir>] [--name <project_name>]");
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
    let repo_root = normalize_powershell_path(&workspace_root()).replace('\\', "\\\\");
    let snippet = format!(
        "{PROFILE_SNIPPET_BEGIN}\n\
function perro {{\n\
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Args)\n\
    Push-Location \"{repo_root}\"\n\
    try {{\n\
        cargo run -p perro_cli -- @Args\n\
    }} finally {{\n\
        Pop-Location\n\
    }}\n\
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
    let base_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_name =
        parse_flag_value(args, "--name").unwrap_or_else(|| DEFAULT_PROJECT_NAME.to_string());
    let project_dir = base_dir.join(sanitize_project_dir_name(&project_name));

    create_new_project(&project_dir, &project_name).map_err(|err| {
        format!(
            "failed to create project at {}: {err}",
            project_dir.display()
        )
    })?;
    log_step("Building Scripts");
    compile_scripts(&project_dir).map_err(|err| {
        format!(
            "scripts pipeline failed for {}: {err}",
            project_dir.display()
        )
    })?;
    log_done("Scripts Built");
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    println!(
        "created project `{}` at {}",
        project_name,
        project_dir.display()
    );
    Ok(())
}

fn update_workspace_vscode_linked_projects(
    workspace_root: &Path,
    project_dir: &Path,
) -> Result<(), String> {
    let settings_path = workspace_root.join(".vscode").join("settings.json");
    if !settings_path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(&settings_path)
        .map_err(|err| format!("failed to read {}: {err}", settings_path.display()))?;
    let mut json: Value = serde_json::from_str(&raw)
        .map_err(|err| format!("failed to parse {} as JSON: {err}", settings_path.display()))?;
    let Some(root) = json.as_object_mut() else {
        return Err(format!(
            "expected {} to contain a JSON object",
            settings_path.display()
        ));
    };

    let workspace_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let scripts_manifest = project_dir
        .join(".perro")
        .join("scripts")
        .join("Cargo.toml")
        .canonicalize()
        .unwrap_or_else(|_| {
            project_dir
                .join(".perro")
                .join("scripts")
                .join("Cargo.toml")
        });
    let res_dir = project_dir
        .join("res")
        .canonicalize()
        .unwrap_or_else(|_| project_dir.join("res"));
    let Ok(rel) = scripts_manifest
        .strip_prefix(&workspace_root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
    else {
        // External project path: skip workspace-level VS Code wiring.
        return Ok(());
    };
    let Ok(rel_res) = res_dir
        .strip_prefix(&workspace_root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
    else {
        // External project path: skip workspace-level VS Code wiring.
        return Ok(());
    };
    let vfs_entry = format!("${{workspaceFolder}}/{rel_res}/");

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

    let already_present = arr.iter().any(|v| v.as_str() == Some(rel.as_str()));
    if already_present {
        // Keep going to also normalize/update vfs.extraIncludes.
    } else {
        arr.push(Value::String(rel));
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

    let vfs_present = vfs_arr
        .iter()
        .any(|v| v.as_str() == Some(vfs_entry.as_str()));
    if !vfs_present {
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
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());

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

    let build_status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .env("CARGO_TARGET_DIR", &target_dir)
        .current_dir(&dev_runner_dir)
        .status()
        .map_err(|err| {
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

fn format_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let base_path = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
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
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    log_step("Building Project Bundle");
    compile_project_bundle(&project_dir)
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

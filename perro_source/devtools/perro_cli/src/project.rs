use crate::install::normalize_powershell_path;
use crate::profiling::ensure_profiling_output_dir;
use crate::scaffold::validate_dlc_name;
use crate::vscode::{
    update_project_vscode_linked_projects, update_workspace_vscode_linked_projects,
};
use crate::{
    find_project_root, log_done, log_note, log_step, parse_flag_value, parse_optional_flag_value,
    resolve_local_path, workspace_root,
};
use perro_compiler::{
    ProjectBuildOptions, ScriptsBuildProfile, compile_dlc_bundle, compile_project_bundle,
    compile_scripts_with_profile, sync_scripts,
};
use perro_project::{ensure_source_overrides, load_project_toml};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn clean_command(args: &[String], _cwd: &Path) -> Result<(), String> {
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

pub(crate) fn maybe_open_file_in_editor(args: &[String], file_path: &Path) -> Result<(), String> {
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

pub(crate) fn maybe_open_project_in_new_window(project_dir: &Path) -> Result<(), String> {
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

pub(crate) fn prompt_yes_no(prompt: &str) -> Result<bool, String> {
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

pub(crate) fn prompt_text(prompt: &str, default: Option<&str>) -> Result<String, String> {
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

pub(crate) fn scripts_command(args: &[String], cwd: &Path) -> Result<(), String> {
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

pub(crate) fn dlc_command(args: &[String], cwd: &Path) -> Result<(), String> {
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

pub(crate) fn dev_command(args: &[String], cwd: &Path) -> Result<(), String> {
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
    let project_cfg = load_project_toml(&project_dir)
        .map_err(|err| format!("failed to load project.toml: {err}"))?;
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
    if project_cfg.steam.enabled {
        features.push("steamworks");
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

pub(crate) fn format_command(args: &[String], cwd: &Path) -> Result<(), String> {
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

pub(crate) fn clippy_command(args: &[String], cwd: &Path) -> Result<(), String> {
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

pub(crate) fn collect_rs_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
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

pub(crate) fn project_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let profile = args.iter().any(|a| a == "--profile");
    let console = args.iter().any(|a| a == "--console");
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;
    log_step("Building Project Bundle");
    compile_project_bundle(&project_dir, ProjectBuildOptions::new(profile, console))
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

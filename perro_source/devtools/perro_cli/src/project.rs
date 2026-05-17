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
    ProjectBuildOptions, ProjectBuildTarget, ScriptsBuildProfile, WebOutputDir, compile_dlc_bundle,
    compile_project_bundle, compile_scripts_with_profile, sync_scripts,
};
use perro_project::{ensure_source_overrides, load_project_toml};
use std::env;
use std::fs;
use std::io::Read;
use std::io::{self, IsTerminal, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CliTarget {
    Native,
    Web,
    Android,
}

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
    let target = parse_cli_target(args)?;
    if target == CliTarget::Web {
        return dev_web_command(args, cwd);
    }
    if target == CliTarget::Android {
        return dev_android_command(args, cwd);
    }
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
    let target = parse_cli_target(args)?;
    if target == CliTarget::Web {
        return build_web_command(args, cwd);
    }
    if target == CliTarget::Android {
        return build_android_command(args, cwd);
    }
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

fn parse_cli_target(args: &[String]) -> Result<CliTarget, String> {
    let Some(raw) = parse_flag_value(args, "--target") else {
        return Ok(CliTarget::Native);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "native" => Ok(CliTarget::Native),
        "web" => Ok(CliTarget::Web),
        "android" => Ok(CliTarget::Android),
        other => Err(format!(
            "invalid `--target {other}`. use `native`, `web`, or `android`."
        )),
    }
}

fn build_web_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let profile = args.iter().any(|a| a == "--profile");
    if args.iter().any(|a| a == "--console") {
        return Err("`--console` is not supported with `--target web`".to_string());
    }
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;
    log_step("Building Web Project Bundle");
    compile_project_bundle(
        &project_dir,
        ProjectBuildOptions::new(profile, false)
            .with_target(ProjectBuildTarget::Web)
            .with_web_output_dir(WebOutputDir::Build),
    )
    .map(|_| {
        log_done("Web Project Bundle Built");
    })
    .map_err(|err| {
        format!(
            "web project pipeline failed for {}: {err}",
            project_dir.display()
        )
    })
}

fn build_android_command(args: &[String], cwd: &Path) -> Result<(), String> {
    if args.iter().any(|a| a == "--console") {
        return Err("`--console` is not supported with `--target android`".to_string());
    }
    let profile = args.iter().any(|a| a == "--profile");
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    let android = prepare_android_toolchain()?;

    log_step("Building Android Project Bundle");
    compile_project_bundle(
        &project_dir,
        ProjectBuildOptions::new(profile, false)
            .with_target(ProjectBuildTarget::Android)
            .with_android_sdk_root(Some(leak_string(
                android.sdk_root.to_string_lossy().to_string(),
            )))
            .with_android_ndk_root(Some(leak_string(
                android.ndk_root.to_string_lossy().to_string(),
            ))),
    )
    .map(|_| {
        log_done("Android Project Bundle Built");
    })
    .map_err(|err| {
        format!(
            "android project pipeline failed for {}: {err}",
            project_dir.display()
        )
    })
}

fn dev_android_command(args: &[String], cwd: &Path) -> Result<(), String> {
    if args.iter().any(|a| a == "--ui-profile") {
        return Err(
            "`--ui-profile` is not supported with `perro dev --target android` yet".to_string(),
        );
    }
    if args.iter().any(|a| a == "--csv-profile") {
        return Err(
            "`--csv-profile` is not supported with `perro dev --target android` yet".to_string(),
        );
    }
    if args.iter().any(|a| a == "--console") {
        return Err("`--console` is not supported with `--target android`".to_string());
    }

    let profile = args.iter().any(|a| a == "--profile");
    let release = args.iter().any(|a| a == "--release");
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    let android = prepare_android_toolchain()?;

    log_step("Building Android Dev Bundle");
    compile_project_bundle(
        &project_dir,
        ProjectBuildOptions::new(profile, false)
            .with_target(ProjectBuildTarget::Android)
            .with_release(release)
            .with_android_sdk_root(Some(leak_string(
                android.sdk_root.to_string_lossy().to_string(),
            )))
            .with_android_ndk_root(Some(leak_string(
                android.ndk_root.to_string_lossy().to_string(),
            ))),
    )
    .map_err(|err| {
        format!(
            "android dev pipeline failed for {}: {err}",
            project_dir.display()
        )
    })?;
    log_done("Android Dev Bundle Built");

    log_note("Running Android App");
    run_android_project(&project_dir, &android, release)
}

fn leak_string(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

struct AndroidToolchain {
    sdk_root: PathBuf,
    ndk_root: PathBuf,
}

fn prepare_android_toolchain() -> Result<AndroidToolchain, String> {
    ensure_rust_target_installed("aarch64-linux-android")?;
    ensure_cargo_apk_installed()?;
    let sdk_root = find_android_sdk_root().ok_or_else(android_sdk_missing_error)?;
    let ndk_root =
        find_android_ndk_root(&sdk_root).ok_or_else(|| android_ndk_missing_error(&sdk_root))?;
    Ok(AndroidToolchain { sdk_root, ndk_root })
}

fn ensure_rust_target_installed(target: &str) -> Result<(), String> {
    let output = Command::new("rustup")
        .arg("target")
        .arg("list")
        .arg("--installed")
        .output()
        .map_err(|err| format!("failed to run `rustup target list --installed`: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "`rustup target list --installed` failed with exit code {:?}",
            output.status.code()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.lines().any(|line| line.trim() == target) {
        return Ok(());
    }

    log_note(&format!("Installing Rust target {target}"));
    let status = Command::new("rustup")
        .arg("target")
        .arg("add")
        .arg(target)
        .status()
        .map_err(|err| format!("failed to run `rustup target add {target}`: {err}"))?;
    if !status.success() {
        return Err(format!(
            "`rustup target add {target}` failed with exit code {:?}",
            status.code()
        ));
    }
    Ok(())
}

fn ensure_cargo_apk_installed() -> Result<(), String> {
    let status = Command::new("cargo").arg("apk").arg("--help").status();
    if status.as_ref().is_ok_and(|status| status.success()) {
        return Ok(());
    }

    log_note("Installing cargo-apk");
    let install_status = Command::new("cargo")
        .arg("install")
        .arg("cargo-apk")
        .arg("--locked")
        .status()
        .map_err(|err| format!("failed to run `cargo install cargo-apk --locked`: {err}"))?;
    if !install_status.success() {
        return Err(format!(
            "`cargo install cargo-apk --locked` failed with exit code {:?}",
            install_status.code()
        ));
    }
    Ok(())
}

fn run_android_project(
    project_dir: &Path,
    android: &AndroidToolchain,
    release: bool,
) -> Result<(), String> {
    let project_crate = project_dir.join(".perro").join("project");
    let target_dir = project_dir.join("target");
    let mut cmd = Command::new("cargo");
    cmd.arg("apk")
        .arg("run")
        .arg("--lib")
        .arg("--target")
        .arg("aarch64-linux-android")
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("ANDROID_SDK_ROOT", &android.sdk_root)
        .env("ANDROID_HOME", &android.sdk_root)
        .env("ANDROID_NDK_ROOT", &android.ndk_root)
        .env("ANDROID_NDK_HOME", &android.ndk_root)
        .env("NDK_HOME", &android.ndk_root)
        .current_dir(&project_crate);
    if release {
        cmd.arg("--release");
    }
    let status = cmd.status().map_err(|err| {
        format!(
            "failed to launch cargo apk run from {}: {err}",
            project_crate.display()
        )
    })?;
    if !status.success() {
        return Err(format!(
            "cargo apk run failed with exit code {:?}. ensure an emulator or android device is available.",
            status.code()
        ));
    }
    log_done("Android App Finished");
    Ok(())
}

fn find_android_sdk_root() -> Option<PathBuf> {
    let mut candidates = Vec::<PathBuf>::new();
    for key in ["ANDROID_SDK_ROOT", "ANDROID_HOME"] {
        if let Some(value) = env::var_os(key) {
            candidates.push(PathBuf::from(value));
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local_app_data).join("Android").join("Sdk"));
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = env::var_os("HOME") {
            candidates.push(PathBuf::from(&home).join("Android").join("Sdk"));
            candidates.push(
                PathBuf::from(home)
                    .join("Library")
                    .join("Android")
                    .join("sdk"),
            );
        }
    }

    candidates
        .into_iter()
        .find(|path| path.join("platform-tools").exists() || path.join("ndk").exists())
}

fn find_android_ndk_root(sdk_root: &Path) -> Option<PathBuf> {
    for key in ["ANDROID_NDK_ROOT", "ANDROID_NDK_HOME", "NDK_HOME"] {
        if let Some(value) = env::var_os(key) {
            let path = PathBuf::from(value);
            if path.exists() {
                return Some(path);
            }
        }
    }

    let ndk_bundle = sdk_root.join("ndk-bundle");
    if ndk_bundle.exists() {
        return Some(ndk_bundle);
    }
    let ndk_dir = sdk_root.join("ndk");
    if !ndk_dir.exists() {
        return None;
    }
    let mut versions = fs::read_dir(&ndk_dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    versions.sort();
    versions.pop()
}

fn android_sdk_missing_error() -> String {
    "android sdk not found. set ANDROID_SDK_ROOT or ANDROID_HOME, or install Android SDK in the default platform location.".to_string()
}

fn android_ndk_missing_error(sdk_root: &Path) -> String {
    format!(
        "android ndk not found. set ANDROID_NDK_ROOT / ANDROID_NDK_HOME / NDK_HOME, or install an NDK under `{}`.",
        sdk_root.display()
    )
}

fn dev_web_command(args: &[String], cwd: &Path) -> Result<(), String> {
    if args.iter().any(|a| a == "--ui-profile") {
        return Err(
            "`--ui-profile` is not supported with `perro dev --target web` yet".to_string(),
        );
    }
    if args.iter().any(|a| a == "--csv-profile") {
        return Err(
            "`--csv-profile` is not supported with `perro dev --target web` yet".to_string(),
        );
    }
    let profile = args.iter().any(|a| a == "--profile");
    let release = args.iter().any(|a| a == "--release");
    let host = parse_flag_value(args, "--host").unwrap_or_else(|| "127.0.0.1".to_string());
    let requested_port = parse_flag_value(args, "--port")
        .map(|raw| {
            raw.parse::<u16>()
                .map_err(|_| format!("invalid `--port {raw}`"))
        })
        .transpose()?;
    let port = requested_port.unwrap_or(8000);
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;
    log_step("Building Web Dev Bundle");
    compile_project_bundle(
        &project_dir,
        ProjectBuildOptions::new(profile, false)
            .with_target(ProjectBuildTarget::Web)
            .with_release(release)
            .with_web_output_dir(WebOutputDir::Dev),
    )
    .map_err(|err| {
        format!(
            "web dev pipeline failed for {}: {err}",
            project_dir.display()
        )
    })?;
    log_done("Web Dev Bundle Built");

    let output_dir = project_dir.join(".output").join("web-dev");
    let (listener, port) = bind_web_dev_listener(&host, port)?;
    let url = format!("http://{host}:{port}/");
    log_note(&format!("Web Dev Bundle -> {}", output_dir.display()));
    if port != requested_port.unwrap_or(8000) {
        log_note(&format!("Port busy -> use {port}"));
    }
    log_note(&format!("Serving {url}"));
    open_browser(&url)?;
    log_note("Stop w/ Ctrl+C");
    run_static_server(&output_dir, listener)
}

fn open_browser(url: &str) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/c").arg("start").arg("").arg(url);
        cmd
    } else if cfg!(target_os = "macos") {
        let mut cmd = Command::new("open");
        cmd.arg(url);
        cmd
    } else {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(url);
        cmd
    };
    let status = cmd
        .status()
        .map_err(|err| format!("failed to open browser for {url}: {err}"))?;
    if !status.success() {
        return Err(format!(
            "browser open failed for {url} with exit code {:?}",
            status.code()
        ));
    }
    Ok(())
}

fn bind_web_dev_listener(host: &str, start_port: u16) -> Result<(TcpListener, u16), String> {
    let mut port = start_port;
    loop {
        match TcpListener::bind((host, port)) {
            Ok(listener) => return Ok((listener, port)),
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                port = port
                    .checked_add(1)
                    .ok_or_else(|| format!("no free web dev port from {start_port}..65535"))?;
            }
            Err(err) => {
                return Err(format!(
                    "failed to bind web dev server on {host}:{port}: {err}"
                ));
            }
        }
    }
}

fn run_static_server(root: &Path, listener: TcpListener) -> Result<(), String> {
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("failed to set nonblocking listener: {err}"))?;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = handle_http_connection(stream, root);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(err) => return Err(format!("web dev server accept failed: {err}")),
        }
    }
}

fn handle_http_connection(mut stream: TcpStream, root: &Path) -> Result<(), String> {
    let mut buffer = [0u8; 4096];
    let read_len = stream
        .read(&mut buffer)
        .map_err(|err| format!("http read failed: {err}"))?;
    if read_len == 0 {
        return Ok(());
    }
    let request = String::from_utf8_lossy(&buffer[..read_len]);
    let first_line = request.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let raw_path = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        return write_http_response(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            b"method not allowed",
        );
    }
    let rel = raw_path.split('?').next().unwrap_or("/");
    let rel = rel.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    let path = root.join(rel.replace('/', "\\"));
    let path = if path.is_dir() {
        path.join("index.html")
    } else {
        path
    };
    if !path.exists() {
        return write_http_response(
            &mut stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"not found",
        );
    }
    let body =
        fs::read(&path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    write_http_response(&mut stream, "200 OK", content_type_for_path(&path), &body)
}

fn write_http_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nCache-Control: no-store, no-cache, must-revalidate\r\nPragma: no-cache\r\nExpires: 0\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .and_then(|_| stream.write_all(body))
        .map_err(|err| format!("http write failed: {err}"))
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "mjs" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::bind_web_dev_listener;
    use std::net::TcpListener;

    #[test]
    fn bind_web_dev_listener_bumps_busy_port() {
        let busy = TcpListener::bind(("127.0.0.1", 0)).expect("bind busy listener");
        let busy_port = busy.local_addr().expect("read busy addr").port();

        let (free, free_port) =
            bind_web_dev_listener("127.0.0.1", busy_port).expect("bind free listener");

        assert_eq!(free_port, busy_port + 1);
        drop(free);
    }
}

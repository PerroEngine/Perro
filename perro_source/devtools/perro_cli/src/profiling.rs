use crate::project::prompt_yes_no;
use crate::vscode::{
    update_project_vscode_linked_projects, update_workspace_vscode_linked_projects,
};
use crate::{
    log_done, log_note, log_step, parse_flag_value, parse_optional_flag_value, resolve_local_path,
    workspace_root,
};
use perro_compiler::{ScriptsBuildProfile, compile_scripts_with_profile};
use std::env;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn mem_profile_command(args: &[String], cwd: &Path) -> Result<(), String> {
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

pub(crate) fn flamegraph_command(args: &[String], cwd: &Path) -> Result<(), String> {
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

pub(crate) fn ensure_profiling_output_dir(project_dir: &Path) -> Result<PathBuf, String> {
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

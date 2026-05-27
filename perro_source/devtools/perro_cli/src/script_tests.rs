use crate::{
    find_project_root, log_done, log_note, log_step, parse_flag_value, resolve_local_path,
};
use perro_compiler::sync_scripts;
use perro_project::{ensure_source_overrides, load_project_toml};
use std::path::Path;
use std::process::Command;

pub(crate) fn test_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .or_else(|| find_project_root(cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    if !project_dir.join("project.toml").exists() {
        return Err(format!(
            "invalid --path `{}` for test. Use project root (directory containing project.toml).",
            project_dir.display()
        ));
    }

    log_step("Syncing Test Scripts");
    ensure_source_overrides(&project_dir)
        .map_err(|err| format!("failed to refresh source overrides: {err}"))?;
    sync_scripts(&project_dir).map_err(|err| format!("failed to sync scripts: {err}"))?;
    log_done("Test Scripts Synced");

    log_note("Running Script Tests");
    let project_cfg = load_project_toml(&project_dir)
        .map_err(|err| format!("failed to load project.toml: {err}"))?;
    let scripts_crate = project_dir.join(".perro").join("scripts");
    let target_dir = project_dir.join("target");
    let mut cmd = Command::new("cargo");
    cmd.arg("test")
        .env("CARGO_TARGET_DIR", &target_dir)
        .current_dir(&scripts_crate);
    if project_cfg.steam.enabled {
        cmd.arg("--features").arg("steamworks");
    }
    cmd.args(passthrough_args(args));

    let status = cmd.status().map_err(|err| {
        format!(
            "failed to run cargo test from {}: {err}",
            scripts_crate.display()
        )
    })?;
    if !status.success() {
        return Err(format!(
            "cargo test failed with exit code {:?}",
            status.code()
        ));
    }
    log_done("Script Tests Finished");
    Ok(())
}

fn passthrough_args(args: &[String]) -> &[String] {
    let Some(idx) = args.iter().position(|arg| arg == "--") else {
        return &[];
    };
    &args[idx + 1..]
}

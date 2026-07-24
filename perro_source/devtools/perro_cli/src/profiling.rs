use crate::project::copy_steam_runtime_library;
#[cfg(target_os = "windows")]
use crate::project::prompt_yes_no;
use crate::vscode::{
    update_project_vscode_linked_projects, update_workspace_vscode_linked_projects,
};
use crate::{
    log_done, log_note, log_step, parse_flag_value, parse_optional_flag_value, resolve_local_path,
    workspace_root,
};
use perro_compiler::{ScriptsBuildProfile, compile_scripts_with_profile};
use perro_project::{ensure_source_overrides, load_project_toml};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::{
    env,
    io::{self, IsTerminal},
};

pub(crate) fn mem_profile_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let release = args.iter().any(|a| a == "--release");
    let csv_name = parse_optional_flag_value(args, "--csv")
        .map(|raw| PathBuf::from(raw.unwrap_or_else(|| "memory_profile.csv".to_string())));
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let project_cfg = load_project_toml(&project_dir)
        .map_err(|err| format!("failed to load project.toml: {err}"))?;
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
    ensure_source_overrides(&project_dir)
        .map_err(|err| format!("failed to refresh source overrides: {err}"))?;

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
    let mut features = vec!["mem_profile"];
    if project_cfg.steam.enabled {
        features.push("steamworks");
    }
    build_cmd.arg("--features").arg(features.join(","));
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
    if project_cfg.steam.enabled {
        copy_steam_runtime_library(&target_dir, profile_dir, &target_dir.join(profile_dir))?;
    }
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

pub(crate) fn spec_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let target_fps = parse_flag_value(args, "--target-fps")
        .map(|raw| {
            raw.parse::<f64>()
                .map_err(|_| format!("invalid --target-fps `{raw}`"))
        })
        .transpose()?
        .unwrap_or(60.0);
    if !target_fps.is_finite() || target_fps <= 0.0 {
        return Err("--target-fps must be > 0".to_string());
    }

    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let project_cfg = load_project_toml(&project_dir)
        .map_err(|err| format!("failed to load project.toml: {err}"))?;
    let output_dir = ensure_profiling_output_dir(&project_dir)?.join("spec");
    fs::create_dir_all(&output_dir)
        .map_err(|err| format!("failed to create {}: {err}", output_dir.display()))?;
    let samples_path = output_dir.join("samples.csv");
    let frames_path = output_dir.join("frames.csv");
    let markers_path = output_dir.join("markers.jsonl");
    fs::write(&samples_path, "")
        .map_err(|err| format!("failed to reset {}: {err}", samples_path.display()))?;
    fs::write(&markers_path, "")
        .map_err(|err| format!("failed to reset {}: {err}", markers_path.display()))?;
    fs::write(&frames_path, "")
        .map_err(|err| format!("failed to reset {}: {err}", frames_path.display()))?;

    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;
    ensure_source_overrides(&project_dir)
        .map_err(|err| format!("failed to refresh source overrides: {err}"))?;

    log_step("Building Spec Scripts");
    compile_scripts_with_profile(&project_dir, ScriptsBuildProfile::Spec).map_err(|err| {
        format!(
            "scripts pipeline failed for {}: {err}",
            project_dir.display()
        )
    })?;
    log_done("Spec Scripts Built");

    let dev_runner_dir = project_dir.join(".perro").join("dev_runner");
    let target_dir = project_dir.join("target");
    log_step("Building Spec Runner");
    let mut build_cmd = Command::new("cargo");
    build_cmd
        .arg("build")
        .arg("--release")
        .env("CARGO_TARGET_DIR", &target_dir)
        .current_dir(&dev_runner_dir);
    let mut features = vec!["mem_profile"];
    if project_cfg.steam.enabled {
        features.push("steamworks");
    }
    build_cmd.arg("--features").arg(features.join(","));
    let status = build_cmd
        .status()
        .map_err(|err| format!("failed to build spec runner: {err}"))?;
    if !status.success() {
        return Err(format!("spec runner build failed with {:?}", status.code()));
    }
    log_done("Spec Runner Built");

    let runner_path = if cfg!(target_os = "windows") {
        target_dir.join("release").join("perro_dev_runner.exe")
    } else {
        target_dir.join("release").join("perro_dev_runner")
    };
    if project_cfg.steam.enabled {
        copy_steam_runtime_library(&target_dir, "release", &target_dir.join("release"))?;
    }
    log_note("Run Test Path; Close Game To Build Report");
    let status = Command::new(&runner_path)
        .arg("--path")
        .arg(project_dir.to_string_lossy().to_string())
        .current_dir(&project_dir)
        .env("PERRO_MEM_PROFILE", "1")
        .env("PERRO_MEM_PROFILE_CSV", &samples_path)
        .env("PERRO_TIMING_CSV", &frames_path)
        .env("PERRO_SPEC_MARKERS", &markers_path)
        .status()
        .map_err(|err| format!("failed to launch spec runner: {err}"))?;
    if !status.success() {
        return Err(format!("spec runner failed with {:?}", status.code()));
    }

    write_spec_report(
        &output_dir,
        &samples_path,
        &frames_path,
        &markers_path,
        target_fps,
    )?;
    log_done(&format!("Spec Report ({})", output_dir.display()));
    Ok(())
}

fn write_spec_report(
    output_dir: &Path,
    samples_path: &Path,
    frames_path: &Path,
    markers_path: &Path,
    target_fps: f64,
) -> Result<(), String> {
    let samples = read_spec_samples(samples_path)?;
    if samples.is_empty() {
        return Err("spec run wrote no samples; keep game open past warmup".to_string());
    }
    let frames = read_frame_samples(frames_path)?;
    if frames.is_empty() {
        return Err("spec run wrote no frame samples".to_string());
    }
    let peak_rss_mib = samples
        .iter()
        .map(|row| row.rss_mib)
        .fold(0.0_f64, f64::max);
    let update_p95_us = percentile(frames.iter().map(|row| row.update_us).collect(), 0.95);
    let render_p95_us = percentile(frames.iter().map(|row| row.render_us).collect(), 0.95);
    let frame_p95_us = percentile(frames.iter().map(|row| row.frame_us).collect(), 0.95);
    let frame_p99_us = percentile(frames.iter().map(|row| row.frame_us).collect(), 0.99);
    let min_fps = frames
        .iter()
        .filter(|row| row.frame_us > 0.0)
        .map(|row| 1_000_000.0 / row.frame_us)
        .filter(|value| value.is_finite())
        .fold(f64::INFINITY, f64::min);
    let target_frame_us = 1_000_000.0 / target_fps;
    let min_cpu_ratio = (update_p95_us / target_frame_us * 1.25).clamp(0.1, 2.0);
    let min_gpu_ratio = (render_p95_us / target_frame_us * 1.25).clamp(0.1, 2.0);
    let reco_cpu_ratio = (update_p95_us / target_frame_us * 1.6).clamp(0.1, 2.0);
    let reco_gpu_ratio = (render_p95_us / target_frame_us * 1.6).clamp(0.1, 2.0);
    let min_ram_gib = common_ram_tier((peak_rss_mib / 1024.0 + 4.0) * 1.25);
    let reco_ram_gib = common_ram_tier((peak_rss_mib / 1024.0 + 4.0) * 1.5);
    let hardware = test_hardware();
    let markers = read_markers(markers_path)?;
    let segments = summarize_segments(&markers, &frames);
    let report = serde_json::json!({
        "schema": 1,
        "estimate": true,
        "target_fps": target_fps,
        "test_pc": hardware,
        "observed": {
            "sample_batches": samples.len(),
            "sample_frames": frames.len(),
            "peak_game_rss_mib": peak_rss_mib,
            "p95_update_us": update_p95_us,
            "p95_render_cpu_us": render_p95_us,
            "p95_frame_us": frame_p95_us,
            "p99_frame_us": frame_p99_us,
            "lowest_frame_fps": if min_fps.is_finite() { min_fps } else { 0.0 }
        },
        "minimum": {
            "cpu_relative_to_test_pc": min_cpu_ratio,
            "gpu_relative_to_test_pc": min_gpu_ratio,
            "system_ram_gib": min_ram_gib,
            "confidence": {"cpu": "low", "gpu": "low", "ram": "medium"}
        },
        "recommended": {
            "cpu_relative_to_test_pc": reco_cpu_ratio,
            "gpu_relative_to_test_pc": reco_gpu_ratio,
            "system_ram_gib": reco_ram_gib,
            "confidence": {"cpu": "low", "gpu": "low", "ram": "medium"}
        },
        "markers": markers,
        "segments": segments,
        "notes": [
            "render timing is CPU-side until GPU timestamp queries land",
            "CPU and GPU equivalents need a versioned benchmark-score database",
            "system RAM includes game peak plus OS/background reserve and headroom"
        ]
    });
    let json = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("failed to encode spec report: {err}"))?;
    fs::write(output_dir.join("report.json"), json)
        .map_err(|err| format!("failed to write spec JSON: {err}"))?;

    let cpu = report["test_pc"]["cpu"].as_str().unwrap_or("unknown");
    let gpu = report["test_pc"]["gpu"].as_str().unwrap_or("unknown");
    let markdown = format!(
        "# Perro Spec Estimate\n\n\
         Target: {target_fps:.0} FPS\n\n\
         Test CPU: {cpu}\n\n\
         Test GPU: {gpu}\n\n\
         Peak game RAM: {peak_rss_mib:.0} MiB\n\n\
         P95 update: {update_p95_us:.0} us\n\n\
         P95 render (CPU-side): {render_p95_us:.0} us\n\n\
         P95 frame: {frame_p95_us:.0} us\n\n\
         P99 frame: {frame_p99_us:.0} us\n\n\
         ## Minimum\n\n\
         CPU: {min_cpu_ratio:.2}x test-PC performance\n\n\
         GPU: {min_gpu_ratio:.2}x test-PC performance\n\n\
         Memory: {min_ram_gib} GB system RAM\n\n\
         ## Recommended\n\n\
         CPU: {reco_cpu_ratio:.2}x test-PC performance\n\n\
         GPU: {reco_gpu_ratio:.2}x test-PC performance\n\n\
         Memory: {reco_ram_gib} GB system RAM\n\n\
         Estimate only. Validate on target hardware before publish.\n"
    );
    fs::write(output_dir.join("report.md"), markdown)
        .map_err(|err| format!("failed to write spec Markdown: {err}"))?;
    let steam = format!(
        "MINIMUM:\nProcessor: {min_cpu_ratio:.2}x {cpu} performance or equivalent\n\
         Memory: {min_ram_gib} GB RAM\n\
         Graphics: {min_gpu_ratio:.2}x {gpu} performance or equivalent\n\n\
         RECOMMENDED:\nProcessor: {reco_cpu_ratio:.2}x {cpu} performance or equivalent\n\
         Memory: {reco_ram_gib} GB RAM\n\
         Graphics: {reco_gpu_ratio:.2}x {gpu} performance or equivalent\n"
    );
    fs::write(output_dir.join("steam.txt"), steam)
        .map_err(|err| format!("failed to write Steam draft: {err}"))?;
    Ok(())
}

#[derive(Clone, Copy)]
struct SpecSample {
    rss_mib: f64,
}

fn read_spec_samples(path: &Path) -> Result<Vec<SpecSample>, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 11 || cols[0] == "batch_end_frame" {
            continue;
        }
        let parse = |index: usize| cols[index].parse::<f64>().ok();
        if let Some(rss_mib) = parse(3) {
            rows.push(SpecSample { rss_mib });
        }
    }
    Ok(rows)
}

#[derive(Clone, Copy)]
struct SpecFrameSample {
    update_us: f64,
    render_us: f64,
    frame_us: f64,
    timestamp_ms: u64,
}

fn read_frame_samples(path: &Path) -> Result<Vec<SpecFrameSample>, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 16 || cols[0] == "frame" || cols[2] != "0" {
            continue;
        }
        let parse = |index: usize| cols[index].parse::<f64>().ok();
        if let (Some(frame_us), Some(update_us), Some(render_us), Ok(timestamp_ms)) =
            (parse(4), parse(6), parse(7), cols[15].parse::<u64>())
        {
            rows.push(SpecFrameSample {
                update_us,
                render_us,
                frame_us,
                timestamp_ms,
            });
        }
    }
    Ok(rows)
}

fn percentile(mut values: Vec<f64>, percentile: f64) -> f64 {
    values.retain(|value| value.is_finite());
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        return 0.0;
    }
    let index = ((values.len() - 1) as f64 * percentile).round() as usize;
    values[index]
}

fn common_ram_tier(required_gib: f64) -> u64 {
    [4, 8, 12, 16, 24, 32, 64, 128]
        .into_iter()
        .find(|tier| *tier as f64 >= required_gib)
        .unwrap_or(128)
}

fn read_markers(path: &Path) -> Result<Vec<serde_json::Value>, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    Ok(text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect())
}

fn summarize_segments(
    markers: &[serde_json::Value],
    frames: &[SpecFrameSample],
) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for begin in markers
        .iter()
        .filter(|marker| marker["kind"].as_str() == Some("begin"))
    {
        let Some(label) = begin["label"].as_str() else {
            continue;
        };
        let Some(start_ms) = begin["timestamp_ms"].as_u64() else {
            continue;
        };
        let Some(end_ms) = markers.iter().find_map(|marker| {
            (marker["kind"].as_str() == Some("end")
                && marker["label"].as_str() == Some(label)
                && marker["timestamp_ms"]
                    .as_u64()
                    .is_some_and(|end| end >= start_ms))
            .then(|| marker["timestamp_ms"].as_u64())
            .flatten()
        }) else {
            continue;
        };
        let segment: Vec<SpecFrameSample> = frames
            .iter()
            .copied()
            .filter(|frame| frame.timestamp_ms >= start_ms && frame.timestamp_ms <= end_ms)
            .collect();
        if segment.is_empty() {
            continue;
        }
        out.push(serde_json::json!({
            "label": label,
            "start_ms": start_ms,
            "end_ms": end_ms,
            "frames": segment.len(),
            "p95_update_us": percentile(segment.iter().map(|row| row.update_us).collect(), 0.95),
            "p95_render_cpu_us": percentile(segment.iter().map(|row| row.render_us).collect(), 0.95),
            "p95_frame_us": percentile(segment.iter().map(|row| row.frame_us).collect(), 0.95)
        }));
    }
    out
}

fn test_hardware() -> HashMap<&'static str, String> {
    let mut out = HashMap::new();
    let cpu = if cfg!(target_os = "windows") {
        windows_cim_name("Win32_Processor")
    } else {
        env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unknown".to_string())
    };
    out.insert("cpu", cpu);
    out.insert("gpu", windows_cim_name("Win32_VideoController"));
    out.insert("os", env::consts::OS.to_string());
    out
}

fn windows_cim_name(class_name: &str) -> String {
    if !cfg!(target_os = "windows") {
        return "unknown".to_string();
    }
    let script =
        format!("(Get-CimInstance {class_name} | Select-Object -First 1 -ExpandProperty Name)");
    Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
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
    let project_cfg = load_project_toml(&project_dir)
        .map_err(|err| format!("failed to load project.toml: {err}"))?;
    let profiling_dir = ensure_profiling_output_dir(&project_dir)?;
    let flamegraph_output_path = profiling_dir.join("flamegraph.svg");
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;
    ensure_source_overrides(&project_dir)
        .map_err(|err| format!("failed to refresh source overrides: {err}"))?;

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
    let mut features = Vec::new();
    if profile {
        features.push("profile");
    }
    if project_cfg.steam.enabled {
        features.push("steamworks");
    }
    if !features.is_empty() {
        cmd.arg("--features").arg(features.join(","));
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

#[cfg(test)]
mod spec_tests {
    use super::*;

    #[test]
    fn percentile_uses_sorted_sample() {
        assert_eq!(percentile(vec![30.0, 10.0, 20.0], 0.5), 20.0);
    }

    #[test]
    fn ram_estimate_rounds_to_common_tier() {
        assert_eq!(common_ram_tier(6.2), 8);
        assert_eq!(common_ram_tier(13.0), 16);
    }
}

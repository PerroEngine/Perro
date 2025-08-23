#![cfg_attr(windows, windows_subsystem = "windows")] // no console on Windows

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::Result;
use dirs;
use eframe::egui::{self, Color32, RichText};
use natord;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Manifest {
    latest: String,
    versions: std::collections::HashMap<String, VersionInfo>,
}

#[derive(Debug, Deserialize)]
struct VersionInfo {
    editor: String,
    runtime: String,
    toolchain: String,
    linker: String,
}

fn perro_dir() -> PathBuf {
    dirs::data_local_dir().unwrap().join("Perro")
}

fn versions_dir() -> PathBuf {
    perro_dir().join("versions")
}

fn installed_versions() -> Vec<String> {
    let mut versions: Vec<_> = match fs::read_dir(versions_dir()) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().into_string().unwrap_or_default())
            .collect(),
        Err(_) => Vec::new(),
    };

    versions.sort_by(|a, b| natord::compare(a, b));
    versions
}

fn latest_installed_version() -> Option<String> {
    installed_versions().last().cloned()
}

/// Global components (shared across all versions)
fn global_components(info: &VersionInfo) -> Vec<(String, PathBuf)> {
    vec![
        (
            "Rust toolchain".to_string(),
            perro_dir().join("toolchains").join(&info.toolchain),
        ),
        (
            "MSYS2 linker".to_string(),
            perro_dir().join("linkers").join(&info.linker),
        ),
    ]
}

/// Versioned components (per engine version)
fn versioned_components(version: &str, info: &VersionInfo) -> Vec<(String, PathBuf)> {
    let version_dir = versions_dir().join(version);
    vec![
        (
            "Perro runtime".to_string(),
            version_dir.join(&info.runtime),
        ),
        (
            "Perro editor".to_string(),
            version_dir.join(&info.editor),
        ),
        (
            "Perro target".to_string(),
            version_dir.join("targets"),
        ),
    ]
}

/// Check if a version is fully installed (global + versioned)
fn version_installed(version: &str, info: &VersionInfo) -> bool {
    global_components(info)
        .iter()
        .chain(versioned_components(version, info).iter())
        .all(|(_, path)| path.exists())
}

enum LauncherAction {
    FirstInstall(String),
    Launch(String),
    LaunchWithUpdate { current: String, latest: String },
    FixInstall(String),
}

fn decide_action(manifest: &Manifest) -> LauncherAction {
    let latest = &manifest.latest;
    let latest_info = &manifest.versions[latest];

    if version_installed(latest, latest_info) {
        LauncherAction::Launch(latest.clone())
    } else {
        if let Some(installed) = latest_installed_version() {
            if installed == *latest {
                LauncherAction::FixInstall(latest.clone())
            } else {
                LauncherAction::LaunchWithUpdate {
                    current: installed,
                    latest: latest.clone(),
                }
            }
        } else {
            LauncherAction::FirstInstall(latest.clone())
        }
    }
}

struct PerroApp {
    manifest: Manifest,
    action: LauncherAction,
    installing: bool,
    start_time: Option<Instant>,
    total_duration: Duration,
    components: Vec<(String, PathBuf)>, // missing components
    spinner_index: usize,
    flash_timer: Instant,
    flash_state: bool,
    finished: bool,
}

impl PerroApp {
    fn new(manifest: Manifest) -> Self {
        let action = decide_action(&manifest);

        // Build list of missing components
        let mut components = vec![];
        if let LauncherAction::FirstInstall(ref version)
        | LauncherAction::LaunchWithUpdate {
            latest: ref version, ..
        }
        | LauncherAction::FixInstall(ref version) = action
        {
            let info = &manifest.versions[version];
            for (name, path) in global_components(info)
                .into_iter()
                .chain(versioned_components(version, info).into_iter())
            {
                if !path.exists() {
                    components.push((name, path));
                }
            }
        }

        Self {
            manifest,
            action,
            installing: false,
            start_time: None,
            total_duration: Duration::from_secs(25),
            components,
            spinner_index: 0,
            flash_timer: Instant::now(),
            flash_state: true,
            finished: false,
        }
    }

    fn progress(&self) -> f32 {
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_secs_f32();
            (elapsed / self.total_duration.as_secs_f32()).min(1.0)
        } else {
            0.0
        }
    }

    fn current_component(&self) -> String {
        if self.components.is_empty() {
            return "Nothing".to_string();
        }
        let p = self.progress();
        let step_size = 1.0 / self.components.len() as f32;
        let idx = (p / step_size).floor() as usize;
        self.components
            .get(idx.min(self.components.len() - 1))
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    fn spinner(&mut self) -> &'static str {
        let frames = ["-", "\\", "|", "/"];
        self.spinner_index = (self.spinner_index + 1) % frames.len();
        frames[self.spinner_index]
    }

    fn create_dummy_files(&self, version: &str, info: &VersionInfo) {
        for (name, path) in global_components(info)
            .into_iter()
            .chain(versioned_components(version, info).into_iter())
        {
            if !path.exists() {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                if path.is_dir() {
                    fs::create_dir_all(&path).unwrap();
                } else {
                    fs::write(&path, format!("fake binary for {}", name)).unwrap();
                }
            }
        }
    }

    fn launch_runtime(&self, version: &str, info: &VersionInfo) {
        let runtime_path = versions_dir().join(version).join(&info.runtime);
        println!("Launching runtime: {:?}", runtime_path);
        if runtime_path.exists() {
            let _ = Command::new(runtime_path).spawn();
        }
    }
}

impl eframe::App for PerroApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);

                ui.heading(
                    RichText::new("🐕 Perro Manager")
                        .color(Color32::from_rgb(220, 80, 40))
                        .size(28.0),
                );

                ui.add_space(20.0);

                match &self.action {
                    // First install: auto
                    LauncherAction::FirstInstall(version) => {
                        ui.label(
                            RichText::new(format!("First-time setup for Perro {}", version))
                                .color(Color32::from_rgb(120, 160, 220)),
                        );

                        if !self.installing {
                            ui.label("Installing required components...");
                            self.installing = true;
                            self.start_time = Some(Instant::now());
                        }

                        let p = self.progress();
                        if p < 1.0 {
                            ui.label(format!(
                                "{} Installing {}...",
                                self.spinner(),
                                self.current_component()
                            ));
                            ui.add(egui::ProgressBar::new(p).text(format!("{:.0}%", p * 100.0)));
                            ctx.request_repaint_after(Duration::from_millis(100));
                        } else if !self.finished {
                            ui.label("✅ Installation complete! Launching...");
                            let info = &self.manifest.versions[&self.manifest.latest];
                            self.create_dummy_files(&self.manifest.latest, info);
                            self.launch_runtime(&self.manifest.latest, info);
                            self.finished = true;
                            std::process::exit(0);
                        }
                    }

                    // Normal launch
                    LauncherAction::Launch(version) => {
                        let info = &self.manifest.versions[version];
                        self.launch_runtime(version, info);
                        std::process::exit(0);
                    }

                    // Fix mode: auto
                    LauncherAction::FixInstall(version) => {
                        ui.label(
                            RichText::new(format!("⚠ Fixing missing components for Perro {}", version))
                                .color(Color32::from_rgb(255, 180, 50)),
                        );

                        if !self.installing {
                            self.installing = true;
                            self.start_time = Some(Instant::now());
                        }

                        let p = self.progress();
                        if p < 1.0 {
                            ui.label(format!(
                                "{} Repairing {}...",
                                self.spinner(),
                                self.current_component()
                            ));
                            ui.add(egui::ProgressBar::new(p).text(format!("{:.0}%", p * 100.0)));
                            ctx.request_repaint_after(Duration::from_millis(100));
                        } else if !self.finished {
                            ui.label("✅ Repair complete! Launching...");
                            let info = &self.manifest.versions[version];
                            self.create_dummy_files(version, info);
                            self.launch_runtime(version, info);
                            self.finished = true;
                            std::process::exit(0);
                        }
                    }

                    // Update: ask first
                    LauncherAction::LaunchWithUpdate { current, latest } => {
                        if self.flash_timer.elapsed() > Duration::from_millis(1000) {
                            self.flash_state = !self.flash_state;
                            self.flash_timer = Instant::now();
                        }

                        let flash_color = if self.flash_state {
                            Color32::from_rgb(255, 120, 60)
                        } else {
                            Color32::from_rgb(180, 80, 40)
                        };

                        ui.label(
                            RichText::new("⚡ Update Available!")
                                .color(flash_color)
                                .size(22.0),
                        );

                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(format!("Current version: {}", current))
                                .color(Color32::from_rgb(120, 160, 220)),
                        );
                        ui.label(
                            RichText::new(format!("Available update: {}", latest))
                                .color(Color32::from_rgb(220, 80, 40)),
                        );

                        let changelog_url =
                            format!("https://perroengine.org/changelog/{}", latest);
                        ui.hyperlink_to("📜 View Changelog", changelog_url);

                        ui.add_space(10.0);
                        ui.label("Updating will not break existing projects.");
                        ui.label("You can continue using past versions unless you explicitly upgrade them in the Project Manager.");
                        ui.label("The update just makes the new editor/runtime/target available.");

                        if !self.installing {
                            if ui
                                .button(
                                    RichText::new("Update Now")
                                        .color(Color32::WHITE)
                                        .background_color(Color32::from_rgb(220, 80, 40)),
                                )
                                .clicked()
                            {
                                self.installing = true;
                                self.start_time = Some(Instant::now());
                            }
                            if ui
                                .button(
                                    RichText::new("Skip (use current)")
                                        .color(Color32::WHITE)
                                        .background_color(Color32::from_rgb(80, 80, 80)),
                                )
                                .clicked()
                            {
                                let info = &self.manifest.versions[current];
                                self.launch_runtime(current, info);
                                std::process::exit(0);
                            }
                        } else {
                            let p = self.progress();
                            if p < 1.0 {
                                ui.label(format!(
                                    "{} Updating {}...",
                                    self.spinner(),
                                    self.current_component()
                                ));
                                ui.add(egui::ProgressBar::new(p).text(format!("{:.0}%", p * 100.0)));
                                ctx.request_repaint_after(Duration::from_millis(100));
                            } else if !self.finished {
                                ui.label("✅ Update complete! Launching...");
                                let info = &self.manifest.versions[latest];
                                self.create_dummy_files(latest, info);
                                self.launch_runtime(latest, info);
                                self.finished = true;
                                std::process::exit(0);
                            }
                        }
                    }
                }
            });
        });
    }
}

fn main() -> Result<()> {
    // Mock manifest
    let manifest_json = r#"
    {
      "latest": "5.5",
      "versions": {
        "4.1": {
          "editor": "PerroEditor.exe",
          "runtime": "perro_runtime-4.1.exe",
          "toolchain": "rust-1.81.0-x86_64-pc-windows-gnu",
          "linker": "msys2-2024.08"
        },
        "5.0": {
          "editor": "PerroEditor.exe",
          "runtime": "perro_runtime-5.0.exe",
          "toolchain": "rust-1.83.0-x86_64-pc-windows-gnu",
          "linker": "msys2-2025.05"
        },
        "5.5": {
          "editor": "PerroEditor.exe",
          "runtime": "perro_runtime-5.1.exe",
          "toolchain": "rust-1.84.0-x86_64-pc-windows-gnu",
          "linker": "msys2-2025.05"
        }
      }
    }
    "#;

    let manifest: Manifest = serde_json::from_str(manifest_json)?;

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Perro Manager 🐕",
        options,
        Box::new(|_cc| Box::new(PerroApp::new(manifest))),
    )
    .unwrap();

    Ok(())
}
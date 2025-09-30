#![cfg_attr(windows, windows_subsystem = "windows")] // no console on Windows

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use std::cmp::Ordering;
use std::collections::HashSet;

use anyhow::Result;
use dirs;
use eframe::egui::{self, Color32, RichText};
use natord;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
struct Manifest {
    latest: String,
    versions: std::collections::HashMap<String, VersionInfo>,
}

#[derive(Debug, Deserialize, Clone)]
struct VersionInfo {
    editor: String,
    runtime: String,
    toolchain: String,
    linker: String,
}


#[derive(Debug, Clone)]
struct VersionFix {
    version: String,
    components: Vec<(String, PathBuf)>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct ManagerManifest {
    skipped_versions: HashSet<String>,
}

impl ManagerManifest {
    fn load() -> Self {
        let path = perro_dir().join("manager_manifest.json");
        if let Ok(content) = fs::read_to_string(&path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self) -> Result<()> {
        let path = perro_dir().join("manager_manifest.json");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    fn skip_version(&mut self, version: String) -> Result<()> {
        self.skipped_versions.insert(version);
        self.save()
    }

    fn is_skipped(&self, version: &str) -> bool {
        self.skipped_versions.contains(version)
    }
}

fn perro_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::env::current_dir().expect("No current dir"))
        .join("Perro")
}


fn versions_dir() -> PathBuf {
    perro_dir().join("versions")
}

fn current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
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

/// Check if a version is fully installed (all required files exist)
fn version_installed(version: &str, info: &VersionInfo) -> bool {
    let version_dir = versions_dir().join(version);
    let editor = version_dir.join(&info.editor);
    let runtime = version_dir.join(&info.runtime);
    let toolchain = perro_dir().join("toolchains").join(&info.toolchain);
    let linker = perro_dir().join("linkers").join(&info.linker);
    

    editor.exists() && runtime.exists() && toolchain.exists() && linker.exists()
}

/// Check if a component exists, accounting for session installs
fn component_exists(path: &PathBuf, session_installed: &HashSet<PathBuf>) -> bool {
    path.exists() || session_installed.contains(path)
}

/// Collect missing components for a version, accounting for session installs
fn missing_components_with_session(version: &str, info: &VersionInfo, session_installed: &HashSet<PathBuf>) -> Vec<(String, PathBuf)> {
    let mut missing = vec![];
    let version_dir = versions_dir().join(version);

    let editor = version_dir.join(&info.editor);
    let runtime = version_dir.join(&info.runtime);
    let toolchain = perro_dir().join("toolchains").join(&info.toolchain);
    let linker = perro_dir().join("linkers").join(&info.linker);

    if !component_exists(&editor, session_installed) {
        missing.push(("Editor".to_string(), editor));
    }
    if !component_exists(&runtime, session_installed) {
        missing.push(("Runtime".to_string(), runtime));
    }
    if !component_exists(&toolchain, session_installed) {
        missing.push(("Toolchain".to_string(), toolchain));
    }
    if !component_exists(&linker, session_installed) {
        missing.push(("Linker".to_string(), linker));
    }

    missing
}

/// Collect missing components for a version
fn missing_components(version: &str, info: &VersionInfo) -> Vec<(String, PathBuf)> {
    missing_components_with_session(version, info, &HashSet::new())
}

enum LauncherAction {
    InstallerMode, // No versions installed
    FixAll(Vec<VersionFix>), // Fix multiple versions with their specific components
    Update { current: String, latest: String }, // Update available
    Launch(String), // Launch latest installed
}

fn decide_action(manifest: &Manifest, manager_manifest: &ManagerManifest) -> LauncherAction {
    let installed = installed_versions();

    if installed.is_empty() {
        return LauncherAction::InstallerMode;
    }

    // Check for broken versions
    let mut broken_fixes = vec![];
    for version in installed.iter().rev() {
        if let Some(info) = manifest.versions.get(version) {
            let missing = missing_components(version, info);
            if !missing.is_empty() {
                broken_fixes.push(VersionFix {
                    version: version.clone(),
                    components: missing,
                });
            }
        }
    }
    if !broken_fixes.is_empty() {
        return LauncherAction::FixAll(broken_fixes);
    }

    // Check for update, but skip if the latest version was skipped
    if let Some(highest) = latest_installed_version() {
        if natord::compare(&highest, &manifest.latest) == Ordering::Less 
            && !manager_manifest.is_skipped(&manifest.latest) {
            return LauncherAction::Update {
                current: highest,
                latest: manifest.latest.clone(),
            };
        } else {
            return LauncherAction::Launch(highest);
        }
    }

    LauncherAction::InstallerMode
}

struct PerroApp {
    manifest: Manifest,
    manager_manifest: ManagerManifest,
    action: LauncherAction,
    installing: bool,
    start_time: Option<Instant>,
    total_duration: Duration,
    components: Vec<(String, PathBuf)>,
    version_fixes: Vec<VersionFix>,
    current_fixes: Vec<VersionFix>, // Store the recalculated fixes
    session_installed: HashSet<PathBuf>, // Track what we've "installed" this session
    current_version_index: usize,
    current_component_index: usize,
    spinner_index: usize,
    finished: bool,
    selected_version: String,
}

impl PerroApp {
    fn new(manifest: Manifest) -> Self {
        let manager_manifest = ManagerManifest::load();
        let action = decide_action(&manifest, &manager_manifest);
        let selected_version = manifest.latest.clone();

        Self {
            manifest,
            manager_manifest,
            action,
            installing: false,
            start_time: None,
            total_duration: Duration::from_secs(25),
            components: vec![],
            version_fixes: vec![],
            current_fixes: vec![],
            session_installed: HashSet::new(),
            current_version_index: 0,
            current_component_index: 0,
            spinner_index: 0,
            finished: false,
            selected_version,
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

    fn current_component(&self) -> Option<&str> {
        if self.components.is_empty() {
            return None;
        }
        
        let progress = self.progress();
        let total_components = self.components.len();
        let component_index = ((progress * total_components as f32) as usize).min(total_components - 1);
        
        Some(&self.components[component_index].0)
    }

    fn update_current_fixes(&mut self) {
        // Recalculate components that still need fixing, accounting for session installs
        self.current_fixes.clear();
        for version_fix in &self.version_fixes {
            if let Some(info) = self.manifest.versions.get(&version_fix.version) {
                let still_missing = missing_components_with_session(&version_fix.version, info, &self.session_installed);
                if !still_missing.is_empty() {
                    self.current_fixes.push(VersionFix {
                        version: version_fix.version.clone(),
                        components: still_missing,
                    });
                }
            }
        }
    }

    fn update_session_installed_based_on_progress(&mut self) {
        // Mark components as "installed" based on progress
        let progress = self.progress();
        let total_components: usize = self.version_fixes.iter().map(|vf| vf.components.len()).sum();
        
        if total_components == 0 {
            return;
        }

        let completed_components = ((progress * total_components as f32) as usize).min(total_components);
        
        let mut component_count = 0;
        for version_fix in &self.version_fixes {
            for (_, path) in &version_fix.components {
                if component_count < completed_components {
                    self.session_installed.insert(path.clone());
                }
                component_count += 1;
                if component_count >= completed_components {
                    return;
                }
            }
        }
    }

    fn current_fix_info(&self) -> Option<(&str, &str)> {
        if self.current_fixes.is_empty() {
            return None;
        }

        let progress = self.progress();
        let total_components: usize = self.current_fixes.iter().map(|vf| vf.components.len()).sum();
        
        if total_components == 0 {
            return None;
        }

        let current_component_global = ((progress * total_components as f32) as usize).min(total_components - 1);
        
        let mut component_count = 0;
        for version_fix in &self.current_fixes {
            let next_count = component_count + version_fix.components.len();
            if current_component_global < next_count {
                let local_component_index = current_component_global - component_count;
                return Some((&version_fix.version, &version_fix.components[local_component_index].0));
            }
            component_count = next_count;
        }

        // Fallback to last component
        if let Some(last_fix) = self.current_fixes.last() {
            if let Some(last_component) = last_fix.components.last() {
                return Some((&last_fix.version, &last_component.0));
            }
        }

        None
    }

    fn spinner(&mut self) -> &'static str {
        let frames = ["-", "\\", "|", "/"];
        self.spinner_index = (self.spinner_index + 1) % frames.len();
        frames[self.spinner_index]
    }

    fn create_dummy_files(&self, version: &str, info: &VersionInfo) {
        let base = perro_dir();

        // Versioned editor/runtime
        let version_dir = base.join("versions").join(version);
        fs::create_dir_all(&version_dir).unwrap();

        let editor_path = version_dir.join(&info.editor);
        let runtime_path = version_dir.join(&info.runtime);

        // Toolchain + linker
        let toolchain_path = base.join("toolchains").join(&info.toolchain);
        let linker_path = base.join("linkers").join(&info.linker);

        // Platform-specific artifacts in version folder
        let artifacts_dir = version_dir.join("artifacts");
        fs::create_dir_all(&artifacts_dir).unwrap();

     
        let files = vec![
            editor_path, 
            runtime_path, 
            toolchain_path, 
            linker_path,
        ];

        for f in files {
            if let Some(parent) = f.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&f, format!("fake binary for {:?}", f.file_name().unwrap())).unwrap();
        }
    }

    fn create_missing_files_for_version(&self, version: &str, info: &VersionInfo) {
        let base = perro_dir();

        // Only create files that are actually missing (not accounting for session installs)
        let missing = missing_components(version, info);
        
        for (component_name, path) in missing {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, format!("fake binary for {:?} ({})", path.file_name().unwrap(), component_name)).unwrap();
        }
    }

    fn launch_editor(&self, version: &str, info: &VersionInfo) {
        let editor_path = versions_dir().join(version).join(&info.editor);
        if editor_path.exists() {
            let _ = Command::new(editor_path).spawn();
        } else {
            eprintln!("Editor not found: {:?}", editor_path);
        }
    }

    fn skip_version(&mut self, version: String) {
        if let Err(e) = self.manager_manifest.skip_version(version) {
            eprintln!("Failed to save skipped version: {}", e);
        }
    }
}

impl eframe::App for PerroApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dark theme
        ctx.set_visuals(egui::Visuals::dark());

        // Track if we need to skip a version after the UI update
        let mut version_to_skip: Option<String> = None;
        let mut should_launch_current = false;
        let mut current_version_for_launch: Option<String> = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);

                ui.heading(
                    RichText::new("🐕 Perro Manager")
                        .color(Color32::from_rgb(220, 80, 40)) // deep red-orange
                        .size(28.0),
                );

                ui.add_space(20.0);

                // Get spinner character before the match to avoid borrow conflicts
                let spinner_char = if self.installing {
                    self.spinner()
                } else {
                    ""
                };

                match &self.action {
                    LauncherAction::InstallerMode => {
                        ui.label("No versions installed. Please install Perro:");

                        ui.add_space(10.0);

                        let mut selected = self.selected_version.clone();
                        
                        // Display text for the selected version
                        let display_text = if selected == self.manifest.latest {
                            if self.manager_manifest.is_skipped(&selected) {
                                format!("{} (Latest, Skipped)", selected)
                            } else {
                                format!("{} (Latest)", selected)
                            }
                        } else if let Some(info) = self.manifest.versions.get(&selected) {
                            if version_installed(&selected, info) {
                                format!("{} (Installed)", selected)
                            } else if self.manager_manifest.is_skipped(&selected) {
                                format!("{} (Skipped)", selected)
                            } else {
                                format!("{} (Not Installed)", selected)
                            }
                        } else {
                            selected.clone()
                        };

                        egui::ComboBox::from_id_source("version_select")
                            .selected_text(
                                RichText::new(&display_text)
                                    .size(18.0)
                                    .color(Color32::WHITE),
                            )
                            .width(260.0)
                            .show_ui(ui, |ui| {
                                let mut versions: Vec<_> =
                                    self.manifest.versions.keys().cloned().collect();
                                versions.sort_by(|a, b| natord::compare(b, a));

                                for version in versions {
                                    if let Some(info) = self.manifest.versions.get(&version) {
                                        let installed = version_installed(&version, info);
                                        let skipped = self.manager_manifest.is_skipped(&version);

                                        let label = if version == self.manifest.latest {
                                            if skipped {
                                                format!("{} (Latest, Skipped)", version)
                                            } else {
                                                format!("{} (Latest)", version)
                                            }
                                        } else if installed {
                                            format!("{} (Installed)", version)
                                        } else if skipped {
                                            format!("{} (Skipped)", version)
                                        } else {
                                            format!("{} (Not Installed)", version)
                                        };

                                        let display = if version == self.manifest.latest && !skipped {
                                            RichText::new(label)
                                                .color(Color32::from_rgb(255, 120, 60))
                                        } else if installed {
                                            RichText::new(label)
                                                .color(Color32::from_rgb(100, 149, 237))
                                        } else if skipped {
                                            RichText::new(label)
                                                .color(Color32::from_rgb(150, 150, 150))
                                        } else {
                                            RichText::new(label).color(Color32::GRAY)
                                        };

                                        ui.selectable_value(&mut selected, version.clone(), display);
                                    }
                                }
                            });
                        self.selected_version = selected;

                        if ui
                            .add_sized(
                                [260.0, 45.0],
                                egui::Button::new(
                                    RichText::new(format!("Install {}", self.selected_version))
                                        .size(18.0)
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(220, 80, 40))
                                .rounding(10.0),
                            )
                            .clicked()
                        {
                            self.installing = true;
                            self.start_time = Some(Instant::now());

                            let selected_version = self.selected_version.clone();
                            if let Some(info) = self.manifest.versions.get(&selected_version) {
                                self.components = missing_components(&selected_version, info);
                            }
                        }

                        if self.installing {
                            let p = self.progress();
                            if p < 1.0 {
                                let current_component = self.current_component()
                                    .unwrap_or("Preparing");
                                
                                ui.label(format!(
                                    "{} Installing {} - {}...",
                                    spinner_char,
                                    self.selected_version,
                                    current_component
                                ));
                                ui.add(egui::ProgressBar::new(p).text(format!("{:.0}%", p * 100.0)));
                                ctx.request_repaint_after(Duration::from_millis(100));
                            } else if !self.finished {
                                let selected_version = self.selected_version.clone();
                                if let Some(info) = self.manifest.versions.get(&selected_version) {
                                    self.create_dummy_files(&selected_version, info);
                                    self.launch_editor(&selected_version, info);
                                }
                                self.finished = true;
                                std::process::exit(0);
                            }
                        }
                    }

                    LauncherAction::FixAll(version_fixes) => {
                        ui.label("⚠ Fixing missing components...");

                        if !self.installing {
                            self.installing = true;
                            self.start_time = Some(Instant::now());
                            self.version_fixes = version_fixes.clone();
                            self.update_current_fixes(); // Initialize current_fixes
                        }

                        // Update session installed components based on progress
                        self.update_session_installed_based_on_progress();
                        
                        // Update current fixes to reflect what's been "installed"
                        self.update_current_fixes();

                        let p = self.progress();
                        if p < 1.0 {
                            if let Some((version, component)) = self.current_fix_info() {
                                ui.label(format!("{} Repairing {} - {}...", spinner_char, version, component));
                            } else {
                                ui.label(format!("{} Preparing repairs...", spinner_char));
                            }
                            ui.add(egui::ProgressBar::new(p).text(format!("{:.0}%", p * 100.0)));
                            ctx.request_repaint_after(Duration::from_millis(100));
                        } else if !self.finished {
                            // Fix each version, but only create files that are still missing
                            for version_fix in &self.version_fixes {
                                if let Some(info) = self.manifest.versions.get(&version_fix.version) {
                                    self.create_missing_files_for_version(&version_fix.version, info);
                                }
                            }
                            if let Some(highest) = latest_installed_version() {
                                if let Some(info) = self.manifest.versions.get(&highest) {
                                    self.launch_editor(&highest, info);
                                }
                            }
                            self.finished = true;
                            std::process::exit(0);
                        }
                    }

                    LauncherAction::Update { current, latest } => {
                        ui.label(
                            RichText::new("⚡ Update Available!")
                                .color(Color32::from_rgb(255, 120, 60))
                                .size(22.0),
                        );
                        ui.label(format!("Current: {}, Latest: {}", current, latest));

                        if !self.installing {
                            if ui
                                .add_sized(
                                    [220.0, 45.0],
                                    egui::Button::new(
                                        RichText::new("Update Now")
                                            .size(18.0)
                                            .color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(220, 80, 40))
                                    .rounding(10.0),
                                )
                                .clicked()
                            {
                                self.installing = true;
                                self.start_time = Some(Instant::now());

                                let latest_version = latest.clone();
                                if let Some(info) = self.manifest.versions.get(&latest_version) {
                                    self.components = missing_components(&latest_version, info);
                                }
                            }
                            if ui
                                .add_sized(
                                    [220.0, 45.0],
                                    egui::Button::new(
                                        RichText::new("Skip (use current)")
                                            .size(18.0)
                                            .color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(80, 80, 80))
                                    .rounding(10.0),
                                )
                                .clicked()
                            {
                                // Set flags to handle after the UI update
                                version_to_skip = Some(latest.clone());
                                should_launch_current = true;
                                current_version_for_launch = Some(current.clone());
                            }
                        } else {
                            let p = self.progress();
                            if p < 1.0 {
                                let current_component = self.current_component()
                                    .unwrap_or("Preparing");
                                
                                ui.label(format!("{} Updating {} - {}...", spinner_char, latest, current_component));
                                ui.add(egui::ProgressBar::new(p).text(format!("{:.0}%", p * 100.0)));
                                ctx.request_repaint_after(Duration::from_millis(100));
                            } else if !self.finished {
                                let latest_version = latest.clone();
                                if let Some(info) = self.manifest.versions.get(&latest_version) {
                                    self.create_dummy_files(&latest_version, info);
                                    self.launch_editor(&latest_version, info);
                                }
                                self.finished = true;
                                std::process::exit(0);
                            }
                        }
                    }

                    LauncherAction::Launch(version) => {
                        let v = version.clone();
                        if let Some(info) = self.manifest.versions.get(&v) {
                            self.launch_editor(&v, info);
                        }
                        std::process::exit(0);
                    }
                }
            });
        });

        // Handle version skipping after the UI update
        if let Some(version) = version_to_skip {
            self.skip_version(version);
        }

        if should_launch_current {
            if let Some(current_version) = current_version_for_launch {
                if let Some(info) = self.manifest.versions.get(&current_version) {
                    self.launch_editor(&current_version, info);
                }
                std::process::exit(0);
            }
        }
    }
}

fn main() -> Result<()> {
    // Mock manifest (pretend we downloaded this JSON)
    let manifest_json = r#"
    {
      "latest": "5.1",
      "versions": {
        "4.1": {
          "editor": "Perro-Editor.exe",
          "runtime": "runtime-4.1.exe",
          "toolchain": "rust-1.81.0-x86_64-pc-windows-gnu",
          "linker": "msys2-2024.08"
        },
        "5.1": {
          "editor": "Perro-Editor.exe",
          "runtime": "runtime-5.1.exe",
          "toolchain": "rust-1.83.0-x86_64-pc-windows-gnu",
          "linker": "msys2-2024.08"
        },
        "5.0": {
          "editor": "Perro-Editor.exe",
          "runtime": "runtime-5.0.exe",
          "toolchain": "rust-1.83.0-x86_64-pc-windows-gnu",
          "linker": "msys2-2025.05"
        }
      }
    }
    "#;

    let manifest: Manifest = serde_json::from_str(manifest_json)?;

    let options = eframe::NativeOptions::default();
    if let Err(e) = eframe::run_native(
        "Perro Manager 🐕",
        options,
        Box::new(|_cc| Box::new(PerroApp::new(manifest))),
    ) {
        eprintln!("Failed to run native app: {:?}", e);
        std::process::exit(1);
    }

    Ok(())
}
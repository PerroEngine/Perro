#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
use serde_json::Value;
use uuid::Uuid;
use perro_core::{
    script::{UpdateOp, Var},
    scripting::api::ScriptApi,
    scripting::script::Script,
    nodes::*,
};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::process::Command;

#[unsafe(no_mangle)]
pub extern "C" fn root_create_script() -> *mut dyn Script {
    Box::into_raw(Box::new(BRootScript {
        node_id: Uuid::nil(),
        toolchain_ver: "".into(),
    })) as *mut dyn Script
}

pub struct BRootScript {
    node_id: Uuid,
    toolchain_ver: String,
}

const MANIFEST_CACHE_HOURS: u64 = 6;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Manifest {
    latest: String,
}

impl BRootScript {
    // Check if toolchain exists (manual extraction layout)
    fn toolchain_exists(&self, api: &ScriptApi, toolchain: &str) -> bool {
        let toolchain_path_str = format!("user://toolchains/{}", toolchain);
        if let Some(toolchain_path) = api.resolve_path(&toolchain_path_str) {
            let base = Path::new(&toolchain_path);
            
            // Check for extracted layout: toolchain/cargo/bin/cargo.exe and toolchain/rustc/bin/rustc.exe
            let cargo = base.join("cargo").join("bin").join("cargo.exe");
            let rustc = base.join("rustc").join("bin").join("rustc.exe");
            
            cargo.exists() && rustc.exists()
        } else {
            false
        }
    }

    // Get Rust toolchain URL (always use tar.gz for manual extraction)
    fn get_rust_url(&self, toolchain: &str) -> String {
        format!("https://static.rust-lang.org/dist/{}.tar.gz", toolchain)
    }

    // Download file via curl/wget
    fn download_file_real(&self, url: &str, dest_path: &Path) -> Result<(), String> {
        eprintln!("📥 Downloading: {}", url);
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir: {}", e))?;
        }

        let curl_result = Command::new("curl")
            .args(&[
                "-L", "-f", "--retry", "3", "--connect-timeout", "30", "-o",
                dest_path.to_str().unwrap(), url,
            ])
            .output();

        match curl_result {
            Ok(output) if output.status.success() => {
                eprintln!("✅ Downloaded with curl to {}", dest_path.display());
                return Ok(());
            }
            Ok(output) => {
                eprintln!("⚠️ curl failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            Err(_) => eprintln!("⚠️ curl not available, trying wget..."),
        }

        let wget_result = Command::new("wget")
            .args(&["--tries=3", "--timeout=30", "-O", dest_path.to_str().unwrap(), url])
            .output();

        match wget_result {
            Ok(output) if output.status.success() => {
                eprintln!("✅ Downloaded with wget to {}", dest_path.display());
                Ok(())
            }
            Ok(output) => Err(format!(
                "wget failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )),
            Err(_) => Err("Neither curl nor wget available".to_string()),
        }
    }

    // Install Rust toolchain (manual tar.gz extraction)
    fn install_rust_toolchain(&self, api: &ScriptApi, toolchain: &str) -> Result<(), String> {
        let toolchain_path_str = format!("user://toolchains/{}", toolchain);
        let toolchain_path = api
            .resolve_path(&toolchain_path_str)
            .ok_or("Failed to resolve toolchain dir")?;
        let toolchain_dir = Path::new(&toolchain_path);

        if self.toolchain_exists(api, toolchain) {
            eprintln!("✅ Toolchain already installed: {}", toolchain);
            return Ok(());
        }

        eprintln!("📦 Installing Rust toolchain: {}", toolchain);
        std::fs::create_dir_all(toolchain_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        let url = self.get_rust_url(toolchain);
        let tar_path = toolchain_dir.join("rust.tar.gz");
        
        self.download_file_real(&url, &tar_path)?;

        eprintln!("📦 Extracting toolchain...");
        
        // Extract directly to the toolchain directory
        let output = Command::new("tar")
            .args(&[
                "-xzf",
                tar_path.to_str().unwrap(),
                "-C",
                toolchain_dir.to_str().unwrap(),
                "--strip-components=1", // This removes the top-level directory from the archive
            ])
            .output()
            .map_err(|e| format!("Failed to run tar: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "tar extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Clean up
        std::fs::remove_file(&tar_path).ok();

        if !self.toolchain_exists(api, toolchain) {
            return Err("Toolchain install verification failed".into());
        }
        
        eprintln!("✅ Rust toolchain installed successfully");
        eprintln!("📁 Cargo at: {}/cargo/bin/cargo.exe", toolchain_dir.display());
        eprintln!("📁 Rustc at: {}/rustc/bin/rustc.exe", toolchain_dir.display());
        Ok(())
    }

    // ----------- Version management -----------

    fn find_exe_in_dir(&self, dir: &Path) -> Option<PathBuf> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().map_or(false, |e| e == "exe") {
                    return Some(p);
                }
            }
        }
        None
    }

    fn component_exists(&self, api: &ScriptApi, version: &str, component: &str) -> bool {
        let comp_path = format!("user://versions/{}/{}/", version, component);
        if let Some(abs) = api.resolve_path(&comp_path) {
            self.find_exe_in_dir(Path::new(&abs)).is_some()
        } else {
            false
        }
    }

    // Manifest caching and fetching
    fn is_manifest_cache_valid(&self, api: &ScriptApi) -> bool {
        if let Some(path) = api.resolve_path("user://manifest.json") {
            if let Ok(md) = std::fs::metadata(&path) {
                if let Ok(modified) = md.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        return elapsed < Duration::from_secs(MANIFEST_CACHE_HOURS * 3600);
                    }
                }
            }
        }
        false
    }

    fn save_manifest_cache(&self, api: &ScriptApi, manifest: &Manifest) {
        if let Some(path) = api.resolve_path("user://manifest.json") {
            if let Ok(json) = serde_json::to_string_pretty(manifest) {
                std::fs::write(path, json).ok();
            }
        }
    }

    fn load_cached_manifest(&self, api: &ScriptApi) -> Option<Manifest> {
        if let Some(path) = api.resolve_path("user://manifest.json") {
            if let Ok(text) = std::fs::read_to_string(path) {
                serde_json::from_str(&text).ok()
            } else { None }
        } else { None }
    }

    fn fetch_manifest(&self, api: &ScriptApi) -> Result<Manifest, String> {
        if self.is_manifest_cache_valid(api) {
            if let Some(m) = self.load_cached_manifest(api) {
                eprintln!("📋 Cached manifest used");
                return Ok(m);
            }
        }

        eprintln!("📡 Fetching latest version manifest");
        // Simple manifest - just the latest version
        let manifest_json = r#"{"latest": "0.2.0"}"#;

        let m: Manifest =
            serde_json::from_str(manifest_json).map_err(|e| format!("Parse manifest: {}", e))?;
        self.save_manifest_cache(api, &m);
        Ok(m)
    }

    fn download_file(&self, url: &str, dest_path: &Path) -> Result<(), String> {
        eprintln!("📥 (SIMULATED) Downloading {} -> {}", url, dest_path.display());
        std::fs::create_dir_all(dest_path.parent().unwrap())
            .map_err(|e| format!("Failed mkdir: {}", e))?;
        std::fs::write(dest_path, format!("DUMMY {}", url))
            .map_err(|e| format!("Write failed: {}", e))?;
        Ok(())
    }

    fn download_and_install_engine_version(&self, api: &ScriptApi, version: &str) -> Result<(), String> {
        eprintln!("🚀 Installing engine version {}...", version);

        let editor_dir_str = format!("user://versions/{}/editor/", version);
        let runtime_dir_str = format!("user://versions/{}/runtime/", version);

        let editor_dir_resolved = api
            .resolve_path(&editor_dir_str)
            .ok_or("Failed to resolve editor dir")?;
        let runtime_dir_resolved = api
            .resolve_path(&runtime_dir_str)
            .ok_or("Failed to resolve runtime dir")?;

        let editor_path = Path::new(&editor_dir_resolved);
        let runtime_path = Path::new(&runtime_dir_resolved);

        std::fs::create_dir_all(editor_path).ok();
        std::fs::create_dir_all(runtime_path).ok();

        let editor_exe = editor_path.join("perro_editor.exe");
        let runtime_exe = runtime_path.join("perro_runtime.exe");

        if !editor_exe.exists() {
            self.download_file(
                &format!("https://perro-downloads.example.com/versions/{}/editor/perro_editor.exe", version),
                &editor_exe,
            )?;
        }

        if !runtime_exe.exists() {
            self.download_file(
                &format!("https://perro-downloads.example.com/versions/{}/runtime/perro_runtime.exe", version),
                &runtime_exe,
            )?;
        }

        eprintln!("✅ Engine version {} installed", version);
        Ok(())
    }

    // Launch version and exit current process
    fn launch_version(&self, version_path: &Path) -> Result<(), String> {
        eprintln!("🚀 Launching {} and exiting current process", version_path.display());

        let parent_dir = version_path
            .parent()
            .ok_or("Could not determine parent directory")?;

        // Pass through command line arguments
        let args: Vec<String> = std::env::args().skip(1).collect();

        std::process::Command::new(version_path)
            .current_dir(parent_dir)
            .args(&args)
            .spawn()
            .map_err(|e| format!("Failed to launch: {}", e))?;
        
        // Exit this process immediately after launching the new version
        std::process::exit(0);
    }

    // Find and launch the best available version
    fn launch_best_version(&self, api: &ScriptApi) -> Result<(), String> {
        let versions_path_str = "user://versions/";
        if let Some(versions_dir) = api.resolve_path(versions_path_str) {
            let versions_path = Path::new(&versions_dir);
            
            if let Ok(entries) = std::fs::read_dir(versions_path) {
                let mut available_versions: Vec<String> = entries
                    .filter_map(|entry| {
                        let entry = entry.ok()?;
                        if entry.file_type().ok()?.is_dir() {
                            let version = entry.file_name().to_string_lossy().to_string();
                            if self.component_exists(api, &version, "editor") {
                                Some(version)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();

                if !available_versions.is_empty() {
                    // Sort versions and pick the highest
                    available_versions.sort_by(|a, b| natord::compare(b, a)); // Reverse for highest first
                    let best_version = &available_versions[0];
                    
                    eprintln!("🎯 Launching best available version: {}", best_version);
                    
                    let path_str = format!("user://versions/{}/editor/", best_version);
                    if let Some(resolved) = api.resolve_path(&path_str) {
                        if let Some(exe) = self.find_exe_in_dir(Path::new(&resolved)) {
                            return self.launch_version(&exe);
                        }
                    }
                }
            }
        }
        
        Err("No valid versions found".to_string())
    }

    // Launch specific version from proper location and exit
    fn launch_version_from_proper_location(&self, api: &ScriptApi, version: &str) -> Result<(), String> {
        let path_str = format!("user://versions/{}/editor/", version);
        if let Some(resolved) = api.resolve_path(&path_str) {
            if let Some(exe) = self.find_exe_in_dir(Path::new(&resolved)) {
                return self.launch_version(&exe);
            }
        }
        Err(format!("No exe found for version {}", version))
    }

    // Check if running from correct location, if not copy and launch from correct location
    fn ensure_correct_location(&self, api: &ScriptApi, my_version: &str, exe_path: &Path) {
        let exe_name = exe_path.file_name().unwrap();
        let expected_str = format!("user://versions/{}/editor/", my_version);
        if let Some(expected_dir) = api.resolve_path(&expected_str) {
            let expected = PathBuf::from(&expected_dir);
            let expected_exe = expected.join(&exe_name);

            if exe_path != expected_exe {
                eprintln!("⚠️  Not running from {}!", expected.display());
                std::fs::create_dir_all(&expected).ok();
                if std::fs::copy(exe_path, &expected_exe).is_ok() {
                    if self.launch_version(&expected_exe).is_err() {
                        eprintln!("❌ Failed to launch from correct location");
                    }
                }
            }
        }
    }

    // Ensure project has required toolchain (only in editor mode)
    fn ensure_project_toolchain(&self, api: &ScriptApi) {
        if !self.toolchain_ver.is_empty() && !self.toolchain_exists(api, &self.toolchain_ver) {
            eprintln!("📦 Project requires toolchain: {}", self.toolchain_ver);
            eprintln!("🔄 This may take a moment for first-time setup...");
            if let Err(e) = self.install_rust_toolchain(api, &self.toolchain_ver) {
                eprintln!("❌ Failed to install toolchain: {}", e);
            }
        }
    }
}

// ---------- Script trait ----------
impl Script for BRootScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        let my_version = api.project().version().to_string();

        api.project().set_runtime_param("project_path", r"D:\Rust\perro\perro_editor");
        
        // Set toolchain version from project metadata
        self.toolchain_ver = api.project().get_meta("toolchain")
            .unwrap_or("")
            .to_string();

        let exe_path = std::env::current_exe().expect("exe path");
        if cfg!(debug_assertions) {
            eprintln!("🐛 Debug build: skipping version mgmt");
            return;
        }

        let has_args = std::env::args().nth(1).is_some();
        let mode = if has_args { "editor" } else { "manager" };
        eprintln!("🎮 Mode: {} ({})", mode, my_version);

        self.ensure_correct_location(api, &my_version, &exe_path);
        eprintln!("✅ Running from correct location");

        if mode == "editor" {
            // Editor mode: ensure project has required toolchain
            self.ensure_project_toolchain(api);
            eprintln!("🎮 Editor mode ready");
            api.compile_project();
        } else {
            // Manager mode: check for engine updates
            if let Ok(manifest) = self.fetch_manifest(api) {
                if natord::compare(&manifest.latest, &my_version).is_gt() {
                    eprintln!("🎉 Update available: {} -> {}", my_version, manifest.latest);
                    
                    if !self.component_exists(api, &manifest.latest, "editor") {
                        eprintln!("📦 Downloading engine version {}...", manifest.latest);
                        if self.download_and_install_engine_version(api, &manifest.latest).is_ok() {
                            eprintln!("✅ Download complete, launching {}...", manifest.latest);
                            if self.launch_version_from_proper_location(api, &manifest.latest).is_err() {
                                eprintln!("❌ Failed to launch new version, trying best available...");
                                self.launch_best_version(api).ok();
                            }
                        } else {
                            eprintln!("❌ Download failed, continuing with current version");
                        }
                    } else {
                        eprintln!("🚀 Version {} already installed, launching...", manifest.latest);
                        if self.launch_version_from_proper_location(api, &manifest.latest).is_err() {
                            eprintln!("❌ Failed to launch latest version, trying best available...");
                            self.launch_best_version(api).ok();
                        }
                    }
                } else {
                    eprintln!("✅ Running latest version: {}", my_version);
                }
            } else {
                eprintln!("⚠️ Failed to fetch manifest, continuing with current version");
            }
            eprintln!("🎮 Manager mode ready");
        }
    }

    fn update(&mut self, _api: &mut ScriptApi<'_>) {}
    fn set_node_id(&mut self, id: Uuid) { self.node_id = id; }
    fn get_node_id(&self) -> Uuid { self.node_id }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn apply_exports(&mut self, _: &std::collections::HashMap<String, Value>) {}
    fn get_var(&self, _: &str) -> Option<Var> { None }
    fn set_var(&mut self, _: &str, _: Var) -> Option<()> { None }
}

mod natord {
    pub fn compare(a: &str, b: &str) -> std::cmp::Ordering {
        let a: Vec<&str> = a.split('.').collect();
        let b: Vec<&str> = b.split('.').collect();
        let len = a.len().max(b.len());
        for i in 0..len {
            let ai = a.get(i).unwrap_or(&"0");
            let bi = b.get(i).unwrap_or(&"0");
            if let (Ok(na), Ok(nb)) = (ai.parse::<u32>(), bi.parse::<u32>()) {
                match na.cmp(&nb) {
                    std::cmp::Ordering::Equal => continue,
                    other => return other,
                }
            } else {
                match ai.cmp(bi) {
                    std::cmp::Ordering::Equal => continue,
                    other => return other,
                }
            }
        }
        std::cmp::Ordering::Equal
    }
}
#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
use serde_json::Value;
use uuid::Uuid;
use perro_core::{script::{UpdateOp, Var}, scripting::api::ScriptApi, scripting::script::Script, nodes::* };

#[unsafe(no_mangle)]
pub extern "C" fn root_rs_create_script() -> *mut dyn Script {
    Box::into_raw(Box::new(BRootScript {
        node_id: Uuid::nil(),
        x: 0.0,
    })) as *mut dyn Script
}

pub struct BRootScript {
    node_id: Uuid,
    x: f32,
}

const MY_TOOLCHAIN: &str = "rust-1.83.0-x86_64-pc-windows-gnu";
const MY_LINKER: &str = "msys2-2024.08";

#[derive(Debug, serde::Deserialize)]
struct Manifest {
    latest: String,
    versions: HashMap<String, VersionInfo>,
}

#[derive(Debug, serde::Deserialize, Clone)]
struct VersionInfo {
    editor: String,
    runtime: String,
    toolchain: String,
    linker: String,
}

impl BRootScript {
    fn find_highest_version(&self, versions_dir: &std::path::Path, exe_name: &std::ffi::OsStr, current_version: &str) -> Option<(String, std::path::PathBuf)> {
        let entries = match std::fs::read_dir(versions_dir) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("⚠️  Could not read versions directory: {}", e);
                return None;
            }
        };
        
        let mut versions = Vec::new();
        
        eprintln!("🔍 Scanning for installed versions...");
        
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let dir_name = entry.file_name();
                    let version_str = dir_name.to_string_lossy().to_string();
                    
                    eprintln!("🔍 Checking version folder: {}", version_str);
                    
                    // Look for ANY .exe file in this version folder
                    let mut found_exe: Option<std::path::PathBuf> = None;
                    
                    // First try the same exe name as current
                    let same_name_exe = entry.path().join(exe_name);
                    if same_name_exe.exists() {
                        eprintln!("  ✅ Found exe with same name: {}", same_name_exe.display());
                        found_exe = Some(same_name_exe);
                    } else {
                        // Look for any .exe file in the directory
                        eprintln!("  🔍 Looking for any .exe in folder...");
                        if let Ok(files) = std::fs::read_dir(&entry.path()) {
                            for file in files.flatten() {
                                if let Some(ext) = file.path().extension() {
                                    if ext == "exe" {
                                        eprintln!("  ✅ Found exe: {}", file.path().display());
                                        found_exe = Some(file.path());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    
                    if let Some(exe_path) = found_exe {
                        // Verify it's in the right location by checking parent directory
                        if let Some(parent) = exe_path.parent() {
                            if parent == entry.path() {
                                eprintln!("✅ Valid version found: {}", version_str);
                                versions.push((version_str, exe_path));
                            } else {
                                eprintln!("⚠️  Skipping {}: exe not in correct location", version_str);
                            }
                        }
                    } else {
                        eprintln!("ℹ️  Version {} folder exists but no exe found", version_str);
                    }
                }
            }
        }
        
        eprintln!("📋 Found {} installed versions", versions.len());
        
        // Sort versions naturally (5.1 > 5.0 > 4.1)
        versions.sort_by(|a, b| natord::compare(&a.0, &b.0));
        
        // Get the highest version that's greater than current
        let result = versions.into_iter()
            .filter(|(v, _)| {
                let is_higher = natord::compare(v.as_str(), current_version) == std::cmp::Ordering::Greater;
                if is_higher {
                    eprintln!("✅ {} > {} (current)", v, current_version);
                } else {
                    eprintln!("ℹ️  {} <= {} (current)", v, current_version);
                }
                is_higher
            })
            .last();
            
        if let Some((ref ver, _)) = result {
            eprintln!("🎯 Highest version to launch: {}", ver);
        } else {
            eprintln!("ℹ️  No higher version found than {}", current_version);
        }
        
        result
    }
    
    fn is_exe_in_correct_location(&self, exe_path: &std::path::Path, expected_version: &str) -> bool {
        // Check if exe_path is actually in user://versions/{expected_version}/
        if let Some(parent) = exe_path.parent() {
            if let Some(parent_name) = parent.file_name() {
                return parent_name.to_string_lossy() == expected_version;
            }
        }
        false
    }

    fn check_my_requirements(&self, api: &ScriptApi) -> Vec<String> {
        let mut missing = Vec::new();

        // Check MY toolchain
        let toolchain_path_str = format!("user://toolchains/{}", MY_TOOLCHAIN);
        if let Some(toolchain_path) = api.resolve_path(&toolchain_path_str) {
            if !std::path::Path::new(&toolchain_path).exists() {
                missing.push(format!("Toolchain: {}", MY_TOOLCHAIN));
            }
        } else {
            missing.push(format!("Toolchain: {}", MY_TOOLCHAIN));
        }

        // Check MY linker
        let linker_path_str = format!("user://linkers/{}", MY_LINKER);
        if let Some(linker_path) = api.resolve_path(&linker_path_str) {
            if !std::path::Path::new(&linker_path).exists() {
                missing.push(format!("Linker: {}", MY_LINKER));
            }
        } else {
            missing.push(format!("Linker: {}", MY_LINKER));
        }

        missing
    }

    fn download_file(&self, url: &str, dest_path: &std::path::Path) -> Result<(), String> {
        eprintln!("📥 Downloading: {} -> {}", url, dest_path.display());
        
        // TODO: Actual HTTP download
        // For now, create dummy file
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        
        std::fs::write(dest_path, format!("dummy file from {}", url))
            .map_err(|e| format!("Failed to write file: {}", e))?;
        
        eprintln!("✅ Downloaded successfully");
        Ok(())
    }

    fn repair_my_requirements(&self, api: &ScriptApi) -> Result<(), String> {
        eprintln!("🔧 Auto-repairing current version requirements...");

        // Download MY toolchain if missing
        let toolchain_path_str = format!("user://toolchains/{}", MY_TOOLCHAIN);
        if let Some(toolchain_path) = api.resolve_path(&toolchain_path_str) {
            if !std::path::Path::new(&toolchain_path).exists() {
                eprintln!("📦 Downloading toolchain: {}", MY_TOOLCHAIN);
                let url = format!("https://perro-downloads.example.com/toolchains/{}", MY_TOOLCHAIN);
                self.download_file(&url, std::path::Path::new(&toolchain_path))?;
            }
        }

        // Download MY linker if missing
        let linker_path_str = format!("user://linkers/{}", MY_LINKER);
        if let Some(linker_path) = api.resolve_path(&linker_path_str) {
            if !std::path::Path::new(&linker_path).exists() {
                eprintln!("📦 Downloading linker: {}", MY_LINKER);
                let url = format!("https://perro-downloads.example.com/linkers/{}", MY_LINKER);
                self.download_file(&url, std::path::Path::new(&linker_path))?;
            }
        }

        eprintln!("✅ Requirements repaired");
        Ok(())
    }

    fn fetch_manifest(&self) -> Result<Manifest, String> {
        eprintln!("📡 Fetching manifest...");
        
        // TODO: Actual HTTP request to download manifest
        // let response = reqwest::blocking::get("https://perro-downloads.example.com/manifest.json")?;
        // let manifest: Manifest = response.json()?;
        
        // For now, return a mock manifest
        let manifest_json = r#"
        {
            "latest": "0.3.0",
            "versions": {
                "0.3.0": {
                    "editor": "Perro-Editor.exe",
                    "runtime": "runtime-0.3.0.exe",
                    "toolchain": "rust-1.84.0-x86_64-pc-windows-gnu",
                    "linker": "msys2-2025.01"
                },
                                "0.2.0": {
                    "editor": "Perro-Editor.exe",
                    "runtime": "runtime-0.1.0.exe",
                    "toolchain": "rust-1.83.0-x86_64-pc-windows-gnu",
                    "linker": "msys2-2024.08"
                },
                "0.1.0": {
                    "editor": "Perro-Editor.exe",
                    "runtime": "runtime-0.1.0.exe",
                    "toolchain": "rust-1.83.0-x86_64-pc-windows-gnu",
                    "linker": "msys2-2024.08"
                }
            }
        }
        "#;

        serde_json::from_str(manifest_json)
            .map_err(|e| format!("Failed to parse manifest: {}", e))
    }

    fn download_and_install_update(&self, api: &ScriptApi, version: &str, info: &VersionInfo) -> Result<(), String> {
        eprintln!("🚀 Downloading version {}...", version);

        let version_dir_str = format!("user://versions/{}/", version);
        let version_dir = api.resolve_path(&version_dir_str)
            .ok_or("Failed to resolve version directory")?;
        let version_path = std::path::Path::new(&version_dir);

        // Create version directory
        std::fs::create_dir_all(&version_path)
            .map_err(|e| format!("Failed to create version directory: {}", e))?;

        // Download editor
        let editor_path = version_path.join(&info.editor);
        if !editor_path.exists() {
            let url = format!("https://perro-downloads.example.com/versions/{}/{}", version, info.editor);
            self.download_file(&url, &editor_path)?;
        }

        // Download runtime
        let runtime_path = version_path.join(&info.runtime);
        if !runtime_path.exists() {
            let url = format!("https://perro-downloads.example.com/versions/{}/{}", version, info.runtime);
            self.download_file(&url, &runtime_path)?;
        }

        // Download toolchain if we don't have it
        let toolchain_path_str = format!("user://toolchains/{}", info.toolchain);
        if let Some(toolchain_path) = api.resolve_path(&toolchain_path_str) {
            let toolchain_path = std::path::Path::new(&toolchain_path);
            if !toolchain_path.exists() {
                eprintln!("📦 New toolchain required: {}", info.toolchain);
                let url = format!("https://perro-downloads.example.com/toolchains/{}", info.toolchain);
                self.download_file(&url, toolchain_path)?;
            } else {
                eprintln!("✅ Toolchain already exists: {}", info.toolchain);
            }
        }

        // Download linker if we don't have it
        let linker_path_str = format!("user://linkers/{}", info.linker);
        if let Some(linker_path) = api.resolve_path(&linker_path_str) {
            let linker_path = std::path::Path::new(&linker_path);
            if !linker_path.exists() {
                eprintln!("📦 New linker required: {}", info.linker);
                let url = format!("https://perro-downloads.example.com/linkers/{}", info.linker);
                self.download_file(&url, linker_path)?;
            } else {
                eprintln!("✅ Linker already exists: {}", info.linker);
            }
        }

        eprintln!("✅ Version {} installed successfully", version);
        Ok(())
    }

    fn launch_version(&self, version_path: &std::path::Path) -> Result<(), String> {
        eprintln!("🚀 Launching: {}", version_path.display());
        
        let parent_dir = version_path.parent()
            .ok_or("Could not determine parent directory")?;
        
        let mut cmd = std::process::Command::new(version_path);
        cmd.current_dir(parent_dir);
        
        // Pass through any arguments (like filepath for editor mode)
        let args: Vec<String> = std::env::args().skip(1).collect();
        if !args.is_empty() {
            cmd.args(&args);
        }
        
        cmd.spawn()
            .map_err(|e| format!("Failed to launch: {}", e))?;
        
        Ok(())
    }

    fn is_editor_mode(&self) -> bool {
        // Check if we were launched with a filepath argument
        std::env::args().nth(1).is_some()
    }
}

impl Script for BRootScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        println!("{}", self.node_id);
        
        // Get MY version from the API
        let my_version = api.project().version().to_string();
        
        let editor_mode = self.is_editor_mode();

        if editor_mode {
            eprintln!("📝 Editor mode detected (filepath argument provided)");
        } else {
            eprintln!("🎮 Manager mode (no filepath argument)");
        }

        // Skip all version management in debug mode
        if cfg!(debug_assertions) {
            api.set_window_title("Perro Engine [DEBUG]".to_string());
            eprintln!("🐛 Debug mode - skipping version management");
            return;
        }

        // Step 1: Am I in the right location?
        let exe_path = match std::env::current_exe() {
            Ok(path) => path,
            Err(e) => {
                eprintln!("❌ Could not determine current executable path: {}", e);
                std::process::exit(1);
            }
        };
        let exe_name = exe_path.file_name().unwrap();

        let expected_path_str = format!("user://versions/{}/", my_version);
        if let Some(expected_base) = api.resolve_path(&expected_path_str) {
            let current_dir = std::env::current_dir().unwrap();
            let expected_pathbuf = std::path::PathBuf::from(&expected_base);
            
            if !current_dir.starts_with(&expected_pathbuf) {
                eprintln!("⚠️  Not running from correct location");
                eprintln!("Expected: {}", expected_pathbuf.display());
                eprintln!("Current: {}", current_dir.display());
                
                let target_exe = expected_pathbuf.join(exe_name);
                
                if !target_exe.exists() {
                    eprintln!("📦 Copying executable to correct location...");
                    
                    if let Err(e) = std::fs::create_dir_all(&expected_pathbuf) {
                        eprintln!("❌ Failed to create directory: {}", e);
                        std::process::exit(1);
                    }
                    
                    if let Err(e) = std::fs::copy(&exe_path, &target_exe) {
                        eprintln!("❌ Failed to copy executable: {}", e);
                        std::process::exit(1);
                    }
                    
                    eprintln!("✅ Copied to: {}", target_exe.display());
                }
                
                if let Err(e) = self.launch_version(&target_exe) {
                    eprintln!("❌ Failed to launch from correct location: {}", e);
                    std::process::exit(1);
                }
                
                eprintln!("👋 Closing current instance");
                std::process::exit(0);
            }
        } else {
            eprintln!("❌ Could not resolve user://versions/{}/", my_version);
            std::process::exit(1);
        }

        eprintln!("✅ Running from correct location");

        // Step 2: Am I the highest version? (Only check in manager mode)
        if !editor_mode {
            // Only look for higher versions in manager mode
            if let Some(versions_base) = api.resolve_path("user://versions/") {
                let versions_dir = std::path::PathBuf::from(versions_base);
                
                if let Some((higher_version, higher_exe_path)) = self.find_highest_version(&versions_dir, exe_name, &my_version) {
                    eprintln!("⬆️  Found higher version: {} (current: {})", higher_version, my_version);
                    
                    // Double-check it's in the correct location before launching
                    if self.is_exe_in_correct_location(&higher_exe_path, &higher_version) {
                        eprintln!("✅ Higher version is in correct location");
                        
                        if let Err(e) = self.launch_version(&higher_exe_path) {
                            eprintln!("❌ Failed to launch higher version: {}", e);
                            std::process::exit(1);
                        }
                        
                        eprintln!("👋 Closing current instance");
                        std::process::exit(0);
                    } else {
                        eprintln!("⚠️  Higher version found but not in correct location, ignoring");
                    }
                }
            }

            eprintln!("✅ Running highest version: {}", my_version);
        } else {
            eprintln!("📝 Editor mode - skipping version check (launched for this specific version)");
        }

        // Step 3: Check MY requirements (only in editor mode - manager doesn't need toolchain/linker)
        if editor_mode {
            let missing = self.check_my_requirements(api);
            if !missing.is_empty() {
                eprintln!("⚠️  Missing my requirements for compilation:");
                for item in &missing {
                    eprintln!("  - {}", item);
                }
                
                if let Err(e) = self.repair_my_requirements(api) {
                    eprintln!("❌ Failed to repair my requirements: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("✅ All my requirements satisfied");
            }
        } else {
            eprintln!("🎮 Manager mode - skipping toolchain/linker check (not needed until compilation)");
        }

        // Step 4: Only check for updates in manager mode
        if !editor_mode {
            match self.fetch_manifest() {
                Ok(manifest) => {
                    // Is there a newer version?
                    if natord::compare(&manifest.latest, &my_version) == std::cmp::Ordering::Greater {
                        eprintln!("🎉 Update available: {} -> {}", my_version, manifest.latest);
                        
                        // Check if this version is already installed
                        if let Some(new_version_info) = manifest.versions.get(&manifest.latest) {
                            let new_version_path_str = format!("user://versions/{}/{}", manifest.latest, new_version_info.editor);
                            let already_installed = if let Some(new_exe_path) = api.resolve_path(&new_version_path_str) {
                                std::path::Path::new(&new_exe_path).exists()
                            } else {
                                false
                            };

                            if already_installed {
                                eprintln!("✅ Version {} already installed, launching it", manifest.latest);
                                let new_exe_path = api.resolve_path(&new_version_path_str).unwrap();
                                let new_exe = std::path::PathBuf::from(new_exe_path);
                                
                                if let Err(e) = self.launch_version(&new_exe) {
                                    eprintln!("❌ Failed to launch existing version: {}", e);
                                    eprintln!("⚠️  Continuing with current version");
                                    api.set_window_title(format!("Perro Engine {}", my_version));
                                } else {
                                    eprintln!("👋 Closing current instance");
                                    std::process::exit(0);
                                }
                            } else {
                                // Download and install the new version
                                eprintln!("📥 Downloading new version...");
                                if let Err(e) = self.download_and_install_update(api, &manifest.latest, new_version_info) {
                                    eprintln!("❌ Failed to download update: {}", e);
                                    eprintln!("⚠️  Continuing with current version");
                                    api.set_window_title(format!("Perro Engine {}", my_version));
                                } else {
                                    // Successfully downloaded, now launch it
                                    let new_version_path_str = format!("user://versions/{}/{}", manifest.latest, new_version_info.editor);
                                    if let Some(new_exe_path) = api.resolve_path(&new_version_path_str) {
                                        let new_exe = std::path::PathBuf::from(new_exe_path);
                                        
                                        eprintln!("🚀 Launching new version: {}", manifest.latest);
                                        if let Err(e) = self.launch_version(&new_exe) {
                                            eprintln!("❌ Failed to launch new version: {}", e);
                                            eprintln!("⚠️  Continuing with current version");
                                            api.set_window_title(format!("Perro Engine {}", my_version));
                                        } else {
                                            eprintln!("👋 Closing current instance");
                                            std::process::exit(0);
                                        }
                                    } else {
                                        eprintln!("❌ Could not resolve path to new version");
                                        api.set_window_title(format!("Perro Engine {}", my_version));
                                    }
                                }
                            }
                        } else {
                            eprintln!("⚠️  Manifest missing info for version {}", manifest.latest);
                            api.set_window_title(format!("Perro Engine {}", my_version));
                        }
                    } else {
                        eprintln!("✅ Running latest version");
                        api.set_window_title(format!("Perro Engine {}", my_version));
                    }
                }
                Err(e) => {
                    eprintln!("⚠️  Failed to fetch manifest: {}", e);
                    eprintln!("⚠️  Continuing with current version");
                    api.set_window_title(format!("Perro Engine {}", my_version));
                }
            }
        } else {
            api.set_window_title(format!("Perro Editor {}", my_version));
            eprintln!("📝 Editor mode - skipping update check");
        }

        eprintln!("🎮 Initialization complete");
        eprintln!("Running from: {}", std::env::current_dir().unwrap().display());
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        // Runtime update logic
    }

    fn set_node_id(&mut self, id: Uuid) {
        self.node_id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node_id
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self as &mut dyn Any
    }

    fn apply_exports(&mut self, hashmap: &std::collections::HashMap<String, serde_json::Value>) {
    }

    fn get_var(&self, name: &str) -> Option<Var> {
        match name {
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, val: Var) -> Option<()> {
        match (name, val) {
            _ => None,
        }
    }
}

mod natord {
    pub fn compare(a: &str, b: &str) -> std::cmp::Ordering {
        let a_parts: Vec<&str> = a.split('.').collect();
        let b_parts: Vec<&str> = b.split('.').collect();
        
        let max_len = a_parts.len().max(b_parts.len());
        
        for i in 0..max_len {
            let a_part = a_parts.get(i).unwrap_or(&"0");
            let b_part = b_parts.get(i).unwrap_or(&"0");
            
            match (a_part.parse::<u32>(), b_part.parse::<u32>()) {
                (Ok(a_num), Ok(b_num)) => {
                    match a_num.cmp(&b_num) {
                        std::cmp::Ordering::Equal => continue,
                        other => return other,
                    }
                }
                _ => {
                    // Fallback to string comparison for non-numeric parts
                    match a_part.cmp(b_part) {
                        std::cmp::Ordering::Equal => continue,
                        other => return other,
                    }
                }
            }
        }
        
        std::cmp::Ordering::Equal
    }
}
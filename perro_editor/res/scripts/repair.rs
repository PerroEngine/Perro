#![allow(improper_ctypes_definitions)]
#![allow(unused)]



use std::any::Any;
use std::collections::HashMap;
use serde_json::{Value, json};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use perro_core::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use smallvec::{SmallVec, smallvec};
use phf::{phf_map, Map};

#[unsafe(no_mangle)]
pub extern "C" fn repair_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(RepairScript {
        id: Uuid::nil(),
        toolchain_ver: String::new(),
        engine_ver: String::new(),
        editor_mode: false,
    })) as *mut dyn ScriptObject
}

/// @PerroScript
pub struct RepairScript {
    id: Uuid,
    toolchain_ver: String,
    engine_ver: String,
    editor_mode: bool,
}

impl RepairScript {
    // ------------------- Toolchain Management -------------------

    fn toolchain_exists(&self, api: &ScriptApi, toolchain: &str) -> bool {
        let toolchain_path_str = format!("user://toolchains/{}", toolchain);
        if let Some(toolchain_path) = api.resolve_path(&toolchain_path_str) {
            let base = Path::new(&toolchain_path);
            let cargo = base.join("cargo").join("bin").join("cargo.exe");
            let rustc = base.join("rustc").join("bin").join("rustc.exe");
            cargo.exists() && rustc.exists()
        } else {
            false
        }
    }

    fn get_rust_url(&self, toolchain: &str) -> String {
        // toolchain should already be in the format rust-VERSION-x86_64-pc-windows-gnu
        // but if it's just a version number, we need to construct the proper format
        let toolchain_name = if toolchain.starts_with("rust-") {
            toolchain.to_string()
        } else {
            // Detect platform and construct proper toolchain name
            #[cfg(target_os = "windows")]
            let platform_suffix = "x86_64-pc-windows-gnu";
            #[cfg(target_os = "macos")]
            let platform_suffix = "x86_64-apple-darwin";
            #[cfg(target_os = "linux")]
            let platform_suffix = "x86_64-unknown-linux-gnu";
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            let platform_suffix = "x86_64-unknown-linux-gnu"; // fallback
            
            format!("rust-{}-{}", toolchain, platform_suffix)
        };
        format!("https://static.rust-lang.org/dist/{}.tar.gz", toolchain_name)
    }

    fn download_file(&self, url: &str, dest_path: &Path) -> Result<(), String> {
        eprintln!("📥 Downloading: {}", url);
        
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir: {}", e))?;
        }

        // Try curl first
        let curl_result = Command::new("curl")
            .args(&[
                "-L",
                "-f",
                "--retry",
                "3",
                "--connect-timeout",
                "30",
                "-o",
                dest_path.to_str().unwrap(),
                url,
            ])
            .output();

        match curl_result {
            Ok(output) if output.status.success() => {
                eprintln!("✅ Downloaded with curl");
                return Ok(());
            }
            Ok(output) => {
                eprintln!("⚠️ curl failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            Err(_) => eprintln!("⚠️ curl not available, trying wget..."),
        }

        // Fallback to wget
        let wget_result = Command::new("wget")
            .args(&[
                "--tries=3",
                "--timeout=30",
                "-O",
                dest_path.to_str().unwrap(),
                url,
            ])
            .output();

        match wget_result {
            Ok(output) if output.status.success() => {
                eprintln!("✅ Downloaded with wget");
                Ok(())
            }
            Ok(output) => Err(format!(
                "wget failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )),
            Err(_) => Err("Neither curl nor wget available".to_string()),
        }
    }

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
        eprintln!("⏳ This may take several minutes...");
        
        std::fs::create_dir_all(toolchain_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        let url = self.get_rust_url(toolchain);
        let tar_path = toolchain_dir.join("rust.tar.gz");

        self.download_file(&url, &tar_path)?;

        eprintln!("📦 Extracting toolchain...");
        let output = Command::new("tar")
            .args(&[
                "-xzf",
                tar_path.to_str().unwrap(),
                "-C",
                toolchain_dir.to_str().unwrap(),
                "--strip-components=1",
            ])
            .output()
            .map_err(|e| format!("Failed to run tar: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "tar extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        std::fs::remove_file(&tar_path).ok();

        if !self.toolchain_exists(api, toolchain) {
            return Err("Toolchain install verification failed".into());
        }

        eprintln!("✅ Rust toolchain installed successfully");
        Ok(())
    }

    // ------------------- Runtime Verification -------------------

    fn find_exe_in_dir(&self, dir: &Path) -> Option<PathBuf> {
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().map_or(false, |e| e == "exe"))
    }

    fn runtime_exists(&self, api: &ScriptApi, version: &str) -> bool {
        let editor_path = format!("user://versions/{}/editor/", version);
        if let Some(abs) = api.resolve_path(&editor_path) {
            let runtime_exe = Path::new(&abs).join("PerroDevRuntime.exe");
            runtime_exe.exists()
        } else {
            false
        }
    }

    fn download_runtime(&self, api: &ScriptApi, version: &str) -> Result<(), String> {
        eprintln!("📦 Installing runtime for version {}...", version);

        let editor_dir_str = format!("user://versions/{}/editor/", version);
        let editor_dir_resolved = api
            .resolve_path(&editor_dir_str)
            .ok_or("Failed to resolve editor dir")?;

        let editor_path = Path::new(&editor_dir_resolved);
        std::fs::create_dir_all(editor_path)
            .map_err(|e| format!("Failed to create editor dir: {}", e))?;

        let runtime_exe = editor_path.join("PerroDevRuntime.exe");

        if !runtime_exe.exists() {
            let url = format!(
                "https://cdn.perroengine.com/versions/{}/PerroDevRuntime.exe",
                version
            );

            eprintln!("📥 Downloading runtime: {}", url);
            self.download_file(&url, &runtime_exe)?;
        }

        eprintln!("✅ Runtime version {} installed", version);
        Ok(())
    }

    // ------------------- Repair Operations -------------------

    /// Check and repair toolchain (called in editor mode)
    pub fn check_and_repair_toolchain(&self, api: &ScriptApi) -> Result<(), String> {
        if self.toolchain_ver.is_empty() {
            eprintln!("⚠️ No toolchain specified in project metadata");
            return Ok(());
        }

        eprintln!("🔧 Checking toolchain: {}", self.toolchain_ver);

        if !self.toolchain_exists(api, &self.toolchain_ver) {
            eprintln!("❌ Toolchain not found: {}", self.toolchain_ver);
            eprintln!("🔄 Installing required toolchain...");
            self.install_rust_toolchain(api, &self.toolchain_ver)?;
        } else {
            eprintln!("✅ Toolchain verified: {}", self.toolchain_ver);
        }

        Ok(())
    }

    /// Check and repair runtime (verifies engine version exists)
    pub fn check_and_repair_runtime(&self, api: &ScriptApi) -> Result<(), String> {
        if self.engine_ver.is_empty() {
            eprintln!("⚠️ No engine version specified");
            return Ok(());
        }

        eprintln!("🔧 Checking runtime: {}", self.engine_ver);

        if !self.runtime_exists(api, &self.engine_ver) {
            eprintln!("❌ Runtime not found: {}", self.engine_ver);
            eprintln!("🔄 Installing required runtime...");
            self.download_runtime(api, &self.engine_ver)?;
        } else {
            eprintln!("✅ Runtime verified: {}", self.engine_ver);
        }

        Ok(())
    }

    /// Full repair - checks both toolchain and runtime
    pub fn full_repair(&self, api: &ScriptApi) -> Result<(), String> {
        eprintln!("🔧 Starting full repair...");
        eprintln!("================================");

        // Check runtime first
        if let Err(e) = self.check_and_repair_runtime(api) {
            eprintln!("❌ Runtime repair failed: {}", e);
        }

        // Check toolchain second
        if let Err(e) = self.check_and_repair_toolchain(api) {
            eprintln!("❌ Toolchain repair failed: {}", e);
        }

        eprintln!("================================");
        eprintln!("✅ Repair complete");
        Ok(())
    }

    /// Handle editor_mode signal - triggered when manager switches to editor mode
    pub fn on_editor_mode(&mut self, api: &mut ScriptApi) {
        eprintln!("🔄 Editor mode signal received: checking dependencies...");
        
        // Step 1: Check/install toolchain
        if let Err(e) = self.check_and_repair_toolchain(api) {
            eprintln!("❌ Toolchain repair failed: {}", e);
            eprintln!("⚠️ Build functionality may not work");
            return; // Can't compile without toolchain
        }

        // Step 2: Check/install runtime
        if let Err(e) = self.check_and_repair_runtime(api) {
            eprintln!("❌ Runtime repair failed: {}", e);
        }

        eprintln!("✅ Dependencies verified");
        
        // Step 3: Always compile scripts when entering editor mode
        eprintln!("🔧 Compiling scripts...");
        match api.compile_scripts() {
            Ok(_) => {
                eprintln!("✅ Scripts compiled successfully");
            }
            Err(e) => {
                eprintln!("❌ Script compilation failed: {}", e);
            }
        }
    }
}

impl Script for RepairScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        self.engine_ver = api.project().version().to_string();
        self.toolchain_ver = api
            .project()
            .get_meta("toolchain")
            .unwrap_or("")
            .to_string();

        eprintln!("🔧 Repair script initialized");
        eprintln!("   Engine: {}", self.engine_ver);
        eprintln!("   Toolchain: {}", if self.toolchain_ver.is_empty() { "none" } else { &self.toolchain_ver });
        eprintln!("   Waiting for editor_mode signal...");

        // Skip in debug builds
        if cfg!(debug_assertions) {
            eprintln!("🐛 Debug build: repair disabled");
            return;
        }

        // Connect to editor_mode signal - will be triggered when manager switches to editor mode
        if self.id == Uuid::nil() {
            eprintln!("❌ ERROR: self.id is nil when trying to connect signal!");
            return;
        }
        eprintln!("🔗 Connecting signal 'editor_mode' to function 'on_editor_mode' for node {}", self.id);
        api.connect_signal("editor_mode", self.id, "on_editor_mode");
        eprintln!("✅ Signal connection made");
    }

}

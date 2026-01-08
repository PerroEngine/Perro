#![allow(improper_ctypes_definitions)]
#![allow(unused)]



use std::any::Any;
use std::collections::HashMap;
use serde_json::{Value, json};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use perro_core::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use smallvec::{SmallVec, smallvec};
use phf::{phf_map, Map};

#[unsafe(no_mangle)]
pub extern "C" fn scripts_repair_rs_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(RepairScript {
        id: Uuid::nil(),
        toolchain_ver: String::new(),
        engine_ver: String::new(),
        editor_mode: false,
    })) as *mut dyn ScriptObject
}

/// @PerroScript
pub static MEMBER_TO_ATTRIBUTES_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {
};

static ATTRIBUTE_TO_MEMBERS_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {
};

struct RepairScript {
    id: Uuid,
    toolchain_ver: String,
    engine_ver: String,
    editor_mode: bool,
}

impl RepairScript {
    // ------------------- Helper Functions -------------------
    
    /// Recursively copy a directory
    fn copy_dir_all(&self, src: &Path, dst: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dst)
            .map_err(|e| format!("Failed to create destination directory: {}", e))?;
        
        for entry in std::fs::read_dir(src)
            .map_err(|e| format!("Failed to read source directory: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            let file_name = entry.file_name();
            let dst_path = dst.join(&file_name);
            
            if path.is_dir() {
                self.copy_dir_all(&path, &dst_path)?;
            } else {
                std::fs::copy(&path, &dst_path)
                    .map_err(|e| format!("Failed to copy file {:?}: {}", path, e))?;
            }
        }
        Ok(())
    }
    
    // ------------------- Toolchain Management -------------------

    /// Normalize toolchain name - converts version number to full toolchain name
    /// e.g., "1.92.0" -> "rust-1.92.0-x86_64-pc-windows-gnu"
    fn normalize_toolchain_name(&self, toolchain: &str) -> String {
        if toolchain.starts_with("rust-") {
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
        }
    }

    fn toolchain_exists(&self, api: &ScriptApi, toolchain: &str) -> bool {
        // Always use the full normalized toolchain name for folder paths
        let toolchain_name = self.normalize_toolchain_name(toolchain);
        let toolchain_path_str = format!("user://toolchains/{}", toolchain_name);
        if let Some(toolchain_path) = api.resolve_path(&toolchain_path_str) {
            let base = Path::new(&toolchain_path);
            let cargo = base.join("cargo").join("bin").join("cargo.exe");
            let rustc = base.join("rustc").join("bin").join("rustc.exe");
            
            // Only require cargo and rustc to exist - these are the essential components
            let essential_exist = cargo.exists() && rustc.exists();
            
            if !essential_exist {
                // [stripped for release] eprintln!("⚠️  Toolchain verification failed:");

                // [stripped for release] eprintln!("   cargo: {}", if cargo.exists() { "✅" } else { "❌" });

                // [stripped for release] eprintln!("   rustc: {}", if rustc.exists() { "✅" } else { "❌" });

                return false;
            }
            
            // Check for standard library as a diagnostic (warning, not failure)
            let target_triple = if toolchain_name.contains("windows-gnu") {
                "x86_64-pc-windows-gnu"
            } else if toolchain_name.contains("apple-darwin") {
                "x86_64-apple-darwin"
            } else {
                "x86_64-unknown-linux-gnu"
            };
            
            // Check for standard library in rust-std subdirectory
            // The standard library is in rust-std-<target>/lib/rustlib/<target>/lib/
            let rust_std_dir_name = format!("rust-std-{}", target_triple);
            let rustlib_dir = base.join(&rust_std_dir_name).join("lib").join("rustlib").join(target_triple).join("lib");
            let possible_locations = vec![
                ("rust-std subdir", rustlib_dir),
            ];
            
            let mut found_location: Option<&str> = None;
            for (name, rustlib_dir) in &possible_locations {
                if rustlib_dir.exists() {
                    let has_files = std::fs::read_dir(rustlib_dir)
                        .map(|d| d.count() > 0)
                        .unwrap_or(false);
                    if has_files {
                        found_location = Some(name);
                        // [stripped for release] eprintln!("✅ Standard library found at {}: {}", name, rustlib_dir.display());

                        break;
                    }
                }
            }
            
            if found_location.is_none() {
                // [stripped for release] eprintln!("⚠️  Warning: Standard library not found in expected locations:");

                for (name, loc) in &possible_locations {
                    // [stripped for release] eprintln!("   ❌ {}: {}", name, loc.display());

                }
                // [stripped for release] eprintln!("   This may cause build errors, but toolchain cargo/rustc are present");

                // Diagnostic: list what's actually in the toolchain directory
                // [stripped for release] eprintln!("   📂 Toolchain directory contents:");

                if let Ok(entries) = std::fs::read_dir(base) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let name = entry.file_name().to_string_lossy().to_string();
                        if path.is_dir() {
                            // [stripped for release] eprintln!("      📁 {}", name);

                            // Check if this directory might contain rustlib
                            if name == "lib" || name == "rustc" {
                                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                                    for sub_entry in sub_entries.flatten() {
                                        let sub_name = sub_entry.file_name().to_string_lossy().to_string();
                                        if sub_name == "rustlib" {
                                            // [stripped for release] eprintln!("         📁 rustlib/ (found!)");

                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            true
        } else {
            false
        }
    }

    fn get_rust_url(&self, toolchain: &str) -> String {
        // Normalize to full toolchain name for URL
        let toolchain_name = self.normalize_toolchain_name(toolchain);
        format!("https://static.rust-lang.org/dist/{}.tar.gz", toolchain_name)
    }

    fn download_file(&self, url: &str, dest_path: &Path) -> Result<(), String> {
        // [stripped for release] eprintln!("📥 Downloading: {}", url);

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
                // [stripped for release] eprintln!("✅ Downloaded with curl");

                return Ok(());
            }
            Ok(output) => {
                // [stripped for release] eprintln!("⚠️ curl failed: {}", String::from_utf8_lossy(&output.stderr));

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
                // [stripped for release] eprintln!("✅ Downloaded with wget");

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
        // Normalize toolchain name to match what compiler expects
        let toolchain_name = self.get_rust_url(toolchain)
            .split('/')
            .last()
            .unwrap_or(toolchain)
            .replace(".tar.gz", "");
        let toolchain_path_str = format!("user://toolchains/{}", toolchain_name);
        let toolchain_path = api
            .resolve_path(&toolchain_path_str)
            .ok_or("Failed to resolve toolchain dir")?;
        let toolchain_dir = Path::new(&toolchain_path);

        if self.toolchain_exists(api, toolchain) {
            // [stripped for release] eprintln!("✅ Toolchain already installed: {}", toolchain_name);

            return Ok(());
        }

        // [stripped for release] eprintln!("📦 Installing Rust toolchain: {}", toolchain_name);

        // [stripped for release] eprintln!("⏳ This may take several minutes...");

        std::fs::create_dir_all(toolchain_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        let url = self.get_rust_url(toolchain);
        let tar_path = toolchain_dir.join("rust.tar.gz");

        self.download_file(&url, &tar_path)?;

        // [stripped for release] eprintln!("📦 Extracting toolchain...");

        // [stripped for release] eprintln!("⏳ This may take several minutes (extracting ~1GB)...");

        // [stripped for release] eprintln!("💡 Tip: Extraction may appear to hang, but it's working in the background");

        // Use PowerShell to run tar.exe with better feedback
        // Note: Rust doesn't provide .zip files, only .tar.gz, so we use tar.exe
        // The MinGW extraction uses zip because w64devkit provides zip files
        #[cfg(target_os = "windows")]
        {
            // Write PowerShell script to run tar.exe with progress indication
            // Properly escape paths with spaces for PowerShell
            // In PowerShell, single quotes in strings need to be doubled
            let tar_path_escaped = tar_path.to_string_lossy().replace('\'', "''");
            let dest_path_escaped = toolchain_dir.to_string_lossy().replace('\'', "''");
            let ps_script = format!(
                r#"$ErrorActionPreference = 'Stop'; 
                try {{
                    Write-Host 'Starting extraction...' -ForegroundColor Yellow;
                    $tarPath = '{}';
                    $destPath = '{}';
                    if (-not (Test-Path $tarPath)) {{
                        throw 'Tar file not found: ' + $tarPath;
                    }}
                    
                    # Use tar.exe (Windows 10+ has it built-in)
                    # Quote paths properly to handle spaces - use & to call with proper quoting
                    Write-Host 'Extracting files (this may take a while)...' -ForegroundColor Cyan;
                    & tar.exe -xzf $tarPath -C $destPath --strip-components=1;
                    if ($LASTEXITCODE -ne 0) {{
                        throw 'tar.exe extraction failed with exit code ' + $LASTEXITCODE;
                    }}
                    Write-Host 'Extraction completed successfully' -ForegroundColor Green;
                    exit 0;
                }} catch {{
                    Write-Host ('Extraction failed: ' + $_.Exception.Message) -ForegroundColor Red;
                    exit 1;
                }}"#,
                tar_path_escaped,
                dest_path_escaped
            );
            
            let temp_script = toolchain_dir.join("extract_rust.ps1");
            std::fs::write(&temp_script, &ps_script)
                .map_err(|e| format!("Failed to write PowerShell script: {}", e))?;
            
            // [stripped for release] eprintln!("🔧 Starting extraction process...");

            // Start PowerShell process (don't wait for it - it may hang)
            let mut child = Command::new("powershell")
                .args(&[
                    "-NoProfile",
                    "-ExecutionPolicy", "Bypass",
                    "-File",
                    temp_script.to_str().unwrap(),
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to start PowerShell: {}", e))?;
            
            // Don't wait for the process - instead poll for extraction completion
            // This way we can detect when extraction actually finishes even if PowerShell hangs
            // [stripped for release] eprintln!("🔍 Monitoring extraction progress...");

            // [stripped for release] eprintln!("📦 Rust toolchain extraction is in progress...");

            let cargo_dir = toolchain_dir.join("cargo");
            let rustc_dir = toolchain_dir.join("rustc");
            let cargo_exe = cargo_dir.join("bin").join("cargo.exe");
            let rustc_exe = rustc_dir.join("bin").join("rustc.exe");
            let mut attempts = 0;
            let max_attempts = 300; // Wait up to 5 minutes for large extraction
            
            let mut extraction_complete = false;
            let mut directories_found = false;
            let mut script_completed = false;
            
            while !extraction_complete && attempts < max_attempts {
                attempts += 1;
                
                // Check if extraction completed by looking for the actual executables
                // This is what toolchain_exists will check, so verify the same thing
                if cargo_exe.exists() && rustc_exe.exists() {
                    extraction_complete = true;
                    // [stripped for release] eprintln!("✅ Extraction completed and verified (executables found)");

                    break;
                }
                
                // Give progress feedback similar to Expand-Archive
                if !directories_found && (cargo_dir.exists() || rustc_dir.exists()) {
                    directories_found = true;
                    // [stripped for release] eprintln!("📁 Toolchain directories detected, extraction continuing...");

                }
                
                // Check if PowerShell process finished
                if !script_completed {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            script_completed = true;
                            if status.success() {
                                // [stripped for release] eprintln!("✅ Extraction script completed, verifying files...");

                            } else {
                                // [stripped for release] eprintln!("⚠️  Extraction script exited with error, but checking if files were extracted...");

                            }
                        }
                        Ok(None) => {
                            // Process still running - show periodic progress
                            if attempts % 10 == 0 {
                                // [stripped for release] eprintln!("📦 Rust toolchain extraction is in progress... ({}s elapsed)", attempts * 2);

                            }
                        }
                        Err(e) => {
                            // [stripped for release] eprintln!("⚠️  Error checking extraction process: {}", e);

                        }
                    }
                } else {
                    // Script finished but files not verified yet - extraction might still be syncing
                    if attempts % 5 == 0 {
                        // [stripped for release] eprintln!("⏳ Verifying extracted files... ({}s elapsed)", attempts * 2);

                    }
                }
                
                std::thread::sleep(std::time::Duration::from_millis(2000)); // Check every 2 seconds
            }
            
            // Clean up temp script
            std::fs::remove_file(&temp_script).ok();
            
            if !extraction_complete {
                // Try to kill the PowerShell process if it's still running
                let _ = child.kill();
                
                if !cargo_dir.exists() || !rustc_dir.exists() {
                    return Err("Rust toolchain extraction did not complete - cargo or rustc directories not found".into());
                } else {
                    // [stripped for release] eprintln!("⚠️  Extraction verification timeout, but directories exist - continuing...");

                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
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
        }

        // Only delete tar.gz after verification that extraction completed
        // This way if extraction fails, we can retry without re-downloading
        if toolchain_dir.join("cargo").exists() && toolchain_dir.join("rustc").exists() {
            std::fs::remove_file(&tar_path).ok();
            // [stripped for release] eprintln!("🗑️  Cleaned up tar.gz file");

        } else {
            // [stripped for release] eprintln!("⚠️  Keeping tar.gz file - extraction may not have completed");

        }

        // Wait a moment for filesystem to sync, then verify the toolchain
        // [stripped for release] eprintln!("🔍 Verifying toolchain installation...");

        std::thread::sleep(std::time::Duration::from_millis(1000)); // Give filesystem a moment
        
        // Retry verification a few times in case filesystem hasn't synced yet
        let mut verification_ok = false;
        for attempt in 1..=5 {
            if self.toolchain_exists(api, toolchain) {
                verification_ok = true;
                break;
            }
            
            if attempt < 5 {
                // [stripped for release] eprintln!("⏳ Waiting for filesystem to sync (attempt {}/5)...", attempt);

                std::thread::sleep(std::time::Duration::from_millis(2000));
            }
        }
        
        if !verification_ok {
            // [stripped for release] eprintln!("❌ Toolchain verification failed after multiple attempts");

            // [stripped for release] eprintln!("💡 This might be a filesystem sync issue. Try restarting the editor.");

            return Err("Toolchain install verification failed - files may still be syncing".into());
        }

        // [stripped for release] eprintln!("✅ Rust toolchain installed successfully");

        // Install minimal GCC compiler (w64devkit) for C/C++ compilation
        self.install_mingw(api, &toolchain_name)?;
        
        Ok(())
    }
    
    fn install_mingw(&self, api: &ScriptApi, toolchain_name: &str) -> Result<(), String> {
        let toolchain_path_str = format!("user://toolchains/{}", toolchain_name);
        let toolchain_path = api
            .resolve_path(&toolchain_path_str)
            .ok_or("Failed to resolve toolchain dir")?;
        let toolchain_dir = Path::new(&toolchain_path);
        let mingw_dir = toolchain_dir.join("mingw");
        
        // Check if MinGW is already installed
        let gcc_exe = mingw_dir.join("bin").join("gcc.exe");
        if gcc_exe.exists() {
            // [stripped for release] eprintln!("✅ MinGW GCC already installed");

            return Ok(());
        }
        
        // [stripped for release] eprintln!("📦 Installing minimal GCC compiler (w64devkit)...");

        // [stripped for release] eprintln!("⏳ This may take a minute...");

        // Download w64devkit - minimal MinGW-w64 distribution
        // Using version 1.20.0 which is stable and minimal (~50MB)
        let mingw_url = "https://github.com/skeeto/w64devkit/releases/download/v1.20.0/w64devkit-1.20.0.zip";
        let zip_path = toolchain_dir.join("w64devkit.zip");
        
        self.download_file(mingw_url, &zip_path)?;
        
        // [stripped for release] eprintln!("📦 Extracting MinGW...");

        // [stripped for release] eprintln!("⏳ This may take a while (extracting ~50MB)...");

        // Extract zip file - on Windows we can use PowerShell
        #[cfg(target_os = "windows")]
        {
            // Use PowerShell with explicit error handling and output flushing
            // Use -ExecutionPolicy Bypass to avoid policy issues
            // Use Start-Process with -Wait -NoNewWindow to ensure it completes
            let ps_script = format!(
                r#"$ErrorActionPreference = 'Stop'; 
                try {{
                    Write-Host 'Starting extraction...' -ForegroundColor Yellow;
                    $zipPath = '{}';
                    $destPath = '{}';
                    if (-not (Test-Path $zipPath)) {{
                        throw 'Zip file not found: ' + $zipPath;
                    }}
                    Expand-Archive -Path $zipPath -DestinationPath $destPath -Force;
                    Write-Host 'Extraction completed successfully' -ForegroundColor Green;
                    exit 0;
                }} catch {{
                    Write-Host ('Extraction failed: ' + $_.Exception.Message) -ForegroundColor Red;
                    exit 1;
                }}"#,
                zip_path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\""),
                toolchain_dir.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"")
            );
            
            // Write script to temp file to avoid command line length issues
            let temp_script = toolchain_dir.join("extract_mingw.ps1");
            std::fs::write(&temp_script, &ps_script)
                .map_err(|e| format!("Failed to write PowerShell script: {}", e))?;
            
            // [stripped for release] eprintln!("🔧 Running extraction script...");

            let output = Command::new("powershell")
                .args(&[
                    "-NoProfile",
                    "-ExecutionPolicy", "Bypass",
                    "-File",
                    temp_script.to_str().unwrap(),
                ])
                .output()
                .map_err(|e| format!("Failed to run PowerShell: {}", e))?;
            
            // Clean up temp script
            std::fs::remove_file(&temp_script).ok();
            
            // Print output for user feedback
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            if !stdout.is_empty() {
                // [stripped for release] eprintln!("{}", stdout);

            }
            
            if !output.status.success() {
                // [stripped for release] eprintln!("❌ MinGW extraction failed");

                if !stderr.is_empty() {
                    // [stripped for release] eprintln!("Error: {}", stderr);

                }
                return Err(format!("MinGW extraction failed: {}", stderr));
            }
            
            // Check if extraction actually completed by looking for the extracted directory
            // Sometimes PowerShell hangs even though extraction completes
            // [stripped for release] eprintln!("🔍 Verifying extraction...");

            let mut extraction_verified = false;
            let mut attempts = 0;
            let max_attempts = 30; // Wait up to 30 seconds
            
            while !extraction_verified && attempts < max_attempts {
                // Check for w64devkit directory
                let extracted_dir = toolchain_dir.join("w64devkit-1.20.0");
                if extracted_dir.exists() {
                    extraction_verified = true;
                    // [stripped for release] eprintln!("✅ Extraction verified (found w64devkit-1.20.0)");

                } else {
                    // Try to find any w64devkit directory
                    if let Ok(entries) = std::fs::read_dir(toolchain_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_dir() {
                                if let Some(name) = path.file_name() {
                                    if name.to_string_lossy().starts_with("w64devkit") {
                                        extraction_verified = true;
                                        // [stripped for release] eprintln!("✅ Extraction verified (found {})", name.to_string_lossy());

                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                
                if !extraction_verified {
                    attempts += 1;
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                    if attempts % 5 == 0 {
                        // [stripped for release] eprintln!("⏳ Still waiting for extraction... ({}s)", attempts);

                    }
                }
            }
            
            if !extraction_verified {
                // Check if output status was successful even though we didn't find the dir
                if output.status.success() {
                    // [stripped for release] eprintln!("⚠️  PowerShell reported success but directory not found, checking again...");

                    // Give it one more second
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                } else {
                    return Err("MinGW extraction failed - extracted directory not found".into());
                }
            }
            
            // Rename extracted directory to mingw/
            let extracted_dir = toolchain_dir.join("w64devkit-1.20.0");
            if extracted_dir.exists() {
                std::fs::rename(&extracted_dir, &mingw_dir)
                    .map_err(|e| format!("Failed to rename MinGW directory: {}", e))?;
            } else {
                // Try to find any w64devkit directory
                if let Ok(entries) = std::fs::read_dir(toolchain_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Some(name) = path.file_name() {
                                if name.to_string_lossy().starts_with("w64devkit") {
                                    std::fs::rename(&path, &mingw_dir)
                                        .map_err(|e| format!("Failed to rename MinGW directory: {}", e))?;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            
            // [stripped for release] eprintln!("✅ Extraction completed and verified");

        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // On non-Windows, try unzip command
            let output = Command::new("unzip")
                .args(&[
                    "-q",
                    zip_path.to_str().unwrap(),
                    "-d",
                    toolchain_dir.to_str().unwrap(),
                ])
                .output()
                .map_err(|e| format!("Failed to run unzip: {}", e))?;
            
            if !output.status.success() {
                return Err(format!(
                    "MinGW extraction failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            
            // w64devkit extracts to w64devkit-<version>/, rename to mingw/
            // Check for any directory starting with w64devkit
            let extracted_dir = toolchain_dir.join("w64devkit-1.20.0");
            if !extracted_dir.exists() {
                // Try to find any w64devkit directory
                if let Ok(entries) = std::fs::read_dir(toolchain_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Some(name) = path.file_name() {
                                if name.to_string_lossy().starts_with("w64devkit") {
                                    std::fs::rename(&path, &mingw_dir)
                                        .map_err(|e| format!("Failed to rename MinGW directory: {}", e))?;
                                    break;
                                }
                            }
                        }
                    }
                }
            } else {
                std::fs::rename(&extracted_dir, &mingw_dir)
                    .map_err(|e| format!("Failed to rename MinGW directory: {}", e))?;
            }
        }
        
        std::fs::remove_file(&zip_path).ok();
        
        // Verify installation
        if !gcc_exe.exists() {
            return Err("MinGW GCC installation verification failed".into());
        }
        
        // [stripped for release] eprintln!("✅ MinGW GCC installed successfully");

        Ok(())
    }


    // ------------------- Repair Operations -------------------

    /// Check and repair toolchain (called in editor mode)
    pub fn check_and_repair_toolchain(&self, api: &ScriptApi) -> Result<(), String> {
        if self.toolchain_ver.is_empty() {
            // [stripped for release] eprintln!("⚠️ No toolchain specified in project metadata");

            return Ok(());
        }

        // Normalize to full toolchain name for display and operations
        let toolchain_name = self.normalize_toolchain_name(&self.toolchain_ver);
        // [stripped for release] eprintln!("🔧 Checking toolchain: {} (from version: {})", toolchain_name, self.toolchain_ver);

        if !self.toolchain_exists(api, &self.toolchain_ver) {
            // [stripped for release] eprintln!("❌ Toolchain not found: {}", toolchain_name);

            // [stripped for release] eprintln!("🔄 Installing required toolchain...");

            self.install_rust_toolchain(api, &self.toolchain_ver)?;
        } else {
            // [stripped for release] eprintln!("✅ Toolchain verified: {}", toolchain_name);

            // Note: Standard library merge is no longer needed - compiler uses rust-std directory directly
        }
        
        // Also check for MinGW GCC compiler
        let toolchain_path_str = format!("user://toolchains/{}", toolchain_name);
        if let Some(toolchain_path) = api.resolve_path(&toolchain_path_str) {
            let toolchain_dir = Path::new(&toolchain_path);
            let gcc_exe = toolchain_dir.join("mingw").join("bin").join("gcc.exe");
            if !gcc_exe.exists() {
                // [stripped for release] eprintln!("❌ MinGW GCC compiler not found");

                // [stripped for release] eprintln!("🔄 Installing MinGW GCC compiler...");

                self.install_mingw(api, &toolchain_name)?;
            } else {
                // [stripped for release] eprintln!("✅ MinGW GCC compiler verified");

            }
        }

        Ok(())
    }

    /// Full repair - checks toolchain
    pub fn full_repair(&self, api: &ScriptApi) -> Result<(), String> {
        // [stripped for release] eprintln!("🔧 Starting full repair...");

        // [stripped for release] eprintln!("================================");

        // Check toolchain
        if let Err(e) = self.check_and_repair_toolchain(api) {
            // [stripped for release] eprintln!("❌ Toolchain repair failed: {}", e);

        }

        // [stripped for release] eprintln!("================================");

        // [stripped for release] eprintln!("✅ Repair complete");

        Ok(())
    }

    /// Handle editor_mode signal - triggered when manager switches to editor mode
    pub fn on_editor_mode(&mut self, api: &mut ScriptApi) {
        // [stripped for release] eprintln!("🔄 Editor mode signal received: checking dependencies...");

        // Step 1: Check/install toolchain
        if let Err(e) = self.check_and_repair_toolchain(api) {
            // [stripped for release] eprintln!("❌ Toolchain repair failed: {}", e);

            // [stripped for release] eprintln!("⚠️ Build functionality may not work");

            return; // Can't compile without toolchain
        }

        // [stripped for release] eprintln!("✅ Dependencies verified");

        // Step 3: Always compile scripts when entering editor mode
        // [stripped for release] eprintln!("🔧 Compiling scripts...");

        match api.compile_scripts() {
            Ok(_) => {
                // [stripped for release] eprintln!("✅ Scripts compiled successfully");

            }
            Err(e) => {
                // [stripped for release] eprintln!("❌ Script compilation failed: {}", e);

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

        // [stripped for release] eprintln!("🔧 Repair script initialized");

        // [stripped for release] eprintln!("   Engine: {}", self.engine_ver);

        // [stripped for release] eprintln!("   Toolchain: {}", if self.toolchain_ver.is_empty() { "none" } else { &self.toolchain_ver });

        // [stripped for release] eprintln!("   Waiting for editor_mode signal...");

        // Skip in debug builds
        if cfg!(debug_assertions) {
            // [stripped for release] eprintln!("🐛 Debug build: repair disabled");

            return;
        }

        // Connect to editor_mode signal - will be triggered when manager switches to editor mode
        if self.id == Uuid::nil() {
            // [stripped for release] eprintln!("❌ ERROR: self.id is nil when trying to connect signal!");

            return;
        }
        // [stripped for release] eprintln!("🔗 Connecting signal 'editor_mode' to function 'on_editor_mode' for node {}", self.id);

        api.connect_signal("editor_mode", self.id, "on_editor_mode");
        // [stripped for release] eprintln!("✅ Signal connection made");

    }

}



impl ScriptObject for RepairScript {
    fn set_id(&mut self, id: Uuid) {
        self.id = id;
    }

    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_var(&self, var_id: u64) -> Option<Value> {
        VAR_GET_TABLE.get(&var_id).and_then(|f| f(self))
    }

    fn set_var(&mut self, var_id: u64, val: Value) -> Option<()> {
        VAR_SET_TABLE.get(&var_id).and_then(|f| f(self, val))
    }

    fn apply_exposed(&mut self, hashmap: &HashMap<u64, Value>) {
        for (var_id, val) in hashmap.iter() {
            if let Some(f) = VAR_APPLY_TABLE.get(var_id) {
                f(self, val);
            }
        }
    }

    fn call_function(
        &mut self,
        id: u64,
        api: &mut ScriptApi<'_>,
        params: &[Value],
    ) {
        if let Some(f) = DISPATCH_TABLE.get(&id) {
            f(self, params, api);
        }
    }

    // Attributes

    fn attributes_of(&self, member: &str) -> Vec<String> {
        MEMBER_TO_ATTRIBUTES_MAP
            .get(member)
            .map(|attrs| attrs.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }

    fn members_with(&self, attribute: &str) -> Vec<String> {
        ATTRIBUTE_TO_MEMBERS_MAP
            .get(attribute)
            .map(|members| members.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }

    fn has_attribute(&self, member: &str, attribute: &str) -> bool {
        MEMBER_TO_ATTRIBUTES_MAP
            .get(member)
            .map(|attrs| attrs.iter().any(|a| *a == attribute))
            .unwrap_or(false)
    }
    
    fn script_flags(&self) -> ScriptFlags {
        ScriptFlags::new(1)
    }
}

// =========================== Static PHF Dispatch Tables ===========================

static VAR_GET_TABLE: phf::Map<u64, fn(&RepairScript) -> Option<Value>> =
    phf::phf_map! {

    };

static VAR_SET_TABLE: phf::Map<u64, fn(&mut RepairScript, Value) -> Option<()>> =
    phf::phf_map! {

    };

static VAR_APPLY_TABLE: phf::Map<u64, fn(&mut RepairScript, &Value)> =
    phf::phf_map! {

    };

static DISPATCH_TABLE: phf::Map<
    u64,
    fn(&mut RepairScript, &[Value], &mut ScriptApi<'_>),
> = phf::phf_map! {
        7702514212446216076u64 => | script: &mut RepairScript, params: &[Value], api: &mut ScriptApi<'_>| {
let __path_buf_src = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| std::path::PathBuf::from(s))
                            .unwrap_or_default();
let src = __path_buf_src.as_path();
let __path_buf_dst = params.get(1)
                            .and_then(|v| v.as_str())
                            .map(|s| std::path::PathBuf::from(s))
                            .unwrap_or_default();
let dst = __path_buf_dst.as_path();
            script.copy_dir_all(src, dst);
        },
        14934642575308325166u64 => | script: &mut RepairScript, params: &[Value], api: &mut ScriptApi<'_>| {
let toolchain = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.normalize_toolchain_name(&toolchain);
        },
        8781902028938481569u64 => | script: &mut RepairScript, params: &[Value], api: &mut ScriptApi<'_>| {
let toolchain = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.toolchain_exists(api, &toolchain);
        },
        4065922309764327804u64 => | script: &mut RepairScript, params: &[Value], api: &mut ScriptApi<'_>| {
let toolchain = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.get_rust_url(&toolchain);
        },
        3093765166371536820u64 => | script: &mut RepairScript, params: &[Value], api: &mut ScriptApi<'_>| {
let url = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
let __path_buf_dest_path = params.get(1)
                            .and_then(|v| v.as_str())
                            .map(|s| std::path::PathBuf::from(s))
                            .unwrap_or_default();
let dest_path = __path_buf_dest_path.as_path();
            script.download_file(&url, dest_path);
        },
        5290021044309007815u64 => | script: &mut RepairScript, params: &[Value], api: &mut ScriptApi<'_>| {
let toolchain = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.install_rust_toolchain(api, &toolchain);
        },
        16371758703605133773u64 => | script: &mut RepairScript, params: &[Value], api: &mut ScriptApi<'_>| {
let toolchain_name = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.install_mingw(api, &toolchain_name);
        },
        18298374359453742255u64 => | script: &mut RepairScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.check_and_repair_toolchain(api);
        },
        7426547840188762352u64 => | script: &mut RepairScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.full_repair(api);
        },
        14837261686240108618u64 => | script: &mut RepairScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.on_editor_mode(api);
        },

    };

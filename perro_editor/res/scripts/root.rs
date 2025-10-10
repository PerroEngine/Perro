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

#[unsafe(no_mangle)]
pub extern "C" fn root_create_script() -> *mut dyn Script {
    Box::into_raw(Box::new(RootScript {
        node_id: Uuid::nil(),
    })) as *mut dyn Script
}

pub struct RootScript {
    node_id: Uuid,
}

impl RootScript {
    /// Find executable in directory
    fn find_exe_in_dir(&self, dir: &Path) -> Option<PathBuf> {
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().map_or(false, |e| e == "exe"))
    }

    /// Check if an editor exists for a version
    fn version_exists(&self, api: &ScriptApi, version: &str) -> bool {
        let path_str = format!("user://versions/{}/editor/", version);
        if let Some(resolved) = api.resolve_path(&path_str) {
            self.find_exe_in_dir(Path::new(&resolved)).is_some()
        } else {
            false
        }
    }

    /// Find the highest available local version
    fn find_highest_local_version(&self, api: &ScriptApi, current: &str) -> Option<String> {
        let versions_path_str = "user://versions/";
        let versions_dir = api.resolve_path(versions_path_str)?;
        let versions_path = Path::new(&versions_dir);
        
        let entries = std::fs::read_dir(versions_path).ok()?;
        
        let mut available_versions: Vec<String> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                if entry.file_type().ok()?.is_dir() {
                    let version = entry.file_name().to_string_lossy().to_string();
                    if self.version_exists(api, &version) {
                        Some(version)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        if available_versions.is_empty() {
            return None;
        }

        // Sort descending (highest first)
        available_versions.sort_by(|a, b| natord::compare(b, a));
        
        let highest = &available_versions[0];
        
        // Only return if higher than current
        if natord::compare(highest, current).is_gt() {
            Some(highest.clone())
        } else {
            None
        }
    }

    /// Launch a version and exit current process
    fn launch_version(&self, version_path: &Path) -> Result<(), String> {
        eprintln!("🚀 Launching {} and exiting", version_path.display());

        let parent_dir = version_path
            .parent()
            .ok_or("Could not determine parent directory")?;

        let args: Vec<String> = std::env::args().skip(1).collect();

        std::process::Command::new(version_path)
            .current_dir(parent_dir)
            .args(&args)
            .spawn()
            .map_err(|e| format!("Failed to launch: {}", e))?;

        std::process::exit(0);
    }

    /// Launch version from proper location
    fn launch_version_from_proper_location(
        &self,
        api: &ScriptApi,
        version: &str,
    ) -> Result<(), String> {
        let path_str = format!("user://versions/{}/editor/", version);
        if let Some(resolved) = api.resolve_path(&path_str) {
            if let Some(exe) = self.find_exe_in_dir(Path::new(&resolved)) {
                return self.launch_version(&exe);
            }
        }
        Err(format!("No exe found for version {}", version))
    }

    /// Ensure running from correct location, relocate if needed
    fn ensure_correct_location(&self, api: &ScriptApi, my_version: &str, exe_path: &Path) {
        let exe_name = exe_path.file_name().unwrap();
        let expected_str = format!("user://versions/{}/editor/", my_version);
        
        if let Some(expected_dir) = api.resolve_path(&expected_str) {
            let expected = PathBuf::from(&expected_dir);
            let expected_exe = expected.join(&exe_name);
            
            if exe_path != expected_exe {
                eprintln!("⚠️  Not running from correct location!");
                eprintln!("   Current: {}", exe_path.display());
                eprintln!("   Expected: {}", expected_exe.display());
                
                std::fs::create_dir_all(&expected).ok();
                
                if std::fs::copy(exe_path, &expected_exe).is_ok() {
                    eprintln!("✅ Copied to correct location, relaunching...");
                    if self.launch_version(&expected_exe).is_err() {
                        eprintln!("❌ Failed to launch from correct location");
                    }
                } else {
                    eprintln!("❌ Failed to copy to correct location");
                }
            }
        }
    }
}

impl Script for RootScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        let my_version = api.project().version().to_string();
        let exe_path = std::env::current_exe().expect("Could not get exe path");

        eprintln!("🎮 Perro Engine v{}", my_version);

        // Skip version management in debug builds
        if cfg!(debug_assertions) {
            eprintln!("🐛 Debug build: skipping version management");
            return;
        }

        // Step 1: Ensure we're in the correct location (fast filesystem check)
        self.ensure_correct_location(api, &my_version, &exe_path);
        eprintln!("✅ Running from correct location");

        // Step 2: Check if higher local version exists (fast filesystem check)
        if let Some(higher_version) = self.find_highest_local_version(api, &my_version) {
            eprintln!("🚀 Found higher local version: {} -> {}", my_version, higher_version);
            eprintln!("   Relaunching with newer version...");
            
            if let Err(e) = self.launch_version_from_proper_location(api, &higher_version) {
                eprintln!("❌ Failed to launch higher version: {}", e);
                eprintln!("   Continuing with current version");
            }
        } else {
            eprintln!("✅ Running highest local version: {}", my_version);
        }

        // Window will open immediately - updater script handles network checks
    }

    fn update(&mut self, _api: &mut ScriptApi<'_>) {}

    fn set_node_id(&mut self, id: Uuid) {
        self.node_id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node_id
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn apply_exports(&mut self, _: &HashMap<String, Value>) {}

    fn get_var(&self, _: &str) -> Option<Var> {
        None
    }

    fn set_var(&mut self, _: &str, _: Var) -> Option<()> {
        None
    }
}

// Natural ordering for version comparison
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
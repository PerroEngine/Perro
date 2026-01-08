#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
use serde_json::{Value, json};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use perro_core::prelude::*;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use std::path::{Path, PathBuf};
use std::{rc::Rc, cell::RefCell};
use phf::{phf_map, Map};
use smallvec::{SmallVec, smallvec};


/// @PerroScript
pub static MEMBER_TO_ATTRIBUTES_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {
};

static ATTRIBUTE_TO_MEMBERS_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {
};

struct RootScript {
    id: Uuid,
}

#[unsafe(no_mangle)]
pub extern "C" fn scripts_root_rs_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(RootScript {
        id: Uuid::nil(),
    })) as *mut dyn ScriptObject
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
        // [stripped for release] eprintln!("🚀 Launching {} and exiting", version_path.display());

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
                // [stripped for release] eprintln!("⚠️  Not running from correct location!");

                // [stripped for release] eprintln!("   Current: {}", exe_path.display());

                // [stripped for release] eprintln!("   Expected: {}", expected_exe.display());

                std::fs::create_dir_all(&expected).ok();
                
                if std::fs::copy(exe_path, &expected_exe).is_ok() {
                    // [stripped for release] eprintln!("✅ Copied to correct location, relaunching...");

                    if self.launch_version(&expected_exe).is_err() {
                        // [stripped for release] eprintln!("❌ Failed to launch from correct location");

                    }
                } else {
                    // [stripped for release] eprintln!("❌ Failed to copy to correct location");

                }
            }
        }
    }
}

impl Script for RootScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        // Check for --editor PATH runtime param
        // If present, switch to editor mode and load editor.scn
        let project_path_opt = api.project().get_runtime_param("editor").map(|s| s.to_string());
        if let Some(project_path) = project_path_opt {
            // [stripped for release] eprintln!("📂 Editor mode detected! Project path: {}", project_path);

            api.print(&format!("📂 Editor mode: Loading project at {}", project_path));
            
            // Set the main scene to editor.scn
            api.project().set_main_scene("res://editor.scn");
            
            // Verify the change was applied (extract value first to avoid borrow issues)
            let new_main_scene = api.project().main_scene().to_string();
            // [stripped for release] eprintln!("✅ Main scene set to: {}", new_main_scene);

            api.print(&format!("✅ Main scene set to: {}", new_main_scene));
            
            // Store the project path as a runtime param for editor scripts to use
            api.project().set_runtime_param("project_path", &project_path);
            
            // Mark that we need to compile scripts (will be done after window shows up)
            api.project().set_runtime_param("needs_initial_compile", "true");
            
            api.print("✅ Switched to editor mode");
        } else {
            // Manager mode: use default manager.scn (set in project.toml)
            let current_main_scene = api.project().main_scene().to_string();
            // [stripped for release] eprintln!("📁 Manager mode: Project selection (main_scene: {})", current_main_scene);

            api.print(&format!("📁 Manager mode: Project selection (main_scene: {})", current_main_scene));
        }

       let my_version = api.project().version().to_string();
        let current_exe_path = std::env::current_exe().expect("Could not get exe path");
        let current_exe_name = current_exe_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string(); // e.g., "perro_editor.exe"

        // [stripped for release] eprintln!("🎮 Perro Engine v{}", my_version);

        let file = api.JSON.stringify(&json!({
            "name": "EXPORT MODE BITCH",
            "age": 20,
            "inventory": ["sword", "shield"]
        }));
        api.save_asset("user://b.json", file).unwrap();

        // Skip version management in debug builds
        if cfg!(debug_assertions) {
            // [stripped for release] eprintln!("🐛 Debug build: skipping version management");

            return;
        }

        // Step 1: Ensure we're in the correct location (fast filesystem check)
        self.ensure_correct_location(api, &my_version, &current_exe_path);
        // [stripped for release] eprintln!("✅ Running from correct location");

        // Step 2: Check if higher local version exists (fast filesystem check)
        if let Some(higher_version) = self.find_highest_local_version(api, &my_version) {
            // [stripped for release] eprintln!("🚀 Found higher local version: {} -> {}", my_version, higher_version);

            // [stripped for release] eprintln!("   Relaunching with newer version...");

            if let Err(e) = self.launch_version_from_proper_location(api, &higher_version) {
                // [stripped for release] eprintln!("❌ Failed to launch higher version: {}", e);

                // [stripped for release] eprintln!("   Continuing with current version");

            }
        } else {
            // [stripped for release] eprintln!("✅ Running highest local version: {}", my_version);

        }

        // Window will open immediately - updater script handles network checks
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


impl ScriptObject for RootScript {
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

static VAR_GET_TABLE: phf::Map<u64, fn(&RootScript) -> Option<Value>> =
    phf::phf_map! {

    };

static VAR_SET_TABLE: phf::Map<u64, fn(&mut RootScript, Value) -> Option<()>> =
    phf::phf_map! {

    };

static VAR_APPLY_TABLE: phf::Map<u64, fn(&mut RootScript, &Value)> =
    phf::phf_map! {

    };

static DISPATCH_TABLE: phf::Map<
    u64,
    fn(&mut RootScript, &[Value], &mut ScriptApi<'_>),
> = phf::phf_map! {
        8474883865169775633u64 => | script: &mut RootScript, params: &[Value], api: &mut ScriptApi<'_>| {
let __path_buf_dir = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| std::path::PathBuf::from(s))
                            .unwrap_or_default();
let dir = __path_buf_dir.as_path();
            script.find_exe_in_dir(dir);
        },
        1072299143536744344u64 => | script: &mut RootScript, params: &[Value], api: &mut ScriptApi<'_>| {
let version = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.version_exists(api, &version);
        },
        350600208379132370u64 => | script: &mut RootScript, params: &[Value], api: &mut ScriptApi<'_>| {
let current = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.find_highest_local_version(api, &current);
        },
        10459365434656882773u64 => | script: &mut RootScript, params: &[Value], api: &mut ScriptApi<'_>| {
let __path_buf_version_path = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| std::path::PathBuf::from(s))
                            .unwrap_or_default();
let version_path = __path_buf_version_path.as_path();
            script.launch_version(version_path);
        },
        10865362270687727973u64 => | script: &mut RootScript, params: &[Value], api: &mut ScriptApi<'_>| {
let version = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.launch_version_from_proper_location(api, &version);
        },
        14963852875113987676u64 => | script: &mut RootScript, params: &[Value], api: &mut ScriptApi<'_>| {
let my_version = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
let __path_buf_exe_path = params.get(1)
                            .and_then(|v| v.as_str())
                            .map(|s| std::path::PathBuf::from(s))
                            .unwrap_or_default();
let exe_path = __path_buf_exe_path.as_path();
            script.ensure_correct_location(api, &my_version, exe_path);
        },

    };

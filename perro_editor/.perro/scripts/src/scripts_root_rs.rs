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
    "b" => &["expose", "Expose"],
    "a" => &["bitch"],
};

static ATTRIBUTE_TO_MEMBERS_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {
    "expose" => &["b"],
    "Expose" => &["b"],
    "bitch" => &["a"],
};

struct RootScript {
    node: Node,
    /// @expose
    pub b: f32,
    /// @bitch
    pub a: i32,
    e: String,
    pub f: F,
    pub h: i64,
}

#[unsafe(no_mangle)]
pub extern "C" fn scripts_root_rs_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(RootScript {
        node: Node::new("Root", None),
        b: 0.0f32,
        a: 0i32,
        e: String::new(),
        f: F { g: 0 },
        h: 0,
    })) as *mut dyn ScriptObject
}

#[derive(Clone, Deserialize, Serialize)]
pub struct F {
    pub g: i32,
}

impl RootScript {

 fn ensure_runtime_exe_in_version_dir(
    &self,
    api: &mut ScriptApi, // Ensure this is &mut ScriptApi
    target_version: &str,
    target_exe_name: &str, // e.g., "perro_runtime.exe"
) -> Result<PathBuf, String> {
    let version_editor_path_str = format!("user://versions/{}/editor/", target_version);
    let resolved_version_dir_path = api
        .resolve_path(&version_editor_path_str)
        .ok_or_else(|| format!("Failed to resolve path for {}", version_editor_path_str))?;
    let version_editor_dir = PathBuf::from(&resolved_version_dir_path);

    // --- THIS IS THE CORRECTED LINE ---
    let expected_runtime_path = version_editor_dir.join(target_exe_name);
    // --- END CORRECTED LINE ---

    // Step 1: Check if the file already exists on disk
    if expected_runtime_path.exists() {
        eprintln!(
            "✅ Runtime executable found on disk: {}",
            expected_runtime_path.display()
        );
        return Ok(expected_runtime_path);
    }

    eprintln!(
        "⚠️  Runtime executable NOT found at {}. Attempting to extract from embedded resources...",
        expected_runtime_path.display()
    );

    // Step 2: If not found, load it from the editor's embedded resources
    let embedded_runtime_asset_path = format!("res://runtime/{target_exe_name}");
    let runtime_bytes = api.load_asset(&embedded_runtime_asset_path)
        .ok_or_else(|| {
            format!(
                "❌ Failed to load embedded runtime asset: {}",
                embedded_runtime_asset_path
            )
        })?;

    // --- CRITICAL DEBUGGING OUTPUT HERE ---
    let path_to_save_str = expected_runtime_path.to_string_lossy();


    // Step 4: Save the bytes to the expected location on disk
    api.save_asset(&path_to_save_str, runtime_bytes) // Use the debug string here
        .map_err(|e| { // Map the io::Error to a String error for the Result
            format!(
                "❌ Failed to save runtime executable to {}: {}", // Include the io::Error details here
                expected_runtime_path.display(),
                e // This `e` is the io::Error
            )
        })?;

    // ... rest of the function
    Ok(expected_runtime_path)
}

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
api.print_info(format!("attributes of b: {:?}", self.attributes_of("a")));
       let my_version = api.project().version().to_string();
        let current_exe_path = std::env::current_exe().expect("Could not get exe path");
        let current_exe_name = current_exe_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string(); // e.g., "perro_editor.exe"

        eprintln!("🎮 Perro Engine v{}", my_version);

        let file = api.JSON.stringify(&json!({
            "name": "EXPORT MODE BITCH",
            "age": 20,
            "inventory": ["sword", "shield"]
        }));
        api.save_asset("user://b.json", file).unwrap();

        // Skip version management in debug builds
        if cfg!(debug_assertions) {
            eprintln!("🐛 Debug build: skipping version management");
            return;
        }

        // --- NEW LOGIC FOR RUNTIME.EXE EXTRACTION ---
        // Ensure the perro_runtime.exe is available on disk at the *current* engine's version path
        // before proceeding with any version checks.
        let target_runtime_exe_name = "PerroDevRuntime.exe"; // The actual name of the runtime binary
        match self.ensure_runtime_exe_in_version_dir(
            api,
            &my_version,
            target_runtime_exe_name,
        ) {
            Ok(runtime_on_disk_path) => {
                eprintln!(
                    "✅ Perro Dev Runtime confirmed at: {}",
                    runtime_on_disk_path.display()
                );
            }
            Err(e) => {
                // Now this `eprintln!` will be hit and show the full error from save_asset!
                eprintln!("❌ CRITICAL ERROR: Failed to ensure perro_runtime.exe: {}", e);
            }
        }
        // --- END NEW LOGIC ---

        // Step 1: Ensure we're in the correct location (fast filesystem check)
        self.ensure_correct_location(api, &my_version, &current_exe_path);
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

    fn update(&mut self, api: &mut ScriptApi<'_>) {

        api.scene.get_scene_node(Uuid::from_str("4f6c6c9c-4e44-4e34-8a9c-0c0f0464fd48").unwrap()).unwrap().internal_fixed_update(api);
        // In your script struct
let mut was_mouse_down = false;

// In update()
let is_mouse_down = api.Input.Mouse.is_button_pressed("MouseLeft");
if is_mouse_down && !was_mouse_down {
    // Mouse was just clicked (transition from up to down)
    println!("Mouse clicked!");
}
was_mouse_down = is_mouse_down;
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
    fn set_node_id(&mut self, id: Uuid) {
        self.node.id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node.id
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
        params: &SmallVec<[Value; 3]>,
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
}

// =========================== Static PHF Dispatch Tables ===========================

static VAR_GET_TABLE: phf::Map<u64, fn(&RootScript) -> Option<Value>> =
    phf::phf_map! {
        12638190499090526629u64 => |script: &RootScript| -> Option<Value> {
                        Some(json!(script.b))
                    },
        12638187200555641996u64 => |script: &RootScript| -> Option<Value> {
                        Some(json!(script.a))
                    },
        12638186101044013785u64 => |script: &RootScript| -> Option<Value> {
                        Some(json!(script.f))
                    },
        12638197096160295895u64 => |script: &RootScript| -> Option<Value> {
                        Some(json!(script.h))
                    },

    };

static VAR_SET_TABLE: phf::Map<u64, fn(&mut RootScript, Value) -> Option<()>> =
    phf::phf_map! {
        12638190499090526629u64 => |script: &mut RootScript, val: Value| -> Option<()> {
                            if let Some(v) = val.as_f64() {
                                script.b = v as f32;
                                return Some(());
                            }
                            None
                        },
        12638187200555641996u64 => |script: &mut RootScript, val: Value| -> Option<()> {
                            if let Some(v) = val.as_i64() {
                                script.a = v as i32;
                                return Some(());
                            }
                            None
                        },
        12638186101044013785u64 => |script: &mut RootScript, val: Value| -> Option<()> {
                            if let Ok(v) = serde_json::from_value::<F>(val) {
                                script.f = v;
                                return Some(());
                            }
                            None
                        },
        12638197096160295895u64 => |script: &mut RootScript, val: Value| -> Option<()> {
                            if let Some(v) = val.as_i64() {
                                script.h = v as i64;
                                return Some(());
                            }
                            None
                        },

    };

static VAR_APPLY_TABLE: phf::Map<u64, fn(&mut RootScript, &Value)> =
    phf::phf_map! {
        12638190499090526629u64 => |script: &mut RootScript, val: &Value| {
                            if let Some(v) = val.as_f64() {
                                script.b = v as f32;
                            }
                        },

    };

static DISPATCH_TABLE: phf::Map<
    u64,
    fn(&mut RootScript, &[Value], &mut ScriptApi<'_>),
> = phf::phf_map! {

    };

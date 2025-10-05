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
    })) as *mut dyn Script
}

pub struct BRootScript {
    node_id: Uuid,
}

impl Script for BRootScript {
fn init(&mut self, api: &mut ScriptApi<'_>) {
    if cfg!(debug_assertions) {
        api.set_window_title("Rootcript [DEBUG]".to_string());
    } else {
        api.set_window_title("fart".to_string());
        
        // Check if running from user://versions/VERSION/ in release mode
        let current_version = api.project().version().to_string();
        let expected_path_str = format!("user://versions/{}/", current_version);
        
        if let Some(expected_base) = api.resolve_path(&expected_path_str) {
            let current_dir = std::env::current_dir().unwrap();
            let expected_pathbuf = std::path::PathBuf::from(&expected_base);
            
            if !current_dir.starts_with(&expected_pathbuf) {
                eprintln!("WARNING: Not running from user://versions/{}/", current_version);
                eprintln!("Current: {}", current_dir.display());
                
                // Get current executable info
                let exe_path = match std::env::current_exe() {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("Could not determine current executable path: {}", e);
                        std::process::exit(1);
                    }
                };
                let exe_name = exe_path.file_name().unwrap();
                let target_exe = expected_pathbuf.join(exe_name);
                
                // ONLY copy to OUR version folder if it doesn't exist
                if !target_exe.exists() {
                    eprintln!("Copying executable to: {}", expected_pathbuf.display());
                    
                    if let Err(e) = std::fs::create_dir_all(&expected_pathbuf) {
                        eprintln!("Failed to create directory: {}", e);
                        std::process::exit(1);
                    }
                    
                    if let Err(e) = std::fs::copy(&exe_path, &target_exe) {
                        eprintln!("Failed to copy executable: {}", e);
                        std::process::exit(1);
                    }
                    
                    eprintln!("Successfully copied to: {}", target_exe.display());
                }
                
                // Now check for higher versions BEFORE launching
                let versions_base = api.resolve_path("user://versions/").unwrap();
                let versions_dir = std::path::PathBuf::from(versions_base);
                let highest_version = self.find_highest_version(&versions_dir, exe_name, &current_version);
                
                let launch_exe = if let Some((version, higher_exe_path)) = highest_version {
                    eprintln!("Found higher version: {}", version);
                    higher_exe_path
                } else {
                    eprintln!("No higher version found. Using current version.");
                    target_exe
                };
                
                eprintln!("Launching: {}", launch_exe.display());
                let parent_dir = launch_exe.parent().unwrap();
                let mut cmd = std::process::Command::new(&launch_exe);
                cmd.current_dir(parent_dir);
                
                if let Err(e) = cmd.spawn() {
                    eprintln!("Failed to launch: {}", e);
                    std::process::exit(1);
                }
                
                eprintln!("Closing current instance.");
                std::process::exit(0);
            }
            
            // We're in the right location, now check for higher versions
            let exe_path = std::env::current_exe().unwrap();
            let exe_name = exe_path.file_name().unwrap();
            let versions_base = api.resolve_path("user://versions/").unwrap();
            let versions_dir = std::path::PathBuf::from(versions_base);
            
            let highest_version = self.find_highest_version(&versions_dir, exe_name, &current_version);
            
            if let Some((version, exe_path)) = highest_version {
                eprintln!("Found higher version: {}", version);
                eprintln!("Launching: {}", exe_path.display());
                api.set_window_title("BITCH".to_string());
                
                let parent_dir = exe_path.parent().unwrap();
                let mut cmd = std::process::Command::new(&exe_path);
                cmd.current_dir(parent_dir);
                
                if let Err(e) = cmd.spawn() {
                    eprintln!("Failed to launch: {}", e);
                    std::process::exit(1);
                }
                
                eprintln!("Closing current instance.");
                std::process::exit(0);
            } else {
                // No higher version found, this is the latest
                api.set_window_title("Perro Engine tho".to_string());
                eprintln!("Running latest version: {}", current_version);
            }
        } else {
            eprintln!("ERROR: Could not resolve user://versions/{}/", current_version);
            std::process::exit(1);
        }
    }

    println!("Running from: {}", std::env::current_dir().unwrap().display());
}

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        let delta = api.get_delta();
        let mut a = api.get_node_clone::<Node2D>(&Uuid::new_v4());
        let x = delta * 10.0;
        let mut b = api.get_node_clone::<Node2D>(&Uuid::new_v4());

        a.transform.position.x += 1.0;
        b.transform.position.x += 1.0;

        api.merge_nodes(vec![
            a.to_scene_node(),
            b.to_scene_node(),
        ]);
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


impl BRootScript {
fn find_highest_version(&self, versions_dir: &std::path::Path, exe_name: &std::ffi::OsStr, current_version: &str) -> Option<(String, std::path::PathBuf)> {
        let entries = match std::fs::read_dir(versions_dir) {
            Ok(entries) => entries,
            Err(_) => return None,
        };
        
        let mut versions = Vec::new();
        
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let dir_name = entry.file_name();
                    let version_str = dir_name.to_string_lossy().to_string();
                    
                    let exe_path = entry.path().join(exe_name);
                    if exe_path.exists() {
                        versions.push((version_str, exe_path));
                    }
                }
            }
        }
        
        // Sort alphabetically/lexicographically (highest last)
        versions.sort_by(|a, b| a.0.cmp(&b.0));
        
        // Get the highest version that's greater than current
        versions.into_iter()
            .filter(|(v, _)| v.as_str() > current_version)
            .last()
    }
}
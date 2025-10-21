#[cfg(debug_assertions)]
use std::ffi::CStr;
#[cfg(debug_assertions)]
use std::os::raw::c_char;
use perro_core::script::CreateFn;
use std::collections::HashMap;

pub mod scripts_root_rs;
pub mod scripts_editor_pup;
pub mod scripts_csharp_cs;
pub mod scripts_updater_rs;
pub mod scripts_player_poop_pup;
pub mod scripts_repair_rs;

use scripts_root_rs::scripts_root_rs_create_script;
use scripts_editor_pup::scripts_editor_pup_create_script;
use scripts_csharp_cs::scripts_csharp_cs_create_script;
use scripts_updater_rs::scripts_updater_rs_create_script;
use scripts_player_poop_pup::scripts_player_poop_pup_create_script;
use scripts_repair_rs::scripts_repair_rs_create_script;

pub fn get_script_registry() -> HashMap<String, CreateFn> {
    let mut map: HashMap<String, CreateFn> = HashMap::new();
    map.insert("scripts_root_rs".to_string(), scripts_root_rs_create_script as CreateFn);
    map.insert("scripts_editor_pup".to_string(), scripts_editor_pup_create_script as CreateFn);
    map.insert("scripts_csharp_cs".to_string(), scripts_csharp_cs_create_script as CreateFn);
    map.insert("scripts_updater_rs".to_string(), scripts_updater_rs_create_script as CreateFn);
    map.insert("scripts_player_poop_pup".to_string(), scripts_player_poop_pup_create_script as CreateFn);
    map.insert("scripts_repair_rs".to_string(), scripts_repair_rs_create_script as CreateFn);
    map
}

#[cfg(debug_assertions)]
#[unsafe(no_mangle)]
pub extern "C" fn perro_set_project_root(path: *const c_char, name: *const c_char) {
    let path_str = unsafe { CStr::from_ptr(path).to_str().unwrap() };
    let name_str = unsafe { CStr::from_ptr(name).to_str().unwrap() };
    perro_core::asset_io::set_project_root(
        perro_core::asset_io::ProjectRoot::Disk {
            root: std::path::PathBuf::from(path_str),
            name: name_str.to_string(),
        }
    );
}

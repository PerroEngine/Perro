#[cfg(debug_assertions)]
use std::ffi::CStr;
#[cfg(debug_assertions)]
use std::os::raw::c_char;
use perro_core::script::CreateFn;
use std::collections::HashMap;

pub mod editor_pup;
pub mod updater_rs;
pub mod csharp_cs;
pub mod root_rs;
pub mod repair_rs;

use editor_pup::editor_pup_create_script;
use updater_rs::updater_rs_create_script;
use csharp_cs::csharp_cs_create_script;
use root_rs::root_rs_create_script;
use repair_rs::repair_rs_create_script;

pub fn get_script_registry() -> HashMap<String, CreateFn> {
    let mut map: HashMap<String, CreateFn> = HashMap::new();
    map.insert("editor_pup".to_string(), editor_pup_create_script as CreateFn);
    map.insert("updater_rs".to_string(), updater_rs_create_script as CreateFn);
    map.insert("csharp_cs".to_string(), csharp_cs_create_script as CreateFn);
    map.insert("root_rs".to_string(), root_rs_create_script as CreateFn);
    map.insert("repair_rs".to_string(), repair_rs_create_script as CreateFn);
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

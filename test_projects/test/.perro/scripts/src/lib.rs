#[cfg(debug_assertions)]
use std::ffi::CStr;
#[cfg(debug_assertions)]
use std::os::raw::c_char;
use perro_core::script::CreateFn;
use std::collections::HashMap;

pub mod types_pup;
// __PERRO_MODULES__
use types_pup::types_pup_create_script;
// __PERRO_IMPORTS__

pub fn get_script_registry() -> HashMap<String, CreateFn> {
let mut map: HashMap<String, CreateFn> = HashMap::new();
    map.insert("types_pup".to_string(), types_pup_create_script as CreateFn);
    // __PERRO_REGISTRY__
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

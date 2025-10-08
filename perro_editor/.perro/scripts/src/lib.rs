use perro_core::script::{CreateFn, Script};
use std::collections::HashMap;

pub mod root_rs;
pub mod editor_pup;

use root_rs::root_rs_create_script;
use editor_pup::editor_pup_create_script;

pub fn get_script_registry() -> HashMap<String, CreateFn> {
    let mut map: HashMap<String, CreateFn> = HashMap::new();
    map.insert("root_rs".to_string(), root_rs_create_script as CreateFn);
    map.insert("editor_pup".to_string(), editor_pup_create_script as CreateFn);
    map
}

use perro_core::script::{CreateFn, Script};
use std::collections::HashMap;

pub mod editor;
// __PERRO_MODULES__
use editor::editor_create_script;
// __PERRO_IMPORTS__

pub fn get_script_registry() -> HashMap<String, CreateFn> {
let mut map: HashMap<String, CreateFn> = HashMap::new();
    map.insert("editor".to_string(), editor_create_script as CreateFn);
        map.insert("editor".to_string(), editor_create_script as CreateFn);
    // __PERRO_REGISTRY__
map
}

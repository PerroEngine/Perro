use perro_core::script::{CreateFn, Script};
use std::collections::HashMap;

pub mod poop_pup;
pub mod editor_pup;
pub mod bob_pup;
pub mod bob_rs;
// __PERRO_MODULES__
use poop_pup::poop_pup_create_script;
use editor_pup::editor_pup_create_script;
use bob_pup::bob_pup_create_script;
use bob_rs::bob_rs_create_script;
// __PERRO_IMPORTS__

pub fn get_script_registry() -> HashMap<String, CreateFn> {
let mut map: HashMap<String, CreateFn> = HashMap::new();
    map.insert("poop_pup".to_string(), poop_pup_create_script as CreateFn);
        map.insert("editor_pup".to_string(), editor_pup_create_script as CreateFn);
        map.insert("bob_pup".to_string(), bob_pup_create_script as CreateFn);
        map.insert("bob_rs".to_string(), bob_rs_create_script as CreateFn);
    // __PERRO_REGISTRY__
map
}

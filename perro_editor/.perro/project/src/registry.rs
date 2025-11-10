use perro_core::script::ScriptProvider;
use perro_core::script::CreateFn;
use std::collections::HashMap;
use perro_core::scene::SceneData;
use perro_core::ui::ast::FurElement;
use std::io;
use scripts::get_script_registry;

use crate::scenes::PERRO_SCENES;
use crate::fur::PERRO_FUR;

pub struct StaticScriptProvider {
    ctors: HashMap<String, CreateFn>,
}

impl StaticScriptProvider {
    pub fn new() -> Self {
        Self { ctors: get_script_registry() }
    }

}

impl ScriptProvider for StaticScriptProvider {
    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
        self.ctors
            .get(short)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("No static ctor for {short}"))
    }

  fn load_scene_data(&self, path: &str) -> io::Result<SceneData> {
    if let Some(scene) = PERRO_SCENES.get(path) {
        Ok((**scene).clone())
    } else {
        Err(io::Error::new(io::ErrorKind::NotFound, format!("Scene not found: {}", path)))
    }
}

fn load_fur_data(&self, path: &str) -> io::Result<Vec<FurElement>> {
    if let Some(fur) = PERRO_FUR.get(path) {
        Ok((*fur).to_vec()) // clone static data to owned Vec<FurElement>
    } else {
        Err(io::Error::new(io::ErrorKind::NotFound, format!("FUR not found: {}", path)))
    }
}

}
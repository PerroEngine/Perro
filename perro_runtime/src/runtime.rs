use perro_api::API;

use crate::{script_collection::ScriptCollection, NodeArena};

pub struct Runtime {
    pub nodes: NodeArena,
    pub scripts: ScriptCollection<Self>,
    pub delta_time: f32,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            nodes: NodeArena::new(),
            scripts: ScriptCollection::new(),
            delta_time: 0.0,
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        self.delta_time = delta_time;
        let script_ids = self.scripts.get_update_ids();
        for id in script_ids {
           perro_api::modules::ScriptAPI::call_update(self, id);
        }
    }
}

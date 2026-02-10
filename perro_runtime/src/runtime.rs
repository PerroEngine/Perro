use perro_api::API;

use crate::{NodeArena, script_collection::ScriptCollection};

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
        let mut api = API::new(self);
        for id in script_ids {
            api.Scripts().call_script_update(id);
        }
    }
}

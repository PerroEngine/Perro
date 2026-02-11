use crate::{NodeArena, script_collection::ScriptCollection};

pub struct Runtime {
    pub nodes: NodeArena,
    pub scripts: ScriptCollection<Self>,
    pub time: Timing,
}

pub struct Timing {
    pub delta: f32,
    pub elapsed: f32,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            nodes: NodeArena::new(),
            scripts: ScriptCollection::new(),
            time: Timing {
                delta: (0.0),
                elapsed: (0.0),
            },
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        self.time.delta = delta_time;
        let script_ids = self.scripts.get_update_ids();
        for id in script_ids {
            perro_api::sub_apis::ScriptAPI::call_update(self, id);
        }
    }
}

use perro_api::API;
use std::cell::{Cell, RefCell};

use crate::{NodeArena, script_collection::ScriptCollection};

pub struct Runtime {
    pub nodes: RefCell<NodeArena>,
    pub scripts: RefCell<ScriptCollection<Self>>,
    pub delta_time: Cell<f32>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            nodes: RefCell::new(NodeArena::new()),
            scripts: RefCell::new(ScriptCollection::new()),
            delta_time: Cell::new(0.0),
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        self.delta_time.set(delta_time);
        let script_ids = self.scripts.borrow().get_update_ids();
        let api = API::new(self);
        for id in script_ids {
            api.Scripts().call_update(id);
        }
    }
}

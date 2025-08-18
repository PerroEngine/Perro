use std::{cell::RefCell, rc::Rc};

// perro_core/src/pup/api.rs
use uuid::Uuid;
use crate::{scene::Scene, script::{Script, UpdateOp, Var}};
/// The only thing your scripts ever see:
pub struct ScriptApi<'a> {
    delta:   f32,
    scene:  &'a mut Scene,
}

impl<'a> ScriptApi<'a> {
    pub fn new(delta: f32, scene: &'a mut Scene) -> Self {
        ScriptApi { delta, scene }
    }

    pub fn call_update(&mut self, id: Uuid) {
        // 1) clone out an Rc so we no longer borrow the map:
        let maybe_rc: Option<Rc<RefCell<Box<dyn Script>>>> =
            self.scene.scripts.get(&id).cloned();

        // (the `&self.scene.scripts` borrow ends here)

        // 2) now we have an owned Rc, no longer tied to the map borrow:
        if let Some(rc_script) = maybe_rc {
            // 3) we can borrow_mut the RefCell and then call update(self)
            let mut script = rc_script.borrow_mut();
            script.update(self);
        }
    }

    pub fn get_delta(&self) -> f32 {
        self.delta
    }

    pub fn get_node_mut<T: 'static>(&mut self, id: &Uuid) -> Option<&mut T> {
        self.scene.get_node_mut::<T>(id)
    }

    pub fn update_script_var(
        &mut self,
        node_id: &Uuid,
        name: &str,
        op: UpdateOp,
        val: Var,
    ) -> Option<()>  {
        self.scene.update_script_var(node_id, name, op, val)
    }


}
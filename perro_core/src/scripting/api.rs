use uuid::Uuid;
use crate::script::{Script, SceneAccess, UpdateOp, Var};

/// The only thing your scripts ever see:
pub struct ScriptApi<'a> {
    delta: f32,
    scene: &'a mut dyn SceneAccess,
}

impl<'a> ScriptApi<'a> {
    pub fn new(delta: f32, scene: &'a mut dyn SceneAccess) -> Self {
        ScriptApi { delta, scene }
    }

    pub fn call_update(&mut self, id: Uuid) {
        if let Some(rc_script) = self.scene.get_script(id) {
            let mut script = rc_script.borrow_mut();
            script.update(self);
        }
    }

    pub fn get_delta(&self) -> f32 {
        self.delta
    }

    pub fn get_node_mut<T: 'static>(&mut self, id: &Uuid) -> Option<&mut T> {
        self.scene.get_node_mut_any(id)?.downcast_mut::<T>()
    }

    pub fn update_script_var(
        &mut self,
        node_id: &Uuid,
        name: &str,
        op: UpdateOp,
        val: Var,
    ) -> Option<()> {
        self.scene.update_script_var(node_id, name, op, val)
    }
}
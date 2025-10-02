use uuid::Uuid;
use std::sync::mpsc::Sender;
use crate::{
    manifest::Project, 
    script::{SceneAccess, Script, UpdateOp, Var},
    app_command::AppCommand, // NEW import
};

pub struct ScriptApi<'a> {
    delta: f32,
    scene: &'a mut dyn SceneAccess,
    project: &'a mut Project,
}

impl<'a> ScriptApi<'a> {
    pub fn new(delta: f32, scene: &'a mut dyn SceneAccess, project: &'a mut Project) -> Self {
        ScriptApi { delta, scene, project }
    }

    pub fn project(&mut self) -> &mut Project {
        self.project
    }

    pub fn set_window_title(&mut self, title: String) {
        // Always update project config
        self.project.set_name(title.clone());

        // Also send command if there’s a runtime app listening
        if let Some(tx) = self.scene.get_command_sender() {
            let _ = tx.send(AppCommand::SetWindowTitle(title));
        }
    }

    pub fn set_target_fps(&mut self, fps: f32) {
        // Always update project config
        self.project.set_target_fps(fps);

        // Also send command
        if let Some(tx) = self.scene.get_command_sender() {
            let _ = tx.send(AppCommand::SetTargetFPS(fps));
        }
    }

    pub fn quit(&self) {
        if let Some(tx) = self.scene.get_command_sender() {
            let _ = tx.send(AppCommand::Quit);
        } else {
            std::process::exit(0);
        }
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
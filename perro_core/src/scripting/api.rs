use uuid::Uuid;
use std::{any::TypeId, path::Path, sync::mpsc::Sender};
use crate::{
    app_command::AppCommand, asset_io::{load_asset, resolve_path, ResolvedPath}, compiler::{BuildProfile, CompileTarget, Compiler}, lang::transpiler::transpile, manifest::Project, scene_node::{BaseNode, IntoInner, SceneNode}, script::{SceneAccess, Script, UpdateOp, Var}, ui_node::Ui, Node, Node2D, Sprite2D // NEW import
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

    pub fn compile_scripts(&mut self) -> Result<(), String> {
        self.run_compile(BuildProfile::Dev, CompileTarget::Scripts)
    }

    /// Compile the full project (Release build)
    pub fn compile_project(&mut self) -> Result<(), String> {
        self.run_compile(BuildProfile::Release, CompileTarget::Project)
    }

    /// Internal shared logic
    fn run_compile(
        &mut self,
        profile: BuildProfile,
        target: CompileTarget,
    ) -> Result<(), String> {

        // 🔹 Get runtime project path
        let project_path_str = self
            .project
            .get_runtime_param("project_path")
            .ok_or("Missing runtime param: project_path")?;

        eprintln!("📁 Project path: {}", project_path_str);

        let project_path = Path::new(project_path_str);

        // 🧩 Transpile step
        transpile(project_path)
            .map_err(|e| format!("Transpile failed: {}", e))?;

        // 🧱 Compile step
        let compiler = Compiler::new(project_path, target, false);
        compiler
            .compile(profile)
            .map_err(|e| format!("Compile failed: {}", e))?;

        Ok(())
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
        self.delta.clone()
    }

    pub fn load_asset(&mut self, path: &str) -> Option<Vec<u8>> {
        // Use the load_asset function from asset_io
        crate::asset_io::load_asset(path).ok()
    }

    pub fn resolve_path(&self, path: &str) -> Option<String> {
        match crate::asset_io::resolve_path(path) {
            ResolvedPath::Disk(pathbuf) => {
                // Convert PathBuf to String
                pathbuf.to_str().map(|s| s.to_string())
            }
            ResolvedPath::Brk(virtual_path) => {

                virtual_path.into()
            }
        }
    }

    pub fn get_node_clone<T: Clone>(&mut self, id: &Uuid) -> T
    where
        SceneNode: IntoInner<T>,
    {
        let node_enum = self.scene.get_scene_node(id)
            .unwrap_or_else(|| panic!("Node {} not found in scene", id))
            .clone();

        node_enum.into_inner()
    }




    pub fn merge_nodes(&mut self, nodes: Vec<SceneNode>) {
        self.scene.merge_nodes(nodes);
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
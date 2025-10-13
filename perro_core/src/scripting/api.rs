use uuid::Uuid;
use std::{path::Path, sync::mpsc::Sender};
use serde::{Serialize, Deserialize};
use serde_json::Value; // JSON support

use crate::{
    app_command::AppCommand,
    asset_io::{load_asset, resolve_path, ResolvedPath},
    compiler::{BuildProfile, CompileTarget, Compiler},
    lang::transpiler::transpile,
    manifest::Project,
    scene_node::{BaseNode, IntoInner, SceneNode},
    script::{SceneAccess, Script, UpdateOp, Var},
    ui_node::Ui,
    Node, Node2D, Sprite2D
};

/// ✅ JSON helper with parse/stringify
pub struct JsonApi;

impl JsonApi {
    /// Serialize any struct that implements `Serialize` into a JSON string
    pub fn stringify<T: Serialize>(&self, val: &T) -> Option<String> {
        serde_json::to_string(val).ok()
    }

    /// Parse a JSON string into `serde_json::Value`
    pub fn parse(&self, text: &str) -> Option<Value> {
        serde_json::from_str(text).ok()
    }
}

#[allow(non_snake_case)]
pub struct ScriptApi<'a> {
    delta: f32,
    scene: &'a mut dyn SceneAccess,
    project: &'a mut Project,
    pub JSON: JsonApi, // JS-style JSON field
}

impl<'a> ScriptApi<'a> {
    pub fn new(delta: f32, scene: &'a mut dyn SceneAccess, project: &'a mut Project) -> Self {
        ScriptApi {
            delta,
            scene,
            project,
            JSON: JsonApi,
        }
    }

    // -----------------------------
    // Engine state access
    // -----------------------------
    pub fn project(&mut self) -> &mut Project {
        self.project
    }

    pub fn delta(&self) -> f32 {
        self.delta
    }

    // -----------------------------
    // Compilation
    // -----------------------------
    pub fn compile_scripts(&mut self) -> Result<(), String> {
        self.run_compile(BuildProfile::Dev, CompileTarget::Scripts)
    }

    pub fn compile_project(&mut self) -> Result<(), String> {
        self.run_compile(BuildProfile::Release, CompileTarget::Project)
    }

    fn run_compile(
        &mut self,
        profile: BuildProfile,
        target: CompileTarget,
    ) -> Result<(), String> {
        let project_path_str = self
            .project
            .get_runtime_param("project_path")
            .ok_or("Missing runtime param: project_path")?;

        eprintln!("📁 Project path: {}", project_path_str);
        let project_path = Path::new(project_path_str);

        transpile(project_path)
            .map_err(|e| format!("Transpile failed: {}", e))?;

        let compiler = Compiler::new(project_path, target, false);
        compiler
            .compile(profile)
            .map_err(|e| format!("Compile failed: {}", e))?;

        Ok(())
    }

    // -----------------------------
    // Window / App commands
    // -----------------------------
    pub fn set_window_title(&mut self, title: String) {
        self.project.set_name(title.clone());
        if let Some(tx) = self.scene.get_command_sender() {
            let _ = tx.send(AppCommand::SetWindowTitle(title));
        }
    }

    pub fn set_target_fps(&mut self, fps: f32) {
        self.project.set_target_fps(fps);
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

    // -----------------------------
    // Script execution
    // -----------------------------
    pub fn call_update(&mut self, id: Uuid) {
        if let Some(rc_script) = self.scene.get_script(id) {
            let mut script = rc_script.borrow_mut();
            script.update(self);
        }
    }

    // -----------------------------
    // Asset I/O
    // -----------------------------
    pub fn load_asset(&mut self, path: &str) -> Option<Vec<u8>> {
        crate::asset_io::load_asset(path).ok()
    }

    pub fn save_asset<D>(&mut self, path: &str, data: D) -> Option<()>
    where
        D: AsRef<[u8]>,
    {
        crate::asset_io::save_asset(path, data.as_ref()).ok()
    }

    pub fn resolve_path(&self, path: &str) -> Option<String> {
        match crate::asset_io::resolve_path(path) {
            ResolvedPath::Disk(pathbuf) => pathbuf.to_str().map(|s| s.to_string()),
            ResolvedPath::Brk(virtual_path) => virtual_path.into(),
        }
    }

    // -----------------------------
    // Scene / Node access
    // -----------------------------
    pub fn get_node_clone<T: Clone>(&mut self, id: &Uuid) -> T
    where
        SceneNode: IntoInner<T>,
    {
        let node_enum = self.scene
            .get_scene_node(id)
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

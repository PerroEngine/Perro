// scripting/lang/script_api.rs
//! Perro Script API (single-file version with Deref)
//! Provides all engine APIs (JSON, Time, OS, Process) directly under `api`

use uuid::Uuid;
use std::{
    cell::RefCell, env, ops::Deref, path::Path, process, rc::Rc, sync::mpsc::Sender, thread, time::{Duration, SystemTime, UNIX_EPOCH}
};
use serde::{Serialize, Deserialize};
use serde_json::Value;
use chrono::{Local, Datelike, Timelike}; // For date/time formatting

use crate::{
    app_command::AppCommand, asset_io::{self, load_asset, resolve_path, ResolvedPath}, compiler::{BuildProfile, CompileTarget, Compiler}, lang::transpiler::{script_path_to_identifier, transpile}, manifest::Project, scene_node::{IntoInner, SceneNode}, script::{CreateFn, SceneAccess, Script, UpdateOp, Var}, types::ScriptType, ui_node::Ui, Node, Node2D, Sprite2D
};

//-----------------------------------------------------
// 1️⃣ Sub‑APIs (Engine modules)
//-----------------------------------------------------

#[derive(Default)]
pub struct JsonApi;
impl JsonApi {
    pub fn stringify<T: Serialize>(&self, val: &T) -> String {
        serde_json::to_string(val).unwrap_or_else(|_| "{}".to_string())
    }
    pub fn parse(&self, text: &str) -> Option<Value> {
        serde_json::from_str(text).ok()
    }
}

#[derive(Default)]
pub struct TimeApi;
impl TimeApi {
    pub fn get_unix_time_msec(&self) -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_millis(0))
            .as_millis()
    }
    pub fn get_unix_time(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
    }
    pub fn get_datetime_string(&self) -> String {
        let now = Local::now();
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            now.year(), now.month(), now.day(), now.hour(), now.minute(), now.second()
        )
    }
    pub fn sleep_msec(&self, ms: u64) {
        thread::sleep(Duration::from_millis(ms));
    }
    pub fn get_ticks_msec(&self) -> u128 {
        self.get_unix_time_msec()
    }
    pub fn get_delta(&self, delta: f32) -> f32 {
        delta
    }
}

#[derive(Default)]
pub struct OsApi;
impl OsApi {
    pub fn get_platform_name(&self) -> &'static str {
        env::consts::OS
    }
    pub fn getenv(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }
    pub fn open_file_explorer(&self, path: &str) -> bool {
        #[cfg(target_os = "windows")]
        { std::process::Command::new("explorer").arg(path).status().is_ok() }

        #[cfg(target_os = "linux")]
        { std::process::Command::new("xdg-open").arg(path).status().is_ok() }

        #[cfg(target_os = "macos")]
        { std::process::Command::new("open").arg(path).status().is_ok() }
    }
}

#[derive(Default)]
pub struct ProcessApi;
impl ProcessApi {
    pub fn quit(&self, code: i32) {
        process::exit(code);
    }
    pub fn get_launch_args(&self) -> Vec<String> {
        env::args().collect()
    }
    pub fn exec(&self, cmd: &str, args: &[&str]) -> bool {
        process::Command::new(cmd).args(args).status().is_ok()
    }
}

//-----------------------------------------------------
// 2️⃣ Engine API Aggregator
//-----------------------------------------------------

#[allow(non_snake_case)]
#[derive(Default)]
pub struct EngineApi {
    pub JSON: JsonApi,
    pub Time: TimeApi,
    pub OS: OsApi,
    pub Process: ProcessApi,
}

//-----------------------------------------------------
// 4️⃣  Deref Implementation
//-----------------------------------------------------

impl<'a> Deref for ScriptApi<'a> {
    type Target = EngineApi;
    fn deref(&self) -> &Self::Target {
        &self.engine
    }
}

//-----------------------------------------------------
// 3️⃣ Script API Context (main entry point for scripts)
//-----------------------------------------------------
pub struct ScriptApi<'a> {
    delta: f32,
    scene: &'a mut dyn SceneAccess,
    project: &'a mut Project,
    engine: EngineApi,
}

impl<'a> ScriptApi<'a> {
    pub fn new(delta: f32, scene: &'a mut dyn SceneAccess, project: &'a mut Project) -> Self {
        Self {
            delta,
            scene,
            project,
            engine: EngineApi::default(),
        }
    }

    //-------------------------------------------------
    // Core access
    //-------------------------------------------------
    pub fn project(&mut self) -> &mut Project {
        self.project
    }
    pub fn delta(&self) -> f32 {
        self.delta
    }

    //-------------------------------------------------
    // Compilation helpers
    //-------------------------------------------------
    pub fn compile_scripts(&mut self) -> Result<(), String> {
        self.run_compile(BuildProfile::Dev, CompileTarget::Scripts)
    }
    pub fn compile_project(&mut self) -> Result<(), String> {
        self.run_compile(BuildProfile::Release, CompileTarget::Project)
    }
    fn run_compile(&mut self, profile: BuildProfile, target: CompileTarget) -> Result<(), String> {
        let project_path_str = self
            .project
            .get_runtime_param("project_path")
            .ok_or("Missing runtime param: project_path")?;
        eprintln!("📁 Project path: {}", project_path_str);
        let project_path = Path::new(project_path_str);
        transpile(project_path).map_err(|e| format!("Transpile failed: {}", e))?;
        let compiler = Compiler::new(project_path, target, false);
        compiler.compile(profile).map_err(|e| format!("Compile failed: {}", e))
    }

    //-------------------------------------------------
    // Window / App Commands
    //-------------------------------------------------
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

    //-------------------------------------------------
    // Lifecycle / Updates
    //-------------------------------------------------
    pub fn call_update(&mut self, id: Uuid) {
        if let Some(rc_script) = self.scene.get_script(id) {
            let mut script = rc_script.borrow_mut();
            script.update(self);
        }
    }

pub fn instantiate_script(&mut self, path: &str) -> Option<ScriptType> {
    // Convert to registry key
    let identifier = match script_path_to_identifier(path) {
        Ok(id) => id,
        Err(err) => {
            eprintln!("[ScriptApi] Invalid path: {}", err);
            return None;
        }
    };

    // Get ctor safely from provider
    let ctor = match self.scene.load_ctor(&identifier) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "[ScriptApi] Failed to find script '{}' ({}).",
                identifier, e
            );
            return None;
        }
    };

    // Construct script (create the Box)
    let raw = ctor();
    let mut boxed: ScriptType = unsafe { Box::from_raw(raw) };
    boxed.set_node_id(Uuid::nil()); // detached standalone script

    // Run init() safely using a temporary api context
    {
        let project_ref = self.project as *mut _;
        let project_mut = unsafe { &mut *project_ref };
        let mut sub_api = ScriptApi::new(0.0, self.scene, project_mut);
        boxed.init(&mut sub_api);
    }

    Some(boxed)
}

    //-------------------------------------------------
    // Asset IO
    //-------------------------------------------------
    pub fn load_asset(&mut self, path: &str) -> Option<Vec<u8>> {
        asset_io::load_asset(path).ok()
    }
    pub fn save_asset<D>(&mut self, path: &str, data: D) -> Option<()>
    where D: AsRef<[u8]> {
        asset_io::save_asset(path, data.as_ref()).ok()
    }
    pub fn resolve_path(&self, path: &str) -> Option<String> {
        match resolve_path(path) {
            ResolvedPath::Disk(pathbuf) => pathbuf.to_str().map(String::from),
            ResolvedPath::Brk(vpath) => vpath.into(),
        }
    }

    //-------------------------------------------------
    // Scene / Node Access
    //-------------------------------------------------
    pub fn get_node_clone<T: Clone>(&mut self, id: &Uuid) -> T
    where SceneNode: IntoInner<T> {
        let node_enum = self.scene
            .get_scene_node(id)
            .unwrap_or_else(|| panic!("Node {} not found", id))
            .clone();
        node_enum.into_inner()
    }
    pub fn merge_nodes(&mut self, nodes: Vec<SceneNode>) {
        self.scene.merge_nodes(nodes);
    }
    pub fn update_script_var(
        &mut self, node_id: &Uuid, name: &str, op: UpdateOp, val: Var,
    ) -> Option<()> {
        self.scene.update_script_var(node_id, name, op, val)
    }


    //prints

    pub fn print<T: std::fmt::Display>(&self, msg: T) {
        println!("{}", msg);
    }

    /// Print a warning in yellow
    pub fn print_warn<T: std::fmt::Display>(&self, msg: T) {
        // ANSI yellow = 33
        println!("\x1b[33m[WARN]\x1b[0m {}", msg);
    }

    /// Print an error in red
    pub fn print_error<T: std::fmt::Display>(&self, msg: T) {
        // ANSI red = 31
        eprintln!("\x1b[31m[ERROR]\x1b[0m {}", msg);
    }

    /// Optionally, print info in cyan if you want another level
    pub fn print_info<T: std::fmt::Display>(&self, msg: T) {
        println!("\x1b[36m[INFO]\x1b[0m {}", msg);
    }
    
}

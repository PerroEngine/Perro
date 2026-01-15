#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
use serde_json::{Value, json};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use perro_core::prelude::*;
use perro_core::ui_element::{BaseElement, UIElement};
use perro_core::ui_elements::ui_list_tree::{UIListTree, ListTreeItem, SelectionMode};
use perro_core::ui_elements::ui_context_menu::{UIContextMenu, ContextMenuItem};
use perro_core::ui_list_tree_manager::ListTreeManager;
use perro_core::nodes::ui::fur_ast::FurAnchor;
use std::path::{Path, PathBuf};
use phf::{phf_map, Map};
use smallvec::{SmallVec, smallvec};

/// Editor mode - determines which panels/content are visible
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum EditorMode {
    UI,
    TwoD,
    ThreeD,
    Script,
}

/// Asset metadata for UID registry
#[derive(Debug, Clone)]
struct AssetMetadata {
    uid: Uuid,
    path: String,
}

/// @PerroScript
pub static MEMBER_TO_ATTRIBUTES_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {
};

static ATTRIBUTE_TO_MEMBERS_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {
};

struct ManagerScript {
    id: Uuid,
    create_project_button_pressed: bool,
    project_creation_in_progress: bool,
    current_project_path: String,
    run_project_key_pressed: bool,
    game_was_running: bool, // Track if we had a game running to detect when it stops
    compilation_in_progress: bool, // Prevent multiple simultaneous compilations
    process_check_counter: u32, // Counter to check process status less frequently
    initial_compile_delay_accumulator: f32, // Accumulate time to delay initial compilation (wait 2 seconds)
    initial_compile_done: bool, // Track if initial compilation has been done
    resources_scanned: bool, // Track if we've scanned the res folder
    resource_files: Vec<String>, // List of res:// paths to all files in the project
    current_mode: Option<EditorMode>, // Current editor mode (None = default viewport mode)
    file_tree_initialized: bool, // Track if file tree UI has been created
    file_tree_id: Option<Uuid>, // Store file tree element ID
    context_menu_id: Option<Uuid>, // Store context menu element ID
    frame_count: u32, // Track frames to delay file tree initialization
    // UID registry - editor-specific: maps file paths to UIDs for tracking renames
    uid_registry: HashMap<Uuid, AssetMetadata>, // UID -> metadata
    path_to_uid: HashMap<String, Uuid>, // path -> UID (reverse lookup)
    // Note: Mouse tracking and double-click tracking are now handled by UIListTree internally
    // Live preview debouncing - updates preview 2 seconds after last text change
    live_preview_timer: f32, // Accumulator for debounce timer after text change
    last_fur_text_hash: u64, // Hash of last text content to detect changes efficiently
}

#[unsafe(no_mangle)]
pub extern "C" fn scripts_manager_rs_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(ManagerScript {
        id: Uuid::nil(),
        create_project_button_pressed: false,
        project_creation_in_progress: false,
        current_project_path: String::new(),
        run_project_key_pressed: false,
        game_was_running: false,
        compilation_in_progress: false,
        process_check_counter: 0,
        initial_compile_delay_accumulator: 0.0,
        initial_compile_done: false,
        resources_scanned: false,
        resource_files: Vec::new(),
        current_mode: Some(EditorMode::TwoD),
        file_tree_initialized: false,
        file_tree_id: None,
        context_menu_id: None,
        frame_count: 0,
        uid_registry: HashMap::new(),
        path_to_uid: HashMap::new(),
        live_preview_timer: 0.0,
        last_fur_text_hash: 0,
    })) as *mut dyn ScriptObject
}

impl ManagerScript {
    // ===== UID REGISTRY METHODS =====
    // Editor-specific: Track file paths with UIDs for rename/move operations
    
    /// Register a file path and get/create its UID
    /// Uses deterministic UUID (v5) so same path = same UID across sessions
    fn register_asset(&mut self, path: &str) -> Uuid {
        // Check if already registered
        if let Some(&uid) = self.path_to_uid.get(path) {
            return uid;
        }
        
        // Create deterministic UID from path hash
        let namespace = Uuid::NAMESPACE_URL;
        let uid = Uuid::new_v5(&namespace, path.as_bytes());
        
        // Store in registry
        let metadata = AssetMetadata {
            uid,
            path: path.to_string(),
        };
        self.uid_registry.insert(uid, metadata);
        self.path_to_uid.insert(path.to_string(), uid);
        
        uid
    }
    
    /// Get UID for a path, or None if not registered
    fn get_uid(&self, path: &str) -> Option<Uuid> {
        self.path_to_uid.get(path).copied()
    }
    
    /// Get path for a UID, or None if not found
    fn get_path(&self, uid: Uuid) -> Option<&str> {
        self.uid_registry.get(&uid).map(|m| m.path.as_str())
    }
    
    /// Rename/move an asset (updates UID registry)
    /// Returns old_path if successful
    fn rename_asset(&mut self, uid: Uuid, new_path: &str) -> Result<String, String> {
        let metadata = self.uid_registry.get_mut(&uid)
            .ok_or_else(|| format!("Asset UID {} not found in registry", uid))?;
        
        let old_path = metadata.path.clone();
        
        // Update path in metadata
        metadata.path = new_path.to_string();
        
        // Update reverse mapping
        self.path_to_uid.remove(&old_path);
        self.path_to_uid.insert(new_path.to_string(), uid);
        
        Ok(old_path)
    }
    
    /// Update all scene files on disk when a file is renamed/moved
    fn update_all_scene_files_on_disk(&self, api: &mut ScriptApi, project_res_path: &str, old_path: &str, new_path: &str) -> Result<(usize, usize), String> {
        use perro_core::scene::SceneData;
        
        let res_dir = Path::new(project_res_path);
        if !res_dir.exists() {
            return Ok((0, 0));
        }
        
        let mut scenes_checked = 0;
        let mut total_nodes_updated = 0;
        
        // Walk through all .scn files in res/ using FileSystem API
        // project_res_path is already a disk path like "d:/path/to/project/res"
        let scn_files = api.FileSystem.walk_files_with_ext(project_res_path, "scn");
        
        for relative_path_str in scn_files {
            scenes_checked += 1;
            
            // Construct the full res:// path (relative_path_str is already relative to res/)
            let scene_res_path = format!("res://{}", relative_path_str);
            
            // Load the scene
            match SceneData::load(&scene_res_path) {
                Ok(mut scene_data) => {
                    // Update asset paths in this scene (inlined to avoid codegen issues)
                    use perro_core::nodes::node_registry::SceneNode;
                    let mut nodes_updated = 0;
                    
                    for (_idx, node) in scene_data.nodes.iter_mut() {
                        // Update script paths
                        if let Some(script_path) = node.get_script_path() {
                            if script_path == old_path {
                                node.set_script_path(new_path);
                                nodes_updated += 1;
                            }
                        }
                        
                        // Update texture paths
                        if let Some(texture_path) = Self::get_texture_path_mut(node) {
                            if texture_path.as_ref() == old_path {
                                *texture_path = new_path.to_string().into();
                                nodes_updated += 1;
                            }
                        }
                        
                        // Update mesh paths
                        if let Some(mesh_path) = Self::get_mesh_path_mut(node) {
                            if mesh_path.as_ref() == old_path {
                                *mesh_path = new_path.to_string().into();
                                nodes_updated += 1;
                            }
                        }
                        
                        // Update material paths
                        if let Some(material_path) = Self::get_material_path_mut(node) {
                            if material_path.as_ref() == old_path {
                                *material_path = new_path.to_string().into();
                                nodes_updated += 1;
                            }
                        }
                        
                        // Update FUR paths
                        if let SceneNode::UINode(ui_node) = node {
                            if let Some(ref fur_path) = ui_node.fur_path {
                                if fur_path.as_ref() == old_path {
                                    ui_node.fur_path = Some(new_path.to_string().into());
                                    nodes_updated += 1;
                                }
                            }
                        }
                    }
                    
                    if nodes_updated > 0 {
                        // Save the scene back to disk
                        if let Err(e) = scene_data.save(&scene_res_path) {
                            eprintln!("⚠️ Failed to save scene {}: {}", scene_res_path, e);
                        } else {
                            total_nodes_updated += nodes_updated;
                            eprintln!("✅ Updated {} node(s) in scene: {}", nodes_updated, scene_res_path);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("⚠️ Failed to load scene {}: {}", scene_res_path, e);
                }
            }
        }
        
        Ok((scenes_checked, total_nodes_updated))
    }
    
    /// Helper to get mutable texture path from a node
    fn get_texture_path_mut<'a>(node: &'a mut perro_core::nodes::node_registry::SceneNode) -> Option<&'a mut std::borrow::Cow<'static, str>> {
        match node {
            perro_core::nodes::node_registry::SceneNode::Sprite2D(sprite) => sprite.texture_path.as_mut(),
            _ => None,
        }
    }
    
    /// Helper to get mutable mesh path from a node
    fn get_mesh_path_mut<'a>(node: &'a mut perro_core::nodes::node_registry::SceneNode) -> Option<&'a mut std::borrow::Cow<'static, str>> {
        match node {
            perro_core::nodes::node_registry::SceneNode::MeshInstance3D(mesh) => mesh.mesh_path.as_mut(),
            _ => None,
        }
    }
    
    /// Helper to get mutable material path from a node
    fn get_material_path_mut<'a>(node: &'a mut perro_core::nodes::node_registry::SceneNode) -> Option<&'a mut std::borrow::Cow<'static, str>> {
        match node {
            perro_core::nodes::node_registry::SceneNode::MeshInstance3D(mesh) => mesh.material_path.as_mut(),
            _ => None,
        }
    }
    
    /// Find the editor executable in a directory
    fn find_editor_exe(&self, dir: &Path) -> Option<PathBuf> {
        use std::fs;
        
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    #[cfg(target_os = "windows")]
                    {
                        if path.extension().map_or(false, |e| e == "exe") {
                            return Some(path);
                        }
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        // On Unix, check if it's executable and has no extension or is named "perro_editor"
                        if let Some(name) = path.file_name() {
                            if name == "perro_editor" || path.extension().is_none() {
                                // Check if executable
                                if fs::metadata(&path)
                                    .ok()
                                    .map(|m| {
                                        #[cfg(unix)]
                                        {
                                            use std::os::unix::fs::PermissionsExt;
                                            m.permissions().mode() & 0o111 != 0
                                        }
                                        #[cfg(not(unix))]
                                        {
                                            true // Assume executable on non-Unix if no extension
                                        }
                                    })
                                    .unwrap_or(false)
                                {
                                    return Some(path);
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
    
    /// Launch the editor via cargo in dev mode
    fn launch_editor_via_cargo(&self, project_path: &str) -> Result<(), String> {
        use std::process::Command;
        
        eprintln!("🚀 Launching editor via cargo: cargo run -p perro_core -- --path --editor {} --run", project_path);
        
        let mut cmd = Command::new("cargo");
        cmd.args(&["run", "-p", "perro_core", "--", "--path", "--editor", project_path, "--run"]);
        
        // Use current directory as working directory
        cmd.current_dir(std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?);
        
        // Platform-specific: Make the process run completely independently
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NEW_CONSOLE = 0x00000010 (new console window)
            // DETACHED_PROCESS = 0x00000008 (detached from parent)
            // CREATE_NEW_PROCESS_GROUP = 0x00000200 (new process group)
            // Combine flags: 0x00000210
            cmd.creation_flags(0x00000210);
            cmd.stdout(std::process::Stdio::inherit());
            cmd.stderr(std::process::Stdio::inherit());
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            cmd.stdout(std::process::Stdio::inherit());
            cmd.stderr(std::process::Stdio::inherit());
        }
        
        cmd.spawn()
            .map_err(|e| format!("Failed to launch editor via cargo: {}", e))?;
        
        Ok(())
    }
    
    /// Launch the editor with --editor PATH and exit current process
    fn launch_editor_with_project(&self, exe_path: &Path, project_path: &str) -> Result<(), String> {
        use std::process::Command;
        
        let parent_dir = exe_path
            .parent()
            .ok_or("Could not determine parent directory")?;
        
        eprintln!("🚀 Launching editor: {} --editor {}", exe_path.display(), project_path);
        
        let mut cmd = Command::new(exe_path);
        cmd.arg("--editor").arg(project_path);
        cmd.current_dir(parent_dir);
        
        // Platform-specific: Make the process run completely independently
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NEW_CONSOLE = 0x00000010 (new console window)
            // DETACHED_PROCESS = 0x00000008 (detached from parent)
            // CREATE_NEW_PROCESS_GROUP = 0x00000200 (new process group)
            // Combine flags: 0x00000210
            cmd.creation_flags(0x00000210);
            cmd.stdout(std::process::Stdio::inherit());
            cmd.stderr(std::process::Stdio::inherit());
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            cmd.stdout(std::process::Stdio::inherit());
            cmd.stderr(std::process::Stdio::inherit());
        }
        
        cmd.spawn()
            .map_err(|e| format!("Failed to launch editor: {}", e))?;
        
        Ok(())
    }
    
    /// Check if a process with the given PID is still running
    /// Uses command-line tools to avoid requiring platform-specific crates
    fn is_process_running(&self, pid: u32) -> bool {
        use std::process::Command;
        
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // Use tasklist to check if process exists
            // /FI "PID eq <pid>" filters by process ID
            // /NH removes header row
            // /FO CSV outputs in CSV format
            Command::new("tasklist")
                .args(&["/FI", &format!("PID eq {}", pid), "/NH", "/FO", "CSV"])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW - hide console window
                .output()
                .map(|output| {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // If process exists, stdout will contain the process info
                    // If not, stdout will be empty or just contain headers
                    !stdout.trim().is_empty() && stdout.contains(&pid.to_string())
                })
                .unwrap_or(false)
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // On Unix-like systems, send signal 0 to check if process exists
            // Try to kill with signal 0 (doesn't actually kill, just checks existence)
            Command::new("kill")
                .args(&["-0", &pid.to_string()])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }
    }
    
    /// Kill/stop a process with the given PID
    fn kill_process(&self, pid: u32) -> bool {
        use std::process::Command;
        
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // Use taskkill to terminate the process
            Command::new("taskkill")
                .args(&["/PID", &pid.to_string(), "/F"])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW - hide console window
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // On Unix-like systems, send SIGTERM to gracefully terminate
            // If that doesn't work, SIGKILL will be needed (but we'll try TERM first)
            Command::new("kill")
                .args(&["-TERM", &pid.to_string()])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }
    }
    
    pub fn run_project(&mut self, api: &mut ScriptApi) {
        api.print("🔔 RunGame_Pressed signal received!");
        
        // Prevent multiple simultaneous compilations
        if self.compilation_in_progress {
            api.print("⚠️ Compilation already in progress, please wait...");
            return;
        }
        
        // Check if game is already running
        if self.game_was_running {
            if let Some(process_id_str) = api.project().get_runtime_param("runtime_process_id") {
                if let Ok(process_id) = process_id_str.parse::<u32>() {
                    if self.is_process_running(process_id) {
                        api.print("⚠️ Game is already running!");
                        return;
                    }
                }
            }
        }
        
        if !self.current_project_path.is_empty() {
            self.compilation_in_progress = true;
            api.print("🚀 Compiling and running project...");
            
            // Debug: Show current project path
            api.print_info(&format!("Current project path (stored): {}", self.current_project_path));
            
            // Resolve the path if it's still a user:// path
            let resolved_path = if self.current_project_path.starts_with("user://") {
                if let Some(resolved) = api.resolve_path(&self.current_project_path) {
                    // Update stored path to resolved path for future use
                    self.current_project_path = resolved.clone();
                    resolved
                } else {
                    api.print(&format!("❌ Failed to resolve path: {}", self.current_project_path));
                    self.compilation_in_progress = false;
                    return;
                }
            } else {
                self.current_project_path.clone()
            };
            
            api.print_info(&format!("Resolved project path: {}", resolved_path));
            
            // Set the resolved project_path in runtime params
            api.project().set_runtime_param("project_path", &resolved_path);
            
            // Debug: Verify what compile_and_run will see
            let verify_path_str = api.project().get_runtime_param("project_path").map(|s| s.to_string());
            if let Some(ref verify_path) = verify_path_str {
                api.print_info(&format!("Runtime param 'project_path' is set to: {}", verify_path));
            } else {
                api.print_error("Runtime param 'project_path' is NOT set!");
            }
            
            match api.compile_and_run() {
                Ok(_) => {
                    api.print("✅ Project compiled and running!");
                    self.game_was_running = true;
                    self.compilation_in_progress = false;
                }
                Err(e) => {
                    api.print(&format!("❌ Failed to compile and run: {}", e));
                    self.compilation_in_progress = false;
                }
            }
        } else {
            api.print("⚠️ No project loaded. Create a project first.");
            self.compilation_in_progress = false;
        }
    }
    
    pub fn stop_project(&mut self, api: &mut ScriptApi) {
        api.print("🔔 StopGame_Pressed signal received!");
        if let Some(process_id_str) = api.project().get_runtime_param("runtime_process_id") {
            if let Ok(process_id) = process_id_str.parse::<u32>() {
                api.print(&format!("🛑 Stopping game process (PID: {})...", process_id));
                
                if self.kill_process(process_id) {
                    api.print("✅ Game process stopped");
                    self.game_was_running = false;
                    // Clear the runtime params
                    api.project().set_runtime_param("runtime_process_id", "");
                    api.project().set_runtime_param("runtime_process_running", "false");
                } else {
                    api.print("⚠️ Failed to stop process (may have already exited)");
                    // Clear params anyway
                    self.game_was_running = false;
                    api.project().set_runtime_param("runtime_process_id", "");
                    api.project().set_runtime_param("runtime_process_running", "false");
                }
            } else {
                api.print("⚠️ No valid process ID found");
            }
        } else {
            api.print("⚠️ No game process is currently running");
        }
    }
    
    fn create_project(&mut self, api: &mut ScriptApi) {
        if self.project_creation_in_progress {
            return;
        }

        self.project_creation_in_progress = true;
        api.print("🚀 Starting project creation...");

        // Hardcoded values for testing
        let project_name = "TestProject";
        let projects_dir = "user://projects/";
        let project_path = format!("{}{}", projects_dir, project_name);

        // Create the project
        api.print(&format!("📁 Creating project '{}' at {}...", project_name, project_path));
        
        if !api.Editor.create_project(project_name, &project_path) {
            api.print("❌ Failed to create project");
            self.project_creation_in_progress = false;
            return;
        }

        // Get the resolved project path from runtime params (create_project sets it)
        // This will be a disk path, not a user:// path
        let resolved_path_opt = api.project().get_runtime_param("project_path").map(|s| s.to_string());
        if let Some(resolved_path) = resolved_path_opt {
            self.current_project_path = resolved_path.clone();
            api.print(&format!("📁 Project path resolved to: {}", resolved_path));
        } else {
            // Fallback: try to resolve the path ourselves
            if let Some(resolved) = api.resolve_path(&project_path) {
                self.current_project_path = resolved;
            } else {
                api.print("⚠️ Warning: Could not resolve project path");
                self.current_project_path = project_path;
            }
        }

        api.print("✅ Project created successfully!");
        
        // Launch editor with --editor PATH to open the project in editor mode
        // Get the resolved project path (should be set by create_project)
        let project_path_to_open = if let Some(resolved) = api.project().get_runtime_param("project_path") {
            resolved.to_string()
        } else if !self.current_project_path.is_empty() {
            self.current_project_path.clone()
        } else {
            api.print("⚠️ Warning: No project path available to open");
            self.project_creation_in_progress = false;
            return;
        };
        
        // Launch editor with --editor PATH
        if cfg!(debug_assertions) {
            // Dev mode: launch via cargo run -p perro_core -- --path --editor PATH --run
            api.print("🐛 Dev mode: Launching via cargo");
            api.print(&format!("🚀 Launching editor with project: {}", project_path_to_open));
            
            if let Err(e) = self.launch_editor_via_cargo(&project_path_to_open) {
                api.print(&format!("❌ Failed to launch editor: {}", e));
                self.project_creation_in_progress = false;
                return;
            }
            
            // Exit current process
            std::process::exit(0);
        } else {
            // Release mode: find in versioned location and launch directly
            let my_version = api.project().version();
            let editor_path_str = format!("user://versions/{}/editor/", my_version);
            
            let editor_exe = if let Some(editor_dir) = api.resolve_path(&editor_path_str) {
                // Try to find editor in versioned location
                if let Some(exe) = self.find_editor_exe(Path::new(&editor_dir)) {
                    Some(exe)
                } else {
                    api.print(&format!("⚠️ Editor not found in {}, falling back to current executable", editor_dir));
                    std::env::current_exe().ok()
                }
            } else {
                api.print(&format!("⚠️ Could not resolve {}, falling back to current executable", editor_path_str));
                std::env::current_exe().ok()
            };
            
            if let Some(exe_path) = editor_exe {
                api.print(&format!("🚀 Launching editor with project: {}", project_path_to_open));
                
                // Launch the editor with --editor PATH and exit current process
                if let Err(e) = self.launch_editor_with_project(&exe_path, &project_path_to_open) {
                    api.print(&format!("❌ Failed to launch editor: {}", e));
                    self.project_creation_in_progress = false;
                    return;
                }
                
                // Exit current process
                std::process::exit(0);
            } else {
                api.print("❌ Could not find editor executable");
            }
        }

        self.project_creation_in_progress = false;
    }
    
    /// Compile scripts if needed (called after window is visible, with 2 second delay)
    fn check_and_compile_if_needed(&mut self, api: &mut ScriptApi) {
        // Check if we need to do initial compilation
        if !self.initial_compile_done {
            if let Some(needs_compile) = api.project().get_runtime_param("needs_initial_compile") {
                if needs_compile == "true" {
                    // Accumulate time and wait 2 seconds before compiling
                    self.initial_compile_delay_accumulator += api.Time.get_delta();
                    if self.initial_compile_delay_accumulator >= 2.0 {
                        // Clear the flag and mark as done
                        api.project().set_runtime_param("needs_initial_compile", "false");
                        self.initial_compile_done = true;
                        
                        // Compile scripts in the background (window is already visible)
                        api.print("🔧 Compiling scripts...");
                        match api.compile_scripts() {
                            Ok(_) => {
                                api.print("✅ Scripts compiled successfully");
                            }
                            Err(e) => {
                                api.print(&format!("⚠️ Script compilation failed: {} (project may still work)", e));
                            }
                        }
                    }
                } else {
                    // Flag is false, mark as done so we don't keep checking
                    self.initial_compile_done = true;
                }
            } else {
                // No flag set, mark as done
                self.initial_compile_done = true;
            }
        }
    }
    
    /// Initialize and populate the file tree UI component
    fn init_file_tree_ui(&mut self, api: &mut ScriptApi, project_path: &str) {
        if self.file_tree_initialized {
            api.print("⚠️ File tree already initialized");
            return;
        }
        
        api.print("🌳 Populating file tree from FUR...");
        
        let ui_node_id = self.id;
        
        // Find the ResourceFileTree element (defined in FUR)
        let file_tree_id = api.with_ui_node(ui_node_id, |ui| {
            ui.find_element_by_name("ResourceFileTree")
                .map(|element| element.get_id())
        });
        
        let Some(file_tree_id) = file_tree_id else {
            api.print("❌ Could not find ResourceFileTree in FUR file");
            return;
        };
        
        api.print(&format!("✅ Found ResourceFileTree: {}", file_tree_id));
        
        // Load files from disk
        let resolved_path = if project_path.starts_with("user://") {
            api.resolve_path(project_path).unwrap_or_else(|| project_path.to_string())
        } else {
            project_path.to_string()
        };
        
        let res_dir = PathBuf::from(&resolved_path).join("res");
        
        // Populate the file tree with directory contents
        let result = api.with_ui_node(ui_node_id, |ui| -> Result<usize, String> {
            if let Some(UIElement::ListTree(file_tree)) = ui.find_element_by_name_mut("ResourceFileTree") {
                file_tree.root_path = "res://".to_string();
                file_tree.selection_mode = SelectionMode::Single;
                file_tree.show_extensions = true;
                file_tree.show_hidden = false;
                
                if res_dir.exists() {
                    if let Err(e) = file_tree.load_from_directory(&res_dir) {
                        return Err(format!("❌ Failed to load directory: {}", e));
                    }
                    let item_count = file_tree.items.len();
                    
                    // Register all files with UID registry (editor-specific)
                    for item in file_tree.items.values_mut() {
                        if !item.is_directory {
                            let uid = self.register_asset(&item.path);
                            item.uid = Some(uid);
                        }
                    }
                    
                    // Mark for rerender
                    ui.mark_element_needs_rerender(file_tree_id);
                    ui.mark_element_needs_layout(file_tree_id);
                    
                    Ok(item_count)
                } else {
                    Err("❌ Resource directory doesn't exist".to_string())
                }
            } else {
                Err("❌ FileTree element not found in UI".to_string())
            }
        });
        
        // Print results after closure
        match result {
            Ok(count) => api.print(&format!("✅ Loaded {} items from res/", count)),
            Err(e) => {
                api.print(&e);
                return;
            }
        }
        
        self.file_tree_id = Some(file_tree_id);
        
        // Create context menu (initially hidden)
        let mut context_menu = UIContextMenu::new();
        context_menu.set_name("FileTreeContextMenu");
        context_menu.set_items(UIContextMenu::create_file_tree_menu());
        context_menu.visible = false;
        context_menu.base.z_index = 1000; // Render on top
        
        // Add context menu to UI (as top-level element)
        let context_menu_element = UIElement::ContextMenu(context_menu);
        if let Some(menu_id) = api.add_ui_element(ui_node_id, "FileTreeContextMenu", context_menu_element, None) {
            api.print(&format!("✅ Context menu added with ID: {}", menu_id));
            self.context_menu_id = Some(menu_id);
        } else {
            api.print("❌ Failed to add context menu to UI");
        }
        
        self.file_tree_initialized = true;
        api.print("✅ File tree UI initialized");
    }
    
    /// Legacy function - now creates file tree instead of buttons
    fn add_resource_buttons_to_ui(&mut self, api: &mut ScriptApi, res_files: &[String]) {
        // Get the UINode (self.id is the UINode's ID)
        let ui_node_id = self.id;
        api.print(&format!("🔍 Looking for ResourcesContainer in UINode: {}", ui_node_id));
        
        // Find the ResourcesContainer panel and collect debug info
        let (resources_container_id, element_names) = api.with_ui_node(ui_node_id, |ui| {
            // Debug: Collect all element names
            let mut names = Vec::new();
            if let Some(elements) = &ui.elements {
                for (uuid, element) in elements.iter() {
                    names.push((element.get_name().to_string(), uuid.to_string()));
                }
            }
            
            // Find the ResourcesContainer element by name
            let container_id = ui.find_element_by_name("ResourcesContainer")
                .map(|element| element.get_id());
            
            (container_id, names)
        });
        
        // Print debug info outside the closure
        api.print(&format!("📋 Found {} elements in UI", element_names.len()));
        for (name, uuid) in element_names.iter() {
            api.print(&format!("  - Element: {} (ID: {})", name, uuid));
        }
        
        if let Some(container_id) = resources_container_id {
            api.print(&format!("✅ Using ResourcesContainer ID: {}", container_id));
            
            // Create buttons for each resource file directly using UIButton::new()
            for (i, file_path) in res_files.iter().enumerate() {
                // Extract just the filename for display
                let filename = file_path.split('/').last().unwrap_or(file_path);
                
                // Create button directly
                use perro_core::nodes::ui::ui_elements::ui_button::UIButton;
                use perro_core::nodes::ui::ui_element::UIElement;
                use perro_core::nodes::ui::fur_ast::FurAnchor;
                
                let mut button = UIButton::new();
                
                // Set button properties
                button.set_name(&format!("resource_{}", i));
                button.set_anchor(FurAnchor::Center);
                
                // Set size using style_map for percentage (95% width, 25 height)
                button.get_style_map_mut().insert("size.x".to_string(), 95.0); // 95%
                button.get_style_map_mut().insert("size.y".to_string(), 25.0);
                
                // Set background color
                if let Some(bg_color) = Color::from_preset("steel-8") {
                    button.panel_props_mut().background_color = Some(bg_color);
                }
                
                // Set text properties
                button.text_props_mut().content = filename.to_string();
                button.text_props_mut().color = Color::new(255, 255, 255, 255); // white
                button.text_props_mut().font_size = 14.0;
                
                // Wrap in UIElement and add to UI
                let button_element = UIElement::Button(button);
                api.print(&format!("  🎨 Creating button {} for: {} (parent: {})", i, filename, container_id));
                if let Some(button_id) = api.add_ui_element(ui_node_id, &format!("resource_{}", i), button_element, Some(container_id)) {
                    api.print(&format!("  ✅ Added button for: {} (ID: {})", filename, button_id));
                } else {
                    api.print(&format!("  ❌ Failed to add button for: {}", filename));
                }
            }
            
            // Verify buttons were added and mark only the container and new buttons for rerender
            let button_count = api.with_ui_node(ui_node_id, |ui| {
                // Collect IDs to mark (immutable borrow first)
                let mut ids_to_mark = Vec::new();
                
                // Mark only the ResourcesContainer and its children for rerender (not all elements)
                if let Some(container) = ui.find_element_by_name("ResourcesContainer") {
                    let container_id = container.get_id();
                    ids_to_mark.push(container_id);
                    
                    // Also collect all children of the container
                    if let Some(elements) = &ui.elements {
                        let mut to_process = vec![container_id];
                        while let Some(current_id) = to_process.pop() {
                            if let Some(element) = elements.get(&current_id) {
                                for child_id in element.get_children() {
                                    if !ids_to_mark.contains(child_id) {
                                        ids_to_mark.push(*child_id);
                                        to_process.push(*child_id);
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Now mark all collected IDs (mutable borrow after immutable is released)
                for element_id in &ids_to_mark {
                    ui.mark_element_needs_rerender(*element_id);
                    ui.mark_element_needs_layout(*element_id);
                }
                
                if let Some(elements) = &ui.elements {
                    elements.values()
                        .filter(|el| el.get_name().starts_with("resource_"))
                        .count()
                } else {
                    0
                }
            });
            api.print(&format!("📊 Total resource buttons in UI: {}", button_count));
        } else {
            api.print("❌ Could not find ResourcesContainer panel");
            api.print("💡 Make sure the FUR file has an element with id='ResourcesContainer'");
        }
    }
    
    /// Set the editor mode - switches between UI, 2D, 3D, Script modes
    fn set_editor_mode(&mut self, mode: EditorMode, api: &mut ScriptApi) {
        // If already in this mode, do nothing
        if self.current_mode == Some(mode) {
            api.print(&format!("⚠️ Already in {:?} mode", mode));
            return;
        }
        
        let ui_node_id = self.id;
        
        // Update current mode
        self.current_mode = Some(mode);
        
        api.print(&format!("🎨 Switching to {:?} mode", mode));
        
        api.with_ui_node(ui_node_id, |ui| {
            // Determine visibility based on mode
            let (panels_visible, ui_editor_visible) = match mode {
                EditorMode::UI => (false, true),  // Hide panels, show UI editor
                EditorMode::TwoD | EditorMode::ThreeD | EditorMode::Script => (true, false), // Show panels, hide UI editor
            };
            
            // Array of (name, visibility) pairs to process
            let elements_to_toggle = [
                ("SceneGraphPanel", panels_visible),
                ("InspectorPanel", panels_visible),
                ("DefaultViewport", panels_visible),
                ("UIEditorContent", ui_editor_visible),
            ];
            
            // Collect IDs of changed elements and their parents
            let mut changed_element_ids = Vec::new();
            let mut parent_ids = Vec::new();
            
            // Process each element: set visibility and collect IDs
            // The UI renderer will automatically handle child visibility via is_effectively_visible
            for (name, visible) in &elements_to_toggle {
                if let Some(element) = ui.find_element_by_name_mut(name) {
                    element.set_visible(*visible);
                    let element_id = element.get_id();
                    changed_element_ids.push(element_id);
                    
                    // Also collect parent IDs for layout recalculation
                    let parent_id = element.get_parent();
                    if !parent_id.is_nil() {
                        parent_ids.push(parent_id);
                    }
                }
            }
            
            // Mark changed elements and their parents for layout recalculation
            // This ensures parent layouts shrink/expand when children become visible/invisible
            for element_id in changed_element_ids {
                ui.mark_element_needs_layout(element_id);
            }
            for parent_id in parent_ids {
                ui.mark_element_needs_layout(parent_id);
            }
        });
        
        api.print(&format!("✅ Editor mode set to {:?}", mode));
    }
    
    /// Handler for UI Editor button
    pub fn on_ui_editor_pressed(&mut self, api: &mut ScriptApi) {
        self.set_editor_mode(EditorMode::UI, api);
    }
    
    /// Handler for 2D Editor button
    pub fn on_two_d_pressed(&mut self, api: &mut ScriptApi) {
        self.set_editor_mode(EditorMode::TwoD, api);
    }
    
    /// Handler for 3D Editor button
    pub fn on_three_d_pressed(&mut self, api: &mut ScriptApi) {
        self.set_editor_mode(EditorMode::ThreeD, api);
    }
    
    /// Handler for Script Editor button
    pub fn on_script_pressed(&mut self, api: &mut ScriptApi) {
        self.set_editor_mode(EditorMode::Script, api);
    }
    
    /// Legacy function name for backwards compatibility
    pub fn toggle_ui_editor(&mut self, api: &mut ScriptApi) {
        self.on_ui_editor_pressed(api);
    }
    
    // ===== FILE TREE SIGNAL HANDLERS =====
    // The UIListTree component handles all mouse interactions and emits signals
    // We just need to respond to those signals
    
    /// Called when a file tree item is clicked
    pub fn on_file_tree_item_clicked(&mut self, api: &mut ScriptApi) {
        eprintln!("🎯 [Manager] on_file_tree_item_clicked HANDLER CALLED!");
        api.print("📄 File tree item clicked");
        // TODO: Handle file selection (e.g., update inspector)
    }
    
    /// Called when a file tree item is double-clicked
    pub fn on_file_tree_item_double_clicked(&mut self, api: &mut ScriptApi) {
        eprintln!("🎯 [Manager] on_file_tree_item_double_clicked HANDLER CALLED!");
        api.print("📂 File tree item double-clicked");
        // TODO: Handle file opening (e.g., open scene, open script editor, etc.)
    }
    
    /// Called when a file tree item is right-clicked
    pub fn on_file_tree_item_right_clicked(&mut self, api: &mut ScriptApi) {
        eprintln!("🎯 [Manager] on_file_tree_item_right_clicked HANDLER CALLED!");
        api.print("🖱️ File tree item right-clicked");
        
        let Some(menu_id) = self.context_menu_id else { 
            eprintln!("⚠️ [Manager] Context menu ID not set!");
            return 
        };
        let ui_node_id = self.id;
        
        // Show context menu at mouse position
        let mouse_pos = api.Input.Mouse.get_position();
        api.with_ui_node(ui_node_id, |ui| {
            if let Some(UIElement::ContextMenu(menu)) = ui.elements.as_mut().and_then(|e| e.get_mut(&menu_id)) {
                menu.show_at(mouse_pos);
                ui.mark_element_needs_rerender(menu_id);
            }
        });
    }
    
    /// Update the live preview by parsing FUR text from TextEditorContainer
    /// and completely rebuilding the UIPreview panel with the parsed elements
    ///@skip
    fn update_live_preview(&mut self, api: &mut ScriptApi) {
        let ui_node_id = self.id;
        
        // Get the text from TextEditorContainer
        let fur_text = api.with_ui_node(ui_node_id, |ui| {
            if let Some(element) = ui.find_element_by_name("TextEditorContainer") {
                match element {
                    UIElement::TextEdit(text_edit) => Some(text_edit.get_text().to_string()),
                    _ => None,
                }
            } else {
                None
            }
        });
        
        let Some(fur_text) = fur_text else {
            return; // TextEditorContainer not found or not a TextEdit
        };
        
        // Find UIPreview element
        let preview_id = api.with_ui_node(ui_node_id, |ui| {
            ui.find_element_by_name("UIPreview")
                .map(|element| element.get_id())
        });
        
        let Some(preview_id) = preview_id else {
            return; // UIPreview not found
        };
        
        // Completely clear all existing children from UIPreview
        self.clear_preview_children(api, ui_node_id);
        
        // Parse FUR and prefix all IDs with "PREVIEW_" to ensure uniqueness
        let parse_success = self.append_fur_to_ui_with_prefix(api, ui_node_id, &fur_text, Some(preview_id), "PREVIEW_");
        
        // Verify final state after parsing
        api.with_ui_node(ui_node_id, |ui| {
            if let Some(preview_element) = ui.find_element_by_name("UIPreview") {
                let _final_children_count = preview_element.get_children().len();
            }
        });
        
        if !parse_success {
            // Parsing failed or no elements found (e.g., empty text) - this is expected
            // Explicitly verify children are cleared and mark for rerender to show the cleared state
            api.with_ui_node(ui_node_id, |ui| {
                // Verify UIPreview has no children
                if let Some(preview_element) = ui.find_element_by_name("UIPreview") {
                    let children_count = preview_element.get_children().len();
                    if children_count > 0 {
                        // Force clear if somehow children remain
                        if let Some(preview_element_mut) = ui.find_element_by_name_mut("UIPreview") {
                            preview_element_mut.set_children(Vec::new());
                        }
                    }
                }
                // Ensure preview is marked for rerender to show the cleared state
                ui.mark_element_needs_rerender(preview_id);
                ui.mark_element_needs_layout(preview_id);
            });
            // Note: with_ui_node automatically marks the UI node itself for rerender
        }
    }
    
    /// Append FUR elements with a prefix on all IDs to ensure uniqueness
    ///@skip
    fn append_fur_to_ui_with_prefix(
        &mut self,
        api: &mut ScriptApi,
        ui_node_id: Uuid,
        fur_string: &str,
        parent_element_id: Option<Uuid>,
        prefix: &str,
    ) -> bool {
        use perro_core::nodes::ui::parser::FurParser;
        use perro_core::nodes::ui::apply_fur::append_fur_elements_to_ui;
        use perro_core::nodes::ui::fur_ast::{FurElement, FurNode};
        use std::borrow::Cow;
        
        // Helper function to recursively prefix IDs (defined locally to avoid codegen issues)
        fn prefix_element_ids(element: &mut FurElement, prefix: &str) {
            // Prefix this element's ID
            if !element.id.starts_with(prefix) {
                let prefixed_id = format!("{}{}", prefix, element.id);
                element.id = Cow::Owned(prefixed_id);
            }
            
            // Recursively prefix all child element IDs
            for child in &mut element.children {
                if let FurNode::Element(child_el) = child {
                    prefix_element_ids(child_el, prefix);
                }
            }
        }
        
        // Parse the FUR string
        let mut parser: perro_core::nodes::ui::parser::FurParser = match FurParser::new(fur_string) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("❌ Failed to create FUR parser: {}", e);
                return false;
            }
        };
        
        let fur_ast: Vec<perro_core::nodes::ui::fur_ast::FurNode> = match parser.parse() {
            Ok(ast) => ast,
            Err(e) => {
                eprintln!("❌ Failed to parse FUR string: {}", e);
                return false;
            }
        };
        
        // Extract elements from AST and prefix all IDs recursively
        let mut fur_elements: Vec<perro_core::nodes::ui::fur_ast::FurElement> = fur_ast
            .into_iter()
            .filter_map(|node| match node {
                FurNode::Element(mut el) => {
                    prefix_element_ids(&mut el, prefix);
                    Some(el)
                }
                _ => None,
            })
            .collect();
        
        if fur_elements.is_empty() {
            eprintln!("⚠️ No FUR elements found in string");
            return false;
        }
        
        // Append to UI node using mutate_node for proper rerender marking
        api.mutate_node::<perro_core::nodes::ui::ui_node::UINode, _>(ui_node_id, |ui| {
            append_fur_elements_to_ui(ui, &fur_elements, parent_element_id);
        });
        
        true
    }
    
    /// Clear all children from the UIPreview panel (recursively removes all descendants)
    ///@skip
    fn clear_preview_children(&mut self, api: &mut ScriptApi, ui_node_id: Uuid) {
        // First, collect all element IDs and their types before removing them
        let (preview_id, all_descendant_ids, element_types) = api.with_ui_node(ui_node_id, |ui| {
            // First, collect the preview_id and direct children while we have the mutable borrow
            let (preview_id, direct_children) = if let Some(preview_element) = ui.find_element_by_name_mut("UIPreview") {
                (preview_element.get_id(), preview_element.get_children().to_vec())
            } else {
                return (Uuid::nil(), Vec::new(), Vec::new());
            };
            
            // Recursively collect ALL descendant IDs (not just direct children)
            // This ensures we remove nested children from the elements map
            let mut all_descendant_ids = Vec::new();
            let mut element_types = Vec::new();
            if let Some(elements) = &ui.elements {
                let mut to_process = direct_children.clone();
                while let Some(current_id) = to_process.pop() {
                    all_descendant_ids.push(current_id);
                    // Store element type for primitive renderer removal
                    if let Some(element) = elements.get(&current_id) {
                        element_types.push((current_id, element.clone()));
                        for &child_id in element.get_children() {
                            to_process.push(child_id);
                        }
                    }
                }
            }
            
            (preview_id, all_descendant_ids, element_types)
        });
        
        if preview_id.is_nil() {
            return;
        }
        
        // Use the new mark_for_deletion mechanism - this properly handles deletion
        // by removing from primitive renderer cache and then from the elements map
        api.with_ui_node(ui_node_id, |ui| {
            // Mark all direct children (and their descendants) for deletion
            // The renderer will handle removing them from primitive renderer and elements map
            for child_id in &all_descendant_ids {
                ui.mark_for_deletion(*child_id);
            }
            
            // Clear the children list from UIPreview
            if let Some(preview_element) = ui.find_element_by_name_mut("UIPreview") {
                preview_element.set_children(Vec::new());
            }
            
            // Mark preview element for rerender and layout to ensure UI updates
            ui.mark_element_needs_rerender(preview_id);
            ui.mark_element_needs_layout(preview_id);
        });
        // Note: with_ui_node automatically marks the UI node itself for rerender
    }
    
    /// Called when a file tree item is renamed
    /// Params: old_path (String), new_path (String)
    pub fn on_file_tree_item_renamed(&mut self, api: &mut ScriptApi, old_path: String, new_path: String) {
        eprintln!("🎯 [Manager] on_file_tree_item_renamed HANDLER CALLED! old_path='{}' new_path='{}'", old_path, new_path);
        api.print(&format!("🔄 File rename: {} -> {}", old_path, new_path));
        
        // Get UID for old path
        if let Some(uid) = self.get_uid(&old_path) {
            // Update UID registry
            if let Err(e) = self.rename_asset(uid, &new_path) {
                api.print(&format!("❌ Failed to update UID registry: {}", e));
                return;
            }
            
            // Update file tree item's UID mapping (path is already updated by commit_rename)
            let ui_node_id = self.id;
            api.with_ui_node(ui_node_id, |ui| {
                if let Some(UIElement::ListTree(file_tree)) = ui.find_element_by_name_mut("ResourceFileTree") {
                    // Find the item with the new path and verify UID is set
                    for item in file_tree.items.values_mut() {
                        if item.path == new_path {
                            if item.uid.is_none() {
                                item.uid = Some(uid);
                            }
                            break;
                        }
                    }
                }
            });
            
            // Update all scene files on disk
            let resolved_path = if self.current_project_path.starts_with("user://") {
                api.resolve_path(&self.current_project_path).unwrap_or_else(|| self.current_project_path.clone())
            } else {
                self.current_project_path.clone()
            };
            
            let project_res_path = format!("{}/res", resolved_path);
            match self.update_all_scene_files_on_disk(api, &project_res_path, &old_path, &new_path) {
                Ok((scenes_checked, nodes_updated)) => {
                    api.print(&format!("✅ Updated {} node(s) across {} scene(s)", nodes_updated, scenes_checked));
                }
                Err(e) => {
                    api.print(&format!("❌ Failed to update scene files: {}", e));
                }
            }
        } else {
            api.print(&format!("⚠️ File {} not in UID registry, skipping scene updates", old_path));
        }
    }
}

impl Script for ManagerScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        api.print("🎮 Manager script initialized");
        
        // UID registry is initialized as part of ManagerScript struct
        api.print("✅ UID registry ready (editor-specific)");
        
        // Check if we're opening a project (project_path runtime param set by root script)
        if let Some(project_path) = api.project().get_runtime_param("project_path") {
            self.current_project_path = project_path.to_string();
            api.print(&format!("📂 Project loaded: {}", self.current_project_path));
            
            // Resolve the project path to get the actual disk path
            let resolved_project_path = if self.current_project_path.starts_with("user://") {
                api.resolve_path(&self.current_project_path).unwrap_or_else(|| self.current_project_path.clone())
            } else {
                self.current_project_path.clone()
            };
            
            // Build the res folder path (disk path)
            let project_res_path = format!("{}/res", resolved_project_path);
            api.print(&format!("🔍 Scanning project resources at: {}", project_res_path));
            
            // Scan the res folder using Directory API (this will print debug info)
            // scan() returns relative paths, then we add "res://" prefix
            let relative_files = api.Directory.scan(&project_res_path);
            let res_files: Vec<String> = relative_files.iter()
                .map(|f| format!("res://{}", f))
                .collect();
            
            api.print(&format!("✅ Found {} resource files in project", res_files.len()));
            
            // Store the resource files for later use
            self.resource_files = res_files.clone();
            self.resources_scanned = true;
            
            // DON'T initialize file tree here - it will be deleted by FUR loading
            // We'll do it in process() after a few frames
            api.print("🌳 File tree will be initialized after FUR loads...");
        } else {
            api.print("   Press the 'Create Project' button to create a new project");
        }
        
        // Set initial visibility: UI editor hidden, default viewport visible
        api.with_ui_node(self.id, |ui| {
            let mut changed_ids = Vec::new();
            
            if let Some(element) = ui.find_element_by_name_mut("UIEditorContent") {
                element.set_visible(false);
                changed_ids.push(element.get_id());
            }
            if let Some(element) = ui.find_element_by_name_mut("DefaultViewport") {
                element.set_visible(true);
                changed_ids.push(element.get_id());
            }
            
            // Mark only the changed elements and their parent layouts for rerender
            for element_id in changed_ids {
                ui.mark_element_needs_rerender(element_id);
                ui.mark_element_needs_layout(element_id);
                
                // Also mark parent layouts
                if let Some(elements) = &ui.elements {
                    let mut current_id = element_id;
                    while let Some(element) = elements.get(&current_id) {
                        let parent_id = element.get_parent();
                        if parent_id.is_nil() {
                            break;
                        }
                        if let Some(parent) = elements.get(&parent_id) {
                            match parent {
                                UIElement::Layout(_) | UIElement::GridLayout(_) => {
                                    ui.mark_element_needs_layout(parent_id);
                                    break; // Only mark immediate parent layout
                                }
                                _ => {}
                            }
                        }
                        current_id = parent_id;
                    }
                }
            }
        });
        
        // Connect to button signals
        api.print(&format!("🔗 Connecting signals for node ID: {}", self.id));
            api.print("🔗 Connecting RunGame_Pressed signal to run_project...");
            // Connect directly to run_project since on_run_game_pressed isn't in dispatch table
            api.connect_signal("RunGame_Pressed", self.id, "run_project");
            api.print("🔗 Connecting StopGame_Pressed signal to stop_project...");
            // Connect directly to stop_project since on_stop_game_pressed isn't in dispatch table
            api.connect_signal("StopGame_Pressed", self.id, "stop_project");
            
            // Connect editor mode buttons
            api.print("🔗 Connecting UIEditor_Pressed signal to on_ui_editor_pressed...");
            api.connect_signal("UIEditor_Pressed", self.id, "on_ui_editor_pressed");
            api.print("🔗 Connecting two_d_Pressed signal to on_two_d_pressed...");
            api.connect_signal("two_d_Pressed", self.id, "on_two_d_pressed");
            // Note: 3D and Script buttons don't have IDs in the FUR file yet, so we'll need to add them
            // For now, we can connect them if they get IDs later
            
            // Connect file tree signals (emitted by UIListTree component)
            api.print("🔗 Connecting file tree signals...");
            eprintln!("🔗 [Manager] Connecting ResourceFileTree_Clicked to on_file_tree_item_clicked");
            api.connect_signal("ResourceFileTree_Clicked", self.id, "on_file_tree_item_clicked");
            eprintln!("🔗 [Manager] Connecting ResourceFileTree_DoubleClicked to on_file_tree_item_double_clicked");
            api.connect_signal("ResourceFileTree_DoubleClicked", self.id, "on_file_tree_item_double_clicked");
            eprintln!("🔗 [Manager] Connecting ResourceFileTree_RightClicked to on_file_tree_item_right_clicked");
            api.connect_signal("ResourceFileTree_RightClicked", self.id, "on_file_tree_item_right_clicked");
            eprintln!("🔗 [Manager] Connecting ResourceFileTree_Renamed to on_file_tree_item_renamed");
            api.connect_signal("ResourceFileTree_Renamed", self.id, "on_file_tree_item_renamed");
            
            api.print("✅ Signal connections complete");
            eprintln!("✅ [Manager] All signal connections registered");
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        // Initialize file tree after a few frames (to let FUR finish loading)
        if !self.file_tree_initialized && self.frame_count == 3 && !self.current_project_path.is_empty() {
            api.print("🌳 Initializing file tree UI (after FUR load)...");
            self.init_file_tree_ui(api, &self.current_project_path.clone());
        }
        self.frame_count += 1;
        
        // Note: File tree input is now handled by the UIListTree component itself
        // It will emit signals that we respond to via the connected signal handlers
        
        // Check if we need to compile scripts (only on first update, after window is visible)
        self.check_and_compile_if_needed(api);
        
        // Check if game process is still running by checking the process ID
        // Only check every 60 updates to reduce overhead
        if self.game_was_running {
            self.process_check_counter += 1;
            if self.process_check_counter >= 60 {
                self.process_check_counter = 0;
                
                if let Some(process_id_str) = api.project().get_runtime_param("runtime_process_id") {
                    if let Ok(process_id) = process_id_str.parse::<u32>() {
                        // Check if process is still running (platform-specific)
                        let is_running = self.is_process_running(process_id);
                        if !is_running {
                            // Process has exited
                            api.print("⚡ Game process has stopped");
                            self.game_was_running = false;
                            // Clear the runtime params
                            api.project().set_runtime_param("runtime_process_id", "");
                            api.project().set_runtime_param("runtime_process_running", "false");
                        }
                    }
                }
            }
        }
        
        // Check for button press (for now, we'll use a simple key press)
        // In a real UI, this would be connected to a button signal
        if api.Input.get_action("create_project") {
            if !self.create_project_button_pressed {
                self.create_project_button_pressed = true;
                self.create_project(api);
            }
        } else {
            self.create_project_button_pressed = false;
        }

        // R key still works for quick testing, but buttons are preferred
        if api.Input.Keyboard.is_key_pressed("KeyR") {
            if !self.run_project_key_pressed {
                self.run_project_key_pressed = true;
                self.run_project(api);
            }
        } else {
            self.run_project_key_pressed = false;
        }
        
        // Update live preview with debouncing: 0.5 seconds after last text change (only in UI editor mode)
        // Use hash comparison to efficiently detect changes without expensive string comparisons
        if self.current_mode == Some(EditorMode::UI) {
            let delta = api.Time.get_delta();
            self.live_preview_timer += delta;
            
            // Check for changes when timer is near threshold (0.4s+) or just started (<0.05s)
            // This avoids expensive hashing every single frame while still being responsive
            let should_check = self.live_preview_timer < 0.05 || self.live_preview_timer >= 0.4;
            
            if should_check {
                // Hash the text directly without cloning the string first
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let current_hash = api.with_ui_node(self.id, |ui| {
                    if let Some(element) = ui.find_element_by_name("TextEditorContainer") {
                        match element {
                            UIElement::TextEdit(text_edit) => {
                                let text = text_edit.get_text();
                                let mut hasher = DefaultHasher::new();
                                text.hash(&mut hasher);
                                Some(hasher.finish())
                            },
                            _ => None,
                        }
                    } else {
                        None
                    }
                });
                
                if let Some(hash) = current_hash {
                    if hash != self.last_fur_text_hash {
                        // Text changed - reset timer and update cached hash
                        self.last_fur_text_hash = hash;
                        self.live_preview_timer = 0.0;
                    } else if self.live_preview_timer >= 0.5 {
                        // Text hasn't changed for 0.5 seconds - update preview
                        self.live_preview_timer = 0.0;
                        self.update_live_preview(api);
                    }
                }
            }
        }
    }
}


impl ScriptObject for ManagerScript {
    fn set_id(&mut self, id: Uuid) {
        self.id = id;
    }

    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_var(&self, var_id: u64) -> Option<Value> {
        VAR_GET_TABLE.get(&var_id).and_then(|f| f(self))
    }

    fn set_var(&mut self, var_id: u64, val: Value) -> Option<()> {
        VAR_SET_TABLE.get(&var_id).and_then(|f| f(self, val))
    }

    fn apply_exposed(&mut self, hashmap: &HashMap<u64, Value>) {
        for (var_id, val) in hashmap.iter() {
            if let Some(f) = VAR_APPLY_TABLE.get(var_id) {
                f(self, val);
            }
        }
    }

    fn call_function(
        &mut self,
        id: u64,
        api: &mut ScriptApi<'_>,
        params: &[Value],
    ) {
        if let Some(f) = DISPATCH_TABLE.get(&id) {
            f(self, params, api);
        }
    }

    // Attributes

    fn attributes_of(&self, member: &str) -> Vec<String> {
        MEMBER_TO_ATTRIBUTES_MAP
            .get(member)
            .map(|attrs| attrs.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }

    fn members_with(&self, attribute: &str) -> Vec<String> {
        ATTRIBUTE_TO_MEMBERS_MAP
            .get(attribute)
            .map(|members| members.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }

    fn has_attribute(&self, member: &str, attribute: &str) -> bool {
        MEMBER_TO_ATTRIBUTES_MAP
            .get(member)
            .map(|attrs| attrs.iter().any(|a| *a == attribute))
            .unwrap_or(false)
    }
    
    fn script_flags(&self) -> ScriptFlags {
        ScriptFlags::new(3)
    }
}

// =========================== Static PHF Dispatch Tables ===========================

static VAR_GET_TABLE: phf::Map<u64, fn(&ManagerScript) -> Option<Value>> =
    phf::phf_map! {

    };

static VAR_SET_TABLE: phf::Map<u64, fn(&mut ManagerScript, Value) -> Option<()>> =
    phf::phf_map! {

    };

static VAR_APPLY_TABLE: phf::Map<u64, fn(&mut ManagerScript, &Value)> =
    phf::phf_map! {

    };

static DISPATCH_TABLE: phf::Map<
    u64,
    fn(&mut ManagerScript, &[Value], &mut ScriptApi<'_>),
> = phf::phf_map! {
        4508745618743899923u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let path = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.register_asset(&path);
        },
        11892411923517466138u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let path = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.get_uid(&path);
        },
        18257970443803474511u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let uid_opt = params.get(0)
                            .and_then(|v| serde_json::from_value::<Uuid>(v.clone()).ok());
let uid = match uid_opt {
    Some(val) => val,
    None => return, // Skip this function call if deserialization failed
};
            script.get_path(uid);
        },
        16921870607580015404u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let uid_opt = params.get(0)
                            .and_then(|v| serde_json::from_value::<Uuid>(v.clone()).ok());
let uid = match uid_opt {
    Some(val) => val,
    None => return, // Skip this function call if deserialization failed
};
let new_path = params.get(1)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.rename_asset(uid, &new_path);
        },
        1580007559800030707u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let project_res_path = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
let old_path = params.get(1)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
let new_path = params.get(2)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.update_all_scene_files_on_disk(api, &project_res_path, &old_path, &new_path);
        },
        7531284223884451901u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let __path_buf_dir = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| std::path::PathBuf::from(s))
                            .unwrap_or_default();
let dir = __path_buf_dir.as_path();
            script.find_editor_exe(dir);
        },
        11724306497971838314u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let project_path = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.launch_editor_via_cargo(&project_path);
        },
        10586536374005137761u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let __path_buf_exe_path = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| std::path::PathBuf::from(s))
                            .unwrap_or_default();
let exe_path = __path_buf_exe_path.as_path();
let project_path = params.get(1)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.launch_editor_with_project(exe_path, &project_path);
        },
        5104941615788957155u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let pid = params.get(0)
                            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                            .unwrap_or_default() as u32;
            script.is_process_running(pid);
        },
        13924012970878586751u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let pid = params.get(0)
                            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                            .unwrap_or_default() as u32;
            script.kill_process(pid);
        },
        2136372962369645708u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.run_project(api);
        },
        1159066582083645583u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.stop_project(api);
        },
        14983575068207283127u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.create_project(api);
        },
        11543083528011375305u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.check_and_compile_if_needed(api);
        },
        6908127992621367286u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let project_path = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.init_file_tree_ui(api, &project_path);
        },
        18234036162573606832u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let __vec_res_files_opt = params.get(0)
                            .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok());
let __vec_res_files = match __vec_res_files_opt {
    Some(val) => val,
    None => return, // Skip this function call if deserialization failed
};
let res_files = __vec_res_files.as_slice();
            script.add_resource_buttons_to_ui(api, res_files);
        },
        12969723914930156605u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let mode_opt = params.get(0)
                            .and_then(|v| serde_json::from_value::<EditorMode>(v.clone()).ok());
let mode = match mode_opt {
    Some(val) => val,
    None => return, // Skip this function call if deserialization failed
};
            script.set_editor_mode(mode, api);
        },
        16452679290048682748u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.on_ui_editor_pressed(api);
        },
        6271441688664419927u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.on_two_d_pressed(api);
        },
        5297026351155972229u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.on_three_d_pressed(api);
        },
        5085426388671876031u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.on_script_pressed(api);
        },
        11006045986918016856u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.toggle_ui_editor(api);
        },
        5378030252301901168u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.on_file_tree_item_clicked(api);
        },
        5214513999877490216u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.on_file_tree_item_double_clicked(api);
        },
        12456821816054257453u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.on_file_tree_item_right_clicked(api);
        },
        12143212617591553931u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
let old_path = params.get(0)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
let new_path = params.get(1)
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
            script.on_file_tree_item_renamed(api, old_path, new_path);
        },

    };

#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
use serde_json::{Value, json};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use perro_core::prelude::*;
use perro_core::ui_element::{BaseElement, UIElement};
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

/// @PerroScript
pub struct ManagerScript {
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
}

#[unsafe(no_mangle)]
pub extern "C" fn manager_create_script() -> *mut dyn ScriptObject {
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
        current_mode: None, // Start with no mode (default viewport)
    })) as *mut dyn ScriptObject
}

impl ManagerScript {
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
    
    /// Add resource buttons to the Resources panel
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
}

impl Script for ManagerScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        api.print("🎮 Manager script initialized");
        
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
            
            // Add resource buttons to the Resources panel
            // Get the UINode (self.id is the UINode's ID)
            api.print("🎨 Adding resource buttons to UI...");
            self.add_resource_buttons_to_ui(api, &res_files);
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
            api.print("✅ Signal connections complete");
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
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
    }
}
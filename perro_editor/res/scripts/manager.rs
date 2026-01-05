#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
use serde_json::{Value, json};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use perro_core::prelude::*;
use std::path::{Path, PathBuf};
use phf::{phf_map, Map};
use smallvec::{SmallVec, smallvec};

/// @PerroScript
pub struct ManagerScript {
    id: Uuid,
    create_project_button_pressed: bool,
    project_creation_in_progress: bool,
}

#[unsafe(no_mangle)]
pub extern "C" fn manager_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(ManagerScript {
        id: Uuid::nil(),
        create_project_button_pressed: false,
        project_creation_in_progress: false,
    })) as *mut dyn ScriptObject
}

impl ManagerScript {
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

        api.print("✅ Project created successfully!");

        // Switch to editor mode by changing fur_path
        use std::borrow::Cow;
        api.mutate_node::<UINode, _>(self.id, |ui_node| {
            // Clear existing elements so the new fur file will be loaded
            ui_node.elements = None;
            ui_node.root_ids = None;
            ui_node.fur_path = Some(Cow::Borrowed("res://fur/editor.fur"));
        });
        
        api.print("🔄 Switched to editor mode");
        
        // Emit editor_mode signal to trigger repair script
        eprintln!("📢 Emitting 'editor_mode' signal");
        api.emit_signal("editor_mode", &[]);
        eprintln!("✅ Signal emitted");

        self.project_creation_in_progress = false;
    }
}

impl Script for ManagerScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        api.print("🎮 Manager script initialized");
        api.print("   Press the 'Create Project' button to create a new project");
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
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
    }
}
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
pub static MEMBER_TO_ATTRIBUTES_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {
};

static ATTRIBUTE_TO_MEMBERS_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {
};

struct ManagerScript {
    id: Uuid,
    create_project_button_pressed: bool,
    project_creation_in_progress: bool,
}

#[unsafe(no_mangle)]
pub extern "C" fn scripts_manager_rs_create_script() -> *mut dyn ScriptObject {
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
        14983575068207283127u64 => | script: &mut ManagerScript, params: &[Value], api: &mut ScriptApi<'_>| {
            script.create_project(api);
        },

    };

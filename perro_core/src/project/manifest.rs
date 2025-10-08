use std::path::{Path, PathBuf};
use std::io;
use std::collections::HashMap;
use serde::Deserialize;
use crate::asset_io::load_asset;

/// Root project manifest structure
#[derive(Deserialize)]
struct ProjectSettings {
    project: ProjectSection,
    #[serde(default)]
    performance: PerformanceSection,
    #[serde(default)]
    root: RootSection,
    #[serde(default)]
    meta: MetadataSection,
}

/// `[project]` section
#[derive(Deserialize)]
struct ProjectSection {
    name: String,
    version: String,               // NEW: semantic version e.g. "0.3.1"
    main_scene: String,
    #[serde(default)]
    icon: Option<String>,
}

/// `[performance]` section
#[derive(Deserialize, Default)]
struct PerformanceSection {
    #[serde(default = "default_target_fps")]
    target_fps: f32,
    #[serde(default = "default_xps")]
    xps: f32,
}

/// `[root]` section
#[derive(Deserialize, Default)]
struct RootSection {
    #[serde(default)]
    script: Option<String>,  // e.g. "res://start.pup"
}

/// `[metadata]` section - acts as a key-value dictionary
#[derive(Deserialize, Default)]
struct MetadataSection {
    #[serde(flatten)]
    data: HashMap<String, String>,
}

fn default_target_fps() -> f32 { 144.0 }
fn default_xps() -> f32 { 60.0 }

/// Project handle — loaded from project.toml
pub struct Project {
    root: Option<PathBuf>, // only meaningful in disk/dev mode
    settings: ProjectSettings,
    runtime_params: HashMap<String, String>, // new
}


impl Project {
    pub fn load(root: Option<impl AsRef<Path>>) -> io::Result<Self> {
        let bytes = load_asset("project.toml")?;
        let contents = std::str::from_utf8(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let settings: ProjectSettings = toml::from_str(contents)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(Self {
            root: root.map(|r| r.as_ref().to_path_buf()),
            settings,
            runtime_params: HashMap::new(),
        })
    }

    /// Load project from a specific project.toml file path
    pub fn load_from_file<P: AsRef<Path>>(project_toml_path: P) -> io::Result<Self> {
        let contents = std::fs::read_to_string(&project_toml_path)?;
        let settings: ProjectSettings = toml::from_str(&contents)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        // Extract root directory from project.toml path
        let root = project_toml_path.as_ref().parent().map(|p| p.to_path_buf());
        
        Ok(Self {
            root,
            settings,
            runtime_params: HashMap::new(),
        })
    }

    // ========== Getters ==========
    
    pub fn name(&self) -> &str { 
        &self.settings.project.name 
    }
    
    pub fn version(&self) -> &str {
        &self.settings.project.version
    }

    pub fn main_scene(&self) -> &str { 
        &self.settings.project.main_scene 
    }
    
    pub fn icon(&self) -> Option<String> { 
        self.settings.project.icon.clone() 
    }
    
    pub fn root(&self) -> Option<&Path> { 
        self.root.as_deref() 
    }
    
    pub fn target_fps(&self) -> f32 { 
        self.settings.performance.target_fps 
    }
    
    pub fn xps(&self) -> f32 { 
        self.settings.performance.xps 
    }
    
    pub fn root_script(&self) -> Option<&str> {
        self.settings.root.script.as_deref()
    }

    // ========== Generic Metadata ==========
    
    /// Get metadata value by key
    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.settings.meta.data.get(key).map(|s| s.as_str())
    }
    
    /// Get all metadata as a HashMap reference
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.settings.meta.data
    }
    
    /// Check if metadata key exists
    pub fn has_meta(&self, key: &str) -> bool {
        self.settings.meta.data.contains_key(key)
    }

    // ========== Setters ==========
    
    pub fn set_name(&mut self, name: String) {
        self.settings.project.name = name;
    }
    
    pub fn set_version(&mut self, version: String) {
        self.settings.project.version = version;
    }

    pub fn set_main_scene(&mut self, path: String) {
        self.settings.project.main_scene = path;
    }
    
    pub fn set_icon(&mut self, path: Option<String>) {
        self.settings.project.icon = path;
    }
    
    pub fn set_target_fps(&mut self, fps: f32) {
        self.settings.performance.target_fps = fps;
    }
    
    pub fn set_xps(&mut self, xps: f32) {
        self.settings.performance.xps = xps;
    }
    
    pub fn set_root_script(&mut self, script: Option<String>) {
        self.settings.root.script = script;
    }
    
    /// Set metadata value
    pub fn set_meta(&mut self, key: String, value: String) {
        self.settings.meta.data.insert(key, value);
    }
    
    /// Remove metadata key
    pub fn remove_meta(&mut self, key: &str) -> Option<String> {
        self.settings.meta.data.remove(key)
    }

    //Runtime

    pub fn set_runtime_param(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.runtime_params.insert(key.into(), value.into());
    }

    pub fn get_runtime_param(&self, key: &str) -> Option<&str> {
        self.runtime_params.get(key).map(|s| s.as_str())
    }

    pub fn runtime_params(&self) -> &HashMap<String, String> {
        &self.runtime_params
    }
}
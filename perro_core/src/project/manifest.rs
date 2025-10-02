use std::path::{Path, PathBuf};
use std::io;
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
}

/// `[project]` section
#[derive(Deserialize)]
struct ProjectSection {
    name: String,
    version: String,               // NEW: semantic version e.g. "0.3.1"
    main_scene: String,
    #[serde(default)]
    icon_path: Option<String>,
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

fn default_target_fps() -> f32 { 144.0 }
fn default_xps() -> f32 { 60.0 }

/// Project handle — loaded from project.toml
pub struct Project {
    root: Option<PathBuf>, // only meaningful in disk/dev mode
    settings: ProjectSettings,
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
    
    pub fn icon_path(&self) -> Option<String> { 
        self.settings.project.icon_path.clone() 
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
    
    pub fn set_icon_path(&mut self, path: Option<String>) {
        self.settings.project.icon_path = path;
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
}
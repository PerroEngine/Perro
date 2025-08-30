use std::path::{Path, PathBuf};
use std::io;
use serde::Deserialize;

use crate::asset_io::load_asset;

#[derive(Deserialize)]
struct ProjectSettings {
    project: ProjectSection,
    #[serde(default)]
    performance: PerformanceSection,
}

#[derive(Deserialize)]
struct ProjectSection {
    name: String,
    main_scene: String,
}

#[derive(Deserialize, Default)]
struct PerformanceSection {
    #[serde(default = "default_target_fps")]
    target_fps: f32,

    #[serde(default = "default_xps")]
    xps: f32,
}

fn default_target_fps() -> f32 { 144.0 }
fn default_xps() -> f32 { 60.0 }

pub struct Project {
    root: Option<PathBuf>, // only meaningful in Disk mode
    settings: ProjectSettings,
}

impl Project {
    /// Load the project manifest (works in both disk + pak mode)
    pub fn load(root: Option<impl AsRef<Path>>) -> io::Result<Self> {
        // Always load project.toml via asset_io
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

    pub fn name(&self) -> &str {
        &self.settings.project.name
    }

    pub fn main_scene(&self) -> &str {
        &self.settings.project.main_scene
    }

    /// Only valid in dev/editor mode
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Performance getters
    pub fn target_fps(&self) -> f32 {
        self.settings.performance.target_fps
    }

    pub fn xps(&self) -> f32 {
        self.settings.performance.xps
    }
}

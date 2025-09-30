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
}

/// `[project]` section of project.toml
#[derive(Deserialize)]
struct ProjectSection {
    name: String,
    main_scene: String,
    #[serde(default)]
    icon_path: Option<String>,
}

/// `[performance]` section of project.toml
#[derive(Deserialize, Default)]
struct PerformanceSection {
    #[serde(default = "default_target_fps")]
    target_fps: f32,

    #[serde(default = "default_xps")]
    xps: f32,
}

fn default_target_fps() -> f32 {
    144.0
}
fn default_xps() -> f32 {
    60.0
}

/// Project handle — loaded from project.toml
pub struct Project {
    root: Option<PathBuf>, // only meaningful in disk/dev mode
    settings: ProjectSettings,
}

impl Project {
    /// Load the project manifest (works on disk + BRK)
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

    /// Human-readable project name
    pub fn name(&self) -> &str {
        &self.settings.project.name
    }

    /// Path to the main scene
    pub fn main_scene(&self) -> &str {
        &self.settings.project.main_scene
    }

    /// Optional path to an icon (res:// or user:// URI, or None)
    pub fn icon_path(&self) -> Option<String> {
        self.settings.project.icon_path.clone()
    }

    /// Only valid in dev/editor mode for disk backends
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Target FPS from project.toml
    pub fn target_fps(&self) -> f32 {
        self.settings.performance.target_fps
    }

    /// "xps" (updates per second?) from project.toml
    pub fn xps(&self) -> f32 {
        self.settings.performance.xps
    }
}
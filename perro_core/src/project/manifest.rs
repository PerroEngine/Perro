use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use serde::Deserialize;
use crate::asset_io::load_asset;

/// Root project manifest structure
#[derive(Deserialize, Clone)]
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
#[derive(Deserialize, Clone)]
struct ProjectSection {
    name: String,
    version: String,
    main_scene: String,
    #[serde(default)]
    icon: Option<String>,
}

/// `[performance]` section
#[derive(Deserialize, Default, Clone)]
struct PerformanceSection {
    #[serde(default = "default_target_fps")]
    target_fps: f32,
    #[serde(default = "default_xps")]
    xps: f32,
}

/// `[root]` section
#[derive(Deserialize, Default, Clone)]
struct RootSection {
    #[serde(default)]
    script: Option<String>, // e.g. "res://start.pup"
}

/// `[metadata]` section
#[derive(Deserialize, Default, Clone)]
struct MetadataSection {
    #[serde(flatten)]
    data: HashMap<String, String>,
}

// Default constants
fn default_target_fps() -> f32 { 144.0 }
fn default_xps() -> f32 { 60.0 }

/// Project handle — represents either a loaded or statically defined project.
#[derive(Clone)]
pub struct Project {
    root: Option<PathBuf>, // only meaningful in dev/disk mode
    settings: ProjectSettings,
    runtime_params: HashMap<String, String>,
}

impl Project {
    // ======================================================
    // ==================== Constructors =====================
    // ======================================================

    /// Creates a static, embedded project (for compile-time manifests)
    pub fn new_static(
    name: impl Into<String>,
    version: impl Into<String>,
    main_scene: impl Into<String>,
    icon: Option<String>,
    target_fps: f32,
    xps: f32,
    root_script: Option<String>,
    metadata: &phf::Map<&'static str, &'static str>,
) -> Self {
    let mut meta = HashMap::new();
    
    // Copy PHF map entries into HashMap
    for (key, value) in metadata.entries() {
        meta.insert(key.to_string(), value.to_string());
    }

    let settings = ProjectSettings {
        project: ProjectSection {
            name: name.into(),
            version: version.into(),
            main_scene: main_scene.into(),
            icon,
        },
        performance: PerformanceSection { target_fps, xps },
        root: RootSection { script: root_script },
        meta: MetadataSection { data: meta },
    };

    Self {
        root: None,
        settings,
        runtime_params: HashMap::new(),
    }
}

    /// Load project.toml from embedded or disk-based asset system.
    pub fn load(root: Option<impl AsRef<Path>>) -> io::Result<Self> {
        let bytes = load_asset("project.toml")?;
        let contents = std::str::from_utf8(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let settings: ProjectSettings =
            toml::from_str(contents).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(Self {
            root: root.map(|r| r.as_ref().to_path_buf()),
            settings,
            runtime_params: HashMap::new(),
        })
    }

    /// Load project from a specified project.toml file path.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let contents = std::fs::read_to_string(&path)?;
        let settings: ProjectSettings =
            toml::from_str(&contents).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let root = path.as_ref().parent().map(|p| p.to_path_buf());

        Ok(Self {
            root,
            settings,
            runtime_params: HashMap::new(),
        })
    }

    // ======================================================
    // ===================== Getters =========================
    // ======================================================

    #[inline]
    pub fn name(&self) -> &str {
        &self.settings.project.name
    }

    #[inline]
    pub fn version(&self) -> &str {
        &self.settings.project.version
    }

    #[inline]
    pub fn main_scene(&self) -> &str {
        &self.settings.project.main_scene
    }

    #[inline]
    pub fn icon(&self) -> Option<String> {
        self.settings.project.icon.clone()
    }

    #[inline]
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    #[inline]
    pub fn target_fps(&self) -> f32 {
        self.settings.performance.target_fps
    }

    #[inline]
    pub fn xps(&self) -> f32 {
        self.settings.performance.xps
    }

    #[inline]
    pub fn root_script(&self) -> Option<&str> {
        self.settings.root.script.as_deref()
    }

    // ======================================================
    // ================== Metadata Access ====================
    // ======================================================

    #[inline]
    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.settings.meta.data.get(key).map(|s| s.as_str())
    }

    #[inline]
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.settings.meta.data
    }

    #[inline]
    pub fn has_meta(&self, key: &str) -> bool {
        self.settings.meta.data.contains_key(key)
    }

    // ======================================================
    // ===================== Setters =========================
    // ======================================================

    #[inline]
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.settings.project.name = name.into();
    }

    #[inline]
    pub fn set_version(&mut self, version: impl Into<String>) {
        self.settings.project.version = version.into();
    }

    #[inline]
    pub fn set_main_scene(&mut self, path: impl Into<String>) {
        self.settings.project.main_scene = path.into();
    }

    #[inline]
    pub fn set_icon(&mut self, path: Option<String>) {
        self.settings.project.icon = path;
    }

    #[inline]
    pub fn set_target_fps(&mut self, fps: f32) {
        self.settings.performance.target_fps = fps;
    }

    #[inline]
    pub fn set_xps(&mut self, xps: f32) {
        self.settings.performance.xps = xps;
    }

    #[inline]
    pub fn set_root_script(&mut self, script: Option<String>) {
        self.settings.root.script = script;
    }

    #[inline]
    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.settings.meta.data.insert(key.into(), value.into());
    }

    #[inline]
    pub fn remove_meta(&mut self, key: &str) -> Option<String> {
        self.settings.meta.data.remove(key)
    }

    // ======================================================
    // ================= Runtime Params ======================
    // ======================================================

    #[inline]
    pub fn set_runtime_param(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.runtime_params.insert(key.into(), value.into());
    }

    #[inline]
    pub fn get_runtime_param(&self, key: &str) -> Option<&str> {
        self.runtime_params.get(key).map(|s| s.as_str())
    }

    #[inline]
    pub fn runtime_params(&self) -> &HashMap<String, String> {
        &self.runtime_params
    }
}

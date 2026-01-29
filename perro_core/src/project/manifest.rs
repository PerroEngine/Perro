use crate::asset_io::load_asset;
use crate::input::{InputMap, parse_input_source};
use serde::Deserialize;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// Root project manifest structure
#[derive(Deserialize, Clone)]
struct ProjectSettings {
    project: ProjectSection,
    #[serde(default)]
    performance: PerformanceSection,
    #[serde(default)]
    graphics: GraphicsSection,
    #[serde(default)]
    root: RootSection,
    #[serde(default)]
    meta: MetadataSection,
    #[serde(default)]
    input: InputSection,
}

/// `[graphics]` section - virtual resolution and window aspect
#[derive(Deserialize, Default, Clone)]
struct GraphicsSection {
    #[serde(default = "default_virtual_width")]
    virtual_width: f32,
    #[serde(default = "default_virtual_height")]
    virtual_height: f32,
}

fn default_virtual_width() -> f32 {
    1920.0
}
fn default_virtual_height() -> f32 {
    1080.0
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
    #[serde(default = "default_fps_cap")]
    fps_cap: f32,
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

/// `[input]` section - maps action names to input sources
/// Example:
/// [input]
/// jump = ["Space", "Up", "MouseLeft"]
/// fire = ["MouseLeft", "KeyF"]
#[derive(Deserialize, Default, Clone)]
struct InputSection {
    #[serde(flatten)]
    actions: HashMap<String, Vec<String>>,
}

// Default constants
fn default_fps_cap() -> f32 {
    500.0
}
fn default_xps() -> f32 {
    30.0
}

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
        fps_cap: f32,
        xps: f32,
        virtual_width: f32,
        virtual_height: f32,
        root_script: Option<String>,
        metadata: &phf::Map<&'static str, &'static str>,
        input_actions: &phf::Map<&'static str, &'static [&'static str]>,
    ) -> Self {
        let mut meta = HashMap::new();

        // Copy PHF map entries into HashMap
        for (key, value) in metadata.entries() {
            meta.insert(key.to_string(), value.to_string());
        }

        // Copy input actions from PHF map into HashMap
        let mut input_map = HashMap::new();
        for (action_name, sources) in input_actions.entries() {
            input_map.insert(action_name.to_string(), sources.iter().map(|s| s.to_string()).collect());
        }

        let settings = ProjectSettings {
            project: ProjectSection {
                name: name.into(),
                version: version.into(),
                main_scene: main_scene.into(),
                icon,
            },
            performance: PerformanceSection { fps_cap, xps },
            graphics: GraphicsSection {
                virtual_width,
                virtual_height,
            },
            root: RootSection {
                script: root_script,
            },
            meta: MetadataSection { data: meta },
            input: InputSection { actions: input_map },
        };

        Self {
            root: None,
            settings,
            runtime_params: HashMap::new(),
        }
    }

    /// Load project.toml from embedded or disk-based asset system.
    pub fn load(root: Option<impl AsRef<Path>>) -> io::Result<Self> {
        // If a root path is provided, load directly from disk to avoid requiring
        // the global project root to be set (chicken-and-egg problem)
        let bytes = if let Some(root_path) = root.as_ref() {
            let project_toml_path = root_path.as_ref().join("project.toml");
            std::fs::read(&project_toml_path).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Failed to read project.toml at {}: {}", project_toml_path.display(), e),
                )
            })?
        } else {
            // No root provided, use asset system (requires project root to be set)
            load_asset("project.toml")?
        };
        
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
    pub fn fps_cap(&self) -> f32 {
        self.settings.performance.fps_cap
    }

    #[inline]
    pub fn xps(&self) -> f32 {
        self.settings.performance.xps
    }

    #[inline]
    pub fn virtual_width(&self) -> f32 {
        self.settings.graphics.virtual_width
    }

    #[inline]
    pub fn virtual_height(&self) -> f32 {
        self.settings.graphics.virtual_height
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
    pub fn set_fps_cap(&mut self, fps: f32) {
        self.settings.performance.fps_cap = fps;
    }

    #[inline]
    pub fn set_xps(&mut self, xps: f32) {
        self.settings.performance.xps = xps;
    }

    #[inline]
    pub fn set_virtual_width(&mut self, w: f32) {
        self.settings.graphics.virtual_width = w;
    }

    #[inline]
    pub fn set_virtual_height(&mut self, h: f32) {
        self.settings.graphics.virtual_height = h;
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

    // ======================================================
    // ================= Input Mapping ======================
    // ======================================================

    /// Get the input action map parsed from project.toml
    pub fn get_input_map(&self) -> InputMap {
        let mut input_map = InputMap::new();

        for (action_name, sources) in &self.settings.input.actions {
            let mut parsed_sources = Vec::new();
            for source_str in sources {
                if let Some(source) = parse_input_source(source_str) {
                    parsed_sources.push(source);
                } else {
                    eprintln!(
                        "Warning: Unknown input source '{}' for action '{}'",
                        source_str, action_name
                    );
                }
            }
            if !parsed_sources.is_empty() {
                input_map.insert(action_name.clone(), parsed_sources);
            }
        }

        input_map
    }

    /// Get the raw input actions as strings (for serialization)
    pub fn get_input_actions(&self) -> &HashMap<String, Vec<String>> {
        &self.settings.input.actions
    }
}

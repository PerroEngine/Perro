use crate::asset_io::load_asset;
use cow_map::{CowMap, cow_map};
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::input::{InputMap, parse_input_source};

/// Metadata map for compiled projects: static refs, no allocation.
/// Key is `&'static str` so CowMap's PHF-backed get/contains_key/remove work.
pub type MetadataMap = CowMap<&'static str, Cow<'static, str>>;

/// Same shape as MetadataMap; used for runtime params so static project can hold empty ref (const fn).
pub type RuntimeParamsMap = CowMap<&'static str, Cow<'static, str>>;

/// Empty runtime params used by const fn new_static (no allocation).
pub static EMPTY_RUNTIME_PARAMS: RuntimeParamsMap =
    cow_map!(EMPTY_RUNTIME_PARAMS: &'static str, Cow<'static, str> => );

/// Metadata storage: static ref (compiled, no alloc) or owned HashMap (loaded from project.toml).
#[derive(Clone)]
enum MetadataStorage {
    Static(&'static MetadataMap),
    Owned(HashMap<String, String>),
}

/// View over metadata for both static and owned storage (used by `Project::metadata()`).
#[derive(Clone)]
pub enum MetadataRef<'a> {
    Static(&'a MetadataMap),
    Owned(&'a HashMap<String, String>),
}

impl MetadataRef<'_> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        match self {
            MetadataRef::Static(m) => m.is_empty(),
            MetadataRef::Owned(m) => m.is_empty(),
        }
    }

    #[inline]
    pub fn iter(&self) -> Box<dyn Iterator<Item = (&str, &str)> + '_> {
        match self {
            MetadataRef::Static(m) => Box::new(m.iter().map(|(k, v)| (*k, v.as_ref()))),
            MetadataRef::Owned(m) => Box::new(m.iter().map(|(k, v)| (k.as_str(), v.as_str()))),
        }
    }
}

/// View over runtime params for both static and owned storage.
#[derive(Clone)]
pub enum RuntimeParamsRef<'a> {
    Static(&'a RuntimeParamsMap),
    Owned(&'a HashMap<String, String>),
}

impl RuntimeParamsRef<'_> {
    #[inline]
    pub fn get(&self, key: &str) -> Option<&str> {
        match self {
            RuntimeParamsRef::Static(m) => m.get(key).map(Cow::as_ref),
            RuntimeParamsRef::Owned(m) => m.get(key).map(String::as_str),
        }
    }

    #[inline]
    pub fn iter(&self) -> Box<dyn Iterator<Item = (&str, &str)> + '_> {
        match self {
            RuntimeParamsRef::Static(m) => Box::new(m.iter().map(|(k, v)| (*k, v.as_ref()))),
            RuntimeParamsRef::Owned(m) => Box::new(m.iter().map(|(k, v)| (k.as_str(), v.as_str()))),
        }
    }
}

/// Root project manifest structure
#[derive(Deserialize, Clone)]
struct ProjectSettings {
    project: ProjectSection,
    #[serde(default)]
    performance: PerformanceSection,
    #[serde(default)]
    graphics: GraphicsSection,
    #[serde(default)]
    meta: MetadataSection,
    #[serde(default)]
    input: InputSection,
}

/// `[graphics]` section - virtual resolution, window aspect, and MSAA
#[derive(Clone)]
struct GraphicsSection {
    virtual_width: f32,
    virtual_height: f32,
    /// MSAA: "off" (1x), "on" (4x). Cow so static project can use borrowed.
    msaa: Cow<'static, str>,
}

impl Default for GraphicsSection {
    fn default() -> Self {
        Self {
            virtual_width: 1920.0,
            virtual_height: 1080.0,
            msaa: Cow::Borrowed("on"),
        }
    }
}

impl<'de> Deserialize<'de> for GraphicsSection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct GraphicsSectionDe {
            #[serde(default = "default_virtual_width")]
            virtual_width: f32,
            #[serde(default = "default_virtual_height")]
            virtual_height: f32,
            #[serde(default = "default_msaa_str")]
            msaa: String,
        }
        fn default_virtual_width() -> f32 {
            1920.0
        }
        fn default_virtual_height() -> f32 {
            1080.0
        }
        fn default_msaa_str() -> String {
            "on".into()
        }
        let d = GraphicsSectionDe::deserialize(deserializer)?;
        Ok(Self {
            virtual_width: d.virtual_width,
            virtual_height: d.virtual_height,
            msaa: Cow::Owned(d.msaa),
        })
    }
}

/// `[project]` section — Cow so static project can use borrowed strings.
#[derive(Clone)]
struct ProjectSection {
    name: Cow<'static, str>,
    version: Cow<'static, str>,
    main_scene: Cow<'static, str>,
    icon: Option<Cow<'static, str>>,
}

impl<'de> Deserialize<'de> for ProjectSection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ProjectSectionDe {
            name: String,
            version: String,
            main_scene: String,
            #[serde(default)]
            icon: Option<String>,
        }
        let d = ProjectSectionDe::deserialize(deserializer)?;
        Ok(Self {
            name: Cow::Owned(d.name),
            version: Cow::Owned(d.version),
            main_scene: Cow::Owned(d.main_scene),
            icon: d.icon.map(Cow::Owned),
        })
    }
}

/// `[performance]` section
#[derive(Deserialize, Default, Clone)]
struct PerformanceSection {
    #[serde(default = "default_fps_cap")]
    fps_cap: f32,
    #[serde(default = "default_xps")]
    xps: f32,
}

/// `[metadata]` section — Static CowMap when compiled, owned HashMap when loaded (no leak).
#[derive(Clone)]
struct MetadataSection {
    data: MetadataStorage,
}

impl Default for MetadataSection {
    fn default() -> Self {
        Self {
            data: MetadataStorage::Owned(HashMap::new()),
        }
    }
}

impl<'de> Deserialize<'de> for MetadataSection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let m: HashMap<String, String> = HashMap::deserialize(deserializer)?;
        Ok(MetadataSection {
            data: MetadataStorage::Owned(m),
        })
    }
}

/// Runtime params storage: static ref (const fn, no alloc) or owned HashMap (loaded/mutated).
#[derive(Clone)]
enum RuntimeParamsStorage {
    Static(&'static RuntimeParamsMap),
    Owned(HashMap<String, String>),
}

/// Input storage: static PHF ref (compiled, no alloc) or owned HashMap (loaded).
#[derive(Clone)]
enum InputStorage {
    Static(&'static phf::Map<&'static str, &'static [&'static str]>),
    Owned(HashMap<String, Vec<String>>),
}

/// `[input]` section - maps action names to input sources
#[derive(Clone)]
struct InputSection {
    storage: InputStorage,
}

impl Default for InputSection {
    fn default() -> Self {
        Self {
            storage: InputStorage::Owned(HashMap::new()),
        }
    }
}

impl<'de> Deserialize<'de> for InputSection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let m: HashMap<String, Vec<String>> = HashMap::deserialize(deserializer)?;
        Ok(Self {
            storage: InputStorage::Owned(m),
        })
    }
}

// Default constants
fn default_fps_cap() -> f32 {
    500.0
}
fn default_xps() -> f32 {
    60.0
}

/// Project handle — represents either a loaded or statically defined project.
#[derive(Clone)]
pub struct Project {
    root: Option<PathBuf>, // only meaningful in dev/disk mode
    settings: ProjectSettings,
    runtime_params: RuntimeParamsStorage,
}

impl Project {
    // ======================================================
    // ==================== Constructors =====================
    // ======================================================

    /// Creates a static, embedded project (all borrowed; no allocation). Const fn so
    /// `pub static PERRO_PROJECT: Project = Project::new_static(...)` works without Lazy.
    pub const fn new_static(
        name: &'static str,
        version: &'static str,
        main_scene: &'static str,
        icon: Option<&'static str>,
        fps_cap: f32,
        xps: f32,
        virtual_width: f32,
        virtual_height: f32,
        msaa: &'static str,
        metadata: &'static MetadataMap,
        input_actions: &'static phf::Map<&'static str, &'static [&'static str]>,
        runtime_params: &'static RuntimeParamsMap,
    ) -> Self {
        let settings = ProjectSettings {
            project: ProjectSection {
                name: Cow::Borrowed(name),
                version: Cow::Borrowed(version),
                main_scene: Cow::Borrowed(main_scene),
                icon: match icon {
                    Some(s) => Some(Cow::Borrowed(s)),
                    None => None,
                },
            },
            performance: PerformanceSection { fps_cap, xps },
            graphics: GraphicsSection {
                virtual_width,
                virtual_height,
                msaa: Cow::Borrowed(msaa),
            },
            meta: MetadataSection {
                data: MetadataStorage::Static(metadata),
            },
            input: InputSection {
                storage: InputStorage::Static(input_actions),
            },
        };

        Self {
            root: None,
            settings,
            runtime_params: RuntimeParamsStorage::Static(runtime_params),
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
                    format!(
                        "Failed to read project.toml at {}: {}",
                        project_toml_path.display(),
                        e
                    ),
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
            runtime_params: RuntimeParamsStorage::Owned(HashMap::new()),
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
            runtime_params: RuntimeParamsStorage::Owned(HashMap::new()),
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
        self.settings.project.icon.as_ref().map(|c| c.to_string())
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

    /// Raw MSAA string from [graphics]: "off" or "on".
    #[inline]
    pub fn msaa(&self) -> &str {
        self.settings.graphics.msaa.as_ref()
    }

    /// MSAA sample count from [graphics] msaa: "off" => 1, "on" => 4 (case-insensitive).
    #[inline]
    pub fn msaa_samples(&self) -> u32 {
        if self.settings.graphics.msaa.to_lowercase().as_str() == "off" {
            1
        } else {
            4 // "on" or unknown
        }
    }

    // ======================================================
    // ================== Metadata Access ====================
    // ======================================================

    #[inline]
    pub fn get_meta(&self, key: &str) -> Option<&str> {
        match &self.settings.meta.data {
            MetadataStorage::Static(m) => m.get(key).map(Cow::as_ref),
            MetadataStorage::Owned(m) => m.get(key).map(String::as_str),
        }
    }

    #[inline]
    pub fn metadata(&self) -> MetadataRef<'_> {
        match &self.settings.meta.data {
            MetadataStorage::Static(m) => MetadataRef::Static(m),
            MetadataStorage::Owned(m) => MetadataRef::Owned(m),
        }
    }

    #[inline]
    pub fn has_meta(&self, key: &str) -> bool {
        match &self.settings.meta.data {
            MetadataStorage::Static(m) => m.contains_key(key),
            MetadataStorage::Owned(m) => m.contains_key(key),
        }
    }

    // ======================================================
    // ===================== Setters =========================
    // ======================================================

    #[inline]
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.settings.project.name = Cow::Owned(name.into());
    }

    #[inline]
    pub fn set_version(&mut self, version: impl Into<String>) {
        self.settings.project.version = Cow::Owned(version.into());
    }

    #[inline]
    pub fn set_main_scene(&mut self, path: impl Into<String>) {
        self.settings.project.main_scene = Cow::Owned(path.into());
    }

    #[inline]
    pub fn set_icon(&mut self, path: Option<String>) {
        self.settings.project.icon = path.map(Cow::Owned);
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
    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        if let MetadataStorage::Owned(m) = &mut self.settings.meta.data {
            m.insert(key.into(), value.into());
        }
        // Static metadata is immutable at runtime; no-op
    }

    #[inline]
    pub fn remove_meta(&mut self, key: &str) -> Option<String> {
        match &mut self.settings.meta.data {
            MetadataStorage::Static(_) => None,
            MetadataStorage::Owned(m) => m.remove(key),
        }
    }

    // ======================================================
    // ================= Runtime Params ======================
    // ======================================================

    #[inline]
    pub fn set_runtime_param(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        match &mut self.runtime_params {
            RuntimeParamsStorage::Static(_) => {
                let mut hm = HashMap::new();
                hm.insert(key, value);
                self.runtime_params = RuntimeParamsStorage::Owned(hm);
            }
            RuntimeParamsStorage::Owned(hm) => {
                hm.insert(key, value);
            }
        }
    }

    #[inline]
    pub fn get_runtime_param(&self, key: &str) -> Option<&str> {
        match &self.runtime_params {
            RuntimeParamsStorage::Static(m) => m.get(key).map(Cow::as_ref),
            RuntimeParamsStorage::Owned(m) => m.get(key).map(String::as_str),
        }
    }

    #[inline]
    pub fn runtime_params(&self) -> RuntimeParamsRef<'_> {
        match &self.runtime_params {
            RuntimeParamsStorage::Static(m) => RuntimeParamsRef::Static(m),
            RuntimeParamsStorage::Owned(m) => RuntimeParamsRef::Owned(m),
        }
    }

    // ======================================================
    // ================= Input Mapping ======================
    // ======================================================

    /// Get the input action map parsed from project.toml (or static manifest)
    pub fn get_input_map(&self) -> InputMap {
        let mut input_map = InputMap::new();
        match &self.settings.input.storage {
            InputStorage::Static(phf_map) => {
                for (action_name, sources) in phf_map.entries() {
                    let mut parsed_sources = Vec::new();
                    for source_str in *sources {
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
                        input_map.insert((*action_name).to_string(), parsed_sources);
                    }
                }
            }
            InputStorage::Owned(hm) => {
                for (action_name, sources) in hm {
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
            }
        }
        input_map
    }

    /// Get the raw input actions (for compiler codegen; only used on loaded projects).
    pub fn get_input_actions(&self) -> &HashMap<String, Vec<String>> {
        match &self.settings.input.storage {
            InputStorage::Static(_) => {
                panic!("get_input_actions() only supported for loaded projects, not static")
            }
            InputStorage::Owned(hm) => hm,
        }
    }
}

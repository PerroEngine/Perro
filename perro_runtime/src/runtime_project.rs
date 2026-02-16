use std::{collections::BTreeMap, path::PathBuf};
use perro_scene::StaticScene;

pub use perro_project::{
    ProjectConfig as RuntimeProjectConfig, ProjectError as ProjectLoadError, StaticProjectConfig,
    default_project_toml, ensure_project_layout, ensure_project_toml, load_project_toml,
    parse_project_toml,
};

/// Script/provider loading mode used when constructing the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProviderMode {
    Dynamic,
    Static,
}

pub type StaticSceneLookup = fn(&str) -> Option<&'static StaticScene>;

/// Immutable project boot data owned by the runtime.
#[derive(Debug, Clone)]
pub struct RuntimeProject {
    pub name: String,
    pub root: PathBuf,
    pub config: RuntimeProjectConfig,
    pub runtime_params: BTreeMap<String, String>,
    pub static_scene_lookup: Option<StaticSceneLookup>,
    pub brk_bytes: Option<&'static [u8]>,
}

impl RuntimeProject {
    pub fn new(name: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        let name = name.into();
        Self {
            name: name.clone(),
            root: root.into(),
            config: perro_project::ProjectConfig::default_for_name(name),
            runtime_params: BTreeMap::new(),
            static_scene_lookup: None,
            brk_bytes: None,
        }
    }

    pub fn from_static(config: StaticProjectConfig, root: impl Into<PathBuf>) -> Self {
        let config = config.to_runtime();
        Self {
            name: config.name.clone(),
            root: root.into(),
            config,
            runtime_params: BTreeMap::new(),
            static_scene_lookup: None,
            brk_bytes: None,
        }
    }

    pub fn from_project_dir(project_root: impl Into<PathBuf>) -> Result<Self, ProjectLoadError> {
        Self::from_project_dir_with_default_name(project_root, "Perro Project")
    }

    pub fn from_project_dir_with_default_name(
        project_root: impl Into<PathBuf>,
        default_name: &str,
    ) -> Result<Self, ProjectLoadError> {
        let root = project_root.into();
        let _ = default_name;
        let config = perro_project::load_project_toml(&root)?;
        Ok(Self {
            name: config.name.clone(),
            root,
            config,
            runtime_params: BTreeMap::new(),
            static_scene_lookup: None,
            brk_bytes: None,
        })
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.runtime_params.insert(key.into(), value.into());
        self
    }

    pub fn with_static_scene_lookup(mut self, lookup: StaticSceneLookup) -> Self {
        self.static_scene_lookup = Some(lookup);
        self
    }

    pub fn with_brk_bytes(mut self, brk_bytes: &'static [u8]) -> Self {
        self.brk_bytes = Some(brk_bytes);
        self
    }
}

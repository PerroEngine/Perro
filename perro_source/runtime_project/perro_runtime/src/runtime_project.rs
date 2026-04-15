use perro_animation::AnimationClip;
use perro_render_bridge::{Material3D, ParticleProfile3D};
use perro_resource_context::sub_apis::Locale;
use perro_scene::Scene;
use std::{collections::BTreeMap, path::PathBuf};

pub use perro_project::{
    LocalizationConfig, OcclusionCulling, ParticleSimDefault,
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

pub type StaticSceneLookup = fn(&str) -> Option<&'static Scene>;
pub type StaticLocalizationLookup = fn(Locale, u64) -> Option<&'static str>;
pub type StaticMaterialLookup = fn(&str) -> Option<&'static Material3D>;
pub type StaticParticleLookup = fn(&str) -> Option<&'static ParticleProfile3D>;
pub type StaticAnimationLookup = fn(&str) -> Option<&'static AnimationClip>;
pub type StaticSkeletonLookup = fn(&str) -> Option<&'static [u8]>;
pub type StaticAudioLookup = fn(&str) -> Option<&'static [u8]>;
pub type StaticBytesLookup = fn(&str) -> Option<&'static [u8]>;

/// Immutable project boot data owned by the runtime.
#[derive(Debug, Clone)]
pub struct RuntimeProject {
    pub name: String,
    pub root: PathBuf,
    pub config: RuntimeProjectConfig,
    pub runtime_params: BTreeMap<String, String>,
    pub static_scene_lookup: Option<StaticSceneLookup>,
    pub static_localization_lookup: Option<StaticLocalizationLookup>,
    pub static_material_lookup: Option<StaticMaterialLookup>,
    pub static_particle_lookup: Option<StaticParticleLookup>,
    pub static_animation_lookup: Option<StaticAnimationLookup>,
    pub static_mesh_lookup: Option<StaticBytesLookup>,
    pub static_skeleton_lookup: Option<StaticSkeletonLookup>,
    pub static_audio_lookup: Option<StaticAudioLookup>,
    pub static_icon_lookup: Option<StaticBytesLookup>,
    pub perro_assets_bytes: Option<&'static [u8]>,
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
            static_localization_lookup: None,
            static_material_lookup: None,
            static_particle_lookup: None,
            static_animation_lookup: None,
            static_mesh_lookup: None,
            static_skeleton_lookup: None,
            static_audio_lookup: None,
            static_icon_lookup: None,
            perro_assets_bytes: None,
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
            static_localization_lookup: None,
            static_material_lookup: None,
            static_particle_lookup: None,
            static_animation_lookup: None,
            static_mesh_lookup: None,
            static_skeleton_lookup: None,
            static_audio_lookup: None,
            static_icon_lookup: None,
            perro_assets_bytes: None,
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
            static_localization_lookup: None,
            static_material_lookup: None,
            static_particle_lookup: None,
            static_animation_lookup: None,
            static_mesh_lookup: None,
            static_skeleton_lookup: None,
            static_audio_lookup: None,
            static_icon_lookup: None,
            perro_assets_bytes: None,
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

    pub fn with_static_localization_lookup(mut self, lookup: StaticLocalizationLookup) -> Self {
        self.static_localization_lookup = Some(lookup);
        self
    }

    pub fn with_static_material_lookup(mut self, lookup: StaticMaterialLookup) -> Self {
        self.static_material_lookup = Some(lookup);
        self
    }

    pub fn with_static_particle_lookup(mut self, lookup: StaticParticleLookup) -> Self {
        self.static_particle_lookup = Some(lookup);
        self
    }

    pub fn with_static_animation_lookup(mut self, lookup: StaticAnimationLookup) -> Self {
        self.static_animation_lookup = Some(lookup);
        self
    }

    pub fn with_static_mesh_lookup(mut self, lookup: StaticBytesLookup) -> Self {
        self.static_mesh_lookup = Some(lookup);
        self
    }

    pub fn with_static_skeleton_lookup(mut self, lookup: StaticSkeletonLookup) -> Self {
        self.static_skeleton_lookup = Some(lookup);
        self
    }

    pub fn with_static_audio_lookup(mut self, lookup: StaticAudioLookup) -> Self {
        self.static_audio_lookup = Some(lookup);
        self
    }

    pub fn with_static_icon_lookup(mut self, lookup: StaticBytesLookup) -> Self {
        self.static_icon_lookup = Some(lookup);
        self
    }

    pub fn with_perro_assets_bytes(mut self, perro_assets_bytes: &'static [u8]) -> Self {
        self.perro_assets_bytes = Some(perro_assets_bytes);
        self
    }
}

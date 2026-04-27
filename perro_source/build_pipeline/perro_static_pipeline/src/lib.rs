mod animations;
mod audios;
mod collision_trimeshes;
mod error;
mod localizations;
mod materials;
mod meshes;
mod particles;
mod scenes;
mod shaders;
mod skeletons;
mod textures;

pub use animations::generate_static_animations;
pub use audios::generate_static_audios;
pub use collision_trimeshes::generate_static_collision_trimeshes;
pub use error::StaticPipelineError;
pub use localizations::generate_empty_localizations;
pub use localizations::generate_static_localizations;
pub use materials::generate_static_materials;
pub use meshes::generate_static_meshes;
pub use particles::generate_static_particles;
pub use scenes::generate_static_scenes;
pub use shaders::generate_static_shaders;
pub use skeletons::generate_static_skeletons;
pub use textures::generate_static_textures;

use std::{
    collections::HashMap,
    fs,
    sync::{OnceLock, RwLock},
    path::{Path, PathBuf},
};

const PERRO_DIR: &str = ".perro";
const PROJECT_DIR: &str = "project";
const SRC_DIR: &str = "src";
const STATIC_DIR: &str = "static";
const EMBEDDED_DIR: &str = "embedded";
const RES_DIR: &str = "res";

#[derive(Clone, Debug)]
pub struct StaticPipelineOverrides {
    pub res_dir: PathBuf,
    pub static_dir: PathBuf,
    pub embedded_dir: PathBuf,
    pub asset_prefix: String,
}

fn overrides_cell() -> &'static RwLock<Option<StaticPipelineOverrides>> {
    static CELL: OnceLock<RwLock<Option<StaticPipelineOverrides>>> = OnceLock::new();
    CELL.get_or_init(|| RwLock::new(None))
}

fn current_overrides() -> Option<StaticPipelineOverrides> {
    overrides_cell().read().ok().and_then(|v| v.clone())
}

pub fn set_static_pipeline_overrides(overrides: Option<StaticPipelineOverrides>) {
    if let Ok(mut slot) = overrides_cell().write() {
        *slot = overrides;
    }
}

pub(crate) fn static_dir(project_root: &Path) -> PathBuf {
    if let Some(overrides) = current_overrides() {
        return overrides.static_dir;
    }
    project_root
        .join(PERRO_DIR)
        .join(PROJECT_DIR)
        .join(SRC_DIR)
        .join(STATIC_DIR)
}

pub(crate) fn embedded_dir(project_root: &Path) -> PathBuf {
    if let Some(overrides) = current_overrides() {
        return overrides.embedded_dir;
    }
    project_root
        .join(PERRO_DIR)
        .join(PROJECT_DIR)
        .join(EMBEDDED_DIR)
}

pub(crate) fn res_dir(project_root: &Path) -> PathBuf {
    if let Some(overrides) = current_overrides() {
        return overrides.res_dir;
    }
    project_root.join(RES_DIR)
}

pub(crate) fn asset_prefix() -> String {
    current_overrides()
        .map(|overrides| overrides.asset_prefix)
        .unwrap_or_else(|| "res://".to_string())
}

pub(crate) fn is_asset_uri(path: &str) -> bool {
    path.starts_with(&asset_prefix())
}

pub(crate) fn asset_uri(rel: &str) -> String {
    format!("{}{}", asset_prefix(), rel.replace('\\', "/"))
}

pub(crate) fn strip_asset_prefix(path: &str) -> Option<String> {
    path.strip_prefix(&asset_prefix()).map(str::to_string)
}

pub(crate) fn ensure_unique_hashes<'a, I>(kind: &str, paths: I) -> Result<(), StaticPipelineError>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut by_hash = HashMap::<u64, &'a str>::new();
    for path in paths {
        let hash = perro_ids::string_to_u64(path);
        if let Some(prev) = by_hash.insert(hash, path) {
            return Err(StaticPipelineError::SceneParse(format!(
                "{kind} hash collision: `{prev}` + `{path}` => {hash}"
            )));
        }
    }
    Ok(())
}

pub fn write_static_mod_rs(project_root: &Path) -> Result<(), StaticPipelineError> {
    let static_dir = static_dir(project_root);
    fs::create_dir_all(&static_dir)?;
    fs::write(
        static_dir.join("mod.rs"),
        "#![allow(unused_imports)]\n\npub mod scenes;\npub mod materials;\npub mod particles;\npub mod animations;\npub mod meshes;\npub mod collision_trimeshes;\npub mod skeletons;\npub mod textures;\npub mod shaders;\npub mod audios;\npub mod localizations;\n",
    )?;
    Ok(())
}

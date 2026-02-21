mod error;
mod materials;
mod scenes;
mod textures;

pub use error::StaticPipelineError;
pub use materials::generate_static_materials;
pub use scenes::generate_static_scenes;
pub use textures::generate_static_textures;

use std::{
    fs,
    path::{Path, PathBuf},
};

const PERRO_DIR: &str = ".perro";
const PROJECT_DIR: &str = "project";
const SRC_DIR: &str = "src";
const STATIC_DIR: &str = "static";
const EMBEDDED_DIR: &str = "embedded";
const RES_DIR: &str = "res";

pub(crate) fn static_dir(project_root: &Path) -> PathBuf {
    project_root
        .join(PERRO_DIR)
        .join(PROJECT_DIR)
        .join(SRC_DIR)
        .join(STATIC_DIR)
}

pub(crate) fn embedded_dir(project_root: &Path) -> PathBuf {
    project_root
        .join(PERRO_DIR)
        .join(PROJECT_DIR)
        .join(EMBEDDED_DIR)
}

pub(crate) fn res_dir(project_root: &Path) -> PathBuf {
    project_root.join(RES_DIR)
}

pub fn write_static_mod_rs(project_root: &Path) -> Result<(), StaticPipelineError> {
    let static_dir = static_dir(project_root);
    fs::create_dir_all(&static_dir)?;
    fs::write(
        static_dir.join("mod.rs"),
        "pub mod scenes;\npub mod materials;\npub mod textures;\n",
    )?;
    Ok(())
}

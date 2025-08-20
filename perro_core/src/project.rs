use std::path::{Path, PathBuf};
use serde::Deserialize;

#[derive(Deserialize)]
struct ProjectSettings {
    project: ProjectSection,
}

#[derive(Deserialize)]
struct ProjectSection {
    name: String,
    main_scene: String,
}

pub struct Project {
    root: PathBuf,
    settings: ProjectSettings,
}

impl Project {
    pub fn load(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();

        // Try game.toml first, then settings.toml
        let settings_path = root.join("settings.toml");

        let contents = std::fs::read_to_string(&settings_path)
            .unwrap_or_else(|_| panic!("Failed to read {:?}", settings_path));

        let settings: ProjectSettings = toml::from_str(&contents)
            .expect("Failed to parse project manifest");

        Self { root, settings }
    }

    pub fn name(&self) -> &str {
        &self.settings.project.name
    }

    pub fn main_scene(&self) -> &str {
        &self.settings.project.main_scene
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}
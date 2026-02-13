use std::{
    fmt::{Display, Formatter},
    fs,
    path::{Path, PathBuf},
};
use toml::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticProjectConfig {
    pub name: &'static str,
    pub main_scene: &'static str,
    pub icon: &'static str,
    pub virtual_width: u32,
    pub virtual_height: u32,
}

impl StaticProjectConfig {
    pub const fn new(
        name: &'static str,
        main_scene: &'static str,
        icon: &'static str,
        virtual_width: u32,
        virtual_height: u32,
    ) -> Self {
        Self {
            name,
            main_scene,
            icon,
            virtual_width,
            virtual_height,
        }
    }

    pub fn to_runtime(self) -> ProjectConfig {
        ProjectConfig {
            name: self.name.to_string(),
            main_scene: self.main_scene.to_string(),
            icon: self.icon.to_string(),
            virtual_width: self.virtual_width,
            virtual_height: self.virtual_height,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfig {
    pub name: String,
    pub main_scene: String,
    pub icon: String,
    pub virtual_width: u32,
    pub virtual_height: u32,
}

impl ProjectConfig {
    pub fn default_for_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            main_scene: "res://main.scn".to_string(),
            icon: "res://icon.png".to_string(),
            virtual_width: 1920,
            virtual_height: 1080,
        }
    }
}

#[derive(Debug)]
pub enum ProjectError {
    Io(std::io::Error),
    ParseToml(toml::de::Error),
    MissingField(&'static str),
    InvalidField(&'static str, String),
    AlreadyExists(PathBuf),
}

impl Display for ProjectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::ParseToml(err) => write!(f, "{err}"),
            Self::MissingField(field) => write!(f, "missing required field `{field}`"),
            Self::InvalidField(field, reason) => write!(f, "invalid field `{field}`: {reason}"),
            Self::AlreadyExists(path) => {
                write!(f, "project directory already exists: {}", path.display())
            }
        }
    }
}

impl std::error::Error for ProjectError {}

impl From<std::io::Error> for ProjectError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for ProjectError {
    fn from(value: toml::de::Error) -> Self {
        Self::ParseToml(value)
    }
}

pub fn create_new_project(project_root: &Path, project_name: &str) -> Result<(), ProjectError> {
    if project_root.exists() {
        return Err(ProjectError::AlreadyExists(project_root.to_path_buf()));
    }
    ensure_project_layout(project_root)?;
    ensure_project_toml(project_root, project_name)?;
    ensure_project_scaffold(project_root, project_name)?;
    Ok(())
}

pub fn resolve_local_path(input: &str, local_root: &Path) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("local://") {
        let rel = stripped.trim_start_matches('/');
        if rel.is_empty() {
            return local_root.to_path_buf();
        }
        return local_root.join(rel);
    }

    if input.starts_with('/') {
        let rel = input.trim_start_matches('/');
        if rel.is_empty() {
            return local_root.to_path_buf();
        }
        return local_root.join(rel);
    }

    PathBuf::from(input)
}

pub fn bootstrap_project(
    project_root: &Path,
    default_name: &str,
) -> Result<ProjectConfig, ProjectError> {
    ensure_project_layout(project_root)?;
    ensure_project_toml(project_root, default_name)?;
    let config = load_project_toml(project_root)?;
    ensure_project_scaffold(project_root, &config.name)?;
    Ok(config)
}

pub fn ensure_project_layout(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root)?;
    fs::create_dir_all(root.join("res"))?;
    fs::create_dir_all(root.join(".perro"))?;
    Ok(())
}

pub fn ensure_project_scaffold(root: &Path, project_name: &str) -> std::io::Result<()> {
    let res_dir = root.join("res");
    let perro_dir = root.join(".perro");
    let project_crate = perro_dir.join("project");
    let scripts_crate = perro_dir.join("scripts");
    let project_src = project_crate.join("src");
    let scripts_src = scripts_crate.join("src");

    fs::create_dir_all(&res_dir)?;
    fs::create_dir_all(&project_src)?;
    fs::create_dir_all(&scripts_src)?;

    let crate_name = crate_name_from_project_name(project_name);

    write_if_missing(root.join(".gitignore"), &default_gitignore())?;
    write_if_missing(res_dir.join("main.scn"), &default_main_scene())?;
    write_if_missing(
        project_crate.join("Cargo.toml"),
        &default_project_crate_toml(&crate_name),
    )?;
    write_if_missing(
        scripts_crate.join("Cargo.toml"),
        &default_scripts_crate_toml(),
    )?;
    write_if_missing(project_src.join("main.rs"), &default_project_main_rs())?;
    write_if_missing(scripts_src.join("lib.rs"), &default_scripts_lib_rs())?;

    Ok(())
}

pub fn ensure_project_toml(root: &Path, default_name: &str) -> std::io::Result<()> {
    let project_toml = root.join("project.toml");
    if project_toml.exists() {
        return Ok(());
    }
    fs::write(project_toml, default_project_toml(default_name))
}

pub fn default_project_toml(name: &str) -> String {
    format!(
        r#"[project]
name = "{name}"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_resolution = "1920x1080"
"#
    )
}

pub fn load_project_toml(root: &Path) -> Result<ProjectConfig, ProjectError> {
    let project_toml = fs::read_to_string(root.join("project.toml"))?;
    parse_project_toml(&project_toml)
}

pub fn parse_project_toml(contents: &str) -> Result<ProjectConfig, ProjectError> {
    let value: Value = contents.parse::<Value>()?;
    let project_table = value
        .get("project")
        .and_then(Value::as_table)
        .ok_or(ProjectError::MissingField("project"))?;

    let graphics_table = value
        .get("graphics")
        .and_then(Value::as_table)
        .ok_or(ProjectError::MissingField("graphics"))?;

    let name = project_table
        .get("name")
        .and_then(Value::as_str)
        .ok_or(ProjectError::MissingField("project.name"))?
        .to_string();

    let main_scene = project_table
        .get("main_scene")
        .and_then(Value::as_str)
        .ok_or(ProjectError::MissingField("project.main_scene"))?
        .to_string();
    validate_res_path("project.main_scene", &main_scene)?;

    let icon = project_table
        .get("icon")
        .and_then(Value::as_str)
        .unwrap_or("res://icon.png")
        .to_string();
    validate_res_path("project.icon", &icon)?;

    let (virtual_width, virtual_height) = if let Some(raw) = graphics_table
        .get("virtual_resolution")
        .and_then(Value::as_str)
    {
        parse_resolution(raw)?
    } else {
        let w = graphics_table
            .get("virtual_width")
            .and_then(Value::as_integer)
            .ok_or(ProjectError::MissingField("graphics.virtual_width"))?;
        let h = graphics_table
            .get("virtual_height")
            .and_then(Value::as_integer)
            .ok_or(ProjectError::MissingField("graphics.virtual_height"))?;
        (
            u32::try_from(w).map_err(|_| {
                ProjectError::InvalidField(
                    "graphics.virtual_width",
                    "must be a positive integer".to_string(),
                )
            })?,
            u32::try_from(h).map_err(|_| {
                ProjectError::InvalidField(
                    "graphics.virtual_height",
                    "must be a positive integer".to_string(),
                )
            })?,
        )
    };

    if virtual_width == 0 || virtual_height == 0 {
        return Err(ProjectError::InvalidField(
            "graphics.virtual_resolution",
            "resolution values must be greater than 0".to_string(),
        ));
    }

    Ok(ProjectConfig {
        name,
        main_scene,
        icon,
        virtual_width,
        virtual_height,
    })
}

fn validate_res_path(field: &'static str, path: &str) -> Result<(), ProjectError> {
    if path.starts_with("res://") {
        return Ok(());
    }
    Err(ProjectError::InvalidField(
        field,
        "must start with `res://`".to_string(),
    ))
}

fn parse_resolution(raw: &str) -> Result<(u32, u32), ProjectError> {
    let raw = raw.trim().to_ascii_lowercase();
    let (w, h) = raw.split_once('x').ok_or(ProjectError::InvalidField(
        "graphics.virtual_resolution",
        "expected format `WIDTHxHEIGHT`, for example `1920x1080`".to_string(),
    ))?;

    let width = w.parse::<u32>().map_err(|_| {
        ProjectError::InvalidField(
            "graphics.virtual_resolution",
            "invalid width component".to_string(),
        )
    })?;
    let height = h.parse::<u32>().map_err(|_| {
        ProjectError::InvalidField(
            "graphics.virtual_resolution",
            "invalid height component".to_string(),
        )
    })?;

    Ok((width, height))
}

fn write_if_missing(path: PathBuf, contents: &str) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, contents)
}

fn crate_name_from_project_name(project_name: &str) -> String {
    let mut out = String::with_capacity(project_name.len() + 8);
    for c in project_name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    let mut normalized = if trimmed.is_empty() {
        "perro_project".to_string()
    } else {
        trimmed.to_string()
    };
    if normalized
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
    {
        normalized.insert(0, '_');
    }
    normalized
}

fn default_main_scene() -> String {
    r#"@root = main

[main]
name = "World"

[Node2D]
    position = (0, 0)
[/Node2D]
[/main]
"#
    .to_string()
}

fn default_gitignore() -> String {
    r#"target/
/.perro/project/target/
/.perro/scripts/target/
"#
    .to_string()
}

fn default_project_crate_toml(crate_name: &str) -> String {
    format!(
        r#"[workspace]

[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2024"

[dependencies]
perro_ids = "0.1.0"
perro_scripting = "0.1.0"
perro_api = "0.1.0"
perro_core = "0.1.0"
scripts = {{ path = "../scripts" }}
"#
    )
}

fn default_scripts_crate_toml() -> String {
    r#"[workspace]

[package]
name = "scripts"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
perro_ids = "0.1.0"
perro_scripting = "0.1.0"
perro_api = "0.1.0"
perro_core = "0.1.0"
"#
    .to_string()
}

fn default_project_main_rs() -> String {
    r#"fn main() {
    println!("Perro project bootstrap binary");
}
"#
    .to_string()
}

fn default_scripts_lib_rs() -> String {
    r#"#[no_mangle]
pub extern "C" fn perro_scripts_init() {}
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_project_toml_reads_virtual_resolution_string() {
        let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_resolution = "1280x720"
"#;

        let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
        assert_eq!(parsed.name, "Game");
        assert_eq!(parsed.main_scene, "res://main.scn");
        assert_eq!(parsed.icon, "res://icon.png");
        assert_eq!(parsed.virtual_width, 1280);
        assert_eq!(parsed.virtual_height, 720);
    }

    #[test]
    fn parse_project_toml_reads_split_virtual_dimensions() {
        let toml = r#"
[project]
name = "Game"
main_scene = "res://main.scn"
icon = "res://icon.png"

[graphics]
virtual_width = 1920
virtual_height = 1080
"#;

        let parsed = parse_project_toml(toml).expect("failed to parse project.toml");
        assert_eq!(parsed.virtual_width, 1920);
        assert_eq!(parsed.virtual_height, 1080);
    }

    #[test]
    fn parse_project_toml_rejects_non_res_path() {
        let toml = r#"
[project]
name = "Game"
main_scene = "./main.scn"
icon = "res://icon.png"

[graphics]
virtual_resolution = "1920x1080"
"#;

        let err = parse_project_toml(toml).expect_err("expected parse failure");
        assert!(matches!(
            err,
            ProjectError::InvalidField("project.main_scene", _)
        ));
    }

    #[test]
    fn resolve_local_path_maps_slash_to_local_root() {
        let root = PathBuf::from("D:/workspace");
        assert_eq!(
            resolve_local_path("/games/demo", &root),
            PathBuf::from("D:/workspace").join("games").join("demo")
        );
        assert_eq!(resolve_local_path("/", &root), root);
    }

    #[test]
    fn resolve_local_path_supports_local_scheme() {
        let root = PathBuf::from("D:/workspace");
        assert_eq!(
            resolve_local_path("local://games/demo", &root),
            PathBuf::from("D:/workspace").join("games").join("demo")
        );
    }

    #[test]
    fn crate_name_from_project_name_normalizes() {
        assert_eq!(crate_name_from_project_name("My Project!"), "my_project");
        assert_eq!(crate_name_from_project_name("123"), "_123");
    }
}

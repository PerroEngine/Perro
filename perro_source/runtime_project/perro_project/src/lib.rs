use std::{
    collections::BTreeSet,
    fmt::{Display, Formatter},
    fs,
    path::{Path, PathBuf},
};
use toml::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcclusionCulling {
    Cpu,
    Gpu,
    Off,
}

impl OcclusionCulling {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Off => "off",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleSimDefault {
    Cpu,
    GpuVertex,
    GpuCompute,
}

impl ParticleSimDefault {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::GpuVertex => "hybrid",
            Self::GpuCompute => "gpu",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationConfig {
    pub source_csv: String,
    pub source_csv_hash: Option<u64>,
    pub key_column: String,
    pub default_locale: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticProjectConfig {
    pub name: &'static str,
    pub main_scene_hash: u64,
    pub icon_hash: u64,
    pub startup_splash_hash: u64,
    pub virtual_width: u32,
    pub virtual_height: u32,
    pub vsync: bool,
    pub target_fixed_update: Option<f32>,
    pub physics_gravity: f32,
    pub physics_coef: f32,
    pub msaa: bool,
    pub meshlets: bool,
    pub dev_meshlets: bool,
    pub release_meshlets: bool,
    pub meshlet_debug_view: bool,
    pub occlusion_culling: OcclusionCulling,
    pub particle_sim_default: ParticleSimDefault,
    pub localization_source_csv_hash: Option<u64>,
    pub localization_key_column: &'static str,
    pub localization_default_locale: &'static str,
}

impl StaticProjectConfig {
    pub const fn new(
        name: &'static str,
        main_scene_hash: u64,
        icon_hash: u64,
        startup_splash_hash: u64,
        virtual_width: u32,
        virtual_height: u32,
    ) -> Self {
        Self {
            name,
            main_scene_hash,
            icon_hash,
            startup_splash_hash,
            virtual_width,
            virtual_height,
            vsync: false,
            target_fixed_update: Some(60.0),
            physics_gravity: -9.81,
            physics_coef: 1.0,
            msaa: true,
            meshlets: false,
            dev_meshlets: false,
            release_meshlets: true,
            meshlet_debug_view: false,
            occlusion_culling: OcclusionCulling::Gpu,
            particle_sim_default: ParticleSimDefault::Cpu,
            localization_source_csv_hash: None,
            localization_key_column: "key",
            localization_default_locale: "en",
        }
    }

    pub const fn with_vsync(mut self, enabled: bool) -> Self {
        self.vsync = enabled;
        self
    }

    pub const fn with_target_fixed_update(mut self, target_fixed_update: Option<f32>) -> Self {
        self.target_fixed_update = target_fixed_update;
        self
    }

    pub const fn with_physics_gravity(mut self, gravity: f32) -> Self {
        self.physics_gravity = gravity;
        self
    }

    pub const fn with_physics_coef(mut self, coef: f32) -> Self {
        self.physics_coef = coef;
        self
    }

    pub const fn with_msaa(mut self, enabled: bool) -> Self {
        self.msaa = enabled;
        self
    }

    pub const fn with_dev_meshlets(mut self, enabled: bool) -> Self {
        self.dev_meshlets = enabled;
        self
    }

    pub const fn with_meshlets(mut self, enabled: bool) -> Self {
        self.meshlets = enabled;
        self
    }

    pub const fn with_release_meshlets(mut self, enabled: bool) -> Self {
        self.release_meshlets = enabled;
        self
    }

    pub const fn with_meshlet_debug_view(mut self, enabled: bool) -> Self {
        self.meshlet_debug_view = enabled;
        self
    }

    pub const fn with_occlusion_culling(mut self, mode: OcclusionCulling) -> Self {
        self.occlusion_culling = mode;
        self
    }

    pub const fn with_particle_sim_default(mut self, mode: ParticleSimDefault) -> Self {
        self.particle_sim_default = mode;
        self
    }

    pub const fn with_localization_hashed(
        mut self,
        source_csv_hash: u64,
        key_column: &'static str,
        default_locale: &'static str,
    ) -> Self {
        self.localization_source_csv_hash = Some(source_csv_hash);
        self.localization_key_column = key_column;
        self.localization_default_locale = default_locale;
        self
    }

    pub fn to_runtime(self) -> ProjectConfig {
        ProjectConfig {
            name: self.name.to_string(),
            main_scene: self.main_scene_hash.to_string(),
            main_scene_hash: Some(self.main_scene_hash),
            icon: self.icon_hash.to_string(),
            icon_hash: Some(self.icon_hash),
            startup_splash: self.startup_splash_hash.to_string(),
            startup_splash_hash: Some(self.startup_splash_hash),
            virtual_width: self.virtual_width,
            virtual_height: self.virtual_height,
            vsync: self.vsync,
            target_fixed_update: self.target_fixed_update,
            physics_gravity: self.physics_gravity,
            physics_coef: self.physics_coef,
            msaa: self.msaa,
            meshlets: self.meshlets,
            dev_meshlets: self.dev_meshlets,
            release_meshlets: self.release_meshlets,
            meshlet_debug_view: self.meshlet_debug_view,
            occlusion_culling: self.occlusion_culling,
            particle_sim_default: self.particle_sim_default,
            localization: self.localization_source_csv_hash.map(|source_csv_hash| {
                LocalizationConfig {
                    source_csv: source_csv_hash.to_string(),
                    source_csv_hash: Some(source_csv_hash),
                    key_column: self.localization_key_column.to_string(),
                    default_locale: self.localization_default_locale.to_string(),
                }
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectConfig {
    pub name: String,
    pub main_scene: String,
    pub main_scene_hash: Option<u64>,
    pub icon: String,
    pub icon_hash: Option<u64>,
    pub startup_splash: String,
    pub startup_splash_hash: Option<u64>,
    pub virtual_width: u32,
    pub virtual_height: u32,
    pub vsync: bool,
    pub target_fixed_update: Option<f32>,
    pub physics_gravity: f32,
    pub physics_coef: f32,
    pub msaa: bool,
    pub meshlets: bool,
    pub dev_meshlets: bool,
    pub release_meshlets: bool,
    pub meshlet_debug_view: bool,
    pub occlusion_culling: OcclusionCulling,
    pub particle_sim_default: ParticleSimDefault,
    pub localization: Option<LocalizationConfig>,
}

impl ProjectConfig {
    pub fn default_for_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            main_scene: "res://main.scn".to_string(),
            main_scene_hash: None,
            icon: "res://icon.png".to_string(),
            icon_hash: None,
            startup_splash: "res://icon.png".to_string(),
            startup_splash_hash: None,
            virtual_width: 1920,
            virtual_height: 1080,
            vsync: false,
            target_fixed_update: Some(60.0),
            physics_gravity: -9.81,
            physics_coef: 1.0,
            msaa: true,
            meshlets: false,
            dev_meshlets: false,
            release_meshlets: true,
            meshlet_debug_view: false,
            occlusion_culling: OcclusionCulling::Gpu,
            particle_sim_default: ParticleSimDefault::Cpu,
            localization: None,
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
    ensure_source_overrides(project_root)?;
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

    #[cfg(not(target_os = "windows"))]
    if input.starts_with('/') {
        return PathBuf::from(input);
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
    ensure_source_overrides(project_root)?;
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
    let res_scripts_dir = res_dir.join("scripts");
    let perro_dir = root.join(".perro");
    let project_crate = perro_dir.join("project");
    let scripts_crate = perro_dir.join("scripts");
    let dev_runner_crate = perro_dir.join("dev_runner");
    let project_cargo_config = project_crate.join(".cargo");
    let scripts_cargo_config = scripts_crate.join(".cargo");
    let project_src = project_crate.join("src");
    let project_static_src = project_src.join("static");
    let project_embedded = project_crate.join("embedded");
    let scripts_src = scripts_crate.join("src");
    let dev_runner_src = dev_runner_crate.join("src");

    fs::create_dir_all(&res_dir)?;
    fs::create_dir_all(&res_scripts_dir)?;
    fs::create_dir_all(&project_src)?;
    fs::create_dir_all(&project_static_src)?;
    fs::create_dir_all(&project_embedded)?;
    fs::create_dir_all(&project_cargo_config)?;
    fs::create_dir_all(&scripts_src)?;
    fs::create_dir_all(&scripts_cargo_config)?;
    fs::create_dir_all(&dev_runner_src)?;

    let crate_name = crate_name_from_project_name(project_name);
    write_if_missing(root.join(".gitignore"), &default_gitignore())?;
    write_if_missing(root.join("deps.toml"), &default_deps_toml())?;
    write_if_missing(
        root.join("README.md"),
        &default_project_readme_md(project_name),
    )?;
    write_if_missing(res_dir.join("main.scn"), &default_main_scene())?;
    write_if_missing(
        res_scripts_dir.join("script.rs"),
        &default_script_empty_rs(),
    )?;
    write_if_missing(
        project_crate.join("Cargo.toml"),
        &default_project_crate_toml(&crate_name),
    )?;
    write_if_missing(project_crate.join("build.rs"), &default_project_build_rs())?;
    write_if_missing(
        scripts_crate.join("Cargo.toml"),
        &default_scripts_crate_toml(),
    )?;
    write_if_missing(
        project_cargo_config.join("config.toml"),
        &default_project_cargo_config_toml(),
    )?;
    write_if_missing(
        scripts_cargo_config.join("config.toml"),
        &default_scripts_cargo_config_toml(),
    )?;
    write_if_missing(
        dev_runner_crate.join("Cargo.toml"),
        &default_dev_runner_crate_toml(),
    )?;
    write_if_missing(
        project_src.join("main.rs"),
        &default_project_main_rs(project_name),
    )?;
    write_if_missing(project_static_src.join("mod.rs"), &default_static_mod_rs())?;
    write_if_missing(
        project_static_src.join("scenes.rs"),
        &default_static_scenes_rs(),
    )?;
    write_if_missing(
        project_static_src.join("materials.rs"),
        &default_static_materials_rs(),
    )?;
    write_if_missing(
        project_static_src.join("particles.rs"),
        &default_static_particles_rs(),
    )?;
    write_if_missing(
        project_static_src.join("animations.rs"),
        &default_static_animations_rs(),
    )?;
    write_if_missing(
        project_static_src.join("textures.rs"),
        &default_static_textures_rs(),
    )?;
    write_if_missing(
        project_static_src.join("shaders.rs"),
        &default_static_shaders_rs(),
    )?;
    write_if_missing(
        project_static_src.join("meshes.rs"),
        &default_static_meshes_rs(),
    )?;
    write_if_missing(
        project_static_src.join("collision_trimeshes.rs"),
        &default_static_collision_trimeshes_rs(),
    )?;
    write_if_missing(
        project_static_src.join("skeletons.rs"),
        &default_static_skeletons_rs(),
    )?;
    write_if_missing(
        project_static_src.join("audios.rs"),
        &default_static_audios_rs(),
    )?;
    write_if_missing(project_embedded.join("assets.perro"), "")?;
    write_if_missing(scripts_src.join("lib.rs"), &default_scripts_lib_rs())?;
    write_if_missing(
        dev_runner_src.join("main.rs"),
        &default_dev_runner_main_rs(),
    )?;
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
startup_splash = "res://icon.png"

[graphics]
virtual_resolution = "1920x1080"
vsync = false

msaa = true

meshlets = false
dev_meshlets = false
release_meshlets = true
meshlet_debug_view = false

occlusion_culling = "gpu"

particle_sim_default = "gpu"

[runtime]
target_fixed_update = 60

[physics]
gravity = -9.81
coef = 1.0

# Optional CSV localization table.
# Columns example: key,en,es,fr,ja,zh
#
# [localization]
# source = "res://localization.csv"
# key = "key"
# default_locale = "en"
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
    let runtime_table = value.get("runtime").and_then(Value::as_table);
    let physics_table = value.get("physics").and_then(Value::as_table);
    let localization_table = value.get("localization").and_then(Value::as_table);

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

    let startup_splash = project_table
        .get("startup_splash")
        .and_then(Value::as_str)
        .unwrap_or("res://icon.png")
        .to_string();
    validate_res_path("project.startup_splash", &startup_splash)?;

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

    let vsync = parse_bool_with_default(graphics_table, "vsync", false)?;
    reject_target_fps(runtime_table)?;
    let target_fixed_update = parse_target_fixed_update(runtime_table)?;
    let physics_gravity = parse_physics_gravity(physics_table)?;
    let physics_coef = parse_physics_coef(physics_table)?;
    let msaa = parse_bool_with_default(graphics_table, "msaa", true)?;
    let meshlets = parse_bool_with_default(graphics_table, "meshlets", false)?;
    let dev_meshlets = parse_bool_with_default(graphics_table, "dev_meshlets", false)?;
    let release_meshlets = parse_bool_with_default(graphics_table, "release_meshlets", true)?;
    let meshlet_debug_view = parse_bool_with_default(graphics_table, "meshlet_debug_view", false)?;
    let occlusion_culling = parse_occlusion_culling_with_default(
        graphics_table,
        "occlusion_culling",
        OcclusionCulling::Gpu,
    )?;
    let particle_sim_default = parse_particle_sim_default_with_default(
        graphics_table,
        "particle_sim_default",
        ParticleSimDefault::Cpu,
    )?;
    let localization = parse_localization(localization_table)?;

    Ok(ProjectConfig {
        name,
        main_scene,
        main_scene_hash: None,
        icon,
        icon_hash: None,
        startup_splash,
        startup_splash_hash: None,
        virtual_width,
        virtual_height,
        vsync,
        target_fixed_update,
        physics_gravity,
        physics_coef,
        msaa,
        meshlets,
        dev_meshlets,
        release_meshlets,
        meshlet_debug_view,
        occlusion_culling,
        particle_sim_default,
        localization,
    })
}

fn parse_bool_with_default(
    table: &toml::map::Map<String, Value>,
    key: &'static str,
    default: bool,
) -> Result<bool, ProjectError> {
    let Some(value) = table.get(key) else {
        return Ok(default);
    };
    value.as_bool().ok_or_else(|| {
        ProjectError::InvalidField(
            match key {
                "vsync" => "graphics.vsync",
                "msaa" => "graphics.msaa",
                "meshlets" => "graphics.meshlets",
                "dev_meshlets" => "graphics.dev_meshlets",
                "release_meshlets" => "graphics.release_meshlets",
                "meshlet_debug_view" => "graphics.meshlet_debug_view",
                _ => "graphics",
            },
            "must be a boolean".to_string(),
        )
    })
}

fn reject_target_fps(runtime: Option<&toml::map::Map<String, Value>>) -> Result<(), ProjectError> {
    let Some(runtime) = runtime else {
        return Ok(());
    };
    let Some(value) = runtime.get("target_fps") else {
        return Ok(());
    };
    Err(ProjectError::InvalidField(
        "runtime.target_fps",
        format!("target_fps unsupported; remove field (found: {value})"),
    ))
}

fn parse_target_fixed_update(
    runtime: Option<&toml::map::Map<String, Value>>,
) -> Result<Option<f32>, ProjectError> {
    let Some(runtime) = runtime else {
        return Ok(Some(60.0));
    };
    let Some(value) = runtime.get("target_fixed_update") else {
        return Ok(Some(60.0));
    };
    if let Some(num) = value.as_float() {
        if num <= 0.0 || !num.is_finite() {
            return Ok(None);
        }
        return Ok(Some(num as f32));
    }
    if let Some(num) = value.as_integer() {
        if num <= 0 {
            return Ok(None);
        }
        return Ok(Some(num as f32));
    }
    Err(ProjectError::InvalidField(
        "runtime.target_fixed_update",
        "expected a positive number".to_string(),
    ))
}

fn parse_physics_gravity(
    physics: Option<&toml::map::Map<String, Value>>,
) -> Result<f32, ProjectError> {
    let Some(physics) = physics else {
        return Ok(-9.81);
    };
    let Some(value) = physics.get("gravity") else {
        return Ok(-9.81);
    };
    let Some(num) = value
        .as_float()
        .or_else(|| value.as_integer().map(|v| v as f64))
    else {
        return Err(ProjectError::InvalidField(
            "physics.gravity",
            "must be a finite number".to_string(),
        ));
    };
    if !num.is_finite() {
        return Err(ProjectError::InvalidField(
            "physics.gravity",
            "must be a finite number".to_string(),
        ));
    }
    Ok(num as f32)
}

fn parse_physics_coef(
    physics: Option<&toml::map::Map<String, Value>>,
) -> Result<f32, ProjectError> {
    let Some(physics) = physics else {
        return Ok(1.0);
    };
    let Some(value) = physics.get("coef") else {
        return Ok(1.0);
    };
    let Some(num) = value
        .as_float()
        .or_else(|| value.as_integer().map(|v| v as f64))
    else {
        return Err(ProjectError::InvalidField(
            "physics.coef",
            "must be a finite positive number".to_string(),
        ));
    };
    if !num.is_finite() || num <= 0.0 {
        return Err(ProjectError::InvalidField(
            "physics.coef",
            "must be a finite positive number".to_string(),
        ));
    }
    Ok(num as f32)
}
fn parse_occlusion_culling_with_default(
    table: &toml::map::Map<String, Value>,
    key: &'static str,
    default: OcclusionCulling,
) -> Result<OcclusionCulling, ProjectError> {
    let Some(value) = table.get(key) else {
        return Ok(default);
    };
    let Some(raw) = value.as_str() else {
        return Err(ProjectError::InvalidField(
            "graphics.occlusion_culling",
            "must be one of \"cpu\", \"gpu\", \"off\"".to_string(),
        ));
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "cpu" => Ok(OcclusionCulling::Cpu),
        "gpu" => Ok(OcclusionCulling::Gpu),
        "off" => Ok(OcclusionCulling::Off),
        _ => Err(ProjectError::InvalidField(
            "graphics.occlusion_culling",
            "must be one of \"cpu\", \"gpu\", \"off\"".to_string(),
        )),
    }
}

fn parse_particle_sim_default_with_default(
    table: &toml::map::Map<String, Value>,
    key: &'static str,
    default: ParticleSimDefault,
) -> Result<ParticleSimDefault, ProjectError> {
    let Some(value) = table.get(key) else {
        return Ok(default);
    };
    let Some(raw) = value.as_str() else {
        return Err(ProjectError::InvalidField(
            "graphics.particle_sim_default",
            "must be one of \"cpu\", \"hybrid\", \"gpu\"".to_string(),
        ));
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "cpu" => Ok(ParticleSimDefault::Cpu),
        "hybrid" => Ok(ParticleSimDefault::GpuVertex),
        "gpu" => Ok(ParticleSimDefault::GpuCompute),
        _ => Err(ProjectError::InvalidField(
            "graphics.particle_sim_default",
            "must be one of \"cpu\", \"hybrid\", \"gpu\"".to_string(),
        )),
    }
}

fn parse_localization(
    table: Option<&toml::map::Map<String, Value>>,
) -> Result<Option<LocalizationConfig>, ProjectError> {
    let Some(table) = table else {
        return Ok(None);
    };

    let source_csv = table
        .get("source")
        .and_then(Value::as_str)
        .ok_or(ProjectError::MissingField("localization.source"))?
        .trim()
        .to_string();
    validate_res_path("localization.source", &source_csv)?;

    let key_column = table
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or("key")
        .trim()
        .to_string();
    if key_column.is_empty() {
        return Err(ProjectError::InvalidField(
            "localization.key",
            "must not be empty".to_string(),
        ));
    }

    let default_locale = table
        .get("default_locale")
        .and_then(Value::as_str)
        .unwrap_or("en")
        .trim()
        .to_ascii_lowercase();
    if default_locale.is_empty() {
        return Err(ProjectError::InvalidField(
            "localization.default_locale",
            "must not be empty".to_string(),
        ));
    }

    Ok(Some(LocalizationConfig {
        source_csv,
        source_csv_hash: None,
        key_column,
        default_locale,
    }))
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

fn write_if_changed(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == contents
    {
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

[Node3D]
    position = (0, 0, 0)
[/Node3D]
[/main]

[camera]
parent = @root

[Camera3D]
    active = true
    [Node3D]
        position = (0, 0, 8)
    [/Node3D]
[/Camera3D]
[/camera]

[ambient]
parent = @root

[AmbientLight3D]
    color = (1.0, 1.0, 1.0)
    intensity = 0.8
[/AmbientLight3D]
[/ambient]
"#
    .to_string()
}

fn default_project_readme_md(project_name: &str) -> String {
    format!(
        r#"# {project_name}

Welcome to your Perro project. This README is a quick map of how things fit together.

Run `perro check` to sync scripts and get rust-analyzer working.

## Project Layout
- `project.toml` is the project config (main scene, icon, graphics defaults).
- `deps.toml` is optional. Add `[dependencies]` here for extra Rust crates used by scripts.
- `res/` holds your assets, scripts, and scenes. `res://` paths resolve into this folder.
- `res/main.scn` is the default scene because `project.toml` points to it by default.
- `.perro/` contains generated Rust crates (project, scripts, dev runner). You generally don’t touch these.
  - `project/` is the static project crate produced by `perro build`. It bakes assets and links scripts into the final executable.
  - `scripts/` is generated from any `.rs` file under `res/` plus Perro’s internal glue. It gets overwritten on build, so don’t edit it directly.
  - `dev_runner/` is built and run by `perro dev`. It loads the scripts dynamic library in dev mode.
  - Output from `perro build` goes to `.output/` for convenience so you do not have to dig through `target/`.

## Common Commands
- `perro new` creates a project (you just ran this).
- `perro dev` builds scripts and runs the dev runner.
- `perro check` builds scripts only.
- `perro build` builds the full static bundle.
- `perro format` runs rustfmt for all `.rs` scripts under `res/`.
- `perro new_script` creates a new script template in `res/` (use `--res` for subfolders).
- `perro new_scene` creates a new scene template in `res/` (use `--res` and `--template 2D|3D`).
- `perro new_animation` creates a new `.panim` animation clip template (defaults to `res/animations`).
- If you run these inside the project root, you do not need `--path`.

## Scenes And Scripts
- Scenes are `.scn` files under `res/`.
- Script files are Rust files under `res/` (any `.rs` file under `res/`).
- You attach scripts to nodes in scenes using a `script` field with a `res://` path.
- Example:
```text
[Player]
    script = "res://scripts/player.rs"
    [Node2D]
            position = (0, 0)
    [/Node2D]
[/Player]
```
- Use `res://` paths to reference files in res/
- Use `user://` when you want user data, either to read or write. On Windows this resolves to:
  `C:\Users\<You>\AppData\Local\<ProjectName>\data\...`
- You cannot write to res in release

## Documentation
The comprehensive docs live in the main Perro repository on GitHub: `https://github.com/PerroEngine/Perro/blob/main/docs/index.md`
"#
    )
}

pub fn default_script_example_rs() -> String {
    r#"use perro_api::prelude::*;

// Script is authored against a node type. This default template uses Node2D.
type SelfNodeType = Node2D;

// State is data-only. Keeping state separate from behavior makes cross-calls memory safe
// and helps the runtime handle recursion/re-entrancy without borrowing issues.

// Custom structs/enums used in #[State] or methods! typed params/returns should derive Variant.
// Without Variant, runtime variant conversion for those types will not compile.
#[derive(Clone, Copy, Variant)]
struct OrbitGoal {
    axis: Vector3,
}

impl Default for OrbitGoal {
    fn default() -> Self {
        Self {
            axis: Vector3::new(0.0, 1.0, 0.0),
        }
    }
}

#[derive(Clone, Copy, Variant)]
struct MotionSample {
    velocity: Vector3,
    drift: Vector3,
}

impl Default for MotionSample {
    fn default() -> Self {
        Self {
            velocity: Vector3::ZERO,
            drift: Vector3::new(0.0, 0.0, 0.25),
        }
    }
}

// Define state struct with #[State] and use #[default = _] for default values on initialization.
#[State]
struct ExampleState {
    #[default = 5]
    count: i32,

    #[default = OrbitGoal::default()]
    orbit_goal: OrbitGoal,

    #[default = MotionSample::default()]
    motion_sample: MotionSample,
}

const SPEED: f32 = 5.0;

lifecycle!({
    // Lifecycle methods are engine entry points. They are called by the runtime.
    // `ctx` is the main interface into the engine core to access runtime data/scripts and nodes.
    // `res` is resource access (meshes/materials/textures) available at runtime.
    // `ipt` is immutable input state for the current frame (keys pressed/released/down).
    // `self` is the NodeID handle of the node this script is attached to.

    // init is called when the script instance is created. This can be used for one-time setup. State is initialized
    fn on_init(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        node: NodeID,
    ) {
        // with_state! gives read-only state access and returns data from the closure.
        // with_state_mut! gives mutable state access; it can mutate and optionally return data.
        let count = with_state!(ctx, ExampleState, node, |state| {
            state.count
        });
        log_info!(count);
    }

    // on_all_init is called after all scripts have had on_init called. This can be used for setup that requires other scripts to be initialized.
    fn on_all_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self: NodeID,
    ) {}

    // on_update is called every frame. This is where most behavior logic goes.
    fn on_update(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        ipt: &InputContext<'_, IP>,
        node: NodeID,
    ) {
        let dt = delta_time!(ctx);
        let _is_space_down = ipt.Keys().down(KeyCode::Space);

        // Regular Rust method calls are for internal methods.
        self.bump_count(ctx, res, ipt, node);

        // with_node! gives read-only typed node access and returns data from the closure.
        // with_node_mut! gives mutable typed node access; it can mutate and optionally return data.
        // Here we mutate the attached node via `self`.
        with_node_mut!(ctx, SelfNodeType, node, |node| {
            node.position.x += dt * SPEED;
        });

        // You can also pass another NodeID with another node type if that id maps
        // to that type at runtime.
        // Example:
        // with_node_mut!(ctx, MeshInstance3D, enemy, |mesh| { mesh.scale.x += 1.0; });
        //
        // For common hierarchy/identity operations, prefer dedicated helper macros:
        // let name = get_node_name!(ctx, node).unwrap_or_default();
        // let parent = get_node_parent_id!(ctx, node).unwrap_or(NodeID::nil());
        // let children = get_node_children_ids!(ctx, node).unwrap_or_default();
        // let _renamed = set_node_name!(ctx, node, "Player");
        // let _ok = reparent!(ctx, NodeID::new(10), node);
        // let _moved = reparent_multi!(ctx, NodeID::new(10), [NodeID::new(11), NodeID::new(12)]);
        //
        // Script attachment helpers:
        // let _attached = script_attach!(ctx, node, "res://scripts/other.rs");
        // let _detached = script_detach!(ctx, node);
        // `script_attach!` takes a target node id + script path.
        // `script_detach!` takes a node/script id and removes the attached script instance.
        //
        // call_method! can invoke methods through the script interface by member id.
        // Here we call our own script through self for demonstration.
        call_method!(ctx, node, func!("test"), params![7123_i32, "bodsasb"]);
        set_var!(ctx, node, var!("count"), 77_i32.into());
        let remote_count = get_var!(ctx, node, var!("count"));
        log_info!(remote_count);
        // For local/internal behavior and local state, prefer direct methods plus
        // with_state!/with_state_mut! (for example self.bump_count(...)).
        // Read-only helpers (`with_state!`, `with_node!`) are for non-mutable access.
        // Mutable helpers (`with_state_mut!`, `with_node_mut!`) can mutate and
        // can return a value if you need one; ignoring the return is also fine.
        // That is simpler and more performant than call_method!/get_var!/set_var!.

        // Typical NodeID lookup is runtime-dependent. NodeID is a handle, not the node value.
        // if let Some(enemy) = find_node!(ctx, "enemy") {
        //     // Cross-script call on another script instance:
        //     call_method!(ctx, enemy, func!("test"), params![1_i32, "ping"]);
        //
        //     // Mutate enemy node directly if you know its runtime node type:
        //     with_node_mut!(ctx, MeshInstance3D, enemy, |enemy| {
        //         enemy.scale.x += 0.1;
        //     });
        //
        //     // If type is uncertain, check metadata/type first, then branch/match.
        // }
    }

    // on_fixed_update is called on a fixed timestep, independent of frame rate. This is useful for physics and other deterministic updates.
    fn on_fixed_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self: NodeID,
    ) {}

    // on_removal is called when the script instance is removed from a node or the node is removed from the scene. This can be used for cleanup.
    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self: NodeID,
    ) {}
});

methods!({
    // methods! defines callable behavior methods (local or cross-script via call_method!)...
    fn bump_count(&self, ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, _ipt: &InputContext<'_, IP>, node: NodeID) {
        //  Use `with_state_mut!` for mutable access to state
        with_state_mut!(ctx, ExampleState, node, |state| {
            state.count += 1;
        });
    }

    fn test(&self, ctx: &mut RuntimeContext<'_, RT>, res: &ResourceContext<'_, RS>, ipt: &InputContext<'_, IP>, node: NodeID, param1: i32, msg: &str) {
        log_info!(param1);
        log_info!(msg);
        self.bump_count(ctx, res, ipt, node);
    }
});
"#
    .to_string()
}

pub fn default_script_empty_rs() -> String {
    r#"use perro_api::prelude::*;

type SelfNodeType = Node2D;

#[State]
struct EmptyState {}

lifecycle!({
    fn on_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_all_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_fixed_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}
});

methods!({
    fn default_method(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}
});
"#
    .to_string()
}

fn default_gitignore() -> String {
    "target/\n.perro/\n.output/\n".to_string()
}

fn default_deps_toml() -> String {
    r#"# Optional script crate dependencies.
# On `perro check`, `perro dev`, and `perro build`, these are merged into:
#   .perro/scripts/Cargo.toml -> [dependencies]
#
# Keep `perro_api` + `perro_runtime` managed by the engine; they are injected automatically.
#
# Example:
# serde = { version = "1", features = ["derive"] }
# rand = "0.9"
[dependencies]
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
build = "build.rs"

[dependencies]
perro_app = "0.1.0"
perro_api = "0.1.0"
perro_scene = "0.1.0"
perro_render_bridge = "0.1.0"
perro_animation = "0.1.0"
perro_structs = "0.1.0"
scripts = {{ path = "../scripts" }}

[features]
profile = ["perro_app/profile"]

[target.'cfg(target_os = "windows")'.build-dependencies]
winresource = "0.1.20"
toml = "0.8.23"
image = {{ version = "0.25.9", default-features = false, features = ["png", "jpeg", "gif", "bmp", "tga", "webp", "ico"] }}

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "none"
incremental = true
debug = false
debug-assertions = false
overflow-checks = false

[profile.release.package.{crate_name}]
strip = "symbols"
 "#
    )
}

fn default_project_build_rs() -> String {
    r#"#[cfg(target_os = "windows")]
fn main() {
    if let Err(err) = embed_windows_icon() {
        println!("cargo:warning=perro icon embedding skipped: {err}");
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}

#[cfg(target_os = "windows")]
fn embed_windows_icon() -> Result<(), String> {
    use std::{
        env, fs,
        path::{Path, PathBuf},
    };
    use toml::Value;

    fn load_icon_res_path(project_toml: &Path) -> Result<String, String> {
        let src = fs::read_to_string(project_toml)
            .map_err(|e| format!("failed to read {}: {e}", project_toml.display()))?;
        let value: Value = src
            .parse::<Value>()
            .map_err(|e| format!("failed to parse {}: {e}", project_toml.display()))?;
        let icon = value
            .get("project")
            .and_then(Value::as_table)
            .and_then(|project| project.get("icon"))
            .and_then(Value::as_str)
            .unwrap_or("res://icon.png")
            .trim()
            .to_string();
        if !icon.starts_with("res://") {
            return Err(format!("project.icon must start with `res://`, got `{icon}`"));
        }
        Ok(icon)
    }

    fn resolve_res_icon_path(project_root: &Path, icon_res_path: &str) -> PathBuf {
        let rel = icon_res_path
            .trim_start_matches("res://")
            .trim_start_matches('/');
        project_root.join("res").join(rel)
    }

    fn convert_icon_to_ico(source: &Path, out_dir: &Path) -> Result<PathBuf, String> {
        let mut image = image::open(source)
            .map_err(|e| format!("failed to decode icon image `{}`: {e}", source.display()))?;
        let (w, h) = (image.width(), image.height());
        if w > 256 || h > 256 {
            image = image.resize(256, 256, image::imageops::FilterType::Lanczos3);
        }
        let out = out_dir.join("perro_project_icon.ico");
        image
            .save_with_format(&out, image::ImageFormat::Ico)
            .map_err(|e| format!("failed to convert `{}` to ico: {e}", source.display()))?;
        Ok(out)
    }

    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").map_err(|e| format!("CARGO_MANIFEST_DIR missing: {e}"))?,
    );
    let project_root = manifest_dir
        .join("..")
        .join("..")
        .canonicalize()
        .map_err(|e| format!("failed to resolve project root from manifest dir: {e}"))?;
    let project_toml = project_root.join("project.toml");
    let icon_res = load_icon_res_path(&project_toml)?;
    let icon_source = resolve_res_icon_path(&project_root, &icon_res);

    println!("cargo:rerun-if-changed={}", project_toml.display());
    println!("cargo:rerun-if-changed={}", icon_source.display());

    if !icon_source.exists() {
        return Err(format!("icon file not found: {}", icon_source.display()));
    }

    let ext = icon_source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let icon_for_resource = if ext == "ico" {
        icon_source
    } else {
        let out_dir = PathBuf::from(
            env::var("OUT_DIR").map_err(|e| format!("OUT_DIR missing: {e}"))?,
        );
        convert_icon_to_ico(&icon_source, &out_dir)?
    };

    let mut res = winresource::WindowsResource::new();
    res.set_icon(icon_for_resource.to_string_lossy().as_ref());
    res.compile()
        .map_err(|e| format!("failed to compile windows resource icon: {e}"))?;
    Ok(())
}
"#
    .to_string()
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
perro_api = "0.1.0"
perro_runtime = "0.1.0"

[profile.dev]
opt-level = 0
incremental = true
codegen-units = 256
lto = false
debug = false
strip = "none"
overflow-checks = false
panic = "abort"

[profile.dev.package."*"]
opt-level = 2
incremental = true
codegen-units = 64
debug = false
strip = "none"
overflow-checks = false

[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ["cfg(rust_analyzer)"] }
"#
    .to_string()
}

fn default_scripts_cargo_config_toml() -> String {
    r#"[build]
target-dir = "../../target"
"#
    .to_string()
}

fn default_project_cargo_config_toml() -> String {
    r#"[build]
target-dir = "../../target"
"#
    .to_string()
}

fn default_dev_runner_crate_toml() -> String {
    r#"[workspace]

[package]
name = "perro_dev_runner"
version = "0.1.0"
edition = "2024"

[dependencies]
perro_app = "0.1.0"
perro_project = "0.1.0"

[features]
profile = ["perro_app/profile"]
ui_profile = ["perro_app/ui_profile"]
mem_profile = ["perro_app/mem_profile"]

[profile.dev]
opt-level = 1

[profile.dev.package.perro_runtime]
opt-level = 3
debug-assertions = false
overflow-checks = false

[profile.dev.package.perro_app]
opt-level = 3

[profile.dev.package.perro_graphics]
opt-level = 3

[profile.dev.package.rapier2d]
opt-level = 3
debug-assertions = false
overflow-checks = false

[profile.dev.package.rapier3d]
opt-level = 3
debug-assertions = false
overflow-checks = false

[profile.dev.package.parry2d]
opt-level = 3
debug-assertions = false
overflow-checks = false

[profile.dev.package.parry3d]
opt-level = 3
debug-assertions = false
overflow-checks = false

[profile.release]
debug = true
"#
    .to_string()
}

fn default_dev_runner_main_rs() -> String {
    r#"use perro_app::entry;
use perro_project::resolve_local_path;
use std::{env, path::PathBuf};

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    args.get(idx + 1).cloned()
}

fn current_dir_fallback() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let local_root = current_dir_fallback();

    let root = parse_flag_value(&args, "--path")
        .map(|p| resolve_local_path(&p, &local_root))
        .unwrap_or_else(|| local_root.clone());

    let fallback_name =
        parse_flag_value(&args, "--name").unwrap_or_else(|| "Perro Project".to_string());

    entry::run_dev_project_from_path(&root, &fallback_name).unwrap_or_else(|err| {
        panic!(
            "failed to load project at `{}`: {err}",
            root.to_string_lossy()
        )
    });
}
"#
    .to_string()
}

fn rel_path(from: &Path, to: &Path) -> String {
    let from_components: Vec<_> = from.components().collect();
    let to_components: Vec<_> = to.components().collect();
    let common = from_components
        .iter()
        .zip(to_components.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut out = PathBuf::new();
    for _ in common..from_components.len() {
        out.push("..");
    }
    for c in &to_components[common..] {
        out.push(c.as_os_str());
    }
    out.to_string_lossy().replace('\\', "/")
}

fn default_project_main_rs(project_name: &str) -> String {
    r#"#[path = "static/mod.rs"]
mod static_assets;

static PERRO_ASSETS: &[u8] = include_bytes!("../embedded/assets.perro");

fn project_root() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for dir in exe_dir.ancestors() {
                if dir.join("project.toml").exists() {
                    return dir.to_path_buf();
                }
            }
            return exe_dir.to_path_buf();
        }
    }
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    if root.join("project.toml").exists() {
        return root.canonicalize().unwrap_or(root);
    }
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

  fn main() {
      let root = project_root();
      perro_app::entry::run_static_embedded_project(perro_app::entry::StaticEmbeddedProject {
          project: perro_app::entry::StaticEmbeddedProjectInfo {
              project_root: &root,
              project_name: "__PROJECT_NAME__",
              main_scene_hash: 7300106721993353294u64,
              icon_hash: 6859512821849760879u64,
              startup_splash_hash: 6859512821849760879u64,
              virtual_width: 1920,
              virtual_height: 1080,
          },
          graphics: perro_app::entry::StaticEmbeddedGraphicsConfig {
              vsync: false,
              msaa: true,
              meshlets: false,
              dev_meshlets: false,
              release_meshlets: true,
              meshlet_debug_view: false,
              occlusion_culling: perro_app::entry::OcclusionCulling::Gpu,
              particle_sim_default: perro_app::entry::ParticleSimDefault::Cpu,
          },
          runtime: perro_app::entry::StaticEmbeddedRuntimeConfig {
              target_fixed_update: Some(60.0),
              physics_gravity: -9.81,
              physics_coef: 1.0,
          },
          localization: perro_app::entry::StaticEmbeddedLocalizationConfig {
              source_csv_hash: None,
              key_column: "key",
              default_locale: "en",
          },
          assets: perro_app::entry::StaticEmbeddedAssetsConfig {
              perro_assets: PERRO_ASSETS,
              scene_lookup: static_assets::scenes::lookup_scene,
              localization_lookup: static_assets::localizations::lookup_localized_string,
              material_lookup: static_assets::materials::lookup_material,
              particle_lookup: static_assets::particles::lookup_particle,
              animation_lookup: static_assets::animations::lookup_animation,
              mesh_lookup: static_assets::meshes::lookup_mesh,
              collision_trimesh_lookup: static_assets::collision_trimeshes::lookup_collision_trimesh,
              skeleton_lookup: static_assets::skeletons::lookup_skeleton,
              texture_lookup: static_assets::textures::lookup_texture,
              shader_lookup: static_assets::shaders::lookup_shader,
              audio_lookup: static_assets::audios::lookup_audio,
              static_script_registry: Some(scripts::SCRIPT_REGISTRY),
          },
      })
      .expect("failed to run embedded static project");
  }
"#
    .replace("__PROJECT_NAME__", project_name)
}

fn default_static_mod_rs() -> String {
    "#![allow(unused_imports)]\n\npub mod scenes;\npub mod materials;\npub mod particles;\npub mod animations;\npub mod meshes;\npub mod collision_trimeshes;\npub mod skeletons;\npub mod textures;\npub mod shaders;\npub mod audios;\npub mod localizations;\n".to_string()
}

fn default_static_scenes_rs() -> String {
    r#"#![allow(unused_imports)]

use perro_scene::Scene;

const EMPTY_SCENE_NODES: &[perro_scene::SceneNodeEntry] = &[];
const EMPTY_SCENE: Scene = Scene {
    nodes: std::borrow::Cow::Borrowed(EMPTY_SCENE_NODES),
    root: None,
};

pub const fn lookup_scene(_path_hash: u64) -> &'static Scene {
    &EMPTY_SCENE
}
"#
    .to_string()
}

fn default_static_materials_rs() -> String {
    r#"#![allow(unused_imports)]

use perro_render_bridge::{Material3D, StandardMaterial3D};

const EMPTY_MATERIAL: Material3D = Material3D::Standard(StandardMaterial3D::const_default());

pub const fn lookup_material(_path_hash: u64) -> &'static Material3D {
    &EMPTY_MATERIAL
}
"#
    .to_string()
}

fn default_static_particles_rs() -> String {
    r#"#![allow(unused_imports)]

use perro_render_bridge::{ParticlePath3D, ParticleProfile3D};

const EMPTY_PARTICLE: ParticleProfile3D = ParticleProfile3D {
    path: ParticlePath3D::None,
    expr_x_ops: None,
    expr_y_ops: None,
    expr_z_ops: None,
    lifetime_min: 0.6,
    lifetime_max: 1.4,
    speed_min: 1.0,
    speed_max: 3.0,
    spread_radians: core::f32::consts::FRAC_PI_3,
    size: 6.0,
    size_min: 0.65,
    size_max: 1.35,
    force: [0.0, 0.0, 0.0],
    color_start: [1.0, 1.0, 1.0, 1.0],
    color_end: [1.0, 0.4, 0.1, 0.0],
    emissive: [0.0, 0.0, 0.0],
    spin_angular_velocity: 0.0,
};

pub const fn lookup_particle(_path_hash: u64) -> &'static ParticleProfile3D {
    &EMPTY_PARTICLE
}
"#
    .to_string()
}

fn default_static_animations_rs() -> String {
    r#"#![allow(unused_imports)]

use perro_animation::AnimationClip;

const EMPTY_ANIMATION_CLIP: AnimationClip = AnimationClip {
    name: std::borrow::Cow::Borrowed(""),
    fps: 0.0,
    total_frames: 0,
    objects: std::borrow::Cow::Borrowed(&[]),
    object_tracks: std::borrow::Cow::Borrowed(&[]),
    frame_events: std::borrow::Cow::Borrowed(&[]),
};

pub const fn lookup_animation(_path_hash: u64) -> &'static AnimationClip {
    &EMPTY_ANIMATION_CLIP
}
"#
    .to_string()
}

fn default_static_textures_rs() -> String {
    r#"#![allow(unused_imports)]

pub const fn lookup_texture(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_static_shaders_rs() -> String {
    r#"#![allow(unused_imports)]

pub const fn lookup_shader(_path_hash: u64) -> &'static str {
    ""
}
"#
    .to_string()
}

fn default_static_meshes_rs() -> String {
    r#"#![allow(unused_imports)]
#![allow(dead_code)]

pub const fn lookup_mesh(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_static_collision_trimeshes_rs() -> String {
    r#"#![allow(unused_imports)]

pub const fn lookup_collision_trimesh(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_static_skeletons_rs() -> String {
    r#"#![allow(unused_imports)]
#![allow(dead_code)]

pub const fn lookup_skeleton(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_static_audios_rs() -> String {
    r#"#![allow(unused_imports)]

pub const fn lookup_audio(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_scripts_lib_rs() -> String {
    r#"use perro_runtime::{Runtime, RuntimeInputApi, RuntimeResourceApi};
use perro_api::scripting::ScriptConstructor;

pub static SCRIPT_REGISTRY: &[(u64, ScriptConstructor<Runtime, RuntimeResourceApi, RuntimeInputApi>)] = &[];

#[unsafe(no_mangle)]
pub extern "C" fn perro_scripts_init() {}
"#
    .to_string()
}

pub fn ensure_source_overrides(project_root: &Path) -> std::io::Result<()> {
    let project_manifest = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let project_build_script = project_root.join(".perro").join("project").join("build.rs");
    let project_cargo_config = project_root
        .join(".perro")
        .join("project")
        .join(".cargo")
        .join("config.toml");
    let scripts_manifest = project_root
        .join(".perro")
        .join("scripts")
        .join("Cargo.toml");
    let dev_runner_manifest = project_root
        .join(".perro")
        .join("dev_runner")
        .join("Cargo.toml");
    let dev_runner_main = project_root
        .join(".perro")
        .join("dev_runner")
        .join("src")
        .join("main.rs");
    let scripts_cargo_config = project_root
        .join(".perro")
        .join("scripts")
        .join(".cargo")
        .join("config.toml");
    ensure_project_build_script(&project_build_script)?;
    ensure_project_target_dir_config(&project_cargo_config)?;
    ensure_project_manifest_deps(&project_manifest)?;
    ensure_project_manifest_icon_build_support(&project_manifest)?;
    ensure_project_manifest_features(&project_manifest)?;
    ensure_scripts_manifest_deps(&scripts_manifest)?;
    ensure_scripts_manifest_user_deps(project_root, &scripts_manifest)?;
    ensure_dev_runner_source_sync(&dev_runner_manifest, &dev_runner_main)?;
    ensure_dev_runner_manifest_deps(&dev_runner_manifest)?;
    ensure_dev_runner_manifest_features(&dev_runner_manifest)?;
    ensure_dev_runner_manifest_profile_debug(&dev_runner_manifest)?;
    ensure_scripts_manifest_rust_analyzer_cfg(&scripts_manifest)?;
    ensure_patch_block_in_manifest(&project_manifest)?;
    ensure_patch_block_in_manifest(&scripts_manifest)?;
    ensure_patch_block_in_manifest(&dev_runner_manifest)?;
    ensure_scripts_target_dir_config(&scripts_cargo_config)?;
    Ok(())
}

fn ensure_dev_runner_source_sync(manifest_path: &Path, main_rs_path: &Path) -> std::io::Result<()> {
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = main_rs_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_if_changed(manifest_path, &default_dev_runner_crate_toml())?;
    write_if_changed(main_rs_path, &default_dev_runner_main_rs())?;
    Ok(())
}

fn ensure_project_build_script(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        let src = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let old = r#"    fn convert_icon_to_ico(source: &Path, out_dir: &Path) -> Result<PathBuf, String> {
        let image = image::open(source)
            .map_err(|e| format!("failed to decode icon image `{}`: {e}", source.display()))?;
        let out = out_dir.join("perro_project_icon.ico");
        image
            .save_with_format(&out, image::ImageFormat::Ico)
            .map_err(|e| format!("failed to convert `{}` to ico: {e}", source.display()))?;
        Ok(out)
    }"#;
        let new = r#"    fn convert_icon_to_ico(source: &Path, out_dir: &Path) -> Result<PathBuf, String> {
        let mut image = image::open(source)
            .map_err(|e| format!("failed to decode icon image `{}`: {e}", source.display()))?;
        let (w, h) = (image.width(), image.height());
        if w > 256 || h > 256 {
            image = image.resize(256, 256, image::imageops::FilterType::Lanczos3);
        }
        let out = out_dir.join("perro_project_icon.ico");
        image
            .save_with_format(&out, image::ImageFormat::Ico)
            .map_err(|e| format!("failed to convert `{}` to ico: {e}", source.display()))?;
        Ok(out)
    }"#;
        if src.contains(old) {
            fs::write(path, src.replace(old, new))?;
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_project_build_rs())
}

fn ensure_scripts_manifest_user_deps(
    project_root: &Path,
    scripts_manifest: &Path,
) -> std::io::Result<()> {
    let deps_toml = project_root.join("deps.toml");
    if !deps_toml.exists() || !scripts_manifest.exists() {
        return Ok(());
    }

    let deps_src = fs::read_to_string(&deps_toml)?;
    let deps_value = deps_src.parse::<Value>().map_err(|err| {
        std::io::Error::other(format!("failed to parse {}: {err}", deps_toml.display()))
    })?;
    let Some(extra_deps) = deps_value.get("dependencies").and_then(Value::as_table) else {
        return Ok(());
    };

    let scripts_src = fs::read_to_string(scripts_manifest)?;
    let Ok(mut scripts_value) = scripts_src.parse::<Value>() else {
        return Ok(());
    };
    let Some(scripts_root) = scripts_value.as_table_mut() else {
        return Ok(());
    };
    let scripts_deps = scripts_root
        .entry("dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(scripts_deps_table) = scripts_deps.as_table_mut() else {
        return Ok(());
    };

    let mut desired = toml::value::Table::new();
    for (name, spec) in extra_deps {
        if name != "perro_api" && name != "perro_runtime" {
            desired.insert(name.clone(), spec.clone());
        }
    }

    let before_len = scripts_deps_table.len();
    let mut changed = false;
    scripts_deps_table.retain(|name, _| {
        name == "perro_api" || name == "perro_runtime" || desired.contains_key(name)
    });
    if scripts_deps_table.len() != before_len {
        changed = true;
    }
    for (name, spec) in &desired {
        if scripts_deps_table.get(name) != Some(spec) {
            scripts_deps_table.insert(name.clone(), spec.clone());
            changed = true;
        }
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&scripts_value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(scripts_manifest, rendered)
}

fn ensure_scripts_target_dir_config(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_scripts_cargo_config_toml())
}

fn ensure_project_target_dir_config(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_project_cargo_config_toml())
}

fn ensure_project_manifest_deps(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let deps = root
        .entry("dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(deps_table) = deps.as_table_mut() else {
        return Ok(());
    };

    let mut changed = false;

    if !deps_table.contains_key("perro_api") {
        deps_table.insert("perro_api".to_string(), Value::String("0.1.0".to_string()));
        changed = true;
    }
    if !deps_table.contains_key("perro_runtime") {
        deps_table.insert(
            "perro_runtime".to_string(),
            Value::String("0.1.0".to_string()),
        );
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_project_manifest_features(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let features = root
        .entry("features")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(features_table) = features.as_table_mut() else {
        return Ok(());
    };

    let mut changed = false;

    if !features_table.contains_key("profile") {
        features_table.insert(
            "profile".to_string(),
            Value::Array(vec![Value::String("perro_app/profile".to_string())]),
        );
        changed = true;
    }
    if !features_table.contains_key("mem_profile") {
        features_table.insert(
            "mem_profile".to_string(),
            Value::Array(vec![Value::String("perro_app/mem_profile".to_string())]),
        );
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_project_manifest_icon_build_support(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let mut changed = false;

    let package = root
        .entry("package")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(package_table) = package.as_table_mut() else {
        return Ok(());
    };
    if package_table.get("build").and_then(Value::as_str) != Some("build.rs") {
        package_table.insert("build".to_string(), Value::String("build.rs".to_string()));
        changed = true;
    }

    let target = root
        .entry("target")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(target_table) = target.as_table_mut() else {
        return Ok(());
    };
    let windows_key = "cfg(target_os = \"windows\")".to_string();
    let windows_target = target_table
        .entry(windows_key)
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(windows_target_table) = windows_target.as_table_mut() else {
        return Ok(());
    };
    let build_deps = windows_target_table
        .entry("build-dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(build_deps_table) = build_deps.as_table_mut() else {
        return Ok(());
    };

    if !build_deps_table.contains_key("winresource") {
        build_deps_table.insert(
            "winresource".to_string(),
            Value::String("0.1.20".to_string()),
        );
        changed = true;
    }
    if build_deps_table.get("toml").and_then(Value::as_str) != Some("0.8.23") {
        build_deps_table.insert("toml".to_string(), Value::String("0.8.23".to_string()));
        changed = true;
    }
    if !build_deps_table.contains_key("image") {
        let mut image = toml::value::Table::new();
        image.insert("version".to_string(), Value::String("0.25.9".to_string()));
        image.insert("default-features".to_string(), Value::Boolean(false));
        image.insert(
            "features".to_string(),
            Value::Array(vec![
                Value::String("png".to_string()),
                Value::String("jpeg".to_string()),
                Value::String("gif".to_string()),
                Value::String("bmp".to_string()),
                Value::String("tga".to_string()),
                Value::String("webp".to_string()),
                Value::String("ico".to_string()),
            ]),
        );
        build_deps_table.insert("image".to_string(), Value::Table(image));
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_scripts_manifest_deps(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let deps = root
        .entry("dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(deps_table) = deps.as_table_mut() else {
        return Ok(());
    };

    let mut changed = false;

    if !deps_table.contains_key("perro_api") {
        deps_table.insert("perro_api".to_string(), Value::String("0.1.0".to_string()));
        changed = true;
    }
    if !deps_table.contains_key("perro_runtime") {
        deps_table.insert(
            "perro_runtime".to_string(),
            Value::String("0.1.0".to_string()),
        );
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_dev_runner_manifest_deps(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let deps = root
        .entry("dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(deps_table) = deps.as_table_mut() else {
        return Ok(());
    };

    let mut changed = false;

    if !deps_table.contains_key("perro_app") {
        deps_table.insert("perro_app".to_string(), Value::String("0.1.0".to_string()));
        changed = true;
    }
    if !deps_table.contains_key("perro_project") {
        deps_table.insert(
            "perro_project".to_string(),
            Value::String("0.1.0".to_string()),
        );
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_dev_runner_manifest_features(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let features = root
        .entry("features")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(features_table) = features.as_table_mut() else {
        return Ok(());
    };

    let mut changed = false;

    if !features_table.contains_key("profile") {
        features_table.insert(
            "profile".to_string(),
            Value::Array(vec![Value::String("perro_app/profile".to_string())]),
        );
        changed = true;
    }
    if !features_table.contains_key("ui_profile") {
        features_table.insert(
            "ui_profile".to_string(),
            Value::Array(vec![Value::String("perro_app/ui_profile".to_string())]),
        );
        changed = true;
    }
    if !features_table.contains_key("mem_profile") {
        features_table.insert(
            "mem_profile".to_string(),
            Value::Array(vec![Value::String("perro_app/mem_profile".to_string())]),
        );
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_dev_runner_manifest_profile_debug(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let profile = root
        .entry("profile")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(profile_table) = profile.as_table_mut() else {
        return Ok(());
    };
    let mut changed = false;

    {
        let release = profile_table
            .entry("release")
            .or_insert_with(|| Value::Table(Default::default()));
        let Some(release_table) = release.as_table_mut() else {
            return Ok(());
        };
        if !release_table.contains_key("debug") {
            release_table.insert("debug".to_string(), Value::Boolean(true));
            changed = true;
        }
    }

    {
        let dev = profile_table
            .entry("dev")
            .or_insert_with(|| Value::Table(Default::default()));
        let Some(dev_table) = dev.as_table_mut() else {
            return Ok(());
        };
        if !dev_table.contains_key("opt-level") {
            dev_table.insert("opt-level".to_string(), Value::Integer(1));
            changed = true;
        }

        let dev_package = dev_table
            .entry("package")
            .or_insert_with(|| Value::Table(Default::default()));
        let Some(dev_package_table) = dev_package.as_table_mut() else {
            return Ok(());
        };
        changed |= ensure_dev_package_opt_level(dev_package_table, "perro_runtime");
        changed |= ensure_dev_package_opt_level(dev_package_table, "perro_app");
        changed |= ensure_dev_package_opt_level(dev_package_table, "perro_graphics");
        changed |= ensure_dev_package_opt_level(dev_package_table, "rapier2d");
        changed |= ensure_dev_package_opt_level(dev_package_table, "rapier3d");
        changed |= ensure_dev_package_opt_level(dev_package_table, "parry2d");
        changed |= ensure_dev_package_opt_level(dev_package_table, "parry3d");
        changed |= ensure_dev_package_fast_checks(dev_package_table, "perro_runtime");
        changed |= ensure_dev_package_fast_checks(dev_package_table, "rapier2d");
        changed |= ensure_dev_package_fast_checks(dev_package_table, "rapier3d");
        changed |= ensure_dev_package_fast_checks(dev_package_table, "parry2d");
        changed |= ensure_dev_package_fast_checks(dev_package_table, "parry3d");
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_dev_package_opt_level(
    dev_package_table: &mut toml::map::Map<String, Value>,
    crate_name: &str,
) -> bool {
    let package = dev_package_table
        .entry(crate_name.to_string())
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(package_table) = package.as_table_mut() else {
        return false;
    };
    if package_table.contains_key("opt-level") {
        return false;
    }
    package_table.insert("opt-level".to_string(), Value::Integer(3));
    true
}

fn ensure_dev_package_fast_checks(
    dev_package_table: &mut toml::map::Map<String, Value>,
    crate_name: &str,
) -> bool {
    let package = dev_package_table
        .entry(crate_name.to_string())
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(package_table) = package.as_table_mut() else {
        return false;
    };
    let mut changed = false;
    if !package_table.contains_key("debug-assertions") {
        package_table.insert("debug-assertions".to_string(), Value::Boolean(false));
        changed = true;
    }
    if !package_table.contains_key("overflow-checks") {
        package_table.insert("overflow-checks".to_string(), Value::Boolean(false));
        changed = true;
    }
    changed
}

fn ensure_scripts_manifest_rust_analyzer_cfg(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    if src.contains("cfg(rust_analyzer)") {
        return Ok(());
    }
    let mut out = src.trim_end().to_string();
    out.push_str(
        "\n\n[lints.rust]\nunexpected_cfgs = { level = \"warn\", check-cfg = [\"cfg(rust_analyzer)\"] }\n",
    );
    fs::write(path, out)
}

fn ensure_patch_block_in_manifest(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let src = fs::read_to_string(path)?;
    let overrides = source_overrides_block_for_manifest(path, &src);
    let stripped = strip_patch_crates_io(&src);
    let mut out = stripped.trim_end().to_string();
    if !overrides.is_empty() {
        out.push_str("\n\n");
        out.push_str(&overrides);
        out.push('\n');
    }
    if src == out {
        return Ok(());
    }
    fs::write(path, out)
}

fn strip_patch_crates_io(src: &str) -> String {
    let mut out = String::new();
    let mut in_patch = false;

    for line in src.lines() {
        let trimmed = line.trim();
        let is_header = trimmed.starts_with('[') && trimmed.ends_with(']');
        let is_patch_header = is_header
            && (trimmed == "[patch.crates-io]" || trimmed.starts_with("[patch.crates-io."));
        if is_patch_header {
            in_patch = true;
            continue;
        }
        if in_patch && is_header {
            in_patch = false;
        }
        if !in_patch {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn source_overrides_block_for_manifest(manifest_path: &Path, manifest_src: &str) -> String {
    let engine_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("..")
        });
    let manifest_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .canonicalize()
        .unwrap_or_else(|_| {
            manifest_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        });

    let Some(mut crates) = direct_perro_deps_from_manifest(manifest_src) else {
        return String::new();
    };
    let mut visited = BTreeSet::new();
    collect_perro_deps_from_local_path_deps(manifest_path, manifest_src, &mut crates, &mut visited);
    expand_transitive_perro_deps(&engine_root, &mut crates);
    if crates.is_empty() {
        return String::new();
    }

    let mut ordered_crates: Vec<_> = crates.into_iter().collect();
    ordered_crates.sort_by(|a, b| {
        let ka = crate_group_sort_key(a);
        let kb = crate_group_sort_key(b);
        ka.cmp(&kb).then_with(|| a.cmp(b))
    });

    let mut lines = Vec::new();
    lines.push("[patch.crates-io]".to_string());
    for crate_name in ordered_crates {
        let Some(rel_crate_path) = crate_workspace_rel_path(&crate_name) else {
            continue;
        };
        let path = rel_path(&manifest_dir, &engine_root.join(rel_crate_path));
        lines.push(format!("{crate_name} = {{ path = \"{path}\" }}"));
    }
    lines.join("\n")
}

fn collect_perro_deps_from_local_path_deps(
    manifest_path: &Path,
    manifest_src: &str,
    crates: &mut BTreeSet<String>,
    visited: &mut BTreeSet<PathBuf>,
) {
    let Some(manifest_dir) = manifest_path.parent() else {
        return;
    };
    for rel_path in local_path_dependencies_from_manifest(manifest_src) {
        let dep_manifest = manifest_dir.join(rel_path).join("Cargo.toml");
        let dep_manifest = dep_manifest.canonicalize().unwrap_or(dep_manifest);
        if !visited.insert(dep_manifest.clone()) {
            continue;
        }
        let Ok(dep_src) = fs::read_to_string(&dep_manifest) else {
            continue;
        };
        if let Some(extra) = direct_perro_deps_from_manifest(&dep_src) {
            crates.extend(extra);
        }
        collect_perro_deps_from_local_path_deps(&dep_manifest, &dep_src, crates, visited);
    }
}

fn direct_perro_deps_from_manifest(src: &str) -> Option<BTreeSet<String>> {
    let value: Value = src.parse::<Value>().ok()?;
    let mut out = BTreeSet::new();
    collect_perro_dep_keys(value.get("dependencies"), &mut out);
    collect_perro_dep_keys(value.get("build-dependencies"), &mut out);
    collect_perro_dep_keys(value.get("dev-dependencies"), &mut out);
    Some(out)
}

fn local_path_dependencies_from_manifest(src: &str) -> Vec<String> {
    let Ok(value) = src.parse::<Value>() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_local_path_deps(value.get("dependencies"), &mut out);
    collect_local_path_deps(value.get("build-dependencies"), &mut out);
    collect_local_path_deps(value.get("dev-dependencies"), &mut out);
    out
}

fn collect_perro_dep_keys(table: Option<&Value>, out: &mut BTreeSet<String>) {
    let Some(table) = table.and_then(Value::as_table) else {
        return;
    };
    for key in table.keys() {
        if key.starts_with("perro_") || key == "perro_api" {
            out.insert(key.to_string());
        }
    }
}

fn collect_local_path_deps(table: Option<&Value>, out: &mut Vec<String>) {
    let Some(table) = table.and_then(Value::as_table) else {
        return;
    };
    for dep in table.values() {
        let Some(dep_table) = dep.as_table() else {
            continue;
        };
        let Some(path) = dep_table.get("path").and_then(Value::as_str) else {
            continue;
        };
        out.push(path.to_string());
    }
}

fn expand_transitive_perro_deps(engine_root: &Path, crates: &mut BTreeSet<String>) {
    let mut queue: Vec<String> = crates.iter().cloned().collect();
    while let Some(crate_name) = queue.pop() {
        let Some(rel_path) = crate_workspace_rel_path(&crate_name) else {
            continue;
        };
        let manifest = engine_root.join(rel_path).join("Cargo.toml");
        let Ok(src) = fs::read_to_string(manifest) else {
            continue;
        };
        let Some(extra) = direct_perro_deps_from_manifest(&src) else {
            continue;
        };
        for dep in extra {
            if crates.insert(dep.clone()) {
                queue.push(dep);
            }
        }
    }
}

fn crate_workspace_rel_path(crate_name: &str) -> Option<&'static str> {
    match crate_name {
        "perro_animation" => Some("perro_source/core/perro_animation"),
        "perro_nodes" => Some("perro_source/core/perro_nodes"),
        "perro_ui" => Some("perro_source/core/perro_ui"),
        "perro_structs" => Some("perro_source/core/perro_structs"),
        "perro_ids" => Some("perro_source/core/perro_ids"),
        "perro_variant" => Some("perro_source/core/perro_variant"),
        "perro_particle_math" => Some("perro_source/core/perro_particle_math"),
        "perro_runtime" => Some("perro_source/runtime_project/perro_runtime"),
        "perro_internal_updates" => Some("perro_source/runtime_project/perro_internal_updates"),
        "perro_scene" => Some("perro_source/runtime_project/perro_scene"),
        "perro_runtime_context" => Some("perro_source/api_modules/perro_runtime_context"),
        "perro_resource_context" => Some("perro_source/api_modules/perro_resource_context"),
        "perro_api" => Some("perro_source/api_modules/perro_api"),
        "perro_modules" => Some("perro_source/api_modules/perro_modules"),
        "perro_input" => Some("perro_source/api_modules/perro_input"),
        "perro_render_bridge" => Some("perro_source/render_stack/perro_render_bridge"),
        "perro_graphics" => Some("perro_source/render_stack/perro_graphics"),
        "perro_meshlets" => Some("perro_source/render_stack/perro_meshlets"),
        "perro_app" => Some("perro_source/render_stack/perro_app"),
        "perro_scripting" => Some("perro_source/script_stack/perro_scripting"),
        "perro_scripting_macros" => Some("perro_source/script_stack/perro_scripting_macros"),
        "perro_compiler" => Some("perro_source/build_pipeline/perro_compiler"),
        "perro_static_pipeline" => Some("perro_source/build_pipeline/perro_static_pipeline"),
        "perro_io" => Some("perro_source/io_stack/perro_io"),
        "perro_assets" => Some("perro_source/io_stack/perro_assets"),
        "perro_bark" => Some("perro_source/audio_stack/perro_bark"),
        "perro_project" => Some("perro_source/runtime_project/perro_project"),
        "perro_cli" => Some("perro_source/devtools/perro_cli"),
        "perro_dev_runner" => Some("perro_source/devtools/perro_dev_runner"),
        _ => None,
    }
}

fn crate_group_sort_key(crate_name: &str) -> u8 {
    let Some(rel) = crate_workspace_rel_path(crate_name) else {
        return u8::MAX;
    };
    if rel.starts_with("perro_source/core/") {
        return 0;
    }
    if rel.starts_with("perro_source/runtime_project/") {
        return 1;
    }
    if rel.starts_with("perro_source/api_modules/") {
        return 2;
    }
    if rel.starts_with("perro_source/render_stack/") {
        return 3;
    }
    if rel.starts_with("perro_source/script_stack/") {
        return 4;
    }
    if rel.starts_with("perro_source/build_pipeline/") {
        return 5;
    }
    if rel.starts_with("perro_source/io_stack/") {
        return 6;
    }
    if rel.starts_with("perro_source/devtools/") {
        return 7;
    }
    8
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;

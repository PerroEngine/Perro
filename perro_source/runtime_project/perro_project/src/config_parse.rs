pub fn default_project_toml(name: &str) -> String {
    format!(
        r#"[project]
name = "{name}"
main_scene = "res://main.scn"
icon = "res://icon.png"
startup_splash = "res://icon.png"

# Optional identity/export info.
# Used for Windows executable version info + engine detection strings.
#
# version = "0.1.0"
# description = "{name}"
# company = "Studio Name"
# copyright = "Copyright (c) 2026 Studio Name"
# trademark = ""

[graphics]
aspect_ratio = "16:9"            # "WIDTH:HEIGHT" game shape
vsync = false
msaa = true
ssao = "medium"                  # off | low | medium | high | ultra
occlusion_culling = "gpu"        # cpu | gpu | off
particle_sim_default = "gpu"     # cpu | hybrid | gpu
texture_filter = "linear_mipmap" # nearest | linear | linear_mipmap | anisotropic
default_font = "default"         # default | system://Name | res://path.ttf

meshlets = false
dev_meshlets = false
release_meshlets = true
meshlet_debug_view = false

[ui]
pixel_snapping = true

[runtime]
frame_rate_cap = "unlimited"     # fps number | "unlimited" | "refresh_rate"
target_fixed_update = 60

[physics]
gravity = -9.81
coef = 1.0

[audio]
listener_max_distance = 500.0
propagation_tick_hz = 20
energy_cutoff = 0.02
debug_rays = false

# Ray propagation. Plain key sets 2D + 3D; `_2d` / `_3d` suffix tunes one path.
max_bounces = 4
max_ray_distance = 500.0
rays_per_tick_2d = 64
rays_per_tick_3d = 128

# Optional localization table.
# Put localization.csv, locale.csv, or translations.csv next to project.toml.
# First column must be key. Other columns use language codes.
#
# [localization]
# default_locale = "en"

# Optional Steam integration.
# [steam]
# enabled = false
# app_id = 480
# input = "off" # off | metadata | actions

# Optional web export metadata.
# [web]
# title = "{name}"
# description = "{name}"
# keywords = ["rust", "game engine"]
"#
    )
}

pub fn default_input_map_toml() -> String {
    "[jump]\nkeys = [\"KeySpace\", \"KeyUp\"]\n".to_string()
}

pub fn load_project_toml(root: &Path) -> Result<ProjectConfig, ProjectError> {
    let project_toml = fs::read_to_string(root.join("project.toml"))?;
    let mut config = parse_project_toml(&project_toml)?;
    apply_sibling_localization(root, &mut config)?;
    config.input_map = load_input_map_toml(root)?;
    Ok(config)
}

pub fn load_input_map_toml(root: &Path) -> Result<perro_input_api::InputMap, ProjectError> {
    let path = root.join("input_map.toml");
    if !path.exists() {
        return Ok(perro_input_api::InputMap::new());
    }
    let input_map_toml = fs::read_to_string(path)?;
    parse_input_map_toml(&input_map_toml)
}

pub fn load_routes_toml(
    root: &Path,
    project: &ProjectConfig,
) -> Result<ProjectRoutesConfig, ProjectError> {
    let path = root.join("routes.toml");
    if !path.exists() {
        return Ok(default_routes_config(project));
    }
    let routes_toml = fs::read_to_string(path)?;
    parse_routes_toml(&routes_toml)
}

const KNOWN_PROJECT_TOML_TABLES: &[&str] = &[
    "project",
    "metadata",
    "web",
    "graphics",
    "rendering",
    "ui",
    "runtime",
    "physics",
    "audio",
    "localization",
    "steam",
];

pub fn parse_project_toml(contents: &str) -> Result<ProjectConfig, ProjectError> {
    let value = parse_toml_document_value(contents)?;
    let project_table = value
        .get("project")
        .and_then(Value::as_table)
        .ok_or(ProjectError::MissingField("project"))?;

    if let Some(root) = value.as_table() {
        for key in root.keys() {
            if !KNOWN_PROJECT_TOML_TABLES.contains(&key.as_str()) {
                eprintln!("perro: project.toml: unknown table `[{key}]` ignored");
            }
        }
    }

    let empty_table = toml::map::Map::new();
    let graphics_table = value
        .get("graphics")
        .and_then(Value::as_table)
        .unwrap_or(&empty_table);
    let runtime_table = value.get("runtime").and_then(Value::as_table);
    let physics_table = value.get("physics").and_then(Value::as_table);
    let localization_table = value.get("localization").and_then(Value::as_table);
    let metadata_table = value.get("metadata").and_then(Value::as_table);
    let steam_table = value.get("steam").and_then(Value::as_table);
    let audio_table = value.get("audio").and_then(Value::as_table);
    let web_table = value.get("web").and_then(Value::as_table);
    let rendering_table = value.get("rendering").and_then(Value::as_table);
    let ui_table = value.get("ui").and_then(Value::as_table);

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

    let (virtual_width, virtual_height) = parse_virtual_canvas(graphics_table)?;

    if virtual_width == 0 || virtual_height == 0 {
        return Err(ProjectError::InvalidField(
            "graphics.aspect_ratio",
            "derived canvas values must be greater than 0".to_string(),
        ));
    }

    let vsync = parse_bool_with_default(graphics_table, "vsync", false)?;
    let frame_rate_cap = parse_frame_rate_cap(runtime_table)?;
    let target_fixed_update = parse_target_fixed_update(runtime_table)?;
    let physics_gravity = parse_physics_gravity(physics_table)?;
    let physics_coef = parse_physics_coef(physics_table)?;
    let msaa = parse_bool_with_default(graphics_table, "msaa", true)?;
    let ssao = parse_ssao_with_default(graphics_table, "ssao", SsaoQuality::Medium)?;
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
    let texture_filter = parse_texture_filter_with_default(
        graphics_table,
        "texture_filter",
        perro_structs::TextureFilterMode::LinearMipmap,
    )?;
    let localization = parse_localization(localization_table)?;
    let mut metadata = parse_metadata(metadata_table)?;
    apply_project_identity(project_table, &mut metadata)?;
    let steam = parse_steam(steam_table)?;
    let audio = parse_audio(audio_table)?;
    let web = parse_web(web_table)?;
    let rendering = parse_rendering(graphics_table, rendering_table, ui_table)?;

    Ok(ProjectConfig {
        name,
        metadata,
        web,
        main_scene,
        main_scene_hash: None,
        icon,
        icon_hash: None,
        startup_splash,
        startup_splash_hash: None,
        virtual_width,
        virtual_height,
        vsync,
        frame_rate_cap,
        target_fixed_update,
        physics_gravity,
        physics_coef,
        msaa,
        ssao,
        meshlets,
        dev_meshlets,
        release_meshlets,
        meshlet_debug_view,
        occlusion_culling,
        particle_sim_default,
        texture_filter,
        rendering,
        audio,
        localization,
        input_map: perro_input_api::InputMap::new(),
        steam,
    })
}

pub fn parse_input_map_toml(contents: &str) -> Result<perro_input_api::InputMap, ProjectError> {
    let value = parse_toml_document_value(contents)?;
    let root = value.as_table().ok_or_else(|| {
        ProjectError::InvalidField("input_map", "must be a TOML table".to_string())
    })?;
    let mut actions = Vec::new();
    for (name, value) in root {
        let table = value.as_table().ok_or_else(|| {
            ProjectError::InvalidField("input_map", format!("action `{name}` must be table"))
        })?;
        let action_name = name.trim();
        if action_name.is_empty() {
            return Err(ProjectError::InvalidField(
                "input_map",
                "action name must not be empty".to_string(),
            ));
        }
        let mut bindings = Vec::new();
        parse_input_map_binding_list(
            table,
            "keys",
            &mut bindings,
            parse_input_map_key_binding,
            "keys",
            action_name,
        )?;
        parse_input_map_binding_list(
            table,
            "mouse",
            &mut bindings,
            parse_input_map_mouse_binding,
            "mouse",
            action_name,
        )?;
        parse_input_map_binding_list(
            table,
            "gamepad",
            &mut bindings,
            parse_input_map_gamepad_binding,
            "gamepad",
            action_name,
        )?;
        parse_input_map_binding_list(
            table,
            "joycon",
            &mut bindings,
            parse_input_map_joycon_binding,
            "joycon",
            action_name,
        )?;
        if bindings.is_empty() {
            return Err(ProjectError::InvalidField(
                "input_map",
                format!("action `{action_name}` needs at least 1 binding"),
            ));
        }
        actions.push(perro_input_api::InputAction::new(action_name, bindings));
    }
    Ok(perro_input_api::InputMap::from_actions(actions))
}

fn apply_project_identity(
    table: &toml::map::Map<String, Value>,
    metadata: &mut ProjectMetadata,
) -> Result<(), ProjectError> {
    if let Some(v) = parse_optional_table_str(table, "description", "project.description")? {
        metadata.description = Some(v);
    }
    if let Some(v) = parse_optional_table_str(table, "company", "project.company")? {
        metadata.company = Some(v);
    }
    if let Some(v) = parse_optional_table_str(table, "version", "project.version")? {
        metadata.version = Some(v);
    }
    if let Some(v) = parse_optional_table_str(table, "copyright", "project.copyright")? {
        metadata.copyright = Some(v);
    }
    if let Some(v) = parse_optional_table_str(table, "trademark", "project.trademark")? {
        metadata.trademark = Some(v);
    }
    Ok(())
}

fn parse_rendering(
    graphics_table: &toml::map::Map<String, Value>,
    rendering_table: Option<&toml::map::Map<String, Value>>,
    ui_table: Option<&toml::map::Map<String, Value>>,
) -> Result<RenderingConfig, ProjectError> {
    // Pixel snapping: legacy [rendering.ui], overridden by [ui].
    let legacy_ui_table = rendering_table
        .and_then(|table| table.get("ui"))
        .and_then(Value::as_table);
    let mut pixel_snapping = true;
    if let Some(table) = legacy_ui_table {
        pixel_snapping = parse_optional_table_bool(
            table,
            "pixel_snapping",
            "rendering.ui.pixel_snapping",
        )?
        .unwrap_or(pixel_snapping);
    }
    if let Some(table) = ui_table {
        pixel_snapping = parse_optional_table_bool(table, "pixel_snapping", "ui.pixel_snapping")?
            .unwrap_or(pixel_snapping);
    }

    // Default font: legacy [rendering], overridden by [graphics].
    let mut default_font = "default".to_string();
    let mut font_path = "rendering.default_font";
    if let Some(table) = rendering_table
        && let Some(value) = table.get("default_font")
    {
        default_font = value
            .as_str()
            .ok_or_else(|| {
                ProjectError::InvalidField(font_path, "must be a string".to_string())
            })?
            .to_string();
    }
    if let Some(value) = graphics_table.get("default_font") {
        font_path = "graphics.default_font";
        default_font = value
            .as_str()
            .ok_or_else(|| {
                ProjectError::InvalidField(font_path, "must be a string".to_string())
            })?
            .to_string();
    }
    if perro_ui::UiFont::parse(&default_font).is_none() {
        return Err(ProjectError::InvalidField(
            font_path,
            "must be `default`, `system://Name`, or `res://path`".to_string(),
        ));
    }
    Ok(RenderingConfig {
        ui: RenderUiConfig { pixel_snapping },
        default_font,
    })
}

fn parse_optional_table_bool(
    table: &toml::map::Map<String, Value>,
    key: &str,
    path: &'static str,
) -> Result<Option<bool>, ProjectError> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    value.as_bool().map(Some).ok_or_else(|| {
        ProjectError::InvalidField(path, "must be a boolean".to_string())
    })
}

fn parse_input_map_binding_list(
    table: &toml::map::Map<String, Value>,
    key: &'static str,
    bindings: &mut Vec<perro_input_api::InputBinding>,
    parse: fn(&str) -> Option<perro_input_api::InputBinding>,
    field: &'static str,
    action_name: &str,
) -> Result<(), ProjectError> {
    let Some(value) = table.get(key) else {
        return Ok(());
    };
    let Value::Array(items) = value else {
        return Err(ProjectError::InvalidField(
            "input_map",
            format!("action `{action_name}` field `{field}` must be array of strings"),
        ));
    };
    for item in items {
        let Some(raw) = item.as_str() else {
            return Err(ProjectError::InvalidField(
                "input_map",
                format!("action `{action_name}` field `{field}` must be array of strings"),
            ));
        };
        let Some(binding) = parse(raw) else {
            return Err(ProjectError::InvalidField(
                "input_map",
                format!("unknown {field} binding `{raw}` in action `{action_name}`"),
            ));
        };
        bindings.push(binding);
    }
    Ok(())
}

fn parse_input_map_key_binding(raw: &str) -> Option<perro_input_api::InputBinding> {
    perro_input_api::KeyCode::from_name(raw).map(perro_input_api::InputBinding::Key)
}

fn parse_input_map_mouse_binding(raw: &str) -> Option<perro_input_api::InputBinding> {
    perro_input_api::MouseButton::from_name(raw).map(perro_input_api::InputBinding::Mouse)
}

fn parse_input_map_gamepad_binding(raw: &str) -> Option<perro_input_api::InputBinding> {
    perro_input_api::GamepadButton::from_name(raw).map(perro_input_api::InputBinding::Gamepad)
}

fn parse_input_map_joycon_binding(raw: &str) -> Option<perro_input_api::InputBinding> {
    perro_input_api::JoyConButton::from_name(raw).map(perro_input_api::InputBinding::JoyCon)
}

pub fn default_routes_config(project: &ProjectConfig) -> ProjectRoutesConfig {
    ProjectRoutesConfig {
        routes: vec![ProjectRoute {
            href: "/".to_string(),
            name: "main".to_string(),
            scene: project.main_scene.clone(),
            title: None,
            description: None,
            keywords: Vec::new(),
        }],
    }
}

pub fn parse_routes_toml(contents: &str) -> Result<ProjectRoutesConfig, ProjectError> {
    let value = parse_toml_document_value(contents)?;
    let route_entries = value
        .get("route")
        .and_then(Value::as_array)
        .ok_or(ProjectError::MissingField("route"))?;
    let mut routes = Vec::with_capacity(route_entries.len());
    for entry in route_entries {
        let table = entry.as_table().ok_or_else(|| ProjectError::InvalidField(
            "route",
            "must be table array".to_string(),
        ))?;
        let href_raw = table
            .get("href")
            .and_then(Value::as_str)
            .ok_or(ProjectError::MissingField("route.href"))?;
        let scene = table
            .get("scene")
            .and_then(Value::as_str)
            .ok_or(ProjectError::MissingField("route.scene"))?
            .to_string();
        validate_res_path("route.scene", &scene)?;
        let name = table
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(ProjectError::MissingField("route.name"))?
            .to_string();
        validate_route_href("route.href", href_raw)?;
        let href = normalize_route_href(href_raw);
        let title = parse_optional_route_str(table, "title")?;
        let description = parse_optional_route_str(table, "description")?;
        let keywords = parse_keywords_table_field(table, "route.keywords", "keywords")?;
        routes.push(ProjectRoute {
            href,
            name,
            scene,
            title,
            description,
            keywords,
        });
    }
    if routes.is_empty() {
        return Err(ProjectError::InvalidField(
            "route",
            "need at least 1 route".to_string(),
        ));
    }
    Ok(ProjectRoutesConfig { routes })
}

fn parse_steam(table: Option<&toml::map::Map<String, Value>>) -> Result<SteamConfig, ProjectError> {
    let Some(table) = table else {
        return Ok(SteamConfig::default());
    };
    let enabled = match table.get("enabled") {
        Some(value) => value.as_bool().ok_or_else(|| {
            ProjectError::InvalidField("steam.enabled", "must be a boolean".to_string())
        })?,
        None => false,
    };
    let app_id = match table.get("app_id") {
        Some(value) => {
            let raw = value.as_integer().ok_or_else(|| {
                ProjectError::InvalidField("steam.app_id", "must be an integer".to_string())
            })?;
            Some(u32::try_from(raw).map_err(|_| {
                ProjectError::InvalidField("steam.app_id", "must fit in u32".to_string())
            })?)
        }
        None => None,
    };
    if enabled && app_id.is_none() {
        return Err(ProjectError::MissingField("steam.app_id"));
    }
    let input_mode = parse_steam_input_mode(table)?;
    Ok(SteamConfig {
        enabled,
        app_id,
        input_mode,
    })
}

fn parse_steam_input_mode(
    table: &toml::map::Map<String, Value>,
) -> Result<SteamInputMode, ProjectError> {
    let Some(value) = table.get("input") else {
        return Ok(SteamInputMode::Off);
    };
    let raw = value
        .as_str()
        .ok_or_else(|| ProjectError::InvalidField("steam.input", "must be a string".to_string()))?;
    match raw {
        "off" => Ok(SteamInputMode::Off),
        "metadata" => Ok(SteamInputMode::Metadata),
        "actions" => Ok(SteamInputMode::Actions),
        _ => Err(ProjectError::InvalidField(
            "steam.input",
            "must be off, metadata, or actions".to_string(),
        )),
    }
}

fn parse_audio(table: Option<&toml::map::Map<String, Value>>) -> Result<AudioConfig, ProjectError> {
    let Some(table) = table else {
        return Ok(AudioConfig::default());
    };
    let mut cfg = AudioConfig::default();
    cfg.listener_max_distance = parse_f32_table_field(
        table,
        "listener_max_distance",
        cfg.listener_max_distance,
        "audio.listener_max_distance",
    )?;
    cfg.propagation_tick_hz = parse_f32_table_field(
        table,
        "propagation_tick_hz",
        cfg.propagation_tick_hz,
        "audio.propagation_tick_hz",
    )?;
    cfg.energy_cutoff = parse_f32_table_field(
        table,
        "energy_cutoff",
        cfg.energy_cutoff,
        "audio.energy_cutoff",
    )?;
    cfg.debug_rays = table
        .get("debug_rays")
        .map(|value| {
            value.as_bool().ok_or_else(|| {
                ProjectError::InvalidField("audio.debug_rays", "must be a boolean".to_string())
            })
        })
        .transpose()?
        .unwrap_or(cfg.debug_rays);
    // Legacy [audio.propagation_2d] / [audio.propagation_3d] tables.
    if let Some(two_d) = table.get("propagation_2d").and_then(Value::as_table) {
        cfg.propagation_2d =
            parse_audio_propagation(two_d, cfg.propagation_2d, "audio.propagation_2d")?;
    }
    if let Some(three_d) = table.get("propagation_3d").and_then(Value::as_table) {
        cfg.propagation_3d =
            parse_audio_propagation(three_d, cfg.propagation_3d, "audio.propagation_3d")?;
    }
    // Flat propagation keys in [audio]: plain key sets both paths, `_2d` / `_3d`
    // suffix overrides one. Flat keys win over legacy subtables.
    apply_flat_audio_propagation(table, &mut cfg)?;
    validate_audio_bounces(&cfg.propagation_2d, "audio.max_bounces_2d")?;
    validate_audio_bounces(&cfg.propagation_3d, "audio.max_bounces_3d")?;
    Ok(cfg)
}

fn apply_flat_audio_propagation(
    table: &toml::map::Map<String, Value>,
    cfg: &mut AudioConfig,
) -> Result<(), ProjectError> {
    cfg.propagation_2d.max_bounces =
        parse_u32_table_field(table, "max_bounces", cfg.propagation_2d.max_bounces, "audio")?;
    cfg.propagation_3d.max_bounces =
        parse_u32_table_field(table, "max_bounces", cfg.propagation_3d.max_bounces, "audio")?;
    cfg.propagation_2d.rays_per_tick = parse_u32_table_field(
        table,
        "rays_per_tick",
        cfg.propagation_2d.rays_per_tick,
        "audio",
    )?;
    cfg.propagation_3d.rays_per_tick = parse_u32_table_field(
        table,
        "rays_per_tick",
        cfg.propagation_3d.rays_per_tick,
        "audio",
    )?;
    cfg.propagation_2d.max_ray_distance = parse_f32_table_field(
        table,
        "max_ray_distance",
        cfg.propagation_2d.max_ray_distance,
        "audio",
    )?;
    cfg.propagation_3d.max_ray_distance = parse_f32_table_field(
        table,
        "max_ray_distance",
        cfg.propagation_3d.max_ray_distance,
        "audio",
    )?;

    cfg.propagation_2d.max_bounces = parse_u32_table_field(
        table,
        "max_bounces_2d",
        cfg.propagation_2d.max_bounces,
        "audio",
    )?;
    cfg.propagation_3d.max_bounces = parse_u32_table_field(
        table,
        "max_bounces_3d",
        cfg.propagation_3d.max_bounces,
        "audio",
    )?;
    cfg.propagation_2d.rays_per_tick = parse_u32_table_field(
        table,
        "rays_per_tick_2d",
        cfg.propagation_2d.rays_per_tick,
        "audio",
    )?;
    cfg.propagation_3d.rays_per_tick = parse_u32_table_field(
        table,
        "rays_per_tick_3d",
        cfg.propagation_3d.rays_per_tick,
        "audio",
    )?;
    cfg.propagation_2d.max_ray_distance = parse_f32_table_field(
        table,
        "max_ray_distance_2d",
        cfg.propagation_2d.max_ray_distance,
        "audio",
    )?;
    cfg.propagation_3d.max_ray_distance = parse_f32_table_field(
        table,
        "max_ray_distance_3d",
        cfg.propagation_3d.max_ray_distance,
        "audio",
    )?;
    Ok(())
}

fn validate_audio_bounces(
    cfg: &AudioPropagationConfig,
    path: &'static str,
) -> Result<(), ProjectError> {
    if cfg.max_bounces > crate::MAX_AUDIO_PROPAGATION_BOUNCES {
        return Err(ProjectError::InvalidField(
            path,
            format!(
                "max_bounces must be <= {}",
                crate::MAX_AUDIO_PROPAGATION_BOUNCES
            ),
        ));
    }
    Ok(())
}

fn parse_audio_propagation(
    table: &toml::map::Map<String, Value>,
    mut cfg: AudioPropagationConfig,
    path: &'static str,
) -> Result<AudioPropagationConfig, ProjectError> {
    cfg.max_bounces = parse_u32_table_field(table, "max_bounces", cfg.max_bounces, path)?;
    cfg.rays_per_tick = parse_u32_table_field(table, "rays_per_tick", cfg.rays_per_tick, path)?;
    cfg.max_ray_distance =
        parse_f32_table_field(table, "max_ray_distance", cfg.max_ray_distance, path)?;
    Ok(cfg)
}

fn parse_u32_table_field(
    table: &toml::map::Map<String, Value>,
    key: &str,
    default: u32,
    path: &'static str,
) -> Result<u32, ProjectError> {
    let Some(value) = table.get(key) else {
        return Ok(default);
    };
    let Some(raw) = value.as_integer() else {
        return Err(ProjectError::InvalidField(
            path,
            format!("{key} must be integer"),
        ));
    };
    u32::try_from(raw).map_err(|_| ProjectError::InvalidField(path, format!("{key} must fit u32")))
}

fn parse_f32_table_field(
    table: &toml::map::Map<String, Value>,
    key: &str,
    default: f32,
    path: &'static str,
) -> Result<f32, ProjectError> {
    let Some(value) = table.get(key) else {
        return Ok(default);
    };
    let Some(raw) = value
        .as_float()
        .or_else(|| value.as_integer().map(|v| v as f64))
    else {
        return Err(ProjectError::InvalidField(
            path,
            format!("{key} must be finite number"),
        ));
    };
    let parsed = raw as f32;
    if raw.is_finite() && raw >= 0.0 && parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(ProjectError::InvalidField(
            path,
            format!("{key} must be finite and >= 0"),
        ))
    }
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

fn parse_frame_rate_cap(
    runtime: Option<&toml::map::Map<String, Value>>,
) -> Result<FrameRateCap, ProjectError> {
    let Some(runtime) = runtime else {
        return Ok(FrameRateCap::Unlimited);
    };
    let Some(value) = runtime
        .get("frame_rate_cap")
        .or_else(|| runtime.get("target_fps"))
    else {
        return Ok(FrameRateCap::Unlimited);
    };
    if let Some(raw) = value.as_str() {
        return match raw.trim().to_ascii_lowercase().as_str() {
            "unlimited" | "uncapped" | "off" | "none" => Ok(FrameRateCap::Unlimited),
            "refresh_rate" | "refresh" | "display" | "monitor" => Ok(FrameRateCap::RefreshRate),
            _ => Err(ProjectError::InvalidField(
                "runtime.frame_rate_cap",
                "must be positive number, \"unlimited\", or \"refresh_rate\"".to_string(),
            )),
        };
    }
    let raw = value
        .as_float()
        .or_else(|| value.as_integer().map(|v| v as f64))
        .ok_or_else(|| {
            ProjectError::InvalidField(
                "runtime.frame_rate_cap",
                "must be positive number, \"unlimited\", or \"refresh_rate\"".to_string(),
            )
        })?;
    let parsed = raw as f32;
    if raw.is_finite() && raw > 0.0 && parsed.is_finite() {
        Ok(FrameRateCap::Fps(parsed))
    } else {
        Ok(FrameRateCap::Unlimited)
    }
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
        let parsed = num as f32;
        if num <= 0.0 || !num.is_finite() || !parsed.is_finite() {
            return Ok(None);
        }
        return Ok(Some(parsed));
    }
    if let Some(num) = value.as_integer() {
        if num <= 0 {
            return Ok(None);
        }
        let parsed = num as f32;
        return Ok(parsed.is_finite().then_some(parsed));
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
    let parsed = num as f32;
    if !num.is_finite() || !parsed.is_finite() {
        return Err(ProjectError::InvalidField(
            "physics.gravity",
            "must be a finite number".to_string(),
        ));
    }
    Ok(parsed)
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
    let parsed = num as f32;
    if !num.is_finite() || num <= 0.0 || !parsed.is_finite() {
        return Err(ProjectError::InvalidField(
            "physics.coef",
            "must be a finite positive number".to_string(),
        ));
    }
    Ok(parsed)
}
fn parse_ssao_with_default(
    table: &toml::map::Map<String, Value>,
    key: &'static str,
    default: SsaoQuality,
) -> Result<SsaoQuality, ProjectError> {
    let Some(value) = table.get(key) else {
        return Ok(default);
    };
    let value = value.as_str().ok_or_else(|| {
        ProjectError::InvalidField("graphics.ssao", "must be a string".to_string())
    })?;
    match value {
        "off" => Ok(SsaoQuality::Off),
        "low" => Ok(SsaoQuality::Low),
        "medium" => Ok(SsaoQuality::Medium),
        "high" => Ok(SsaoQuality::High),
        "ultra" => Ok(SsaoQuality::Ultra),
        _ => Err(ProjectError::InvalidField(
            "graphics.ssao",
            "must be one of: off, low, medium, high, ultra".to_string(),
        )),
    }
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

fn parse_texture_filter_with_default(
    table: &toml::map::Map<String, Value>,
    key: &'static str,
    default: perro_structs::TextureFilterMode,
) -> Result<perro_structs::TextureFilterMode, ProjectError> {
    let Some(value) = table.get(key) else {
        return Ok(default);
    };
    let Some(raw) = value.as_str() else {
        return Err(ProjectError::InvalidField(
            "graphics.texture_filter",
            "must be one of \"nearest\", \"linear\", \"linear_mipmap\", \"anisotropic\""
                .to_string(),
        ));
    };
    perro_structs::TextureFilterMode::parse(raw).ok_or_else(|| {
        ProjectError::InvalidField(
            "graphics.texture_filter",
            "must be one of \"nearest\", \"linear\", \"linear_mipmap\", \"anisotropic\""
                .to_string(),
        )
    })
}

fn parse_localization(
    table: Option<&toml::map::Map<String, Value>>,
) -> Result<Option<LocalizationConfig>, ProjectError> {
    let Some(table) = table else {
        return Ok(None);
    };

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
        source_csv: String::new(),
        key_column: "key".to_string(),
        default_locale,
    }))
}

fn apply_sibling_localization(root: &Path, config: &mut ProjectConfig) -> Result<(), ProjectError> {
    let source_csv = find_sibling_localization_csv(root);
    match (&mut config.localization, source_csv) {
        (Some(localization), Some(source_csv)) => {
            localization.source_csv = source_csv;
            localization.key_column = "key".to_string();
        }
        (Some(_), None) => {
            return Err(ProjectError::InvalidField(
                "localization",
                "expected localization.csv, locale.csv, or translations.csv next to project.toml"
                    .to_string(),
            ));
        }
        (None, Some(source_csv)) => {
            config.localization = Some(LocalizationConfig {
                source_csv,
                key_column: "key".to_string(),
                default_locale: "en".to_string(),
            });
        }
        (None, None) => {}
    }
    Ok(())
}

fn find_sibling_localization_csv(root: &Path) -> Option<String> {
    LOCALIZATION_CSV_CANDIDATES
        .iter()
        .copied()
        .find(|name| root.join(name).is_file())
        .map(str::to_string)
}

fn parse_metadata(
    table: Option<&toml::map::Map<String, Value>>,
) -> Result<ProjectMetadata, ProjectError> {
    let Some(table) = table else {
        return Ok(ProjectMetadata::default());
    };

    Ok(ProjectMetadata {
        description: parse_optional_metadata_str(table, "description")?,
        company: parse_optional_metadata_str(table, "company")?,
        version: parse_optional_metadata_str(table, "version")?,
        copyright: parse_optional_metadata_str(table, "copyright")?,
        trademark: parse_optional_metadata_str(table, "trademark")?,
    })
}

fn parse_web(
    table: Option<&toml::map::Map<String, Value>>,
) -> Result<ProjectWebConfig, ProjectError> {
    let Some(table) = table else {
        return Ok(ProjectWebConfig::default());
    };

    Ok(ProjectWebConfig {
        title: parse_optional_table_str(table, "title", "web.title")?,
        description: parse_optional_table_str(table, "description", "web.description")?,
        keywords: parse_keywords_table_field(table, "web.keywords", "keywords")?,
    })
}

fn parse_optional_metadata_str(
    table: &toml::map::Map<String, Value>,
    key: &'static str,
) -> Result<Option<String>, ProjectError> {
    parse_optional_table_str(
        table,
        key,
        match key {
            "description" => "metadata.description",
            "company" => "metadata.company",
            "version" => "metadata.version",
            "copyright" => "metadata.copyright",
            "trademark" => "metadata.trademark",
            _ => "metadata",
        },
    )
}

fn parse_optional_route_str(
    table: &toml::map::Map<String, Value>,
    key: &'static str,
) -> Result<Option<String>, ProjectError> {
    parse_optional_table_str(
        table,
        key,
        match key {
            "title" => "route.title",
            "description" => "route.description",
            _ => "route",
        },
    )
}

fn parse_optional_table_str(
    table: &toml::map::Map<String, Value>,
    key: &'static str,
    path: &'static str,
) -> Result<Option<String>, ProjectError> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    let Some(raw) = value.as_str() else {
        return Err(ProjectError::InvalidField(
            path,
            "must be a string".to_string(),
        ));
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn parse_keywords_table_field(
    table: &toml::map::Map<String, Value>,
    path: &'static str,
    key: &'static str,
) -> Result<Vec<String>, ProjectError> {
    let Some(value) = table.get(key) else {
        return Ok(Vec::new());
    };
    match value {
        Value::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                let Some(raw) = item.as_str() else {
                    return Err(ProjectError::InvalidField(
                        path,
                        "must be array of strings".to_string(),
                    ));
                };
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_string());
                }
            }
            Ok(out)
        }
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Ok(Vec::new())
            } else {
                Ok(vec![trimmed.to_string()])
            }
        }
        _ => Err(ProjectError::InvalidField(
            path,
            "must be string or array of strings".to_string(),
        )),
    }
}

fn validate_res_path(field: &'static str, path: &str) -> Result<(), ProjectError> {
    let Some(relative) = path.strip_prefix("res://") else {
        return Err(ProjectError::InvalidField(
            field,
            "must start with `res://`".to_string(),
        ));
    };
    if is_portable_relative_path(relative, false) {
        return Ok(());
    }
    Err(ProjectError::InvalidField(
        field,
        "must stay inside `res://` and use normal path components".to_string(),
    ))
}

fn validate_route_href(field: &'static str, href: &str) -> Result<(), ProjectError> {
    let normalized = normalize_route_href(href);
    let relative = normalized.strip_prefix('/').unwrap_or(&normalized);
    if is_portable_relative_path(relative, true) {
        return Ok(());
    }
    Err(ProjectError::InvalidField(
        field,
        "must use normal URL path components".to_string(),
    ))
}

fn is_portable_relative_path(path: &str, allow_empty: bool) -> bool {
    if path.is_empty() {
        return allow_empty;
    }
    if path.contains(['\\', ':']) || path.chars().any(char::is_control) {
        return false;
    }
    path.split('/')
        .all(|component| !component.is_empty() && component != "." && component != "..")
}

pub fn normalize_route_href(path: &str) -> String {
    let trimmed = path.trim();
    let core = trimmed.split(['?', '#']).next().unwrap_or("/").trim();
    let mut out = if core.is_empty() {
        "/".to_string()
    } else if core.starts_with('/') {
        core.to_string()
    } else {
        format!("/{core}")
    };
    if out.len() > "/index.html".len() && out.ends_with("/index.html") {
        out.truncate(out.len() - "/index.html".len());
    }
    while out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    out
}

fn parse_virtual_canvas(
    graphics_table: &toml::map::Map<String, Value>,
) -> Result<(u32, u32), ProjectError> {
    if graphics_table.contains_key("virtual_resolution") {
        return Err(ProjectError::InvalidField(
            "graphics.virtual_resolution",
            "removed; use graphics.aspect_ratio".to_string(),
        ));
    }
    if graphics_table.contains_key("virtual_width") || graphics_table.contains_key("virtual_height")
    {
        return Err(ProjectError::InvalidField(
            "graphics.virtual_width",
            "removed; use graphics.aspect_ratio".to_string(),
        ));
    }

    if let Some(value) = graphics_table.get("aspect_ratio") {
        let Some(raw) = value.as_str() else {
            return Err(ProjectError::InvalidField(
                "graphics.aspect_ratio",
                "must be a string".to_string(),
            ));
        };
        return virtual_canvas_from_aspect_ratio(raw);
    }

    virtual_canvas_from_aspect_ratio("16:9")
}

fn virtual_canvas_from_aspect_ratio(raw: &str) -> Result<(u32, u32), ProjectError> {
    let (w, h) = parse_aspect_ratio(raw)?;
    let (width, height) = if w >= h {
        let height = 1080u32;
        let width = ((height as f32) * (w as f32 / h as f32)).round() as u32;
        (width.max(1), height)
    } else {
        let width = 1080u32;
        let height = ((width as f32) * (h as f32 / w as f32)).round() as u32;
        (width, height.max(1))
    };
    Ok((width, height))
}

fn parse_aspect_ratio(raw: &str) -> Result<(u32, u32), ProjectError> {
    let raw = raw.trim().to_ascii_lowercase();
    let (w, h) =
        raw.split_once(':')
            .or_else(|| raw.split_once('x'))
            .ok_or_else(|| ProjectError::InvalidField(
                "graphics.aspect_ratio",
                "expected format `WIDTH:HEIGHT`, for example `16:9`".to_string(),
            ))?;

    let width = w.parse::<u32>().map_err(|_| {
        ProjectError::InvalidField(
            "graphics.aspect_ratio",
            "invalid width component".to_string(),
        )
    })?;
    let height = h.parse::<u32>().map_err(|_| {
        ProjectError::InvalidField(
            "graphics.aspect_ratio",
            "invalid height component".to_string(),
        )
    })?;
    if width == 0 || height == 0 {
        return Err(ProjectError::InvalidField(
            "graphics.aspect_ratio",
            "ratio values must be greater than 0".to_string(),
        ));
    }
    Ok((width, height))
}

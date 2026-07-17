use super::*;

pub(super) fn emit_occlusion_culling_expr(mode: perro_project::OcclusionCulling) -> &'static str {
    match mode {
        perro_project::OcclusionCulling::Cpu => "perro_app::entry::OcclusionCulling::Cpu",
        perro_project::OcclusionCulling::Gpu => "perro_app::entry::OcclusionCulling::Gpu",
        perro_project::OcclusionCulling::Off => "perro_app::entry::OcclusionCulling::Off",
    }
}

pub(super) fn emit_ssao_expr(quality: perro_project::SsaoQuality) -> &'static str {
    match quality {
        perro_project::SsaoQuality::Off => "perro_runtime::SsaoQuality::Off",
        perro_project::SsaoQuality::Low => "perro_runtime::SsaoQuality::Low",
        perro_project::SsaoQuality::Medium => "perro_runtime::SsaoQuality::Medium",
        perro_project::SsaoQuality::High => "perro_runtime::SsaoQuality::High",
        perro_project::SsaoQuality::Ultra => "perro_runtime::SsaoQuality::Ultra",
    }
}

pub(super) fn emit_particle_sim_default_expr(mode: perro_project::ParticleSimDefault) -> &'static str {
    match mode {
        perro_project::ParticleSimDefault::Cpu => "perro_app::entry::ParticleSimDefault::Cpu",
        perro_project::ParticleSimDefault::GpuVertex => {
            "perro_app::entry::ParticleSimDefault::GpuVertex"
        }
        perro_project::ParticleSimDefault::GpuCompute => {
            "perro_app::entry::ParticleSimDefault::GpuCompute"
        }
    }
}

pub(super) fn emit_frame_rate_cap_expr(cap: perro_project::FrameRateCap) -> String {
    match cap {
        perro_project::FrameRateCap::Unlimited => {
            "perro_app::entry::FrameRateCap::Unlimited".to_string()
        }
        perro_project::FrameRateCap::Fps(fps) if fps.is_finite() && fps > 0.0 => {
            format!("perro_app::entry::FrameRateCap::Fps({}f32)", fps)
        }
        perro_project::FrameRateCap::Fps(_) => {
            "perro_app::entry::FrameRateCap::Unlimited".to_string()
        }
        perro_project::FrameRateCap::RefreshRate => {
            "perro_app::entry::FrameRateCap::RefreshRate".to_string()
        }
    }
}

pub(super) fn emit_optional_f32(value: Option<f32>) -> String {
    match value {
        Some(v) if v.is_finite() => format!("Some({}f32)", v),
        _ => "None".to_string(),
    }
}

pub(super) fn emit_optional_steam_app_id_fn(value: Option<u32>) -> String {
    match value {
        Some(_) => "Some(steam_app_id)".to_string(),
        None => "None".to_string(),
    }
}

pub(super) fn emit_steam_input_mode(mode: perro_project::SteamInputMode) -> &'static str {
    match mode {
        perro_project::SteamInputMode::Off => "perro_runtime::SteamInputMode::Off",
        perro_project::SteamInputMode::Metadata => "perro_runtime::SteamInputMode::Metadata",
        perro_project::SteamInputMode::Actions => "perro_runtime::SteamInputMode::Actions",
    }
}

pub(super) fn emit_static_steam_app_id_fn(value: Option<u32>, project_name: &str) -> String {
    let Some(app_id) = value else {
        return String::new();
    };

    let mut seed = 0x9e37_79b9_7f4a_7c15u64 ^ u64::from(app_id);
    for byte in project_name.as_bytes() {
        seed = splitmix64(seed ^ u64::from(*byte));
    }

    let data_key = next_nonzero_u32(&mut seed);
    let data_mask = next_nonzero_u32(&mut seed);
    let add = next_nonzero_u32(&mut seed);
    let split = next_nonzero_u32(&mut seed);
    let noise = next_nonzero_u32(&mut seed);
    let check_key = next_nonzero_u32(&mut seed) | 1;
    let rot_a = (next_nonzero_u32(&mut seed) % 31) + 1;
    let rot_b = (next_nonzero_u32(&mut seed) % 31) + 1;
    let encoded = app_id
        .rotate_left(rot_a)
        .wrapping_add(add)
        .rotate_left(rot_b)
        ^ data_key
        ^ data_mask;
    let data_a = encoded ^ split;
    let data_b = split;
    let check = app_id.wrapping_mul(check_key).rotate_left(rot_b) ^ noise;
    let poison = next_nonzero_u32(&mut seed);

    format!(
        "fn steam_app_id() -> u32 {{\n\
    const DATA_A: u32 = 0x{data_a:08x};\n\
    const DATA_B: u32 = 0x{data_b:08x};\n\
    const DATA_KEY: u32 = 0x{data_key:08x};\n\
    const DATA_MASK: u32 = 0x{data_mask:08x};\n\
    const ADD: u32 = 0x{add:08x};\n\
    const CHECK_KEY: u32 = 0x{check_key:08x};\n\
    const CHECK: u32 = 0x{check:08x};\n\
    const NOISE: u32 = 0x{noise:08x};\n\
    const POISON: u32 = 0x{poison:08x};\n\
    let mut x = std::hint::black_box(DATA_A) ^ std::hint::black_box(DATA_B);\n\
    x = std::hint::black_box(x ^ std::hint::black_box(DATA_KEY));\n\
    x = std::hint::black_box(x ^ std::hint::black_box(DATA_MASK));\n\
    x = std::hint::black_box(x.rotate_right({rot_b}));\n\
    x = std::hint::black_box(x.wrapping_sub(std::hint::black_box(ADD)));\n\
    let id = std::hint::black_box(x.rotate_right({rot_a}));\n\
    let check_key = std::hint::black_box(CHECK_KEY);\n\
    let noise = std::hint::black_box(NOISE);\n\
    let check = std::hint::black_box(id.wrapping_mul(check_key).rotate_left({rot_b}) ^ noise);\n\
    if check == CHECK {{\n\
        id\n\
    }} else {{\n\
        id ^ POISON\n\
    }}\n\
}}\n\n"
    )
}

pub(super) fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

pub(super) fn next_nonzero_u32(seed: &mut u64) -> u32 {
    loop {
        *seed = splitmix64(*seed);
        let value = (*seed >> 16) as u32;
        if value != 0 {
            return value;
        }
    }
}

pub(super) fn emit_f32(value: f32) -> String {
    if value.is_finite() {
        format!("{value}f32")
    } else {
        "0.0f32".to_string()
    }
}

pub(super) fn emit_optional_static_str(value: Option<&str>) -> String {
    match value {
        Some(v) => format!("Some({})", emit_static_str(v)),
        None => "None".to_string(),
    }
}

pub(super) fn emit_static_str(value: &str) -> String {
    format!("\"{}\"", escape_str(value))
}

pub(super) fn emit_static_routes_block(routes: &perro_project::ProjectRoutesConfig) -> String {
    let mut out = String::from("&[");
    for route in &routes.routes {
        out.push_str("\n            perro_app::entry::StaticEmbeddedRoute { ");
        out.push_str(&format!(
            "href: {}, name: {}, scene_hash: {}u64 }},",
            emit_static_str(&route.href),
            emit_static_str(&route.name),
            perro_ids::parse_hashed_source_uri(&route.scene)
                .unwrap_or_else(|| perro_ids::string_to_u64(&route.scene))
        ));
    }
    if !routes.routes.is_empty() {
        out.push_str("\n        ");
    }
    out.push(']');
    out
}

pub(super) fn emit_static_input_map_block(input_map: &perro_input_api::InputMap) -> String {
    let mut out = String::from("&[");
    for action in input_map.actions() {
        let mut keys = Vec::new();
        let mut mouse = Vec::new();
        let mut gamepad = Vec::new();
        let mut joycon = Vec::new();
        for binding in &action.bindings {
            match binding {
                perro_input_api::InputBinding::Key(key) => {
                    keys.push(format!("perro_input_api::KeyCode::{key:?}"));
                }
                perro_input_api::InputBinding::Mouse(button) => {
                    mouse.push(format!("perro_input_api::MouseButton::{button:?}"));
                }
                perro_input_api::InputBinding::Gamepad(button) => {
                    gamepad.push(format!("perro_input_api::GamepadButton::{button:?}"));
                }
                perro_input_api::InputBinding::JoyCon(button) => {
                    joycon.push(format!("perro_input_api::JoyConButton::{button:?}"));
                }
            }
        }
        out.push_str("\n            perro_app::entry::StaticEmbeddedInputAction { ");
        out.push_str(&format!(
            "name: {}, keys: &{}, mouse: &{}, gamepad: &{}, joycon: &{} }},",
            emit_static_str(&action.name),
            emit_static_input_binding_array(&keys),
            emit_static_input_binding_array(&mouse),
            emit_static_input_binding_array(&gamepad),
            emit_static_input_binding_array(&joycon)
        ));
    }
    if !input_map.actions().is_empty() {
        out.push_str("\n        ");
    }
    out.push(']');
    out
}

pub(super) fn emit_static_input_binding_array(items: &[String]) -> String {
    if items.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", items.join(", "))
    }
}

pub(super) fn indent_block(src: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    src.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{pad}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

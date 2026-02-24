use perro_io::load_asset;
use perro_render_bridge::Material3D;
use perro_scene::{Parser, RuntimeValue, StaticSceneValue};

pub fn load_from_source(source: &str) -> Option<Material3D> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    if source.ends_with(".pmat") {
        return load_pmat(source);
    }
    None
}

pub fn from_runtime_object(entries: &[(String, RuntimeValue)]) -> Option<Material3D> {
    let mut out = Material3D::default();
    let mut any = false;
    apply_runtime_entries(entries, &mut out, &mut any);
    any.then_some(out)
}

pub fn from_static_object(entries: &[(&str, StaticSceneValue)]) -> Option<Material3D> {
    let mut out = Material3D::default();
    let mut any = false;
    apply_static_entries(entries, &mut out, &mut any);
    any.then_some(out)
}

fn load_pmat(source: &str) -> Option<Material3D> {
    let bytes = load_asset(source).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let value = std::panic::catch_unwind(|| Parser::new(text).parse_value_literal()).ok()?;
    let RuntimeValue::Object(entries) = value else {
        return None;
    };
    from_runtime_object(&entries)
}

fn apply_runtime_entries(entries: &[(String, RuntimeValue)], out: &mut Material3D, any: &mut bool) {
    for (name, value) in entries {
        match canonical_key(name) {
            Some("roughnessFactor") => set_f32(value, any, |v| out.roughness_factor = v),
            Some("metallicFactor") => set_f32(value, any, |v| out.metallic_factor = v),
            Some("occlusionStrength") => set_f32(value, any, |v| out.occlusion_strength = v),
            Some("emissiveFactor") => set_color3(value, any, |v| out.emissive_factor = v),
            Some("baseColorFactor") => set_color4(value, any, |v| out.base_color_factor = v),
            Some("normalScale") => set_f32(value, any, |v| out.normal_scale = v),
            Some("alphaCutoff") => set_f32(value, any, |v| out.alpha_cutoff = v),
            Some("alphaMode") => set_alpha_mode(value, any, |v| out.alpha_mode = v),
            Some("doubleSided") => set_bool(value, any, |v| out.double_sided = v),
            Some("baseColorTexture") => {
                set_texture_slot(value, any, |v| out.base_color_texture = v)
            }
            Some("metallicRoughnessTexture") => {
                set_texture_slot(value, any, |v| out.metallic_roughness_texture = v)
            }
            Some("normalTexture") => set_texture_slot(value, any, |v| out.normal_texture = v),
            Some("occlusionTexture") => set_texture_slot(value, any, |v| out.occlusion_texture = v),
            Some("emissiveTexture") => set_texture_slot(value, any, |v| out.emissive_texture = v),
            Some("pbrMetallicRoughness") => {
                if let RuntimeValue::Object(inner) = value {
                    apply_runtime_entries(inner, out, any);
                }
            }
            _ => {}
        }
    }
}

fn apply_static_entries(
    entries: &[(&str, StaticSceneValue)],
    out: &mut Material3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_key(name) {
            Some("roughnessFactor") => set_f32_static(value, any, |v| out.roughness_factor = v),
            Some("metallicFactor") => set_f32_static(value, any, |v| out.metallic_factor = v),
            Some("occlusionStrength") => set_f32_static(value, any, |v| out.occlusion_strength = v),
            Some("emissiveFactor") => set_color3_static(value, any, |v| out.emissive_factor = v),
            Some("baseColorFactor") => set_color4_static(value, any, |v| out.base_color_factor = v),
            Some("normalScale") => set_f32_static(value, any, |v| out.normal_scale = v),
            Some("alphaCutoff") => set_f32_static(value, any, |v| out.alpha_cutoff = v),
            Some("alphaMode") => set_alpha_mode_static(value, any, |v| out.alpha_mode = v),
            Some("doubleSided") => set_bool_static(value, any, |v| out.double_sided = v),
            Some("baseColorTexture") => {
                set_texture_slot_static(value, any, |v| out.base_color_texture = v)
            }
            Some("metallicRoughnessTexture") => {
                set_texture_slot_static(value, any, |v| out.metallic_roughness_texture = v)
            }
            Some("normalTexture") => {
                set_texture_slot_static(value, any, |v| out.normal_texture = v)
            }
            Some("occlusionTexture") => {
                set_texture_slot_static(value, any, |v| out.occlusion_texture = v)
            }
            Some("emissiveTexture") => {
                set_texture_slot_static(value, any, |v| out.emissive_texture = v)
            }
            Some("pbrMetallicRoughness") => {
                if let StaticSceneValue::Object(inner) = value {
                    apply_static_entries(inner, out, any);
                }
            }
            _ => {}
        }
    }
}

fn canonical_key(name: &str) -> Option<&'static str> {
    match name {
        "roughnessFactor" | "roughness_factor" => Some("roughnessFactor"),
        "metallicFactor" | "metallic_factor" => Some("metallicFactor"),
        "occlusionStrength" | "occlusion_strength" => Some("occlusionStrength"),
        "emissiveFactor" | "emissive_factor" => Some("emissiveFactor"),
        "baseColorFactor" | "base_color_factor" | "color" => Some("baseColorFactor"),
        "normalScale" | "normal_scale" => Some("normalScale"),
        "alphaCutoff" | "alpha_cutoff" => Some("alphaCutoff"),
        "alphaMode" | "alpha_mode" => Some("alphaMode"),
        "doubleSided" | "double_sided" => Some("doubleSided"),
        "baseColorTexture" | "base_color_texture" => Some("baseColorTexture"),
        "metallicRoughnessTexture" | "metallic_roughness_texture" => {
            Some("metallicRoughnessTexture")
        }
        "normalTexture" | "normal_texture" => Some("normalTexture"),
        "occlusionTexture" | "occlusion_texture" => Some("occlusionTexture"),
        "emissiveTexture" | "emissive_texture" => Some("emissiveTexture"),
        "pbrMetallicRoughness" | "pbr_metallic_roughness" => Some("pbrMetallicRoughness"),
        _ => None,
    }
}

fn set_f32(value: &RuntimeValue, any: &mut bool, set: impl FnOnce(f32)) {
    if let Some(v) = as_f32(value) {
        set(v);
        *any = true;
    }
}

fn set_bool(value: &RuntimeValue, any: &mut bool, set: impl FnOnce(bool)) {
    if let Some(v) = as_bool(value) {
        set(v);
        *any = true;
    }
}

fn set_alpha_mode(value: &RuntimeValue, any: &mut bool, set: impl FnOnce(u32)) {
    if let Some(v) = as_alpha_mode(value) {
        set(v);
        *any = true;
    }
}

fn set_color4(value: &RuntimeValue, any: &mut bool, set: impl FnOnce([f32; 4])) {
    if let Some(v) = as_color4(value) {
        set(v);
        *any = true;
    }
}

fn set_color3(value: &RuntimeValue, any: &mut bool, set: impl FnOnce([f32; 3])) {
    if let Some(v) = as_color4(value) {
        set([v[0], v[1], v[2]]);
        *any = true;
    }
}

fn set_texture_slot(value: &RuntimeValue, any: &mut bool, set: impl FnOnce(u32)) {
    if let Some(v) = as_texture_slot(value) {
        set(v);
        *any = true;
    }
}

fn as_f32(value: &RuntimeValue) -> Option<f32> {
    match value {
        RuntimeValue::F32(v) => Some(*v),
        RuntimeValue::I32(v) => Some(*v as f32),
        _ => None,
    }
}

fn as_bool(value: &RuntimeValue) -> Option<bool> {
    match value {
        RuntimeValue::Bool(v) => Some(*v),
        _ => None,
    }
}

fn as_alpha_mode(value: &RuntimeValue) -> Option<u32> {
    match value {
        RuntimeValue::Str(v) => match v.as_str() {
            "OPAQUE" | "opaque" => Some(0),
            "MASK" | "mask" => Some(1),
            "BLEND" | "blend" => Some(2),
            _ => None,
        },
        RuntimeValue::I32(v) if (0..=2).contains(v) => Some(*v as u32),
        _ => None,
    }
}

fn as_color4(value: &RuntimeValue) -> Option<[f32; 4]> {
    match value {
        RuntimeValue::Vec4 { x, y, z, w } => Some([*x, *y, *z, *w]),
        RuntimeValue::Vec3 { x, y, z } => Some([*x, *y, *z, 1.0]),
        _ => None,
    }
}

fn as_texture_slot(value: &RuntimeValue) -> Option<u32> {
    match value {
        RuntimeValue::I32(v) if *v >= 0 => Some(*v as u32),
        RuntimeValue::Object(entries) => {
            entries
                .iter()
                .find_map(|(name, inner)| match name.as_str() {
                    "index" | "slot" => match inner {
                        RuntimeValue::I32(v) if *v >= 0 => Some(*v as u32),
                        _ => None,
                    },
                    _ => None,
                })
        }
        _ => None,
    }
}

fn set_f32_static(value: &StaticSceneValue, any: &mut bool, set: impl FnOnce(f32)) {
    if let Some(v) = as_f32_static(value) {
        set(v);
        *any = true;
    }
}

fn set_bool_static(value: &StaticSceneValue, any: &mut bool, set: impl FnOnce(bool)) {
    if let Some(v) = as_bool_static(value) {
        set(v);
        *any = true;
    }
}

fn set_alpha_mode_static(value: &StaticSceneValue, any: &mut bool, set: impl FnOnce(u32)) {
    if let Some(v) = as_alpha_mode_static(value) {
        set(v);
        *any = true;
    }
}

fn set_color4_static(value: &StaticSceneValue, any: &mut bool, set: impl FnOnce([f32; 4])) {
    if let Some(v) = as_color4_static(value) {
        set(v);
        *any = true;
    }
}

fn set_color3_static(value: &StaticSceneValue, any: &mut bool, set: impl FnOnce([f32; 3])) {
    if let Some(v) = as_color4_static(value) {
        set([v[0], v[1], v[2]]);
        *any = true;
    }
}

fn set_texture_slot_static(value: &StaticSceneValue, any: &mut bool, set: impl FnOnce(u32)) {
    if let Some(v) = as_texture_slot_static(value) {
        set(v);
        *any = true;
    }
}

fn as_f32_static(value: &StaticSceneValue) -> Option<f32> {
    match value {
        StaticSceneValue::F32(v) => Some(*v),
        StaticSceneValue::I32(v) => Some(*v as f32),
        _ => None,
    }
}

fn as_bool_static(value: &StaticSceneValue) -> Option<bool> {
    match value {
        StaticSceneValue::Bool(v) => Some(*v),
        _ => None,
    }
}

fn as_alpha_mode_static(value: &StaticSceneValue) -> Option<u32> {
    match value {
        StaticSceneValue::Str(v) => match *v {
            "OPAQUE" | "opaque" => Some(0),
            "MASK" | "mask" => Some(1),
            "BLEND" | "blend" => Some(2),
            _ => None,
        },
        StaticSceneValue::I32(v) if (0..=2).contains(v) => Some(*v as u32),
        _ => None,
    }
}

fn as_color4_static(value: &StaticSceneValue) -> Option<[f32; 4]> {
    match value {
        StaticSceneValue::Vec4 { x, y, z, w } => Some([*x, *y, *z, *w]),
        StaticSceneValue::Vec3 { x, y, z } => Some([*x, *y, *z, 1.0]),
        _ => None,
    }
}

fn as_texture_slot_static(value: &StaticSceneValue) -> Option<u32> {
    match value {
        StaticSceneValue::I32(v) if *v >= 0 => Some(*v as u32),
        StaticSceneValue::Object(entries) => entries.iter().find_map(|(name, inner)| match *name {
            "index" | "slot" => match inner {
                StaticSceneValue::I32(v) if *v >= 0 => Some(*v as u32),
                _ => None,
            },
            _ => None,
        }),
        _ => None,
    }
}

use perro_io::load_asset;
use perro_render_bridge::{
    CustomMaterial3D, CustomMaterialParam3D, CustomMaterialParamValue3D, Material3D,
    StandardMaterial3D, ToonMaterial3D, UnlitMaterial3D,
};
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
    let mut any = false;
    let out = material_from_runtime_entries(entries, &mut any);
    any.then_some(out)
}

pub fn from_static_object(entries: &[(&str, StaticSceneValue)]) -> Option<Material3D> {
    let mut any = false;
    let out = material_from_static_entries(entries, &mut any);
    any.then_some(out)
}

fn load_pmat(source: &str) -> Option<Material3D> {
    let bytes = load_asset(source).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    if pmat_looks_like_object(text) {
        if let Some(value) =
            std::panic::catch_unwind(|| Parser::new(text).parse_value_literal()).ok()
            && let RuntimeValue::Object(entries) = value
            && let Some(material) = from_runtime_object(&entries)
        {
            return Some(material);
        }
        return None;
    }

    let entries = parse_pmat_key_values(text)?;
    from_runtime_object(&entries)
}

fn pmat_looks_like_object(text: &str) -> bool {
    text.lines()
        .map(strip_line_comment)
        .map(str::trim)
        .find(|line| !line.is_empty())
        .is_some_and(|line| line.starts_with('{'))
}

fn parse_pmat_key_values(text: &str) -> Option<Vec<(String, RuntimeValue)>> {
    let mut entries = Vec::new();
    for raw_line in text.lines() {
        let line = strip_line_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let (raw_key, raw_value) = line.split_once('=')?;
        let key = raw_key.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let value_text = raw_value.trim().trim_end_matches(',');
        if value_text.is_empty() {
            continue;
        }
        let value = parse_kv_value(value_text)?;
        entries.push((key, value));
    }
    (!entries.is_empty()).then_some(entries)
}

fn strip_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut escape = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b == b'#' {
            return &line[..i];
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            return &line[..i];
        }
        i += 1;
    }
    line
}

fn parse_kv_value(text: &str) -> Option<RuntimeValue> {
    let text = text.trim();
    if let Some(value) = parse_vec_value(text) {
        return Some(value);
    }
    if text.eq_ignore_ascii_case("true") {
        return Some(RuntimeValue::Bool(true));
    }
    if text.eq_ignore_ascii_case("false") {
        return Some(RuntimeValue::Bool(false));
    }
    if let Some(inner) = text.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        return Some(RuntimeValue::Str(inner.to_string()));
    }
    if let Ok(v) = text.parse::<i32>() {
        return Some(RuntimeValue::I32(v));
    }
    if let Ok(v) = text.parse::<f32>() {
        return Some(RuntimeValue::F32(v));
    }
    Some(RuntimeValue::Str(text.to_string()))
}

fn parse_vec_value(text: &str) -> Option<RuntimeValue> {
    let inner = text.strip_prefix('(')?.strip_suffix(')')?;
    let numbers = inner
        .split(',')
        .map(|token| token.trim().parse::<f32>().ok())
        .collect::<Option<Vec<_>>>()?;
    match numbers.as_slice() {
        [x, y] => Some(RuntimeValue::Vec2 { x: *x, y: *y }),
        [x, y, z] => Some(RuntimeValue::Vec3 {
            x: *x,
            y: *y,
            z: *z,
        }),
        [x, y, z, w] => Some(RuntimeValue::Vec4 {
            x: *x,
            y: *y,
            z: *z,
            w: *w,
        }),
        _ => None,
    }
}

fn material_from_runtime_entries(entries: &[(String, RuntimeValue)], any: &mut bool) -> Material3D {
    let kind = material_type_from_first_runtime(entries);
    if has_type_first_runtime(entries) {
        *any = true;
    }
    match kind {
        MaterialType3D::Standard => {
            let mut params = StandardMaterial3D::default();
            apply_standard_runtime(entries, &mut params, any);
            Material3D::Standard(params)
        }
        MaterialType3D::Unlit => {
            let mut params = UnlitMaterial3D::default();
            apply_unlit_runtime(entries, &mut params, any);
            Material3D::Unlit(params)
        }
        MaterialType3D::Toon => {
            let mut params = ToonMaterial3D::default();
            apply_toon_runtime(entries, &mut params, any);
            Material3D::Toon(params)
        }
        MaterialType3D::Custom => {
            let mut params = CustomMaterial3D {
                shader_path: "".into(),
                params: std::borrow::Cow::Borrowed(&[]),
            };
            apply_custom_runtime(entries, &mut params, any);
            Material3D::Custom(params)
        }
    }
}

fn material_from_static_entries(
    entries: &[(&str, StaticSceneValue)],
    any: &mut bool,
) -> Material3D {
    let kind = material_type_from_first_static(entries);
    if has_type_first_static(entries) {
        *any = true;
    }
    match kind {
        MaterialType3D::Standard => {
            let mut params = StandardMaterial3D::default();
            apply_standard_static(entries, &mut params, any);
            Material3D::Standard(params)
        }
        MaterialType3D::Unlit => {
            let mut params = UnlitMaterial3D::default();
            apply_unlit_static(entries, &mut params, any);
            Material3D::Unlit(params)
        }
        MaterialType3D::Toon => {
            let mut params = ToonMaterial3D::default();
            apply_toon_static(entries, &mut params, any);
            Material3D::Toon(params)
        }
        MaterialType3D::Custom => {
            let mut params = CustomMaterial3D {
                shader_path: "".into(),
                params: std::borrow::Cow::Borrowed(&[]),
            };
            apply_custom_static(entries, &mut params, any);
            Material3D::Custom(params)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaterialType3D {
    Standard,
    Unlit,
    Toon,
    Custom,
}

fn material_type_from_first_runtime(entries: &[(String, RuntimeValue)]) -> MaterialType3D {
    let Some((name, value)) = entries.first() else {
        return MaterialType3D::Standard;
    };
    if !name.eq_ignore_ascii_case("type") {
        return MaterialType3D::Standard;
    }
    match value {
        RuntimeValue::Str(v) => parse_material_type(v),
        _ => MaterialType3D::Standard,
    }
}

fn has_type_first_runtime(entries: &[(String, RuntimeValue)]) -> bool {
    entries
        .first()
        .is_some_and(|(name, _)| name.eq_ignore_ascii_case("type"))
}

fn material_type_from_first_static(entries: &[(&str, StaticSceneValue)]) -> MaterialType3D {
    let Some((name, value)) = entries.first() else {
        return MaterialType3D::Standard;
    };
    if !name.eq_ignore_ascii_case("type") {
        return MaterialType3D::Standard;
    }
    match value {
        StaticSceneValue::Str(v) => parse_material_type(v),
        _ => MaterialType3D::Standard,
    }
}

fn has_type_first_static(entries: &[(&str, StaticSceneValue)]) -> bool {
    entries
        .first()
        .is_some_and(|(name, _)| name.eq_ignore_ascii_case("type"))
}

fn parse_material_type(value: &str) -> MaterialType3D {
    match value {
        "standard" | "Standard" | "pbr" | "PBR" => MaterialType3D::Standard,
        "unlit" | "Unlit" => MaterialType3D::Unlit,
        "toon" | "Toon" | "cel" | "Cel" => MaterialType3D::Toon,
        "custom" | "Custom" => MaterialType3D::Custom,
        _ => MaterialType3D::Standard,
    }
}

fn apply_standard_runtime(
    entries: &[(String, RuntimeValue)],
    out: &mut StandardMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_standard_key(name) {
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
                    apply_standard_runtime(inner, out, any);
                }
            }
            _ => {}
        }
    }
}

fn apply_unlit_runtime(
    entries: &[(String, RuntimeValue)],
    out: &mut UnlitMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_unlit_key(name) {
            Some("baseColorFactor") => set_color4(value, any, |v| out.base_color_factor = v),
            Some("emissiveFactor") => set_color3(value, any, |v| out.emissive_factor = v),
            Some("alphaCutoff") => set_f32(value, any, |v| out.alpha_cutoff = v),
            Some("alphaMode") => set_alpha_mode(value, any, |v| out.alpha_mode = v),
            Some("doubleSided") => set_bool(value, any, |v| out.double_sided = v),
            Some("baseColorTexture") => {
                set_texture_slot(value, any, |v| out.base_color_texture = v)
            }
            _ => {}
        }
    }
}

fn apply_toon_runtime(
    entries: &[(String, RuntimeValue)],
    out: &mut ToonMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_toon_key(name) {
            Some("baseColorFactor") => set_color4(value, any, |v| out.base_color_factor = v),
            Some("emissiveFactor") => set_color3(value, any, |v| out.emissive_factor = v),
            Some("alphaCutoff") => set_f32(value, any, |v| out.alpha_cutoff = v),
            Some("alphaMode") => set_alpha_mode(value, any, |v| out.alpha_mode = v),
            Some("doubleSided") => set_bool(value, any, |v| out.double_sided = v),
            Some("baseColorTexture") => {
                set_texture_slot(value, any, |v| out.base_color_texture = v)
            }
            Some("rampTexture") => set_texture_slot(value, any, |v| out.ramp_texture = v),
            Some("bandCount") => set_u32(value, any, |v| out.band_count = v),
            Some("rimStrength") => set_f32(value, any, |v| out.rim_strength = v),
            Some("outlineWidth") => set_f32(value, any, |v| out.outline_width = v),
            _ => {}
        }
    }
}

fn apply_custom_runtime(
    entries: &[(String, RuntimeValue)],
    out: &mut CustomMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_custom_key(name) {
            Some("shaderPath") => {
                if let RuntimeValue::Str(v) = value {
                    out.shader_path = v.clone().into();
                    *any = true;
                }
            }
            Some("params") => {
                if let Some(params) = as_custom_params(value) {
                    out.params = std::borrow::Cow::Owned(params);
                    *any = true;
                }
            }
            _ => {}
        }
    }
}

fn apply_standard_static(
    entries: &[(&str, StaticSceneValue)],
    out: &mut StandardMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_standard_key(name) {
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
                    apply_standard_static(*inner, out, any);
                }
            }
            _ => {}
        }
    }
}

fn apply_unlit_static(
    entries: &[(&str, StaticSceneValue)],
    out: &mut UnlitMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_unlit_key(name) {
            Some("baseColorFactor") => set_color4_static(value, any, |v| out.base_color_factor = v),
            Some("emissiveFactor") => set_color3_static(value, any, |v| out.emissive_factor = v),
            Some("alphaCutoff") => set_f32_static(value, any, |v| out.alpha_cutoff = v),
            Some("alphaMode") => set_alpha_mode_static(value, any, |v| out.alpha_mode = v),
            Some("doubleSided") => set_bool_static(value, any, |v| out.double_sided = v),
            Some("baseColorTexture") => {
                set_texture_slot_static(value, any, |v| out.base_color_texture = v)
            }
            _ => {}
        }
    }
}

fn apply_toon_static(
    entries: &[(&str, StaticSceneValue)],
    out: &mut ToonMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_toon_key(name) {
            Some("baseColorFactor") => set_color4_static(value, any, |v| out.base_color_factor = v),
            Some("emissiveFactor") => set_color3_static(value, any, |v| out.emissive_factor = v),
            Some("alphaCutoff") => set_f32_static(value, any, |v| out.alpha_cutoff = v),
            Some("alphaMode") => set_alpha_mode_static(value, any, |v| out.alpha_mode = v),
            Some("doubleSided") => set_bool_static(value, any, |v| out.double_sided = v),
            Some("baseColorTexture") => {
                set_texture_slot_static(value, any, |v| out.base_color_texture = v)
            }
            Some("rampTexture") => set_texture_slot_static(value, any, |v| out.ramp_texture = v),
            Some("bandCount") => set_u32_static(value, any, |v| out.band_count = v),
            Some("rimStrength") => set_f32_static(value, any, |v| out.rim_strength = v),
            Some("outlineWidth") => set_f32_static(value, any, |v| out.outline_width = v),
            _ => {}
        }
    }
}

fn apply_custom_static(
    entries: &[(&str, StaticSceneValue)],
    out: &mut CustomMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_custom_key(name) {
            Some("shaderPath") => {
                if let StaticSceneValue::Str(v) = value {
                    out.shader_path = (*v).into();
                    *any = true;
                }
            }
            Some("params") => {
                if let Some(params) = as_custom_params_static(value) {
                    out.params = std::borrow::Cow::Owned(params);
                    *any = true;
                }
            }
            _ => {}
        }
    }
}

fn canonical_standard_key(name: &str) -> Option<&'static str> {
    match name {
        "type" | "Type" => None,
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

fn canonical_unlit_key(name: &str) -> Option<&'static str> {
    match name {
        "type" | "Type" => None,
        "baseColorFactor" | "base_color_factor" | "color" => Some("baseColorFactor"),
        "emissiveFactor" | "emissive_factor" => Some("emissiveFactor"),
        "alphaCutoff" | "alpha_cutoff" => Some("alphaCutoff"),
        "alphaMode" | "alpha_mode" => Some("alphaMode"),
        "doubleSided" | "double_sided" => Some("doubleSided"),
        "baseColorTexture" | "base_color_texture" => Some("baseColorTexture"),
        _ => None,
    }
}

fn canonical_toon_key(name: &str) -> Option<&'static str> {
    match name {
        "type" | "Type" => None,
        "baseColorFactor" | "base_color_factor" | "color" => Some("baseColorFactor"),
        "emissiveFactor" | "emissive_factor" => Some("emissiveFactor"),
        "alphaCutoff" | "alpha_cutoff" => Some("alphaCutoff"),
        "alphaMode" | "alpha_mode" => Some("alphaMode"),
        "doubleSided" | "double_sided" => Some("doubleSided"),
        "baseColorTexture" | "base_color_texture" => Some("baseColorTexture"),
        "rampTexture" | "ramp_texture" => Some("rampTexture"),
        "bandCount" | "band_count" => Some("bandCount"),
        "rimStrength" | "rim_strength" => Some("rimStrength"),
        "outlineWidth" | "outline_width" => Some("outlineWidth"),
        _ => None,
    }
}

fn canonical_custom_key(name: &str) -> Option<&'static str> {
    match name {
        "type" | "Type" => None,
        "shader" | "shaderPath" | "shader_path" | "path" => Some("shaderPath"),
        "params" | "customParams" | "custom_params" => Some("params"),
        _ => None,
    }
}

fn set_f32(value: &RuntimeValue, any: &mut bool, set: impl FnOnce(f32)) {
    if let Some(v) = as_f32(value) {
        set(v);
        *any = true;
    }
}

fn set_u32(value: &RuntimeValue, any: &mut bool, set: impl FnOnce(u32)) {
    if let Some(v) = as_u32(value) {
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

fn as_u32(value: &RuntimeValue) -> Option<u32> {
    match value {
        RuntimeValue::I32(v) if *v >= 0 => Some(*v as u32),
        RuntimeValue::F32(v) if *v >= 0.0 => Some(*v as u32),
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

fn as_custom_param_value(value: &RuntimeValue) -> Option<CustomMaterialParamValue3D> {
    match value {
        RuntimeValue::Bool(v) => Some(CustomMaterialParamValue3D::Bool(*v)),
        RuntimeValue::I32(v) => Some(CustomMaterialParamValue3D::I32(*v)),
        RuntimeValue::F32(v) => Some(CustomMaterialParamValue3D::F32(*v)),
        RuntimeValue::Vec2 { x, y } => Some(CustomMaterialParamValue3D::Vec2([*x, *y])),
        RuntimeValue::Vec3 { x, y, z } => Some(CustomMaterialParamValue3D::Vec3([*x, *y, *z])),
        RuntimeValue::Vec4 { x, y, z, w } => {
            Some(CustomMaterialParamValue3D::Vec4([*x, *y, *z, *w]))
        }
        _ => None,
    }
}

fn as_custom_params(value: &RuntimeValue) -> Option<Vec<CustomMaterialParam3D>> {
    match value {
        RuntimeValue::Object(entries) => {
            let mut out = Vec::new();
            for (name, inner) in entries {
                if let Some(val) = as_custom_param_value(inner) {
                    out.push(CustomMaterialParam3D {
                        name: Some(name.clone().into()),
                        value: val,
                    });
                }
            }
            Some(out)
        }
        other => as_custom_param_value(other).map(|val| {
            vec![CustomMaterialParam3D {
                name: None,
                value: val,
            }]
        }),
    }
}

fn set_f32_static(value: &StaticSceneValue, any: &mut bool, set: impl FnOnce(f32)) {
    if let Some(v) = as_f32_static(value) {
        set(v);
        *any = true;
    }
}

fn set_u32_static(value: &StaticSceneValue, any: &mut bool, set: impl FnOnce(u32)) {
    if let Some(v) = as_u32_static(value) {
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

fn as_u32_static(value: &StaticSceneValue) -> Option<u32> {
    match value {
        StaticSceneValue::I32(v) if *v >= 0 => Some(*v as u32),
        StaticSceneValue::F32(v) if *v >= 0.0 => Some(*v as u32),
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
        StaticSceneValue::Object(entries) => {
            (*entries).iter().find_map(|(name, inner)| match *name {
                "index" | "slot" => match inner {
                    StaticSceneValue::I32(v) if *v >= 0 => Some(*v as u32),
                    _ => None,
                },
                _ => None,
            })
        }
        _ => None,
    }
}

fn as_custom_param_value_static(value: &StaticSceneValue) -> Option<CustomMaterialParamValue3D> {
    match value {
        StaticSceneValue::Bool(v) => Some(CustomMaterialParamValue3D::Bool(*v)),
        StaticSceneValue::I32(v) => Some(CustomMaterialParamValue3D::I32(*v)),
        StaticSceneValue::F32(v) => Some(CustomMaterialParamValue3D::F32(*v)),
        StaticSceneValue::Vec2 { x, y } => Some(CustomMaterialParamValue3D::Vec2([*x, *y])),
        StaticSceneValue::Vec3 { x, y, z } => Some(CustomMaterialParamValue3D::Vec3([*x, *y, *z])),
        StaticSceneValue::Vec4 { x, y, z, w } => {
            Some(CustomMaterialParamValue3D::Vec4([*x, *y, *z, *w]))
        }
        _ => None,
    }
}

fn as_custom_params_static(value: &StaticSceneValue) -> Option<Vec<CustomMaterialParam3D>> {
    match value {
        StaticSceneValue::Object(entries) => {
            let mut out = Vec::new();
            for (name, inner) in (*entries).iter() {
                if let Some(val) = as_custom_param_value_static(inner) {
                    out.push(CustomMaterialParam3D {
                        name: Some((*name).into()),
                        value: val,
                    });
                }
            }
            Some(out)
        }
        other => as_custom_param_value_static(other).map(|val| {
            vec![CustomMaterialParam3D {
                name: None,
                value: val,
            }]
        }),
    }
}

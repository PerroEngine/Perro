use perro_io::load_asset;
use perro_render_bridge::{
    CustomMaterial3D, CustomMaterialParam3D, CustomMaterialParamValue3D, Material3D,
    StandardMaterial3D, ToonMaterial3D, UnlitMaterial3D,
};
use perro_scene::{Parser, SceneObjectField, SceneValue};

pub fn load_from_source(source: &str) -> Option<Material3D> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    let (path, fragment) = split_source_fragment(source);
    if path.ends_with(".pmat") {
        return load_pmat(path);
    }
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        return load_gltf_material(path, fragment);
    }
    None
}

pub fn from_object(entries: &[SceneObjectField]) -> Option<Material3D> {
    let mut any = false;
    let out = material_from_entries(entries, &mut any);
    any.then_some(out)
}

fn load_pmat(source: &str) -> Option<Material3D> {
    let bytes = load_asset(source).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    if pmat_looks_like_object(text) {
        if let Some(value) =
            std::panic::catch_unwind(|| Parser::new(text).parse_value_literal()).ok()
            && let SceneValue::Object(entries) = value
            && let Some(material) = from_object(entries.as_ref())
        {
            return Some(material);
        }
        return None;
    }

    let entries = parse_pmat_key_values(text)?;
    from_object(entries.as_ref())
}

fn load_gltf_material(path: &str, fragment: Option<&str>) -> Option<Material3D> {
    let bytes = load_asset(path).ok()?;
    let (doc, _buffers, _images) = gltf::import_slice(&bytes).ok()?;
    let index = parse_fragment_index(fragment, &["mat", "material"]).unwrap_or(0) as usize;

    let material = doc.materials().nth(index);
    let Some(material) = material else {
        return Some(Material3D::Standard(StandardMaterial3D::default()));
    };

    let pbr = material.pbr_metallic_roughness();
    let base_color = pbr.base_color_factor();
    let emissive_factor = material.emissive_factor();
    Some(Material3D::Standard(StandardMaterial3D {
        base_color_factor: base_color,
        roughness_factor: pbr.roughness_factor(),
        metallic_factor: pbr.metallic_factor(),
        occlusion_strength: material
            .occlusion_texture()
            .map(|occ| occ.strength())
            .unwrap_or(1.0),
        emissive_factor,
        alpha_mode: match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => 0,
            gltf::material::AlphaMode::Mask => 1,
            gltf::material::AlphaMode::Blend => 2,
        },
        alpha_cutoff: material.alpha_cutoff().unwrap_or(0.5),
        double_sided: material.double_sided(),
        flat_shading: false,
        normal_scale: material
            .normal_texture()
            .map(|normal| normal.scale())
            .unwrap_or(1.0),
        base_color_texture: pbr
            .base_color_texture()
            .map(|tex| tex.texture().index() as u32)
            .unwrap_or(u32::MAX),
        metallic_roughness_texture: pbr
            .metallic_roughness_texture()
            .map(|tex| tex.texture().index() as u32)
            .unwrap_or(u32::MAX),
        normal_texture: material
            .normal_texture()
            .map(|tex| tex.texture().index() as u32)
            .unwrap_or(u32::MAX),
        occlusion_texture: material
            .occlusion_texture()
            .map(|tex| tex.texture().index() as u32)
            .unwrap_or(u32::MAX),
        emissive_texture: material
            .emissive_texture()
            .map(|tex| tex.texture().index() as u32)
            .unwrap_or(u32::MAX),
    }))
}

fn split_source_fragment(source: &str) -> (&str, Option<&str>) {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return (source, None);
    };
    if path.is_empty() {
        return (source, None);
    }
    if selector.contains('/') || selector.contains('\\') {
        return (source, None);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return (path, Some(selector));
    }
    (source, None)
}

fn parse_fragment_index(fragment: Option<&str>, keys: &[&str]) -> Option<u32> {
    let fragment = fragment?;
    if let Some((name, rest)) = fragment.split_once('[') {
        let name = name.trim();
        if keys.contains(&name) {
            let value = rest.strip_suffix(']')?.trim();
            if let Ok(parsed) = value.parse::<u32>() {
                return Some(parsed);
            }
        }
    }
    None
}

fn pmat_looks_like_object(text: &str) -> bool {
    text.lines()
        .map(strip_line_comment)
        .map(str::trim)
        .find(|line| !line.is_empty())
        .is_some_and(|line| line.starts_with('{'))
}

fn parse_pmat_key_values(text: &str) -> Option<Vec<SceneObjectField>> {
    let mut entries = Vec::new();
    for raw_line in text.lines() {
        let line = strip_line_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let (raw_key, raw_value) = line.split_once('=')?;
        let key: std::borrow::Cow<'static, str> =
            std::borrow::Cow::Owned(raw_key.trim().to_string());
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

fn parse_kv_value(text: &str) -> Option<SceneValue> {
    let text = text.trim();
    if let Some(value) = parse_vec_value(text) {
        return Some(value);
    }
    if text.eq_ignore_ascii_case("true") {
        return Some(SceneValue::Bool(true));
    }
    if text.eq_ignore_ascii_case("false") {
        return Some(SceneValue::Bool(false));
    }
    if let Some(inner) = text.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        return Some(SceneValue::Str(inner.to_string().into()));
    }
    if let Ok(v) = text.parse::<i32>() {
        return Some(SceneValue::I32(v));
    }
    if let Ok(v) = text.parse::<f32>() {
        return Some(SceneValue::F32(v));
    }
    Some(SceneValue::Str(text.to_string().into()))
}

fn parse_vec_value(text: &str) -> Option<SceneValue> {
    let inner = text.strip_prefix('(')?.strip_suffix(')')?;
    let numbers = inner
        .split(',')
        .map(|token| token.trim().parse::<f32>().ok())
        .collect::<Option<Vec<_>>>()?;
    match numbers.as_slice() {
        [x, y] => Some(SceneValue::Vec2 { x: *x, y: *y }),
        [x, y, z] => Some(SceneValue::Vec3 {
            x: *x,
            y: *y,
            z: *z,
        }),
        [x, y, z, w] => Some(SceneValue::Vec4 {
            x: *x,
            y: *y,
            z: *z,
            w: *w,
        }),
        _ => None,
    }
}

fn material_from_entries(entries: &[SceneObjectField], any: &mut bool) -> Material3D {
    let kind = material_type_from_first(entries);
    if has_type_first(entries) {
        *any = true;
    }
    match kind {
        MaterialType3D::Standard => {
            let mut params = StandardMaterial3D::default();
            apply_standard(entries, &mut params, any);
            Material3D::Standard(params)
        }
        MaterialType3D::Unlit => {
            let mut params = UnlitMaterial3D::default();
            apply_unlit(entries, &mut params, any);
            Material3D::Unlit(params)
        }
        MaterialType3D::Toon => {
            let mut params = ToonMaterial3D::default();
            apply_toon(entries, &mut params, any);
            Material3D::Toon(params)
        }
        MaterialType3D::Custom => {
            let mut params = CustomMaterial3D {
                shader_path: "".into(),
                params: std::borrow::Cow::Borrowed(&[]),
            };
            apply_custom(entries, &mut params, any);
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

fn material_type_from_first(entries: &[SceneObjectField]) -> MaterialType3D {
    let Some((name, value)) = entries.first() else {
        return MaterialType3D::Standard;
    };
    if !name.eq_ignore_ascii_case("type") {
        return MaterialType3D::Standard;
    }
    match value {
        SceneValue::Str(v) => parse_material_type(v),
        _ => MaterialType3D::Standard,
    }
}

fn has_type_first(entries: &[SceneObjectField]) -> bool {
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

fn apply_standard(entries: &[SceneObjectField], out: &mut StandardMaterial3D, any: &mut bool) {
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
            Some("flatShading") => set_bool(value, any, |v| out.flat_shading = v),
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
                if let SceneValue::Object(inner) = value {
                    apply_standard(inner.as_ref(), out, any);
                }
            }
            _ => {}
        }
    }
}

fn apply_unlit(entries: &[SceneObjectField], out: &mut UnlitMaterial3D, any: &mut bool) {
    for (name, value) in entries {
        match canonical_unlit_key(name) {
            Some("baseColorFactor") => set_color4(value, any, |v| out.base_color_factor = v),
            Some("emissiveFactor") => set_color3(value, any, |v| out.emissive_factor = v),
            Some("alphaCutoff") => set_f32(value, any, |v| out.alpha_cutoff = v),
            Some("alphaMode") => set_alpha_mode(value, any, |v| out.alpha_mode = v),
            Some("doubleSided") => set_bool(value, any, |v| out.double_sided = v),
            Some("flatShading") => set_bool(value, any, |v| out.flat_shading = v),
            Some("baseColorTexture") => {
                set_texture_slot(value, any, |v| out.base_color_texture = v)
            }
            _ => {}
        }
    }
}

fn apply_toon(entries: &[SceneObjectField], out: &mut ToonMaterial3D, any: &mut bool) {
    for (name, value) in entries {
        match canonical_toon_key(name) {
            Some("baseColorFactor") => set_color4(value, any, |v| out.base_color_factor = v),
            Some("emissiveFactor") => set_color3(value, any, |v| out.emissive_factor = v),
            Some("alphaCutoff") => set_f32(value, any, |v| out.alpha_cutoff = v),
            Some("alphaMode") => set_alpha_mode(value, any, |v| out.alpha_mode = v),
            Some("doubleSided") => set_bool(value, any, |v| out.double_sided = v),
            Some("flatShading") => set_bool(value, any, |v| out.flat_shading = v),
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

fn apply_custom(entries: &[SceneObjectField], out: &mut CustomMaterial3D, any: &mut bool) {
    for (name, value) in entries {
        match canonical_custom_key(name) {
            Some("shaderPath") => {
                if let Some(v) = as_source_token(value) {
                    out.shader_path = v.into();
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
        "flatShading" | "flat_shading" => Some("flatShading"),
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
        "flatShading" | "flat_shading" => Some("flatShading"),
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
        "flatShading" | "flat_shading" => Some("flatShading"),
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

fn set_f32(value: &SceneValue, any: &mut bool, set: impl FnOnce(f32)) {
    if let Some(v) = as_f32(value) {
        set(v);
        *any = true;
    }
}

fn set_u32(value: &SceneValue, any: &mut bool, set: impl FnOnce(u32)) {
    if let Some(v) = as_u32(value) {
        set(v);
        *any = true;
    }
}

fn set_bool(value: &SceneValue, any: &mut bool, set: impl FnOnce(bool)) {
    if let Some(v) = as_bool(value) {
        set(v);
        *any = true;
    }
}

fn set_alpha_mode(value: &SceneValue, any: &mut bool, set: impl FnOnce(u32)) {
    if let Some(v) = as_alpha_mode(value) {
        set(v);
        *any = true;
    }
}

fn set_color4(value: &SceneValue, any: &mut bool, set: impl FnOnce([f32; 4])) {
    if let Some(v) = as_color4(value) {
        set(v);
        *any = true;
    }
}

fn set_color3(value: &SceneValue, any: &mut bool, set: impl FnOnce([f32; 3])) {
    if let Some(v) = as_color4(value) {
        set([v[0], v[1], v[2]]);
        *any = true;
    }
}

fn set_texture_slot(value: &SceneValue, any: &mut bool, set: impl FnOnce(u32)) {
    if let Some(v) = as_texture_slot(value) {
        set(v);
        *any = true;
    }
}

fn as_f32(value: &SceneValue) -> Option<f32> {
    match value {
        SceneValue::F32(v) => Some(*v),
        SceneValue::I32(v) => Some(*v as f32),
        _ => None,
    }
}

fn as_u32(value: &SceneValue) -> Option<u32> {
    match value {
        SceneValue::I32(v) if *v >= 0 => Some(*v as u32),
        SceneValue::F32(v) if *v >= 0.0 => Some(*v as u32),
        _ => None,
    }
}

fn as_bool(value: &SceneValue) -> Option<bool> {
    match value {
        SceneValue::Bool(v) => Some(*v),
        _ => None,
    }
}

fn as_alpha_mode(value: &SceneValue) -> Option<u32> {
    match value {
        SceneValue::Str(v) => match v.as_ref() {
            "OPAQUE" | "opaque" => Some(0),
            "MASK" | "mask" => Some(1),
            "BLEND" | "blend" => Some(2),
            _ => None,
        },
        SceneValue::I32(v) if (0..=2).contains(v) => Some(*v as u32),
        _ => None,
    }
}

fn as_source_token(value: &SceneValue) -> Option<String> {
    match value {
        SceneValue::Str(v) => Some(v.to_string()),
        SceneValue::Hashed(v) => Some(v.to_string()),
        SceneValue::Key(v) => Some(v.to_string()),
        _ => None,
    }
}

fn as_color4(value: &SceneValue) -> Option<[f32; 4]> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some([*x, *y, *z, *w]),
        SceneValue::Vec3 { x, y, z } => Some([*x, *y, *z, 1.0]),
        _ => None,
    }
}

fn as_texture_slot(value: &SceneValue) -> Option<u32> {
    match value {
        SceneValue::I32(v) if *v >= 0 => Some(*v as u32),
        SceneValue::Object(entries) => {
            entries
                .iter()
                .find_map(|(name, inner)| match name.as_ref() {
                    "index" | "slot" => match inner {
                        SceneValue::I32(v) if *v >= 0 => Some(*v as u32),
                        _ => None,
                    },
                    _ => None,
                })
        }
        _ => None,
    }
}

fn as_custom_param_value(value: &SceneValue) -> Option<CustomMaterialParamValue3D> {
    match value {
        SceneValue::Bool(v) => Some(CustomMaterialParamValue3D::Bool(*v)),
        SceneValue::I32(v) => Some(CustomMaterialParamValue3D::I32(*v)),
        SceneValue::F32(v) => Some(CustomMaterialParamValue3D::F32(*v)),
        SceneValue::Vec2 { x, y } => Some(CustomMaterialParamValue3D::Vec2([*x, *y])),
        SceneValue::Vec3 { x, y, z } => Some(CustomMaterialParamValue3D::Vec3([*x, *y, *z])),
        SceneValue::Vec4 { x, y, z, w } => Some(CustomMaterialParamValue3D::Vec4([*x, *y, *z, *w])),
        _ => None,
    }
}

fn as_custom_params(value: &SceneValue) -> Option<Vec<CustomMaterialParam3D>> {
    match value {
        SceneValue::Object(entries) => {
            let mut out = Vec::new();
            for (name, inner) in entries.as_ref() {
                if let Some(val) = as_custom_param_value(inner) {
                    out.push(CustomMaterialParam3D {
                        name: Some(name.clone()),
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

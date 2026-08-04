use super::*;

pub(super) fn material_from_runtime_entries(
    entries: &[SceneObjectField],
) -> Option<MaterialLiteral> {
    let mut any = false;
    let kind = material_type_from_first_runtime(entries);
    match kind {
        MaterialType::Standard => {
            let mut out = StandardMaterial3D::default();
            apply_standard_runtime_entries(entries, &mut out, &mut any);
            apply_vertex_modifiers(entries, &mut out.vertex_modifiers, &mut any);
            any.then_some(MaterialLiteral::Standard(out))
        }
        MaterialType::Unlit => {
            let mut out = UnlitMaterial3D::default();
            apply_unlit_runtime_entries(entries, &mut out, &mut any);
            apply_vertex_modifiers(entries, &mut out.vertex_modifiers, &mut any);
            any.then_some(MaterialLiteral::Unlit(out))
        }
        MaterialType::Toon => {
            let mut out = ToonMaterial3D::default();
            apply_toon_runtime_entries(entries, &mut out, &mut any);
            apply_vertex_modifiers(entries, &mut out.vertex_modifiers, &mut any);
            any.then_some(MaterialLiteral::Toon(out))
        }
        MaterialType::HandDrawn => {
            let mut out = HandDrawnMaterial3D::default();
            apply_hand_drawn_runtime_entries(entries, &mut out, &mut any);
            apply_vertex_modifiers(entries, &mut out.vertex_modifiers, &mut any);
            any.then_some(MaterialLiteral::HandDrawn(out))
        }
        MaterialType::PixelSurface => {
            let mut out = PixelSurfaceMaterial3D::default();
            apply_pixel_surface_runtime_entries(entries, &mut out, &mut any);
            apply_vertex_modifiers(entries, &mut out.vertex_modifiers, &mut any);
            any.then_some(MaterialLiteral::PixelSurface(out))
        }
        MaterialType::Custom => {
            let mut out = CustomMaterialLiteral {
                shader_path: String::new(),
                params: Vec::new(),
                images: Vec::new(),
                lighting: CustomMaterialLighting3D::Standard,
                surface: StandardMaterial3D::default(),
                release_bake: false,
                bake_resolution: None,
            };
            apply_custom_runtime_entries(entries, &mut out, &mut any);
            apply_vertex_modifiers(entries, &mut out.surface.vertex_modifiers, &mut any);
            if out.release_bake && out.bake_resolution.is_none() {
                out.bake_resolution = Some([1024, 1024]);
            }
            any.then_some(MaterialLiteral::Custom(out))
        }
    }
}

pub(super) fn load_pmat_literal(source: &str) -> Option<MaterialLiteral> {
    if pmat_looks_like_object(source) {
        if let Some(entries) = parse_pmat_object(source)
            && let Some(material) = material_from_runtime_entries(entries.as_ref())
        {
            return Some(material);
        }
        return None;
    }

    if let Some(entries) = parse_pmat_top_level_object(source)
        && let Some(material) = material_from_runtime_entries(entries.as_ref())
    {
        return Some(material);
    }

    let entries = parse_pmat_key_values(source)?;
    material_from_runtime_entries(&entries)
}

pub(super) fn parse_pmat_object(text: &str) -> Option<Vec<SceneObjectField>> {
    let value = std::panic::catch_unwind(|| Parser::new(text).parse_value_literal()).ok()?;
    match value {
        SceneValue::Object(entries) => Some(entries.into_owned()),
        _ => None,
    }
}

pub(super) fn parse_pmat_top_level_object(text: &str) -> Option<Vec<SceneObjectField>> {
    let wrapped = format!("{{\n{text}\n}}");
    parse_pmat_object(&wrapped)
}

pub(super) fn parse_pmat_key_values(text: &str) -> Option<Vec<SceneObjectField>> {
    let mut entries = Vec::new();
    for raw_line in text.lines() {
        let line = strip_line_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let (raw_key, raw_value) = line.split_once('=')?;
        let key = SceneFieldName::from(raw_key.trim().to_string());
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

pub(super) fn strip_line_comment(line: &str) -> &str {
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

pub(super) fn pmat_looks_like_object(text: &str) -> bool {
    text.lines()
        .map(strip_line_comment)
        .map(str::trim)
        .find(|line| !line.is_empty())
        .is_some_and(|line| line.starts_with('{'))
}

pub(super) fn parse_kv_value(text: &str) -> Option<SceneValue> {
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
        return Some(SceneValue::Str(Cow::Owned(inner.to_string())));
    }
    if let Ok(v) = text.parse::<i32>() {
        return Some(SceneValue::I32(v));
    }
    if let Ok(v) = text.parse::<f32>() {
        return Some(SceneValue::F32(v));
    }
    Some(SceneValue::Str(Cow::Owned(text.to_string())))
}

pub(super) fn parse_vec_value(text: &str) -> Option<SceneValue> {
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

pub(super) fn material_type_from_first_runtime(entries: &[SceneObjectField]) -> MaterialType {
    let Some((name, value)) = entries.first() else {
        return MaterialType::Standard;
    };
    if !name.as_ref().eq_ignore_ascii_case("type") {
        return MaterialType::Standard;
    }
    match value {
        SceneValue::Str(v) => parse_material_type(v.as_ref()),
        SceneValue::Key(v) => parse_material_type(v.as_ref()),
        _ => MaterialType::Standard,
    }
}

pub(super) fn parse_material_type(value: &str) -> MaterialType {
    match value {
        "standard" | "Standard" | "pbr" | "PBR" => MaterialType::Standard,
        "unlit" | "Unlit" => MaterialType::Unlit,
        "toon" | "Toon" | "cel" | "Cel" => MaterialType::Toon,
        "hand_drawn" | "hand-drawn" | "HandDrawn" | "drawn" => MaterialType::HandDrawn,
        "pixel_surface" | "pixel-surface" | "PixelSurface" | "pixel" => MaterialType::PixelSurface,
        "custom" | "Custom" => MaterialType::Custom,
        _ => MaterialType::Standard,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MaterialType {
    Standard,
    Unlit,
    Toon,
    HandDrawn,
    PixelSurface,
    Custom,
}

pub(super) fn apply_standard_runtime_entries(
    entries: &[SceneObjectField],
    out: &mut StandardMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_standard_key(name.as_ref()) {
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
                    apply_standard_runtime_entries(inner.as_ref(), out, any);
                }
            }
            _ => {}
        }
    }
}

pub(super) fn apply_unlit_runtime_entries(
    entries: &[SceneObjectField],
    out: &mut UnlitMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_unlit_key(name.as_ref()) {
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

pub(super) fn apply_toon_runtime_entries(
    entries: &[SceneObjectField],
    out: &mut ToonMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_toon_key(name.as_ref()) {
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

pub(super) fn apply_hand_drawn_runtime_entries(
    entries: &[SceneObjectField],
    out: &mut HandDrawnMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_hand_drawn_key(name.as_ref()) {
            Some("baseColorFactor") => set_color4(value, any, |v| out.base_color_factor = v),
            Some("emissiveFactor") => set_color3(value, any, |v| out.emissive_factor = v),
            Some("alphaCutoff") => set_f32(value, any, |v| out.alpha_cutoff = v),
            Some("alphaMode") => set_alpha_mode(value, any, |v| out.alpha_mode = v),
            Some("doubleSided") => set_bool(value, any, |v| out.double_sided = v),
            Some("flatShading") => set_bool(value, any, |v| out.flat_shading = v),
            Some("baseColorTexture") => {
                set_texture_slot(value, any, |v| out.base_color_texture = v)
            }
            Some("bandCount") => set_u32(value, any, |v| out.band_count = v),
            Some("hatchScale") => set_f32(value, any, |v| out.hatch_scale = v),
            Some("grainStrength") => set_f32(value, any, |v| out.grain_strength = v),
            _ => {}
        }
    }
}

pub(super) fn apply_pixel_surface_runtime_entries(
    entries: &[SceneObjectField],
    out: &mut PixelSurfaceMaterial3D,
    any: &mut bool,
) {
    for (name, value) in entries {
        match canonical_pixel_surface_key(name.as_ref()) {
            Some("baseColorFactor") => set_color4(value, any, |v| out.base_color_factor = v),
            Some("emissiveFactor") => set_color3(value, any, |v| out.emissive_factor = v),
            Some("alphaCutoff") => set_f32(value, any, |v| out.alpha_cutoff = v),
            Some("alphaMode") => set_alpha_mode(value, any, |v| out.alpha_mode = v),
            Some("doubleSided") => set_bool(value, any, |v| out.double_sided = v),
            Some("flatShading") => set_bool(value, any, |v| out.flat_shading = v),
            Some("baseColorTexture") => {
                set_texture_slot(value, any, |v| out.base_color_texture = v)
            }
            Some("pixelCount") => set_u32(value, any, |v| out.pixel_count = v),
            Some("colorLevels") => set_u32(value, any, |v| out.color_levels = v),
            Some("ditherStrength") => set_f32(value, any, |v| out.dither_strength = v),
            _ => {}
        }
    }
}

pub(super) fn apply_custom_runtime_entries(
    entries: &[SceneObjectField],
    out: &mut CustomMaterialLiteral,
    any: &mut bool,
) {
    apply_standard_runtime_entries(entries, &mut out.surface, any);
    for (name, value) in entries {
        match canonical_custom_key(name.as_ref()) {
            Some("shaderPath") => {
                if let Some(v) = as_string_value(value) {
                    out.shader_path = v;
                    *any = true;
                }
            }
            Some("params") => {
                if let Some(params) = as_custom_params(value) {
                    out.params = params;
                    *any = true;
                }
            }
            Some("images") => {
                if let Some(images) = as_custom_images(value) {
                    out.images = images;
                    *any = true;
                }
            }
            Some("output") => {
                if let Some(lighting) = as_custom_lighting(value) {
                    out.lighting = lighting;
                    *any = true;
                }
            }
            Some("releaseBake") => {
                if let Some(value) = as_bool(value) {
                    out.release_bake = value;
                    *any = true;
                }
            }
            Some("shaderUse") => {
                if let Some(value) = as_string_value(value) {
                    out.release_bake = matches!(value.as_str(), "baked" | "bake" | "auto" | "both");
                    *any = true;
                }
            }
            Some("bakeResolution") => {
                let resolution = match value {
                    SceneValue::Vec2 { x, y } => Some([*x as u32, *y as u32]),
                    SceneValue::UVec2 { x, y } => Some([*x, *y]),
                    SceneValue::IVec2 { x, y } => Some([(*x).max(1) as u32, (*y).max(1) as u32]),
                    _ => None,
                };
                if let Some([width, height]) = resolution {
                    out.bake_resolution = Some([width.clamp(1, 8192), height.clamp(1, 8192)]);
                    *any = true;
                }
            }
            _ => {}
        }
    }
}

pub(super) fn canonical_standard_key(name: &str) -> Option<&'static str> {
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

pub(super) fn canonical_unlit_key(name: &str) -> Option<&'static str> {
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

pub(super) fn canonical_toon_key(name: &str) -> Option<&'static str> {
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

pub(super) fn canonical_hand_drawn_key(name: &str) -> Option<&'static str> {
    match name {
        "type" | "Type" => None,
        "baseColorFactor" | "base_color_factor" | "color" => Some("baseColorFactor"),
        "emissiveFactor" | "emissive_factor" => Some("emissiveFactor"),
        "alphaCutoff" | "alpha_cutoff" => Some("alphaCutoff"),
        "alphaMode" | "alpha_mode" => Some("alphaMode"),
        "doubleSided" | "double_sided" => Some("doubleSided"),
        "flatShading" | "flat_shading" => Some("flatShading"),
        "baseColorTexture" | "base_color_texture" => Some("baseColorTexture"),
        "bandCount" | "band_count" => Some("bandCount"),
        "hatchScale" | "hatch_scale" => Some("hatchScale"),
        "grainStrength" | "grain_strength" => Some("grainStrength"),
        _ => None,
    }
}

pub(super) fn canonical_pixel_surface_key(name: &str) -> Option<&'static str> {
    match name {
        "type" | "Type" => None,
        "baseColorFactor" | "base_color_factor" | "color" => Some("baseColorFactor"),
        "emissiveFactor" | "emissive_factor" => Some("emissiveFactor"),
        "alphaCutoff" | "alpha_cutoff" => Some("alphaCutoff"),
        "alphaMode" | "alpha_mode" => Some("alphaMode"),
        "doubleSided" | "double_sided" => Some("doubleSided"),
        "flatShading" | "flat_shading" => Some("flatShading"),
        "baseColorTexture" | "base_color_texture" => Some("baseColorTexture"),
        "pixelCount" | "pixel_count" => Some("pixelCount"),
        "colorLevels" | "color_levels" => Some("colorLevels"),
        "ditherStrength" | "dither_strength" => Some("ditherStrength"),
        _ => None,
    }
}

pub(super) fn canonical_custom_key(name: &str) -> Option<&'static str> {
    match name {
        "type" | "Type" => None,
        "shader" | "shaderPath" | "shader_path" | "path" => Some("shaderPath"),
        "params" | "customParams" | "custom_params" => Some("params"),
        "images" | "imageParams" | "image_params" | "textures" => Some("images"),
        "output" | "shaderOutput" | "shader_output" | "lighting" | "light" | "lit" => {
            Some("output")
        }
        "release_bake" | "releaseBake" => Some("releaseBake"),
        "shader_use" | "shaderUse" | "bake_mode" => Some("shaderUse"),
        "bake_resolution" | "bakeResolution" | "bake_size" => Some("bakeResolution"),
        _ => None,
    }
}

pub(super) fn set_f32(value: &SceneValue, any: &mut bool, set: impl FnOnce(f32)) {
    if let Some(v) = as_f32(value) {
        set(v);
        *any = true;
    }
}

pub(super) fn set_u32(value: &SceneValue, any: &mut bool, set: impl FnOnce(u32)) {
    if let Some(v) = as_u32(value) {
        set(v);
        *any = true;
    }
}

pub(super) fn set_bool(value: &SceneValue, any: &mut bool, set: impl FnOnce(bool)) {
    if let Some(v) = as_bool(value) {
        set(v);
        *any = true;
    }
}

pub(super) fn set_alpha_mode(value: &SceneValue, any: &mut bool, set: impl FnOnce(u8)) {
    if let Some(v) = as_alpha_mode(value) {
        set(v);
        *any = true;
    }
}

pub(super) fn set_color4(value: &SceneValue, any: &mut bool, set: impl FnOnce([f32; 4])) {
    if let Some(v) = as_color4(value) {
        set(v);
        *any = true;
    }
}

pub(super) fn set_color3(value: &SceneValue, any: &mut bool, set: impl FnOnce([f32; 3])) {
    if let Some(v) = as_color4(value) {
        set([v[0], v[1], v[2]]);
        *any = true;
    }
}

pub(super) fn set_texture_slot(value: &SceneValue, any: &mut bool, set: impl FnOnce(u32)) {
    if let Some(v) = as_texture_slot(value) {
        set(v);
        *any = true;
    }
}

fn apply_vertex_modifiers(
    entries: &[SceneObjectField],
    out: &mut Cow<'static, [VertexModifier3D]>,
    any: &mut bool,
) {
    let Some(value) = entries
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("vertex_modifiers"))
        .map(|(_, value)| value)
    else {
        return;
    };
    let Some(modifiers) = parse_vertex_modifiers(value) else {
        return;
    };
    *out = Cow::Owned(modifiers);
    *any = true;
}

fn parse_vertex_modifiers(value: &SceneValue) -> Option<Vec<VertexModifier3D>> {
    let SceneValue::Array(items) = value else {
        return None;
    };
    if items.len() > MAX_VERTEX_MODIFIERS {
        return None;
    }
    items
        .iter()
        .map(parse_vertex_modifier)
        .collect::<Option<Vec<_>>>()
}

fn parse_vertex_modifier(value: &SceneValue) -> Option<VertexModifier3D> {
    let SceneValue::Object(fields) = value else {
        return None;
    };
    let kind = modifier_token(modifier_field(fields, "type")?)?;
    match kind.as_str() {
        "wind" => {
            let VertexModifier3D::Wind {
                mut direction,
                mut strength,
                mut speed,
                mut frequency,
                mut mask,
            } = VertexModifier3D::wind()
            else {
                unreachable!()
            };
            direction = modifier_vec3(fields, "direction").unwrap_or(direction);
            strength = modifier_f32(fields, "strength").unwrap_or(strength);
            speed = modifier_f32(fields, "speed").unwrap_or(speed);
            frequency = modifier_f32(fields, "frequency").unwrap_or(frequency);
            mask = parse_modifier_mask(fields).unwrap_or(mask);
            Some(VertexModifier3D::Wind {
                direction,
                strength,
                speed,
                frequency,
                mask,
            })
        }
        "wave" => Some(VertexModifier3D::Wave {
            axis: modifier_axis(fields, "axis").unwrap_or(VertexAxis3D::X),
            direction: modifier_vec3(fields, "direction").unwrap_or([0.0, 1.0, 0.0]),
            amplitude: modifier_f32(fields, "amplitude").unwrap_or(0.1),
            speed: modifier_f32(fields, "speed").unwrap_or(1.0),
            frequency: modifier_f32(fields, "frequency").unwrap_or(1.0),
            phase: modifier_f32(fields, "phase").unwrap_or(0.0),
            mask: parse_modifier_mask(fields),
        }),
        "bend" => Some(VertexModifier3D::Bend {
            along_axis: modifier_axis(fields, "along_axis").unwrap_or(VertexAxis3D::Y),
            bend_axis: modifier_axis(fields, "bend_axis").unwrap_or(VertexAxis3D::Z),
            angle_radians: modifier_angle(fields, 20.0),
            start: modifier_f32(fields, "start").unwrap_or(0.0),
            end: modifier_f32(fields, "end").unwrap_or(1.0),
        }),
        "twist" => Some(VertexModifier3D::Twist {
            axis: modifier_axis(fields, "axis").unwrap_or(VertexAxis3D::Y),
            angle_radians: modifier_angle(fields, 45.0),
            start: modifier_f32(fields, "start").unwrap_or(0.0),
            end: modifier_f32(fields, "end").unwrap_or(1.0),
        }),
        "inflate" => Some(VertexModifier3D::Inflate {
            amount: modifier_f32(fields, "amount").unwrap_or(0.1),
            mask: parse_modifier_mask(fields),
        }),
        "jitter" => Some(VertexModifier3D::Jitter {
            amount: modifier_f32(fields, "amount").unwrap_or(0.02),
            scale: modifier_f32(fields, "scale").unwrap_or(8.0),
            rate: modifier_f32(fields, "rate").unwrap_or(8.0),
            seed: modifier_u32(fields, "seed").unwrap_or(0),
            mask: parse_modifier_mask(fields),
        }),
        "pixel_snap" | "pixel-snap" => Some(VertexModifier3D::PixelSnap {
            virtual_height: modifier_u32(fields, "virtual_height").unwrap_or(180),
            strength: modifier_f32(fields, "strength").unwrap_or(1.0),
        }),
        _ => None,
    }
}

fn modifier_field<'a>(fields: &'a [SceneObjectField], wanted: &str) -> Option<&'a SceneValue> {
    fields
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
        .map(|(_, value)| value)
}

fn modifier_token(value: &SceneValue) -> Option<String> {
    match value {
        SceneValue::Str(value) => Some(value.to_ascii_lowercase()),
        SceneValue::Key(value) => Some(value.as_ref().to_ascii_lowercase()),
        _ => None,
    }
}

fn modifier_f32(fields: &[SceneObjectField], name: &str) -> Option<f32> {
    as_f32(modifier_field(fields, name)?)
}

fn modifier_u32(fields: &[SceneObjectField], name: &str) -> Option<u32> {
    as_u32(modifier_field(fields, name)?)
}

fn modifier_vec3(fields: &[SceneObjectField], name: &str) -> Option<[f32; 3]> {
    match modifier_field(fields, name)? {
        SceneValue::Vec3 { x, y, z } => Some([*x, *y, *z]),
        _ => None,
    }
}

fn modifier_axis(fields: &[SceneObjectField], name: &str) -> Option<VertexAxis3D> {
    match modifier_token(modifier_field(fields, name)?)?.as_str() {
        "x" => Some(VertexAxis3D::X),
        "y" => Some(VertexAxis3D::Y),
        "z" => Some(VertexAxis3D::Z),
        _ => None,
    }
}

fn parse_modifier_mask(fields: &[SceneObjectField]) -> Option<VertexModifierMask3D> {
    if let Some(SceneValue::Object(mask)) = modifier_field(fields, "mask") {
        return Some(VertexModifierMask3D {
            axis: modifier_axis(mask, "axis")?,
            start: modifier_f32(mask, "start").unwrap_or(0.0),
            end: modifier_f32(mask, "end").unwrap_or(1.0),
        });
    }
    Some(VertexModifierMask3D {
        axis: modifier_axis(fields, "mask_axis")?,
        start: modifier_f32(fields, "mask_start").unwrap_or(0.0),
        end: modifier_f32(fields, "mask_end").unwrap_or(1.0),
    })
}

fn modifier_angle(fields: &[SceneObjectField], default_degrees: f32) -> f32 {
    modifier_f32(fields, "angle_radians").unwrap_or_else(|| {
        modifier_f32(fields, "angle_degrees")
            .unwrap_or(default_degrees)
            .to_radians()
    })
}

pub(super) fn as_f32(value: &SceneValue) -> Option<f32> {
    match value {
        SceneValue::F32(v) => Some(*v),
        SceneValue::I32(v) => Some(*v as f32),
        _ => None,
    }
}

pub(super) fn as_u32(value: &SceneValue) -> Option<u32> {
    match value {
        SceneValue::I32(v) if *v >= 0 => Some(*v as u32),
        SceneValue::F32(v) if *v >= 0.0 => Some(*v as u32),
        _ => None,
    }
}

pub(super) fn as_bool(value: &SceneValue) -> Option<bool> {
    match value {
        SceneValue::Bool(v) => Some(*v),
        _ => None,
    }
}

pub(super) fn as_alpha_mode(value: &SceneValue) -> Option<u8> {
    match value {
        SceneValue::Str(v) => match v.as_ref() {
            "OPAQUE" | "opaque" => Some(0),
            "MASK" | "mask" => Some(1),
            "BLEND" | "blend" => Some(2),
            _ => None,
        },
        SceneValue::Key(v) => match v.as_ref() {
            "OPAQUE" | "opaque" => Some(0),
            "MASK" | "mask" => Some(1),
            "BLEND" | "blend" => Some(2),
            _ => None,
        },
        SceneValue::I32(v) if (0..=2).contains(v) => Some(*v as u8),
        _ => None,
    }
}

pub(super) fn as_color4(value: &SceneValue) -> Option<[f32; 4]> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some([*x, *y, *z, *w]),
        SceneValue::Vec3 { x, y, z } => Some([*x, *y, *z, 1.0]),
        _ => None,
    }
}

pub(super) fn as_texture_slot(value: &SceneValue) -> Option<u32> {
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

pub(super) fn as_string_value(value: &SceneValue) -> Option<String> {
    match value {
        SceneValue::Str(v) => Some(v.to_string()),
        SceneValue::Key(v) => Some(v.to_string()),
        _ => None,
    }
}

pub(super) fn as_custom_param_value(value: &SceneValue) -> Option<CustomMaterialParamValue3D> {
    value.as_const_param()
}

pub(super) fn as_custom_lighting(value: &SceneValue) -> Option<CustomMaterialLighting3D> {
    match value {
        SceneValue::Str(v) => parse_custom_lighting_token(v.as_ref()),
        SceneValue::Key(v) => parse_custom_lighting_token(v.as_ref()),
        SceneValue::Bool(v) => Some(if *v {
            CustomMaterialLighting3D::Standard
        } else {
            CustomMaterialLighting3D::Raw
        }),
        _ => None,
    }
}

pub(super) fn parse_custom_lighting_token(value: &str) -> Option<CustomMaterialLighting3D> {
    match value {
        "surface" | "Surface" | "standard" | "Standard" | "lit" | "Lit" | "pbr" | "PBR" => {
            Some(CustomMaterialLighting3D::Standard)
        }
        "final" | "Final" | "raw" | "Raw" | "unlit" | "Unlit" | "none" | "None" => {
            Some(CustomMaterialLighting3D::Raw)
        }
        _ => None,
    }
}

pub(super) fn as_custom_params(value: &SceneValue) -> Option<Vec<CustomParamLiteral>> {
    match value {
        SceneValue::Object(entries) => {
            let mut out = Vec::new();
            for (name, inner) in entries.as_ref() {
                if let Some(val) = as_custom_param_value(inner) {
                    out.push(CustomParamLiteral {
                        name: Some(name.to_string()),
                        value: val,
                    });
                }
            }
            Some(out)
        }
        other => as_custom_param_value(other).map(|val| {
            vec![CustomParamLiteral {
                name: None,
                value: val,
            }]
        }),
    }
}

pub(super) fn as_custom_images(value: &SceneValue) -> Option<Vec<CustomImageLiteral>> {
    match value {
        SceneValue::Object(entries) => {
            let mut out = Vec::new();
            for (name, inner) in entries.as_ref() {
                if let Some(source) = as_string_value(inner) {
                    out.push(CustomImageLiteral {
                        name: Some(name.to_string()),
                        source,
                    });
                }
            }
            Some(out)
        }
        other => {
            as_string_value(other).map(|source| vec![CustomImageLiteral { name: None, source }])
        }
    }
}

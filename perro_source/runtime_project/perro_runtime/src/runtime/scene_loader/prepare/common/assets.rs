fn as_asset_source(value: &SceneValue) -> Option<String> {
    match value {
        SceneValue::Str(v) => Some(v.to_string()),
        SceneValue::Hashed(v) => Some(v.to_string()),
        SceneValue::Key(v) => Some(v.to_string()),
        _ => None,
    }
}

// [albedo, normal, emission] sources for Decal3D; resolved to TextureIDs at
// merge time once the resource api is reachable.
fn extract_decal_texture_sources(data: &SceneDefNodeData) -> [Option<String>; 3] {
    let mut out = [None, None, None];
    if data.node_type != NodeType::Decal3D {
        return out;
    }
    for (name, value) in data.fields.iter() {
        let slot = match name.as_ref() {
            "albedo_texture" | "texture" => 0,
            "normal_texture" => 1,
            "emission_texture" => 2,
            _ => continue,
        };
        if let Some(source) = as_asset_source(value) {
            let source = source.trim().to_string();
            if !source.is_empty() {
                out[slot] = Some(source);
            }
        }
    }
    out
}

fn extract_texture_source(data: &SceneDefNodeData) -> Option<String> {
    let texture_field = scene_node_field(data.node_type, "texture")?;
    if !matches!(
        texture_field.ty,
        NodeFieldType::Asset(SceneAssetKind::Texture)
    ) {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (name.as_ref() == texture_field.name
            || texture_field.aliases.contains(&name.as_ref()))
            .then(|| as_asset_source(value))
            .flatten()
    })
}

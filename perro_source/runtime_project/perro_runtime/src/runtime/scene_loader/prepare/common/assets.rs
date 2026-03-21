fn as_asset_source(value: &SceneValue) -> Option<String> {
    match value {
        SceneValue::Str(v) => Some(v.to_string()),
        SceneValue::Key(v) => Some(v.to_string()),
        _ => None,
    }
}

fn extract_texture_source(data: &SceneDefNodeData) -> Option<String> {
    if data.ty != "Sprite2D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (name == "texture")
            .then(|| as_asset_source(value))
            .flatten()
    })
}

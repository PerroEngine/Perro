fn as_asset_source(value: &SceneValue) -> Option<String> {
    match value {
        SceneValue::Str(v) => Some(v.to_string()),
        SceneValue::Hashed(v) => Some(format_hashed_source_uri(*v)),
        SceneValue::Key(v) => Some(v.to_string()),
        _ => None,
    }
}

fn format_hashed_source_uri(hash: u64) -> String {
    format!("hash://{hash}")
}

fn extract_texture_source(data: &SceneDefNodeData) -> Option<String> {
    if data.ty != "Sprite2D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (resolve_node_field("Sprite2D", name) == Some(NodeField::Sprite2D(Sprite2DField::Texture)))
            .then(|| as_asset_source(value))
            .flatten()
    })
}

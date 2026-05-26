fn as_asset_source(value: &SceneValue) -> Option<String> {
    match value {
        SceneValue::Str(v) => Some(v.to_string()),
        SceneValue::Hashed(v) => Some(v.to_string()),
        SceneValue::Key(v) => Some(v.to_string()),
        _ => None,
    }
}

fn extract_texture_source(data: &SceneDefNodeData) -> Option<String> {
    let texture_field = match data.node_type {
        NodeType::Sprite2D => NodeField::Sprite2D(Sprite2DField::Texture),
        NodeType::AnimatedSprite2D => {
            NodeField::AnimatedSprite2D(AnimatedSprite2DField::Texture)
        }
        NodeType::UiImage => NodeField::UiImage(UiImageField::Texture),
        NodeType::UiAnimatedImage => NodeField::UiAnimatedImage(UiAnimatedImageField::Texture),
        _ => return None,
    };
    data.fields.iter().find_map(|(name, value)| {
        (resolve_scene_node_field(data.type_name(), name) == Some(texture_field))
            .then(|| as_asset_source(value))
            .flatten()
    })
}


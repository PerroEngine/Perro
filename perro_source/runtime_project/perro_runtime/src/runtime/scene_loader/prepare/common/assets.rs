fn as_asset_source(value: &SceneValue) -> Option<String> {
    match value {
        SceneValue::Str(v) => Some(v.to_string()),
        SceneValue::Hashed(v) => Some(v.to_string()),
        SceneValue::Key(v) => Some(v.to_string()),
        _ => None,
    }
}

fn extract_texture_source(data: &SceneDefNodeData) -> Option<String> {
    if data.ty != "Sprite2D"
        && data.ty != "AnimatedSprite2D"
        && data.ty != "UiImage"
        && data.ty != "UiAnimatedImage"
    {
        return None;
    }
    let texture_field = match data.ty.as_ref() {
        "AnimatedSprite2D" => NodeField::AnimatedSprite2D(AnimatedSprite2DField::Texture),
        "UiImage" => NodeField::UiImage(UiImageField::Texture),
        "UiAnimatedImage" => NodeField::UiAnimatedImage(UiAnimatedImageField::Texture),
        _ => NodeField::Sprite2D(Sprite2DField::Texture),
    };
    data.fields.iter().find_map(|(name, value)| {
        (resolve_scene_node_field(data.ty.as_ref(), name) == Some(texture_field))
            .then(|| as_asset_source(value))
            .flatten()
    })
}

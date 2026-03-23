fn build_animation_player(data: &SceneDefNodeData) -> AnimationPlayer {
    let mut node = AnimationPlayer::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_animation_player_fields(&mut node, &data.fields);
    node
}

fn apply_animation_player_fields(node: &mut AnimationPlayer, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "speed" => {
            if let Some(v) = as_f32(value) {
                node.speed = v;
            }
        }
        "playing" => {
            if let Some(v) = as_bool(value) {
                node.paused = !v;
            }
        }
        "paused" => {
            if let Some(v) = as_bool(value) {
                node.paused = v;
            }
        }
        "playback" => {
            if let Some(playback_type) = parse_animation_playback_type(value) {
                node.playback_type = playback_type;
            }
        }
        "loop" | "looping" => {
            if let Some(v) = as_bool(value) {
                node.playback_type = if v {
                    perro_nodes::AnimationPlaybackType::Loop
                } else {
                    perro_nodes::AnimationPlaybackType::Once
                };
            }
        }
        _ => {}
    });
}

fn parse_animation_playback_type(value: &SceneValue) -> Option<perro_nodes::AnimationPlaybackType> {
    let token = as_str(value)?;
    if token.eq_ignore_ascii_case("once") {
        return Some(perro_nodes::AnimationPlaybackType::Once);
    }
    if token.eq_ignore_ascii_case("loop") {
        return Some(perro_nodes::AnimationPlaybackType::Loop);
    }
    if token.eq_ignore_ascii_case("boomerang") {
        return Some(perro_nodes::AnimationPlaybackType::Boomerang);
    }
    None
}

fn extract_animation_source(data: &SceneDefNodeData) -> Option<String> {
    if data.ty != "AnimationPlayer" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (name == "animation")
            .then(|| as_asset_source(value))
            .flatten()
    })
}

fn extract_animation_scene_bindings(data: &SceneDefNodeData) -> Vec<(String, String)> {
    let mut out = Vec::new();
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| {
        if name == "bindings"
            && let Some(bindings) = parse_animation_bindings(value)
        {
            out = bindings;
        }
    });
    out
}

fn parse_animation_bindings(value: &SceneValue) -> Option<Vec<(String, String)>> {
    let SceneValue::Array(items) = value else {
        return None;
    };

    let mut out = Vec::new();
    for item in items.as_ref() {
        let SceneValue::Object(entries) = item else {
            continue;
        };

        let mut object = None::<String>;
        let mut node = None::<String>;

        for (name, value) in entries.as_ref() {
            match name.as_ref() {
                "object" | "track" => {
                    if let Some(v) = as_str(value) {
                        object = Some(v.to_string());
                    }
                }
                "node" => {
                    if let Some(v) = as_str(value) {
                        node = Some(v.to_string());
                    }
                }
                _ => {}
            }
        }

        if let (Some(object), Some(node)) = (object, node) {
            out.push((object, node));
        }
    }

    Some(out)
}

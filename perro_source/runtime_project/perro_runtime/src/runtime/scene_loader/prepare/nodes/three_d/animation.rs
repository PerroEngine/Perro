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
        "loop" | "looping" => {
            if let Some(v) = as_bool(value) {
                node.looping = v;
            }
        }
        _ => {}
    });
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

        let mut track = None::<String>;
        let mut node = None::<String>;

        for (name, value) in entries.as_ref() {
            match name.as_ref() {
                "track" => {
                    if let Some(v) = as_str(value) {
                        track = Some(v.to_string());
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

        if let (Some(track), Some(node)) = (track, node) {
            out.push((track, node));
        }
    }

    Some(out)
}

define_scene_node_builder! {
    fn build_animation_player -> AnimationPlayer = AnimationPlayer::new();
    base none;
    apply [apply_animation_player_fields];
}

define_scene_node_builder! {
    fn build_animation_tree -> AnimationTree = AnimationTree::new();
    base none;
    apply [apply_animation_tree_fields];
}

fn apply_animation_tree_fields(node: &mut AnimationTree, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("AnimationTree", name) {
            Some(NodeField::AnimationTree(AnimationTreeField::Speed)) => {
                if let Some(v) = as_f32(value) {
                    node.speed = v;
                }
            }
            Some(NodeField::AnimationTree(AnimationTreeField::Paused)) => {
                if let Some(v) = as_bool(value) {
                    node.paused = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_animation_player_fields(node: &mut AnimationPlayer, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("AnimationPlayer", name) {
            Some(NodeField::AnimationPlayer(AnimationPlayerField::Speed)) => {
                if let Some(v) = as_f32(value) {
                    node.speed = v;
                }
            }
            Some(NodeField::AnimationPlayer(AnimationPlayerField::Paused)) => {
                if let Some(v) = as_bool(value) {
                    node.paused = v;
                }
            }
            Some(NodeField::AnimationPlayer(AnimationPlayerField::Playback)) => {
                if let Some(playback_type) = parse_animation_playback_type(value) {
                    node.playback_type = playback_type;
                }
            }
            _ => {}
        }
    });
}

fn parse_animation_playback_type(value: &SceneValue) -> Option<perro_nodes::AnimationPlaybackType> {
    let token = as_str(value)?.trim();
    let normalized = token
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "");

    match normalized.as_str() {
        "once" => Some(perro_nodes::AnimationPlaybackType::Once),
        "loop" => Some(perro_nodes::AnimationPlaybackType::Loop),
        "boomerang" | "pingpong" => {
            Some(perro_nodes::AnimationPlaybackType::Boomerang)
        }
        _ => None,
    }
}

fn extract_animation_source(data: &SceneDefNodeData) -> Option<String> {
    if data.node_type != NodeType::AnimationPlayer {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (resolve_node_field("AnimationPlayer", name)
            == Some(NodeField::AnimationPlayer(AnimationPlayerField::Animation)))
            .then(|| as_asset_source(value))
            .flatten()
    })
}

fn extract_animation_scene_bindings(data: &SceneDefNodeData) -> Vec<(String, String)> {
    let mut out = Vec::new();
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| {
        if resolve_node_field("AnimationPlayer", name)
            == Some(NodeField::AnimationPlayer(AnimationPlayerField::Bindings))
            && let Some(bindings) = parse_animation_bindings(value)
        {
            out = bindings;
        }
    });
    out
}

fn extract_animation_tree_source(data: &SceneDefNodeData) -> Option<String> {
    if data.node_type != NodeType::AnimationTree {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (resolve_node_field("AnimationTree", name)
            == Some(NodeField::AnimationTree(AnimationTreeField::Tree)))
            .then(|| as_asset_source(value))
            .flatten()
    })
}

fn extract_animation_tree_animations(data: &SceneDefNodeData) -> AnimationTreeAnimationEntries {
    if data.node_type != NodeType::AnimationTree {
        return Vec::new();
    }
    data.fields
        .iter()
        .find_map(|(name, value)| {
            (resolve_node_field("AnimationTree", name)
                == Some(NodeField::AnimationTree(AnimationTreeField::Animations)))
                .then(|| parse_animation_tree_animation_list(value))
                .flatten()
        })
        .unwrap_or_default()
}

fn parse_animation_tree_animation_list(
    value: &SceneValue,
) -> Option<AnimationTreeAnimationEntries> {
    let SceneValue::Array(items) = value else {
        return None;
    };
    let mut out = Vec::new();
    for item in items.iter() {
        if let Some(source) = as_asset_source(item) {
            out.push((
                source,
                Vec::new(),
                1.0,
                false,
                perro_nodes::AnimationPlaybackType::Loop,
            ));
            continue;
        }
        let SceneValue::Object(entries) = item else {
            continue;
        };
        let mut source = None;
        let mut bindings = Vec::new();
        let mut speed = 1.0;
        let mut paused = false;
        let mut playback_type = perro_nodes::AnimationPlaybackType::Loop;
        for (name, value) in entries.iter() {
            match name.as_ref() {
                "animation" | "clip" | "source" => source = as_asset_source(value),
                "bindings" => {
                    if let Some(v) = parse_animation_bindings(value) {
                        bindings = v;
                    }
                }
                "speed" => {
                    if let Some(v) = as_f32(value) {
                        speed = v;
                    }
                }
                "paused" => {
                    if let Some(v) = as_bool(value) {
                        paused = v;
                    }
                }
                "playback" | "playback_type" => {
                    if let Some(v) = parse_animation_playback_type(value) {
                        playback_type = v;
                    }
                }
                _ => {}
            }
        }
        if let Some(source) = source {
            out.push((source, bindings, speed, paused, playback_type));
        }
    }
    Some(out)
}

fn parse_animation_bindings(value: &SceneValue) -> Option<Vec<(String, String)>> {
    let mut out = Vec::new();
    if let SceneValue::Object(entries) = value {
        for (name, value) in entries.as_ref() {
            if let Some(v) = as_str(value).or_else(|| value.as_key()) {
                out.push((name.to_string(), v.trim_start_matches('%').to_string()));
            }
        }
        return Some(out);
    }

    if let SceneValue::Array(items) = value {
        for item in items.as_ref() {
            let SceneValue::Object(entries) = item else {
                continue;
            };
            for (name, value) in entries.as_ref() {
                if matches!(name.as_ref(), "object" | "track" | "node") {
                    continue;
                }
                if let Some(v) = as_str(value).or_else(|| value.as_key()) {
                    out.push((name.to_string(), v.trim_start_matches('%').to_string()));
                }
            }
        }
        return Some(out);
    }

    None
}


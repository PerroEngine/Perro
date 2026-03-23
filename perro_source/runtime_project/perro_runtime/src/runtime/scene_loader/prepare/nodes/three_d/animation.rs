use perro_animation::AnimationSceneBinding;

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
        "animation" => {
            if let Some(v) = as_str(value) {
                node.animation_source = Some(v.to_string().into());
            }
        }
        "speed" => {
            if let Some(v) = as_f32(value) {
                node.speed = v;
            }
        }
        "playing" => {
            if let Some(v) = as_bool(value) {
                node.playing = v;
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
        "bindings" => {
            if let Some(bindings) = parse_animation_bindings(value) {
                node.scene_bindings = bindings;
            }
        }
        _ => {}
    });
}

fn parse_animation_bindings(value: &SceneValue) -> Option<Vec<AnimationSceneBinding>> {
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
            out.push(AnimationSceneBinding {
                track: track.into(),
                node: node.into(),
            });
        }
    }

    Some(out)
}

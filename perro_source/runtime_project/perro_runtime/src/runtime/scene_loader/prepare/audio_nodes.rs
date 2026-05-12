use super::*;

pub(super) fn build_audio_mask_2d(data: &SceneDefNodeData) -> AudioMask2D {
    let mut node = AudioMask2D::new();
    apply_node_2d_data(&mut node.base, data);
    node
}

pub(super) fn build_audio_effect_zone_2d(data: &SceneDefNodeData) -> AudioEffectZone2D {
    let mut node = AudioEffectZone2D::new();
    apply_node_2d_data(&mut node.base, data);
    apply_audio_effect_zone_2d_data(&mut node, data);
    node
}

pub(super) fn build_audio_portal_2d(data: &SceneDefNodeData) -> AudioPortal2D {
    let mut node = AudioPortal2D::new();
    apply_node_2d_data(&mut node.base, data);
    apply_audio_portal_2d_data(&mut node, data);
    node
}

pub(super) fn build_audio_mask_3d(data: &SceneDefNodeData) -> AudioMask3D {
    let mut node = AudioMask3D::new();
    apply_node_3d_data(&mut node.base, data);
    node
}

pub(super) fn build_audio_effect_zone_3d(data: &SceneDefNodeData) -> AudioEffectZone3D {
    let mut node = AudioEffectZone3D::new();
    apply_node_3d_data(&mut node.base, data);
    apply_audio_effect_zone_3d_data(&mut node, data);
    node
}

pub(super) fn build_audio_portal_3d(data: &SceneDefNodeData) -> AudioPortal3D {
    let mut node = AudioPortal3D::new();
    apply_node_3d_data(&mut node.base, data);
    apply_audio_portal_3d_data(&mut node, data);
    node
}

pub(super) fn apply_audio_portal_2d_data(node: &mut AudioPortal2D, data: &SceneDefNodeData) {
    for (name, value) in flatten_scene_node_fields(data) {
        match name.as_ref() {
            "enabled" => {
                if let Some(v) = as_bool(&value) {
                    node.enabled = v;
                }
            }
            "strength" => {
                if let Some(v) = as_f32(&value) {
                    node.strength = v;
                }
            }
            "targets" | "connections" | "connected" => {
                node.targets = as_node_ids(&value);
            }
            _ => {}
        }
    }
}

pub(super) fn apply_audio_portal_3d_data(node: &mut AudioPortal3D, data: &SceneDefNodeData) {
    for (name, value) in flatten_scene_node_fields(data) {
        match name.as_ref() {
            "enabled" => {
                if let Some(v) = as_bool(&value) {
                    node.enabled = v;
                }
            }
            "strength" => {
                if let Some(v) = as_f32(&value) {
                    node.strength = v;
                }
            }
            "targets" | "connections" | "connected" => {
                node.targets = as_node_ids(&value);
            }
            _ => {}
        }
    }
}

pub(super) fn apply_audio_effect_zone_2d_data(node: &mut AudioEffectZone2D, data: &SceneDefNodeData) {
    for (name, value) in flatten_scene_node_fields(data) {
        match name.as_ref() {
            "enabled" => {
                if let Some(v) = as_bool(&value) {
                    node.enabled = v;
                }
            }
            "audio_mask" | "audio_masks" | "audio_mask_layers" | "mask" | "masks" => {
                if let Some(v) = as_bitmask(&value) {
                    node.audio_mask = v;
                }
            }
            "bounce" => {
                if let Some(v) = as_bool(&value) {
                    node.bounce = v;
                }
            }
            "reverb" | "reverb_send" | "reverbSend" => {
                if let Some(v) = as_f32(&value) {
                    first_audio_effect_zone_effect_mut(&mut node.effects).reverb_send = v;
                }
            }
            "echo" => {
                if let Some(v) = as_f32(&value) {
                    first_audio_effect_zone_effect_mut(&mut node.effects).echo = v;
                }
            }
            "dampening" | "damping" | "low_pass" | "lowPass" => {
                if let Some(v) = as_f32(&value) {
                    first_audio_effect_zone_effect_mut(&mut node.effects).dampening = v;
                }
            }
            "effect" => node.effects = vec![audio_effect_zone_effect_from_value(&value)],
            "effects" | "effect_chain" | "effectChain" => {
                node.effects = audio_effect_zone_effects_from_value(&value);
            }
            _ => {}
        }
    }
}

pub(super) fn apply_audio_effect_zone_3d_data(node: &mut AudioEffectZone3D, data: &SceneDefNodeData) {
    for (name, value) in flatten_scene_node_fields(data) {
        match name.as_ref() {
            "enabled" => {
                if let Some(v) = as_bool(&value) {
                    node.enabled = v;
                }
            }
            "audio_mask" | "audio_masks" | "audio_mask_layers" | "mask" | "masks" => {
                if let Some(v) = as_bitmask(&value) {
                    node.audio_mask = v;
                }
            }
            "bounce" => {
                if let Some(v) = as_bool(&value) {
                    node.bounce = v;
                }
            }
            "reverb" | "reverb_send" | "reverbSend" => {
                if let Some(v) = as_f32(&value) {
                    first_audio_effect_zone_effect_mut(&mut node.effects).reverb_send = v;
                }
            }
            "echo" => {
                if let Some(v) = as_f32(&value) {
                    first_audio_effect_zone_effect_mut(&mut node.effects).echo = v;
                }
            }
            "dampening" | "damping" | "low_pass" | "lowPass" => {
                if let Some(v) = as_f32(&value) {
                    first_audio_effect_zone_effect_mut(&mut node.effects).dampening = v;
                }
            }
            "effect" => node.effects = vec![audio_effect_zone_effect_from_value(&value)],
            "effects" | "effect_chain" | "effectChain" => {
                node.effects = audio_effect_zone_effects_from_value(&value);
            }
            _ => {}
        }
    }
}

pub(super) fn apply_audio_listener_options_data(
    options: &mut perro_structs::AudioListenerOptions,
    fields: &[SceneObjectField],
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        apply_audio_listener_options_field(options, name, value);
    });
}

fn apply_audio_listener_options_field(
    options: &mut perro_structs::AudioListenerOptions,
    name: &str,
    value: &SceneValue,
) {
    match name {
        "audio_options" => apply_audio_listener_options_value(options, value),
        "audio_mask" | "audio_masks" | "audio_mask_layers" => {
            if let Some(v) = as_bitmask(value) {
                options.audio_mask = v;
            }
        }
        "reverb" | "reverb_send" | "reverbSend" => {
            if let Some(v) = as_f32(value) {
                first_audio_effect_zone_effect_mut(&mut options.effects).reverb_send = v;
            }
        }
        "echo" => {
            if let Some(v) = as_f32(value) {
                first_audio_effect_zone_effect_mut(&mut options.effects).echo = v;
            }
        }
        "dampening" | "damping" | "low_pass" | "lowPass" => {
            if let Some(v) = as_f32(value) {
                first_audio_effect_zone_effect_mut(&mut options.effects).dampening = v;
            }
        }
        "effect" => options.effects = vec![audio_effect_zone_effect_from_value(value)],
        "effects" | "effect_chain" | "effectChain" => {
            options.effects = audio_effect_zone_effects_from_value(value);
        }
        _ => {}
    }
}

fn apply_audio_listener_options_value(
    options: &mut perro_structs::AudioListenerOptions,
    value: &SceneValue,
) {
    let SceneValue::Object(fields) = value else {
        return;
    };
    for (name, value) in fields.iter() {
        apply_audio_listener_options_field(options, name, value);
    }
}

fn first_audio_effect_zone_effect_mut(
    effects: &mut Vec<perro_structs::AudioEffect>,
) -> &mut perro_structs::AudioEffect {
    if effects.is_empty() {
        effects.push(perro_structs::AudioEffect::new());
    }
    &mut effects[0]
}

fn audio_effect_zone_effects_from_value(
    value: &SceneValue,
) -> Vec<perro_structs::AudioEffect> {
    let effects: Vec<_> = match value {
        SceneValue::Array(items) => items
            .iter()
            .map(audio_effect_zone_effect_from_value)
            .collect(),
        _ => vec![audio_effect_zone_effect_from_value(value)],
    };
    if effects.is_empty() {
        vec![perro_structs::AudioEffect::new()]
    } else {
        effects
    }
}

fn audio_effect_zone_effect_from_value(value: &SceneValue) -> perro_structs::AudioEffect {
    let mut effect = perro_structs::AudioEffect::new();
    apply_audio_effect_zone_effect(&mut effect, value);
    effect
}

fn apply_audio_effect_zone_effect(effect: &mut perro_structs::AudioEffect, value: &SceneValue) {
    let SceneValue::Object(fields) = value else {
        return;
    };
    for (name, value) in fields.iter() {
        match name.as_ref() {
            "reverb" | "reverb_send" | "reverbSend" => {
                if let Some(v) = as_f32(value) {
                    effect.reverb_send = v;
                }
            }
            "echo" => {
                if let Some(v) = as_f32(value) {
                    effect.echo = v;
                }
            }
            "dampening" | "damping" | "low_pass" | "lowPass" => {
                if let Some(v) = as_f32(value) {
                    effect.dampening = v;
                }
            }
            _ => {}
        }
    }
}

fn as_node_ids(value: &SceneValue) -> Vec<NodeID> {
    match value {
        SceneValue::Array(items) => items.iter().filter_map(as_node_id).collect(),
        _ => as_node_id(value).into_iter().collect(),
    }
}

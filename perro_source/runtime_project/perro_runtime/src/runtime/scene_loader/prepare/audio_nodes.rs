use super::*;

define_scene_node_builder! {
    pub(super) fn build_audio_mask_2d -> AudioMask2D = AudioMask2D::default();
    base embedded_node_2d;
    data_apply [apply_audio_mask_2d_data];
    apply [];
}

define_scene_node_builder! {
    pub(super) fn build_audio_effect_zone_2d -> AudioEffectZone2D = AudioEffectZone2D::default();
    base embedded_node_2d;
    data_apply [apply_audio_effect_zone_2d_data];
    apply [];
}

define_scene_node_builder! {
    pub(super) fn build_audio_portal_2d -> AudioPortal2D = AudioPortal2D::default();
    base embedded_node_2d;
    data_apply [apply_audio_portal_2d_data];
    apply [];
}

define_scene_node_builder! {
    pub(super) fn build_audio_mask_3d -> AudioMask3D = AudioMask3D::default();
    base embedded_node_3d;
    data_apply [apply_audio_mask_3d_data];
    apply [];
}

define_scene_node_builder! {
    pub(super) fn build_audio_effect_zone_3d -> AudioEffectZone3D = AudioEffectZone3D::default();
    base embedded_node_3d;
    data_apply [apply_audio_effect_zone_3d_data];
    apply [];
}

define_scene_node_builder! {
    pub(super) fn build_audio_portal_3d -> AudioPortal3D = AudioPortal3D::default();
    base embedded_node_3d;
    data_apply [apply_audio_portal_3d_data];
    apply [];
}

pub(super) fn apply_audio_portal_2d_data(node: &mut AudioPortal2D, data: &SceneDefNodeData) {
    apply_scene_fields!(data, {
        audio_portal_fields::ACTIVE => |value| { node.active = value; },
        audio_portal_fields::STRENGTH => |value| { node.strength = value; },
        audio_portal_fields::TARGETS => |value| { node.targets = as_node_ids(&value); },
    });
}

pub(super) fn apply_audio_portal_3d_data(node: &mut AudioPortal3D, data: &SceneDefNodeData) {
    apply_scene_fields!(data, {
        audio_portal_fields::ACTIVE => |value| { node.active = value; },
        audio_portal_fields::STRENGTH => |value| { node.strength = value; },
        audio_portal_fields::TARGETS => |value| { node.targets = as_node_ids(&value); },
    });
}

pub(super) fn apply_audio_mask_2d_data(node: &mut AudioMask2D, data: &SceneDefNodeData) {
    apply_scene_fields!(data, {
        audio_mask_fields::ACTIVE => |value| { node.active = value; },
    });
}

pub(super) fn apply_audio_mask_3d_data(node: &mut AudioMask3D, data: &SceneDefNodeData) {
    apply_scene_fields!(data, {
        audio_mask_fields::ACTIVE => |value| { node.active = value; },
    });
}

pub(super) fn apply_audio_effect_zone_2d_data(
    node: &mut AudioEffectZone2D,
    data: &SceneDefNodeData,
) {
    apply_scene_fields!(data, {
        audio_effect_zone_fields::ACTIVE => |value| { node.active = value; },
        audio_effect_zone_fields::AUDIO_MASK => |value| { node.audio_mask = value; },
        audio_effect_zone_fields::BOUNCE => |value| { node.bounce = value; },
        audio_effect_zone_fields::REVERB => |value| {
            first_audio_effect_zone_effect_mut(&mut node.effects).reverb_send = value;
        },
        audio_effect_zone_fields::ECHO => |value| {
            first_audio_effect_zone_effect_mut(&mut node.effects).echo = value;
        },
        audio_effect_zone_fields::DAMPENING => |value| {
            first_audio_effect_zone_effect_mut(&mut node.effects).dampening = value;
        },
        audio_effect_zone_fields::EFFECTS => |value| {
            node.effects = audio_effect_zone_effects_from_value(&value);
        },
    });
}

pub(super) fn apply_audio_effect_zone_3d_data(
    node: &mut AudioEffectZone3D,
    data: &SceneDefNodeData,
) {
    apply_scene_fields!(data, {
        audio_effect_zone_fields::ACTIVE => |value| { node.active = value; },
        audio_effect_zone_fields::AUDIO_MASK => |value| { node.audio_mask = value; },
        audio_effect_zone_fields::BOUNCE => |value| { node.bounce = value; },
        audio_effect_zone_fields::REVERB => |value| {
            first_audio_effect_zone_effect_mut(&mut node.effects).reverb_send = value;
        },
        audio_effect_zone_fields::ECHO => |value| {
            first_audio_effect_zone_effect_mut(&mut node.effects).echo = value;
        },
        audio_effect_zone_fields::DAMPENING => |value| {
            first_audio_effect_zone_effect_mut(&mut node.effects).dampening = value;
        },
        audio_effect_zone_fields::EFFECTS => |value| {
            node.effects = audio_effect_zone_effects_from_value(&value);
        },
    });
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
        "audio_mask" | "audio_masks" => {
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

fn audio_effect_zone_effects_from_value(value: &SceneValue) -> Vec<perro_structs::AudioEffect> {
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

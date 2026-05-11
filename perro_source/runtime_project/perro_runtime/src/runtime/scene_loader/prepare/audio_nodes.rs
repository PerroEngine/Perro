use super::*;

pub(super) fn build_audio_mask_2d(data: &SceneDefNodeData) -> AudioMask2D {
    let mut node = AudioMask2D::new();
    apply_node_2d_data(&mut node.base, data);
    node
}

pub(super) fn build_audio_zone_2d(data: &SceneDefNodeData) -> AudioZone2D {
    let mut node = AudioZone2D::new();
    apply_node_2d_data(&mut node.base, data);
    apply_audio_zone_2d_data(&mut node, data);
    node
}

pub(super) fn build_audio_portal_2d(data: &SceneDefNodeData) -> AudioPortal2D {
    let mut node = AudioPortal2D::new();
    apply_node_2d_data(&mut node.base, data);
    node
}

pub(super) fn build_audio_mask_3d(data: &SceneDefNodeData) -> AudioMask3D {
    let mut node = AudioMask3D::new();
    apply_node_3d_data(&mut node.base, data);
    node
}

pub(super) fn build_audio_zone_3d(data: &SceneDefNodeData) -> AudioZone3D {
    let mut node = AudioZone3D::new();
    apply_node_3d_data(&mut node.base, data);
    apply_audio_zone_3d_data(&mut node, data);
    node
}

pub(super) fn build_audio_portal_3d(data: &SceneDefNodeData) -> AudioPortal3D {
    let mut node = AudioPortal3D::new();
    apply_node_3d_data(&mut node.base, data);
    node
}

pub(super) fn apply_audio_zone_2d_data(node: &mut AudioZone2D, data: &SceneDefNodeData) {
    for (name, value) in flatten_scene_node_fields(data) {
        match name.as_ref() {
            "enabled" => {
                if let Some(v) = as_bool(&value) {
                    node.enabled = v;
                }
            }
            "reverb" | "reverb_send" | "reverbSend" => {
                if let Some(v) = as_f32(&value) {
                    node.effect.reverb_send = v;
                }
            }
            "echo" => {
                if let Some(v) = as_f32(&value) {
                    node.effect.echo = v;
                }
            }
            "dampening" | "damping" | "low_pass" | "lowPass" => {
                if let Some(v) = as_f32(&value) {
                    node.effect.dampening = v;
                }
            }
            "effect" => apply_audio_zone_effect(&mut node.effect, &value),
            "affect_listener" | "affectListener" => {
                if let Some(v) = as_bool(&value) {
                    node.affect_listener = v;
                }
            }
            "affect_emitters" | "affectEmitters" | "affect_sources" | "affectSources" => {
                if let Some(v) = as_bool(&value) {
                    node.affect_emitters = v;
                }
            }
            "affect_path" | "affectPath" => {
                if let Some(v) = as_bool(&value) {
                    node.affect_path = v;
                }
            }
            _ => {}
        }
    }
}

pub(super) fn apply_audio_zone_3d_data(node: &mut AudioZone3D, data: &SceneDefNodeData) {
    for (name, value) in flatten_scene_node_fields(data) {
        match name.as_ref() {
            "enabled" => {
                if let Some(v) = as_bool(&value) {
                    node.enabled = v;
                }
            }
            "reverb" | "reverb_send" | "reverbSend" => {
                if let Some(v) = as_f32(&value) {
                    node.effect.reverb_send = v;
                }
            }
            "echo" => {
                if let Some(v) = as_f32(&value) {
                    node.effect.echo = v;
                }
            }
            "dampening" | "damping" | "low_pass" | "lowPass" => {
                if let Some(v) = as_f32(&value) {
                    node.effect.dampening = v;
                }
            }
            "effect" => apply_audio_zone_effect(&mut node.effect, &value),
            "affect_listener" | "affectListener" => {
                if let Some(v) = as_bool(&value) {
                    node.affect_listener = v;
                }
            }
            "affect_emitters" | "affectEmitters" | "affect_sources" | "affectSources" => {
                if let Some(v) = as_bool(&value) {
                    node.affect_emitters = v;
                }
            }
            "affect_path" | "affectPath" => {
                if let Some(v) = as_bool(&value) {
                    node.affect_path = v;
                }
            }
            _ => {}
        }
    }
}

pub(super) fn apply_audio_zone_effect(effect: &mut perro_nodes::AudioZoneEffect, value: &SceneValue) {
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

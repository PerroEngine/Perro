use super::*;

pub(super) fn apply_ui_label_fields(node: &mut UiLabel, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "text" => {
            if let Some(v) = as_str(value) {
                node.text = Cow::Owned(decode_scene_text_literal(v));
            }
        }
        "color" | "text_color" => {
            if let Some(v) = as_scene_color(value) {
                node.color = v;
            }
        }
        // Absolute text size unsupported.
        // Use `text_size_ratio`.
        "font_size" => {}
        "font" => {
            if let Some(v) = as_str(value).and_then(perro_ui::UiFont::parse) {
                node.font = v;
            }
        }
        "text_size_ratio" | "font_size_ratio" => {
            if let Some(v) = as_f32(value) {
                node.text_size_ratio = v;
            }
        }
        "font_relative" | "font_size_relative" | "font_size_relative_to_virtual" => {
            if let Some(v) = as_bool(value) {
                node.font_sizing.relative_to_virtual = v;
            }
        }
        "font_min_scale" | "font_size_min_scale" => {
            if let Some(v) = as_f32(value) {
                node.font_sizing.min_scale = v;
            }
        }
        "font_max_scale" | "font_size_max_scale" => {
            if let Some(v) = as_f32(value) {
                node.font_sizing.max_scale = v;
            }
        }
        "h_align" | "text_h_align" => {
            if let Some(v) = as_ui_text_align(value) {
                node.h_align = v;
            }
        }
        "v_align" | "text_v_align" => {
            if let Some(v) = as_ui_text_align(value) {
                node.v_align = v;
            }
        }
        _ => {}
    });
}

pub(super) fn apply_ui_image_fields(node: &mut UiImage, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        "scale_mode" | "image_scale" | "fit" => {
            if let Some(v) = as_ui_image_scale_mode(value) {
                node.scale_mode = v;
            }
        }
        "h_align" | "image_h_align" => {
            if let Some(v) = as_ui_text_align(value) {
                node.h_align = v;
            }
        }
        "v_align" | "image_v_align" => {
            if let Some(v) = as_ui_text_align(value) {
                node.v_align = v;
            }
        }
        "aspect_ratio" | "ratio" => {
            if let Some(v) = as_f32(value) {
                node.aspect_ratio = v.max(0.0);
            }
        }
        name if scene_key_in(name, TEXTURE_REGION_KEYS) => {
            if let Some(v) = as_vec4_array(value) {
                node.texture_region = Some(v);
            }
        }
        _ => {}
    });
}

pub(super) fn apply_ui_nine_slice_fields(node: &mut UiNineSlice, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        name if scene_key_in(name, TEXTURE_REGION_KEYS) => {
            if let Some((x, y, w, h)) = value.as_vec4() && w > 0.0 && h > 0.0 {
                node.texture_region = Some([x, y, w, h]);
            }
        }
        "margins" | "slice" | "slices" => {
            if let Some(v) = as_margins_4(value) {
                node.margins = v;
            }
        }
        _ => {}
    });
}

pub(super) fn apply_ui_nine_slice_button_fields(node: &mut UiNineSliceButton, fields: &[SceneObjectField]) {
    apply_ui_input_mask_fields(&mut node.input_mask, fields);
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "disabled" => {
            if let Some(v) = as_bool(value) {
                node.disabled = v;
            }
        }
        name if scene_key_in(name, HOVER_ENTER_SIGNAL_KEYS) => node.hover_signals = as_signal_ids(value),
        name if scene_key_in(name, HOVER_EXIT_SIGNAL_KEYS) => node.hover_exit_signals = as_signal_ids(value),
        "pressed_signals" | "press_signals" => node.pressed_signals = as_signal_ids(value),
        "released_signals" | "release_signals" => node.released_signals = as_signal_ids(value),
        "clicked_signals" | "click_signals" => node.clicked_signals = as_signal_ids(value),
        "web" => node.web = parse_ui_button_web_action(value),
        "cursor_icon" | "hover_cursor_icon" => {
            if let Some(v) = as_cursor_icon(value) {
                node.cursor_icon = v;
            }
        }
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        name if scene_key_in(name, TEXTURE_REGION_KEYS) => {
            if let Some((x, y, w, h)) = value.as_vec4() && w > 0.0 && h > 0.0 {
                node.texture_region = Some([x, y, w, h]);
            }
        }
        "margins" | "slice" | "slices" => {
            if let Some(v) = as_margins_4(value) {
                node.margins = v;
            }
        }
        _ => {}
    });
    node.hover_tint = node.tint;
    node.pressed_tint = node.tint;
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "hover_tint" | "hover_color" | "hover_modulate" => {
            if let Some(v) = as_scene_color(value) {
                node.hover_tint = v;
            }
        }
        "pressed_tint" | "pressed_color" | "pressed_modulate" => {
            if let Some(v) = as_scene_color(value) {
                node.pressed_tint = v;
            }
        }
        _ => {}
    });
    apply_ui_nine_slice_button_state_fields(node, fields, "hover");
    apply_ui_nine_slice_button_state_fields(node, fields, "pressed");
}

pub(super) fn apply_ui_nine_slice_button_state_fields(
    node: &mut UiNineSliceButton,
    fields: &[SceneObjectField],
    state_name: &str,
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if name != state_name {
            return;
        }
        let SceneValue::Object(entries) = value else {
            return;
        };
        let mut base = node.base.clone();
        let size_override = ui_state_has_explicit_size_override(entries.as_ref());
        apply_ui_root_fields(&mut base, entries.as_ref());
        let tint = ui_state_tint(entries.as_ref());
        match state_name {
            "hover" => {
                if let Some(tint) = tint {
                    node.hover_tint = tint;
                }
                node.hover_base = Some(base);
                node.hover_size_override = size_override;
            }
            "pressed" => {
                if let Some(tint) = tint {
                    node.pressed_tint = tint;
                }
                node.pressed_base = Some(base);
                node.pressed_size_override = size_override;
            }
            _ => {}
        }
    });
}

pub(super) fn apply_ui_image_button_image_fields(
    node: &mut UiImageButton,
    fields: &[SceneObjectField],
    prefix: &str,
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        let Some(field) = name.strip_prefix(prefix) else {
            return;
        };
        match field {
            name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
                if let Some(v) = as_scene_color(value) {
                    match prefix {
                        "hover_" => node.hover_tint = v,
                        "pressed_" => node.pressed_tint = v,
                        _ => node.tint = v,
                    }
                }
            }
            "scale_mode" | "image_scale" | "fit" => {
                if let Some(v) = as_ui_image_scale_mode(value) {
                    node.scale_mode = v;
                }
            }
            "h_align" | "image_h_align" => {
                if let Some(v) = as_ui_text_align(value) {
                    node.h_align = v;
                }
            }
            "v_align" | "image_v_align" => {
                if let Some(v) = as_ui_text_align(value) {
                    node.v_align = v;
                }
            }
            "aspect_ratio" | "ratio" => {
                if let Some(v) = as_f32(value) {
                    node.aspect_ratio = v.max(0.0);
                }
            }
            name if scene_key_in(name, TEXTURE_REGION_KEYS) => {
                if let Some(v) = as_vec4_array(value) {
                    node.texture_region = Some(v);
                }
            }
            _ => {}
        }
    });
}

pub(super) fn apply_ui_animated_image_fields(node: &mut UiAnimatedImage, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("UiAnimatedImage", name) {
            Some(NodeField::UiAnimatedImage(UiAnimatedImageField::Animations)) => {
                if let Some(animations) = parse_ui_animated_image_list(value) {
                    node.animations = animations;
                }
            }
            Some(NodeField::UiAnimatedImage(UiAnimatedImageField::CurrentAnimation)) => {
                if let Some(v) = as_str(value) {
                    node.current_animation = Cow::Owned(v.to_string());
                }
            }
            Some(NodeField::UiAnimatedImage(UiAnimatedImageField::CurrentFrame)) => {
                if let Some(v) = as_i32(value) {
                    node.current_frame = u32::try_from(v.max(0)).unwrap_or(0);
                }
            }
            Some(NodeField::UiAnimatedImage(UiAnimatedImageField::FpsScale)) => {
                if let Some(v) = as_f32(value) {
                    node.fps_scale = v.max(0.0);
                }
            }
            Some(NodeField::UiAnimatedImage(UiAnimatedImageField::Playing)) => {
                if let Some(v) = as_bool(value) {
                    node.playing = v;
                }
            }
            Some(NodeField::UiAnimatedImage(UiAnimatedImageField::Looping)) => {
                if let Some(v) = as_bool(value) {
                    node.looping = v;
                }
            }
            Some(NodeField::UiAnimatedImage(UiAnimatedImageField::TextureRegion)) => {
                if let Some(v) = as_vec4_array(value) {
                    node.animations = vec![UiAnimatedImageFrameSet {
                        name: Cow::Borrowed("default"),
                        start: [v[0], v[1]],
                        frame_size: [v[2], v[3]],
                        frame_count: 1,
                        columns: 1,
                        fps: 0.0,
                    }];
                }
            }
            _ => match name {
                name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
                    if let Some(v) = as_scene_color(value) {
                        node.tint = v;
                    }
                }
                "scale_mode" | "image_scale" | "fit" => {
                    if let Some(v) = as_ui_image_scale_mode(value) {
                        node.scale_mode = v;
                    }
                }
                "h_align" | "image_h_align" => {
                    if let Some(v) = as_ui_text_align(value) {
                        node.h_align = v;
                    }
                }
                "v_align" | "image_v_align" => {
                    if let Some(v) = as_ui_text_align(value) {
                        node.v_align = v;
                    }
                }
                "aspect_ratio" | "ratio" => {
                    if let Some(v) = as_f32(value) {
                        node.aspect_ratio = v.max(0.0);
                    }
                }
                _ => {}
            },
        }
    });
    if node.current_animation_data().is_none() {
        node.animations.push(UiAnimatedImageFrameSet::default());
    }
    let max_frame = node
        .current_animation_data()
        .map(|animation| animation.frame_count.max(1).saturating_sub(1))
        .unwrap_or(0);
    node.current_frame = node.current_frame.min(max_frame);
}

pub(super) fn parse_ui_animated_image_list(value: &SceneValue) -> Option<Vec<UiAnimatedImageFrameSet>> {
    let SceneValue::Array(items) = value else {
        return None;
    };
    let mut out = Vec::new();
    for item in items.iter() {
        if let Some(animation) = parse_ui_animated_image(item) {
            out.push(animation);
        }
    }
    (!out.is_empty()).then_some(out)
}

pub(super) fn parse_ui_animated_image(value: &SceneValue) -> Option<UiAnimatedImageFrameSet> {
    let SceneValue::Object(fields) = value else {
        return None;
    };

    let mut animation = UiAnimatedImageFrameSet::default();
    for (name, value) in fields.iter() {
        let key = name
            .as_ref()
            .trim()
            .trim_start_matches(',')
            .trim_end_matches(',')
            .trim();
        match key {
            "name" => {
                if let Some(v) = as_str(value) {
                    animation.name = Cow::Owned(v.to_string());
                }
            }
            "start" | "offset" | "origin" => {
                if let Some(v) = as_vec2(value) {
                    animation.start = [v.x, v.y];
                }
            }
            name if scene_key_in(name, TEXTURE_REGION_KEYS) => {
                if let Some([x, y, _, _]) = as_vec4_array(value) {
                    animation.start = [x, y];
                }
            }
            "frame_size" | "cell_size" => {
                if let Some(v) = as_vec2(value)
                    && v.x > 0.0
                    && v.y > 0.0
                {
                    animation.frame_size = [v.x, v.y];
                }
            }
            "frame_count" | "frames" => {
                if let Some(v) = as_i32(value) {
                    animation.frame_count = u32::try_from(v.max(1)).unwrap_or(1);
                }
            }
            "columns" | "cols" => {
                if let Some(v) = as_i32(value) {
                    animation.columns = u32::try_from(v.max(1)).unwrap_or(1);
                }
            }
            "fps" => {
                if let Some(v) = as_f32(value) {
                    animation.fps = v.max(0.0);
                }
            }
            _ => {}
        }
    }
    animation.frame_count = animation.frame_count.max(1);
    Some(animation)
}

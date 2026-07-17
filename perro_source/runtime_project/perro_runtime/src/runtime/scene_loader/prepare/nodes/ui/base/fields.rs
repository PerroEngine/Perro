use super::*;

pub(super) fn apply_ui_root_data(target: &mut UiNode, data: &SceneDefNodeData) {
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(target, base);
    }
    apply_ui_root_fields(target, &data.fields);
}

pub(super) fn apply_ui_root_fields(node: &mut UiNode, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "visible" => {
            if let Some(v) = as_bool(value) {
                node.visible = v;
            }
        }
        "modulate" | "tint" => {
            if let Some(v) = as_scene_color(value) {
                node.modulate.modulate = v;
            }
        }
        "self_modulate" | "self_tint" | "self_color" => {
            if let Some(v) = as_scene_color(value) {
                node.modulate.self_modulate = v;
            }
        }
        "children_modulate" | "child_modulate" | "children_tint" | "child_tint" => {
            if let Some(v) = as_scene_color(value) {
                node.modulate.children_modulate = v;
            }
        }
        "input_enabled" => {
            if let Some(v) = as_bool(value) {
                node.input_enabled = v;
            }
        }
        "mouse_filter" => {
            if let Some(v) = as_ui_mouse_filter(value) {
                node.mouse_filter = v;
            }
        }
        "clip_children" => {
            if let Some(v) = as_bool(value) {
                node.clip_children = v;
            }
        }
        "anchor" => {
            if let Some(v) = as_ui_anchor(value) {
                node.layout.anchor = v;
            }
        }
        // Scene-authored UI position is intentionally unsupported.
        // Anchor chooses the base placement; translation moves after layout.
        "position" | "position_percent" | "position_pct" | "position_ratio" => {}
        // Scene-authored UI size must be relative to parent layout.
        // Use `size_ratio` or `size_percent`.
        "size" | "size_px" | "pixel_size" => {}
        "size_percent" | "size_pct" => {
            if let Some(v) = as_vec2(value) {
                node.layout.size = perro_ui::UiVector2::percent(v.x, v.y);
            }
        }
        "size_ratio" => {
            if let Some(v) = as_vec2(value) {
                node.layout.size = perro_ui::UiVector2::ratio(v.x, v.y);
            }
        }
        // Absolute UI pivot unsupported.
        // Use `pivot_ratio` or `pivot_percent`.
        "pivot" => {}
        "pivot_percent" | "pivot_pct" => {
            if let Some(v) = as_vec2(value) {
                node.transform.pivot = perro_ui::UiVector2::percent(v.x, v.y);
            }
        }
        "pivot_ratio" => {
            if let Some(v) = as_vec2(value) {
                node.transform.pivot = perro_ui::UiVector2::ratio(v.x, v.y);
            }
        }
        // Absolute translation unsupported for UI authoring.
        // Use `translation_ratio`, `translation_percent`, `self_translation_ratio`, or
        // `self_translation_percent`.
        "translation" => {}
        "translation_percent" | "translation_pct" => {
            if let Some(v) = as_vec2(value) {
                node.transform.translation = perro_structs::Vector2::new(v.x * 0.01, v.y * 0.01);
            }
        }
        "translation_ratio" => {
            if let Some(v) = as_vec2(value) {
                node.transform.translation = v;
            }
        }
        "self_translation_percent" | "self_translation_pct" => {
            if let Some(v) = as_vec2(value) {
                node.transform.self_translation =
                    perro_structs::Vector2::new(v.x * 0.01, v.y * 0.01);
            }
        }
        "self_translation_ratio" => {
            if let Some(v) = as_vec2(value) {
                node.transform.self_translation = v;
            }
        }
        "scale" => {
            if let Some(v) = as_vec2(value) {
                node.transform.scale = v;
            }
        }
        "rotation" => {
            if let Some(v) = as_f32(value) {
                node.transform.rotation = v;
            }
        }
        "rotation_deg" => {
            if let Some(v) = as_f32(value) {
                node.transform.rotation = v.to_radians();
            }
        }
        "h_size" | "horizontal_size" | "width_mode" => {
            if let Some(v) = as_ui_size_mode(value) {
                node.layout.h_size = v;
            }
        }
        "v_size" | "vertical_size" | "height_mode" => {
            if let Some(v) = as_ui_size_mode(value) {
                node.layout.v_size = v;
            }
        }
        "h_align" | "horizontal_align" => {
            if let Some(v) = as_ui_horizontal_align(value) {
                node.layout.h_align = v;
            }
        }
        "v_align" | "vertical_align" => {
            if let Some(v) = as_ui_vertical_align(value) {
                node.layout.v_align = v;
            }
        }
        // Absolute scene-authored UI size clamps unsupported.
        // Use `min_size_ratio` and `max_size_ratio`.
        "min_size" | "max_size" | "min_w" | "min_width" | "min_h" | "min_height" | "max_w"
        | "max_width" | "max_h" | "max_height" => {}
        "min_size_scale" | "min_scale" | "min_size_ratio" => {
            if let Some(v) = as_vec2(value) {
                node.layout.min_size_scale = v;
            }
        }
        "max_size_scale" | "max_scale" | "max_size_ratio" => {
            if let Some(v) = as_vec2(value) {
                node.layout.max_size_scale = v;
            }
        }
        "padding" => {
            if let Some(v) = as_ui_rect(value) {
                node.layout.padding = perro_ui::UiRect::new(
                    v.left.clamp(0.0, 1.0),
                    v.top.clamp(0.0, 1.0),
                    v.right.clamp(0.0, 1.0),
                    v.bottom.clamp(0.0, 1.0),
                );
            }
        }
        "margin" => {
            if let Some(v) = as_ui_rect(value) {
                node.layout.margin = v;
            }
        }
        "z_index" => {
            if let Some(v) = as_i32(value) {
                node.layout.z_index = v;
            }
        }
        _ => {}
    });
}

pub(super) fn apply_ui_panel_fields(
    node: &mut UiPanel,
    fields: &[SceneObjectField],
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) {
    apply_ui_style_fields(&mut node.style, fields, "");
    apply_ui_style_object_fields(&mut node.style, fields, "style", static_ui_style_lookup);
}

pub(super) fn apply_ui_button_fields(
    node: &mut UiButton,
    fields: &[SceneObjectField],
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) {
    apply_ui_input_mask_fields(&mut node.input_mask, fields);
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "disabled" => {
            if let Some(v) = as_bool(value) {
                node.disabled = v;
            }
        }
        name if scene_key_in(name, HOVER_ENTER_SIGNAL_KEYS) => {
            node.hover_signals = as_signal_ids(value);
        }
        name if scene_key_in(name, HOVER_EXIT_SIGNAL_KEYS) => {
            node.hover_exit_signals = as_signal_ids(value);
        }
        "pressed_signals" | "press_signals" => {
            node.pressed_signals = as_signal_ids(value);
        }
        "released_signals" | "release_signals" => {
            node.released_signals = as_signal_ids(value);
        }
        "clicked_signals" | "click_signals" => {
            node.clicked_signals = as_signal_ids(value);
        }
        "web" => {
            node.web = parse_ui_button_web_action(value);
        }
        "cursor_icon" | "hover_cursor_icon" => {
            if let Some(v) = as_cursor_icon(value) {
                node.cursor_icon = v;
            }
        }
        _ => {}
    });
    apply_ui_style_fields(&mut node.style, fields, "");
    apply_ui_style_object_fields(&mut node.style, fields, "style", static_ui_style_lookup);
    node.hover_style = node.style.clone();
    node.pressed_style = node.style.clone();
    apply_ui_style_fields(&mut node.hover_style, fields, "hover_");
    apply_ui_style_fields(&mut node.pressed_style, fields, "pressed_");
    apply_ui_button_state_fields(node, fields, "hover", static_ui_style_lookup);
    apply_ui_button_state_fields(node, fields, "pressed", static_ui_style_lookup);
}

pub(super) fn apply_ui_checkbox_fields(
    node: &mut UiCheckbox,
    fields: &[SceneObjectField],
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) {
    apply_ui_button_fields(&mut node.button, fields, static_ui_style_lookup);
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if matches!(name, "checked" | "value")
            && let Some(v) = as_bool(value)
        {
            node.checked = v;
        }
        if matches!(name, "dot_fill" | "dot_color" | "mark_color")
            && let Some(v) = as_scene_color(value)
        {
            node.dot_fill = v;
        }
    });
    node.checked_style = node.button.style.clone();
    node.checked_hover_style = node.button.hover_style.clone();
    node.checked_pressed_style = node.button.pressed_style.clone();
    apply_ui_style_fields(&mut node.checked_style, fields, "checked_");
    apply_ui_style_fields(&mut node.checked_hover_style, fields, "checked_hover_");
    apply_ui_style_fields(&mut node.checked_pressed_style, fields, "checked_pressed_");
    apply_ui_style_object_fields(
        &mut node.checked_style,
        fields,
        "checked_style",
        static_ui_style_lookup,
    );
}

pub(super) fn apply_ui_color_picker_fields(
    node: &mut UiColorPicker,
    fields: &[SceneObjectField],
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) {
    apply_ui_button_fields(&mut node.button, fields, static_ui_style_lookup);
    apply_ui_style_object_fields(&mut node.popup_style, fields, "popup_style", static_ui_style_lookup);
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "color" | "value" | "tint" => {
            if let Some(v) = as_scene_color(value) {
                node.color = v;
            }
        }
        "popup_open" | "open" => {
            if let Some(v) = as_bool(value) {
                node.popup_open = v;
            }
        }
        "popup_size" => {
            if let Some(v) = as_vec2(value) {
                node.popup_size = [v.x.max(32.0), v.y.max(32.0)];
            }
        }
        "wheel_radius" => {
            if let Some(v) = as_f32(value) {
                node.wheel_radius = v.max(8.0);
            }
        }
        "picker_mode" | "wheel_type" | "selection_mode" => {
            if let Some(v) = as_str(value).and_then(perro_ui::UiColorPickerMode::parse) {
                node.picker_mode = v;
            }
        }
        "show_selector" | "selector_visible" | "wheel_visible" => {
            if let Some(v) = as_bool(value) {
                node.show_selector = v;
            }
        }
        "show_hex" | "hex_visible" => {
            if let Some(v) = as_bool(value) {
                node.show_hex = v;
            }
        }
        "show_rgba" | "rgba_visible" => {
            if let Some(v) = as_bool(value) {
                node.show_rgba = v;
            }
        }
        "show_hsl" | "hsl_visible" => {
            if let Some(v) = as_bool(value) {
                node.show_hsl = v;
            }
        }
        "color_changed_signals" | "changed_signals" | "value_changed_signals" => {
            node.color_changed_signals = as_signal_ids(value);
        }
        _ => {}
    });
}

pub(super) fn apply_ui_dropdown_fields(
    node: &mut UiDropdown,
    fields: &[SceneObjectField],
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) {
    apply_ui_button_fields(&mut node.button, fields, static_ui_style_lookup);
    apply_ui_style_object_fields(&mut node.popup_style, fields, "popup_style", static_ui_style_lookup);
    apply_ui_style_object_fields(&mut node.option_style, fields, "option_style", static_ui_style_lookup);
    node.option_hover_style = node.option_style.clone();
    node.option_pressed_style = node.option_style.clone();
    apply_ui_style_object_fields(
        &mut node.option_hover_style,
        fields,
        "option_hover_style",
        static_ui_style_lookup,
    );
    apply_ui_style_object_fields(
        &mut node.option_pressed_style,
        fields,
        "option_pressed_style",
        static_ui_style_lookup,
    );
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "options" | "items" => {
            node.options = as_dropdown_options(value);
        }
        "selected" | "selected_index" | "index" => {
            if let Some(v) = as_i32(value) {
                node.selected_index = v.max(0) as usize;
            }
        }
        "open" | "popup_open" => {
            if let Some(v) = as_bool(value) {
                node.open = v;
            }
        }
        "option_height" | "item_height" => {
            if let Some(v) = as_f32(value) {
                node.option_height = v.max(8.0);
            }
        }
        "popup_size" => {
            if let Some(v) = as_vec2(value) {
                node.popup_size = [v.x.max(0.0), v.y.max(0.0)];
            }
        }
        "popup_offset" => {
            if let Some(v) = as_vec2(value) {
                node.popup_offset = [v.x, v.y];
            }
        }
        "popup_direction" | "direction" => {
            if let Some(v) = as_str(value) {
                node.popup_direction = match v {
                    "up" => perro_ui::UiDropdownDirection::Up,
                    "left" => perro_ui::UiDropdownDirection::Left,
                    "right" => perro_ui::UiDropdownDirection::Right,
                    _ => perro_ui::UiDropdownDirection::Down,
                };
            }
        }
        "open_animation" | "popup_animation" => {
            if let Some(v) = as_str(value) {
                node.open_animation = match v {
                    "extend" | "expand" => perro_ui::UiDropdownOpenAnimation::Extend,
                    _ => perro_ui::UiDropdownOpenAnimation::Pop,
                };
            }
        }
        "open_animation_duration" | "popup_animation_duration" => {
            if let Some(v) = as_f32(value) {
                node.open_animation_duration = v.max(0.0);
            }
        }
        "selected_signals" | "changed_signals" | "value_changed_signals" => {
            node.selected_signals = as_signal_ids(value);
        }
        _ => {}
    });
    if node.selected_index >= node.options.len() {
        node.selected_index = 0;
    }
}

pub(super) fn apply_ui_shape_fields(node: &mut UiShape, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "shape" | "kind" => {
            if let Some(v) = as_str(value) {
                node.kind = match v {
                    "circle" => UiShapeKind::Circle,
                    "triangle" => UiShapeKind::Triangle,
                    _ => UiShapeKind::Rect,
                };
            }
        }
        "fill" | "color" => {
            if let Some(v) = as_scene_color(value) {
                node.fill = v;
            }
        }
        "stroke" | "stroke_color" => {
            if let Some(v) = as_scene_color(value) {
                node.stroke = v;
            }
        }
        "stroke_width" => {
            if let Some(v) = as_f32(value) {
                node.stroke_width = v.max(0.0);
            }
        }
        _ => {}
    });
}

pub(super) fn apply_ui_image_button_fields(node: &mut UiImageButton, fields: &[SceneObjectField]) {
    apply_ui_input_mask_fields(&mut node.input_mask, fields);
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "disabled" => {
            if let Some(v) = as_bool(value) {
                node.disabled = v;
            }
        }
        name if scene_key_in(name, HOVER_ENTER_SIGNAL_KEYS) => {
            node.hover_signals = as_signal_ids(value);
        }
        name if scene_key_in(name, HOVER_EXIT_SIGNAL_KEYS) => {
            node.hover_exit_signals = as_signal_ids(value);
        }
        "pressed_signals" | "press_signals" => {
            node.pressed_signals = as_signal_ids(value);
        }
        "released_signals" | "release_signals" => {
            node.released_signals = as_signal_ids(value);
        }
        "clicked_signals" | "click_signals" => {
            node.clicked_signals = as_signal_ids(value);
        }
        "web" => {
            node.web = parse_ui_button_web_action(value);
        }
        "cursor_icon" | "hover_cursor_icon" => {
            if let Some(v) = as_cursor_icon(value) {
                node.cursor_icon = v;
            }
        }
        _ => {}
    });
    apply_ui_image_button_image_fields(node, fields, "");
    node.hover_tint = node.tint;
    node.pressed_tint = node.tint;
    apply_ui_image_button_image_fields(node, fields, "hover_");
    apply_ui_image_button_image_fields(node, fields, "pressed_");
    apply_ui_image_button_state_fields(node, fields, "hover");
    apply_ui_image_button_state_fields(node, fields, "pressed");
}

pub(super) fn apply_ui_image_button_state_fields(
    node: &mut UiImageButton,
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
        match state_name {
            "hover" => {
                if let Some(tint) = ui_state_tint(entries.as_ref()) {
                    node.hover_tint = tint;
                }
                node.hover_base = Some(base);
                node.hover_size_override = size_override;
            }
            "pressed" => {
                if let Some(tint) = ui_state_tint(entries.as_ref()) {
                    node.pressed_tint = tint;
                }
                node.pressed_base = Some(base);
                node.pressed_size_override = size_override;
            }
            _ => {}
        }
    });
}

pub(super) fn ui_state_tint(fields: &[SceneObjectField]) -> Option<Color> {
    fields.iter().find_map(|(name, value)| match name.as_ref() {
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => as_scene_color(value),
        _ => None,
    })
}

pub(super) fn parse_ui_button_web_action(value: &SceneValue) -> Option<perro_ui::UiButtonWebAction> {
    let SceneValue::Object(fields) = value else {
        return None;
    };
    let href = fields.iter().find_map(|(name, value)| {
        (name.as_ref().trim() == "href")
            .then(|| as_str(value).map(perro_project::normalize_route_href))
            .flatten()
    })?;
    Some(perro_ui::UiButtonWebAction {
        href: std::borrow::Cow::Owned(href),
    })
}

pub(super) fn as_dropdown_options(value: &SceneValue) -> Vec<perro_ui::UiDropdownOption> {
    let SceneValue::Array(items) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match item {
            SceneValue::Object(fields) => {
                let label = fields
                    .iter()
                    .find_map(|(name, value)| {
                        matches!(name.as_ref(), "label" | "name" | "text")
                            .then(|| as_str(value))
                            .flatten()
                    })?
                    .to_string();
                let variant = fields
                    .iter()
                    .find_map(|(name, value)| {
                        matches!(name.as_ref(), "value" | "val")
                            .then(|| scene_value_to_variant(value))
                            .flatten()
                    })
                    .unwrap_or_else(|| perro_variant::Variant::from(label.as_str()));
                Some(perro_ui::UiDropdownOption::new(label, variant))
            }
            _ => {
                let label = scene_value_label(item)?;
                let variant = scene_value_to_variant(item)
                    .unwrap_or_else(|| perro_variant::Variant::from(label.as_str()));
                Some(perro_ui::UiDropdownOption::new(label, variant))
            }
        })
        .collect()
}

pub(super) fn as_tree_list_items(value: &SceneValue) -> Vec<UiTreeListItem> {
    let SceneValue::Array(items) = value else {
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| match item {
            SceneValue::Object(fields) => {
                let label = fields
                    .iter()
                    .find_map(|(name, value)| {
                        matches!(name.as_ref(), "label" | "name" | "text")
                            .then(|| as_str(value))
                            .flatten()
                    })
                    .unwrap_or("")
                    .to_string();
                let id = fields
                    .iter()
                    .find_map(|(name, value)| {
                        matches!(name.as_ref(), "id" | "key")
                            .then(|| as_str(value))
                            .flatten()
                    })
                    .map(str::to_string)
                    .unwrap_or_else(|| label.clone());
                let parent = fields.iter().find_map(|(name, value)| {
                    matches!(name.as_ref(), "parent" | "parent_index")
                        .then(|| as_i32(value))
                        .flatten()
                        .and_then(|v| (v >= 0).then_some(v as usize))
                });
                let open = fields
                    .iter()
                    .find_map(|(name, value)| {
                        matches!(name.as_ref(), "open" | "expanded")
                            .then(|| as_bool(value))
                            .flatten()
                    })
                    .unwrap_or(true);
                let selectable = fields
                    .iter()
                    .find_map(|(name, value)| {
                        matches!(name.as_ref(), "selectable")
                            .then(|| as_bool(value))
                            .flatten()
                    })
                    .unwrap_or(true);
                let value = fields
                    .iter()
                    .find_map(|(name, value)| {
                        matches!(name.as_ref(), "value" | "val")
                            .then(|| scene_value_to_variant(value))
                            .flatten()
                    })
                    .unwrap_or_else(|| perro_variant::Variant::from(id.as_str()));
                let mut out = UiTreeListItem::new(label)
                    .with_id(id)
                    .with_value(value);
                out.parent = parent.filter(|parent| *parent < idx);
                out.open = open;
                out.selectable = selectable;
                Some(out)
            }
            _ => {
                let label = scene_value_label(item)?;
                Some(UiTreeListItem::new(label))
            }
        })
        .collect()
}

pub(super) fn scene_value_label(value: &SceneValue) -> Option<String> {
    match value {
        SceneValue::Str(value) => Some(value.to_string()),
        SceneValue::Key(value) => Some(value.to_string()),
        SceneValue::Bool(value) => Some(value.to_string()),
        SceneValue::F32(value) => Some(value.to_string()),
        SceneValue::I32(value) => Some(value.to_string()),
        SceneValue::Hashed(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn scene_value_to_variant(value: &SceneValue) -> Option<perro_variant::Variant> {
    match value {
        SceneValue::Str(value) => Some(perro_variant::Variant::from(value.as_ref())),
        SceneValue::Key(value) => Some(perro_variant::Variant::from(value.as_ref())),
        SceneValue::Bool(value) => Some(perro_variant::Variant::from(*value)),
        SceneValue::F32(value) => Some(perro_variant::Variant::from(*value)),
        SceneValue::I32(value) => Some(perro_variant::Variant::from(*value)),
        SceneValue::Hashed(value) => Some(perro_variant::Variant::from(*value as i64)),
        _ => None,
    }
}

pub(super) fn apply_ui_input_mask_fields(mask: &mut perro_ui::UiInputMask, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "input_allow_players" | "input_only_players" | "allow_players" | "only_players" => {
            if let Some(v) = as_usize_list(value) {
                mask.allow_players = v;
            }
        }
        "input_deny_players" | "input_block_players" | "deny_players" | "block_players" => {
            if let Some(v) = as_usize_list(value) {
                mask.deny_players = v;
            }
        }
        "input_allow_gamepads" | "input_only_gamepads" | "allow_gamepads" | "only_gamepads" => {
            if let Some(v) = as_usize_list(value) {
                mask.allow_gamepads = v;
            }
        }
        "input_deny_gamepads" | "input_block_gamepads" | "deny_gamepads" | "block_gamepads" => {
            if let Some(v) = as_usize_list(value) {
                mask.deny_gamepads = v;
            }
        }
        "input_allow_joycons" | "input_only_joycons" | "allow_joycons" | "only_joycons" => {
            if let Some(v) = as_usize_list(value) {
                mask.allow_joycons = v;
            }
        }
        "input_deny_joycons" | "input_block_joycons" | "deny_joycons" | "block_joycons" => {
            if let Some(v) = as_usize_list(value) {
                mask.deny_joycons = v;
            }
        }
        "input_allow_kbm" | "input_only_kbm" | "allow_kbm" | "only_kbm" => {
            if let Some(v) = as_bool(value) {
                mask.allow_kbm = v;
            }
        }
        "input_deny_kbm" | "input_block_kbm" | "deny_kbm" | "block_kbm" => {
            if let Some(v) = as_bool(value) {
                mask.deny_kbm = v;
            }
        }
        _ => {}
    });
}

pub(super) fn as_usize_list(value: &SceneValue) -> Option<Vec<usize>> {
    match value {
        SceneValue::Array(items) => Some(
            items
                .iter()
                .filter_map(as_i32)
                .filter_map(|v| usize::try_from(v).ok())
                .collect(),
        ),
        _ => as_i32(value)
            .and_then(|v| usize::try_from(v).ok())
            .map(|v| vec![v]),
    }
}

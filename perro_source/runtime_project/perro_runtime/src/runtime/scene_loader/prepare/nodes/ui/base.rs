mod style;
use style::*;

fn build_ui_box(data: &SceneDefNodeData) -> UiBox {
    let mut node = UiBox::new();
    apply_ui_root_data(&mut node, data);
    node
}

fn load_ui_style_source(
    source: &str,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> Option<perro_ui::UiStyle> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    if let Some(lookup) = static_ui_style_lookup {
        let hash = perro_ids::parse_hashed_source_uri(source)
            .unwrap_or_else(|| perro_ids::string_to_u64(source));
        return Some(lookup(hash).clone());
    }
    let bytes = load_asset(source).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    parse_ui_style_source(text)
}

fn build_ui_panel(
    data: &SceneDefNodeData,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> UiPanel {
    let mut node = UiPanel::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_panel_fields(&mut node, &data.fields, static_ui_style_lookup);
    node
}

fn build_ui_button(
    data: &SceneDefNodeData,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> UiButton {
    let mut node = UiButton::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_button_fields(&mut node, &data.fields, static_ui_style_lookup);
    node
}

fn build_ui_checkbox(
    data: &SceneDefNodeData,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> UiCheckbox {
    let mut node = UiCheckbox::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.button.base, base);
    }
    apply_ui_root_fields(&mut node.button.base, &data.fields);
    apply_ui_checkbox_fields(&mut node, &data.fields, static_ui_style_lookup);
    node
}

fn build_ui_label(data: &SceneDefNodeData) -> UiLabel {
    let mut node = UiLabel::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_label_fields(&mut node, &data.fields);
    node
}

fn build_ui_image(data: &SceneDefNodeData) -> UiImage {
    let mut node = UiImage::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_image_fields(&mut node, &data.fields);
    node
}

fn build_ui_image_button(data: &SceneDefNodeData) -> UiImageButton {
    let mut node = UiImageButton::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_image_button_fields(&mut node, &data.fields);
    node
}

fn build_ui_nine_slice(data: &SceneDefNodeData) -> UiNineSlice {
    let mut node = UiNineSlice::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_nine_slice_fields(&mut node, &data.fields);
    node
}

fn build_ui_camera_stream(data: &SceneDefNodeData) -> UiCameraStream {
    let mut node = UiCameraStream::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_camera_stream_fields(&mut node.stream, &data.fields);
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| match name {
        "tint" | "color" | "modulate" => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        "corner_radius" | "radius" => {
            if let Some(v) = as_ui_corner_radius(value) {
                node.corner_radius = v;
            }
        }
        _ => {}
    });
    node
}

fn build_ui_animated_image(data: &SceneDefNodeData) -> UiAnimatedImage {
    let mut node = UiAnimatedImage::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_animated_image_fields(&mut node, &data.fields);
    node
}

fn build_ui_text_box(
    data: &SceneDefNodeData,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> UiTextBox {
    let mut node = UiTextBox::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_text_edit_fields(&mut node.inner, &data.fields, static_ui_style_lookup);
    node.inner.multiline = false;
    node
}

fn build_ui_text_block(
    data: &SceneDefNodeData,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> UiTextBlock {
    let mut node = UiTextBlock::new();
    node.inner.multiline = true;
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_text_edit_fields(&mut node.inner, &data.fields, static_ui_style_lookup);
    node
}

fn build_ui_scroll_container(data: &SceneDefNodeData) -> UiScrollContainer {
    let mut node = UiScrollContainer::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_scroll_container_fields(&mut node, &data.fields);
    node
}

fn build_ui_layout(data: &SceneDefNodeData) -> UiLayout {
    let mut node = UiLayout::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_container_fields(&mut node.inner, &data.fields, true);
    node
}

fn build_ui_hlayout(data: &SceneDefNodeData) -> UiHLayout {
    let mut node = UiHLayout::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_fixed_container_fields(&mut node.inner, &data.fields);
    node
}

fn build_ui_vlayout(data: &SceneDefNodeData) -> UiVLayout {
    let mut node = UiVLayout::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_fixed_container_fields(&mut node.inner, &data.fields);
    node
}

fn build_ui_grid(data: &SceneDefNodeData) -> UiGrid {
    let mut node = UiGrid::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| match name {
        "columns" => {
            if let Some(v) = as_i32(value)
                && v > 0
            {
                node.columns = v as u32;
            }
        }
        "h_spacing" | "horizontal_spacing" => {
            if let Some(v) = as_f32(value) {
                node.h_spacing = v;
            }
        }
        "v_spacing" | "vertical_spacing" => {
            if let Some(v) = as_f32(value) {
                node.v_spacing = v;
            }
        }
        _ => {}
    });
    node
}

fn build_ui_list(data: &SceneDefNodeData) -> UiList {
    let mut node = UiList::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| match name {
        "indent" => {
            if let Some(v) = as_f32(value) {
                node.indent = v.max(0.0);
            }
        }
        "v_spacing" | "vertical_spacing" | "spacing" => {
            if let Some(v) = as_f32(value) {
                node.v_spacing = v.max(0.0);
            }
        }
        _ => {}
    });
    node
}

fn build_ui_list_indent(data: &SceneDefNodeData) -> UiListIndent {
    let mut node = UiListIndent::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    node
}

fn apply_ui_root_data(target: &mut UiBox, data: &SceneDefNodeData) {
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(target, base);
    }
    apply_ui_root_fields(target, &data.fields);
}

fn apply_ui_root_fields(node: &mut UiBox, fields: &[SceneObjectField]) {
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

fn apply_ui_panel_fields(
    node: &mut UiPanel,
    fields: &[SceneObjectField],
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) {
    apply_ui_style_fields(&mut node.style, fields, "");
    apply_ui_style_object_fields(&mut node.style, fields, "style", static_ui_style_lookup);
}

fn apply_ui_button_fields(
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
        "hover_signals" | "hovered_signals" | "hover_enter_signals" => {
            node.hover_signals = as_signal_ids(value);
        }
        "hover_exit_signals" | "unhover_signals" => {
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

fn apply_ui_checkbox_fields(
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

fn apply_ui_image_button_fields(node: &mut UiImageButton, fields: &[SceneObjectField]) {
    apply_ui_input_mask_fields(&mut node.input_mask, fields);
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "disabled" => {
            if let Some(v) = as_bool(value) {
                node.disabled = v;
            }
        }
        "hover_signals" | "hovered_signals" | "hover_enter_signals" => {
            node.hover_signals = as_signal_ids(value);
        }
        "hover_exit_signals" | "unhover_signals" => {
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

fn apply_ui_image_button_state_fields(
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

fn ui_state_tint(fields: &[SceneObjectField]) -> Option<Color> {
    fields.iter().find_map(|(name, value)| match name.as_ref() {
        "tint" | "color" | "modulate" => as_scene_color(value),
        _ => None,
    })
}

fn parse_ui_button_web_action(value: &SceneValue) -> Option<perro_ui::UiButtonWebAction> {
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

fn apply_ui_input_mask_fields(mask: &mut perro_ui::UiInputMask, fields: &[SceneObjectField]) {
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

fn as_usize_list(value: &SceneValue) -> Option<Vec<usize>> {
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

fn apply_ui_label_fields(node: &mut UiLabel, fields: &[SceneObjectField]) {
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

fn apply_ui_image_fields(node: &mut UiImage, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "tint" | "color" | "modulate" => {
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
        "atlas_region" | "texture_region" | "region" => {
            if let Some(v) = as_vec4_array(value) {
                node.texture_region = Some(v);
            }
        }
        _ => {}
    });
}

fn apply_ui_nine_slice_fields(node: &mut UiNineSlice, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "tint" | "color" | "modulate" => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        "atlas_region" | "texture_region" | "region" => {
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

fn apply_ui_image_button_image_fields(
    node: &mut UiImageButton,
    fields: &[SceneObjectField],
    prefix: &str,
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        let Some(field) = name.strip_prefix(prefix) else {
            return;
        };
        match field {
            "tint" | "color" | "modulate" => {
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
            "atlas_region" | "texture_region" | "region" => {
                if let Some(v) = as_vec4_array(value) {
                    node.texture_region = Some(v);
                }
            }
            _ => {}
        }
    });
}

fn apply_ui_animated_image_fields(node: &mut UiAnimatedImage, fields: &[SceneObjectField]) {
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
                "tint" | "color" | "modulate" => {
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

fn parse_ui_animated_image_list(value: &SceneValue) -> Option<Vec<UiAnimatedImageFrameSet>> {
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

fn parse_ui_animated_image(value: &SceneValue) -> Option<UiAnimatedImageFrameSet> {
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
            "atlas_region" | "texture_region" | "region" => {
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

fn apply_ui_text_edit_fields(
    node: &mut perro_ui::UiTextEdit,
    fields: &[SceneObjectField],
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) {
    apply_ui_input_mask_fields(&mut node.input_mask, fields);
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "text" => {
            if let Some(v) = as_str(value) {
                node.text = Cow::Owned(decode_scene_text_edit_literal(v, node.multiline));
                node.caret = node.text.len();
                node.anchor = node.caret;
            }
        }
        "placeholder" | "hint" => {
            if let Some(v) = as_str(value) {
                node.placeholder = Cow::Owned(decode_scene_text_edit_literal(v, node.multiline));
            }
        }
        "color" | "text_color" => {
            if let Some(v) = as_scene_color(value) {
                node.color = v;
            }
        }
        "placeholder_color" | "hint_color" => {
            if let Some(v) = as_scene_color(value) {
                node.placeholder_color = v;
            }
        }
        "selection_color" => {
            if let Some(v) = as_scene_color(value) {
                node.selection_color = v;
            }
        }
        "caret_color" | "cursor_color" => {
            if let Some(v) = as_scene_color(value) {
                node.caret_color = v;
            }
        }
        // Absolute text size unsupported.
        // Use `text_size_ratio`.
        "font_size" => {}
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
        "text_padding" | "content_padding" => {
            if let Some(v) = as_ui_rect(value) {
                node.padding = v;
            }
        }
        "editable" => {
            if let Some(v) = as_bool(value) {
                node.editable = v;
            }
        }
        "hover_signals" | "hovered_signals" | "hover_enter_signals" => {
            node.hover_signals = as_signal_ids(value);
        }
        "hover_exit_signals" | "unhover_signals" | "unhovered_signals" => {
            node.hover_exit_signals = as_signal_ids(value);
        }
        "focused_signals" | "focus_signals" => {
            node.focused_signals = as_signal_ids(value);
        }
        "unfocused_signals" | "unfocus_signals" => {
            node.unfocused_signals = as_signal_ids(value);
        }
        "text_changed_signals" | "changed_signals" => {
            node.text_changed_signals = as_signal_ids(value);
        }
        _ => {}
    });
    apply_ui_style_fields(&mut node.style, fields, "");
    apply_ui_style_fields(&mut node.focused_style, fields, "focused_");
    apply_ui_style_object_fields(&mut node.style, fields, "style", static_ui_style_lookup);
    apply_ui_style_object_fields(
        &mut node.focused_style,
        fields,
        "focused_style",
        static_ui_style_lookup,
    );
}

fn decode_scene_text_literal(text: &str) -> String {
    if let Some(stripped) = text.strip_prefix("%%loc:") {
        return decode_text_escapes(&format!("%loc:{stripped}"));
    }
    if let Some(key) = parse_locale_text_key(text) {
        return key.to_string();
    }
    decode_text_escapes(text)
}

fn decode_scene_text_edit_literal(text: &str, multiline: bool) -> String {
    let text = decode_scene_text_literal(text);
    if multiline {
        text
    } else {
        text.replace(['\r', '\n', '\t'], " ")
    }
}

fn parse_locale_text_key(text: &str) -> Option<&str> {
    let raw = text.strip_prefix("%loc:")?.trim();
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        let key = raw[1..raw.len() - 1].trim();
        return (!key.is_empty()).then_some(key);
    }
    (!raw.is_empty()).then_some(raw)
}

fn decode_text_escapes(text: &str) -> String {
    if !text.contains('\\') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn apply_ui_scroll_container_fields(node: &mut UiScrollContainer, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "scroll" | "scroll_offset" => {
            if let Some(v) = as_vec2(value) {
                node.scroll = v;
            }
        }
        "h_scroll" | "horizontal_scroll" | "scroll_x" => {
            if let Some(v) = as_f32(value) {
                node.scroll.x = v;
            }
        }
        "v_scroll" | "vertical_scroll" | "scroll_y" => {
            if let Some(v) = as_f32(value) {
                node.scroll.y = v;
            }
        }
        _ => {}
    });
}

fn apply_ui_container_fields(
    node: &mut perro_ui::UiLayoutContainer,
    fields: &[SceneObjectField],
    allow_mode: bool,
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "mode" | "layout" | "kind" => {
            if allow_mode && let Some(v) = as_ui_layout_mode(value) {
                node.mode = v;
            }
        }
        "spacing" => {
            if let Some(v) = as_f32(value) {
                node.spacing = v;
            }
        }
        "h_spacing" | "horizontal_spacing" => {
            if let Some(v) = as_f32(value) {
                node.h_spacing = v;
            }
        }
        "v_spacing" | "vertical_spacing" => {
            if let Some(v) = as_f32(value) {
                node.v_spacing = v;
            }
        }
        "columns" => {
            if let Some(v) = as_i32(value)
                && v > 0
            {
                node.columns = v as u32;
            }
        }
        _ => {}
    });
}

fn apply_ui_fixed_container_fields(
    node: &mut perro_ui::UiFixedLayoutContainer,
    fields: &[SceneObjectField],
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "spacing" => {
            if let Some(v) = as_f32(value) {
                node.spacing = v;
            }
        }
        "h_spacing" | "horizontal_spacing" => {
            if let Some(v) = as_f32(value) {
                node.h_spacing = v;
            }
        }
        "v_spacing" | "vertical_spacing" => {
            if let Some(v) = as_f32(value) {
                node.v_spacing = v;
            }
        }
        "columns" => {
            if let Some(v) = as_i32(value)
                && v > 0
            {
                node.columns = v as u32;
            }
        }
        _ => {}
    });
}

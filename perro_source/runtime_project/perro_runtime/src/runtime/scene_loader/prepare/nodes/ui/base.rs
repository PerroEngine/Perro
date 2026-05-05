fn build_ui_box(data: &SceneDefNodeData) -> UiBox {
    let mut node = UiBox::new();
    apply_ui_root_data(&mut node, data);
    node
}

fn build_ui_panel(data: &SceneDefNodeData) -> UiPanel {
    let mut node = UiPanel::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_panel_fields(&mut node, &data.fields);
    node
}

fn build_ui_button(data: &SceneDefNodeData) -> UiButton {
    let mut node = UiButton::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_button_fields(&mut node, &data.fields);
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

fn build_ui_text_box(data: &SceneDefNodeData) -> UiTextBox {
    let mut node = UiTextBox::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_text_edit_fields(&mut node.inner, &data.fields);
    node.inner.multiline = false;
    node
}

fn build_ui_text_block(data: &SceneDefNodeData) -> UiTextBlock {
    let mut node = UiTextBlock::new();
    node.inner.multiline = true;
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_text_edit_fields(&mut node.inner, &data.fields);
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

fn build_ui_tree_list(data: &SceneDefNodeData) -> UiTreeList {
    let mut node = UiTreeList::new();
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
        // Absolute UI position unsupported.
        // Use `position_ratio` or `position_percent`.
        "position" => {}
        "position_percent" | "position_pct" => {
            if let Some(v) = as_vec2(value) {
                node.transform.position = perro_ui::UiVector2::percent(v.x, v.y);
            }
        }
        "position_ratio" => {
            if let Some(v) = as_vec2(value) {
                node.transform.position = perro_ui::UiVector2::ratio(v.x, v.y);
            }
        }
        // Intentionally ignore absolute UI `size` in scene parsing.
        // Use `size_ratio` or `size_percent` instead.
        "size" => {}
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
        // Use `translation_ratio` or `translation_percent`.
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
        // Absolute min/max size unsupported.
        // Use `min_size_ratio` / `max_size_ratio`.
        "min_size" | "max_size" | "min_w" | "min_width" | "min_h" | "min_height"
        | "max_w" | "max_width" | "max_h" | "max_height" => {}
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
                node.layout.padding = v;
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

fn apply_ui_panel_fields(node: &mut UiPanel, fields: &[SceneObjectField]) {
    apply_ui_style_fields(&mut node.style, fields, "");
}

fn apply_ui_button_fields(node: &mut UiButton, fields: &[SceneObjectField]) {
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
        "click_signals" | "clicked_signals" => {
            node.click_signals = as_signal_ids(value);
        }
        "cursor_icon" | "hover_cursor_icon" => {
            if let Some(v) = as_cursor_icon(value) {
                node.cursor_icon = v;
            }
        }
        _ => {}
    });
    apply_ui_style_fields(&mut node.style, fields, "");
    apply_ui_style_fields(&mut node.hover_style, fields, "hover_");
    apply_ui_style_fields(&mut node.pressed_style, fields, "pressed_");
    apply_ui_style_object_fields(&mut node.style, fields, "style");
    apply_ui_button_state_fields(node, fields, "hover");
    apply_ui_button_state_fields(node, fields, "pressed");
}

fn apply_ui_label_fields(node: &mut UiLabel, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "text" => {
            if let Some(v) = as_str(value) {
                node.text = Cow::Owned(decode_text_escapes(v));
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

fn apply_ui_text_edit_fields(node: &mut perro_ui::UiTextEdit, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "text" => {
            if let Some(v) = as_str(value) {
                node.text = Cow::Owned(decode_text_edit_escapes(v, node.multiline));
                node.caret = node.text.len();
                node.anchor = node.caret;
            }
        }
        "placeholder" | "hint" => {
            if let Some(v) = as_str(value) {
                node.placeholder = Cow::Owned(decode_text_edit_escapes(v, node.multiline));
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
    apply_ui_style_object_fields(&mut node.style, fields, "style");
    apply_ui_style_object_fields(&mut node.focused_style, fields, "focused_style");
}

fn decode_text_edit_escapes(text: &str, multiline: bool) -> String {
    let text = decode_text_escapes(text);
    if multiline {
        text
    } else {
        text.replace(['\r', '\n', '\t'], " ")
    }
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

fn apply_ui_container_fields(
    node: &mut perro_ui::UiLayoutContainer,
    fields: &[SceneObjectField],
    allow_mode: bool,
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "mode" | "layout" | "kind" => {
            if allow_mode
                && let Some(v) = as_ui_layout_mode(value)
            {
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

fn apply_ui_style_fields(style: &mut perro_ui::UiStyle, fields: &[SceneObjectField], prefix: &str) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        let Some(field) = name.strip_prefix(prefix) else {
            return;
        };
        match field {
            "fill" | "color" => {
                if let Some(v) = as_scene_color(value) {
                    style.fill = v;
                }
            }
            "stroke" => {
                if let Some(v) = as_scene_color(value) {
                    style.stroke = v;
                }
            }
            "stroke_width" => {
                if let Some(v) = as_f32(value) {
                    style.stroke_width = v;
                }
            }
            "corner_radius" | "radius" => {
                if let Some(v) = as_ui_corner_radius(value) {
                    style.corner_radius = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_ui_style_object_fields(
    style: &mut perro_ui::UiStyle,
    fields: &[SceneObjectField],
    object_name: &str,
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if name != object_name {
            return;
        }
        let SceneValue::Object(entries) = value else {
            return;
        };
        apply_ui_style_fields(style, entries.as_ref(), "");
    });
}

fn apply_ui_button_state_fields(node: &mut UiButton, fields: &[SceneObjectField], state_name: &str) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if name != state_name {
            return;
        }
        let SceneValue::Object(entries) = value else {
            return;
        };

        let mut base = node.base.clone();
        let mut style = match state_name {
            "hover" => node.hover_style.clone(),
            "pressed" => node.pressed_style.clone(),
            _ => return,
        };

        let size_override = ui_state_has_explicit_size_override(entries.as_ref());
        apply_ui_root_fields(&mut base, entries.as_ref());
        apply_ui_style_fields(&mut style, entries.as_ref(), "");
        apply_ui_style_object_fields(&mut style, entries.as_ref(), "style");

        match state_name {
            "hover" => {
                node.hover_base = Some(base);
                node.hover_size_override = size_override;
                node.hover_style = style;
            }
            "pressed" => {
                node.pressed_base = Some(base);
                node.pressed_size_override = size_override;
                node.pressed_style = style;
            }
            _ => {}
        }
    });
}

fn ui_state_has_explicit_size_override(fields: &[SceneObjectField]) -> bool {
    let mut found = false;
    SceneFieldIterRef::new(fields).for_each(|name, _| {
        if matches!(
            name,
            "size"
                | "size_percent"
                | "size_ratio"
                | "h_size"
                | "horizontal_size"
                | "v_size"
                | "vertical_size"
                | "min_size"
                | "max_size"
                | "min_w"
                | "min_width"
                | "min_h"
                | "min_height"
                | "max_w"
                | "max_width"
                | "max_h"
                | "max_height"
                | "min_size_scale"
                | "min_scale"
                | "min_size_ratio"
                | "max_size_scale"
                | "max_scale"
                | "max_size_ratio"
        ) {
            found = true;
        }
    });
    found
}

fn as_ui_corner_radius(value: &SceneValue) -> Option<f32> {
    if let Some(v) = as_f32(value) {
        return Some(v);
    }
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "full" | "pill" | "round" | "rounded" => Some(f32::INFINITY),
        _ => None,
    }
}

fn as_signal_id(value: &SceneValue) -> Option<perro_ids::SignalID> {
    if let Some(v) = as_str(value) {
        return Some(perro_ids::SignalID::from_string(v));
    }
    if let Some(v) = value.as_key() {
        return Some(perro_ids::SignalID::from_string(v));
    }
    value.as_hashed().map(perro_ids::SignalID::from_u64)
}

fn as_signal_ids(value: &SceneValue) -> Vec<perro_ids::SignalID> {
    match value {
        SceneValue::Array(values) => values.iter().filter_map(as_signal_id).collect(),
        _ => as_signal_id(value).into_iter().collect(),
    }
}

fn as_ui_mouse_filter(value: &SceneValue) -> Option<UiMouseFilter> {
    match as_str(value)?.to_ascii_lowercase().as_str() {
        "stop" => Some(UiMouseFilter::Stop),
        "pass" => Some(UiMouseFilter::Pass),
        "ignore" => Some(UiMouseFilter::Ignore),
        _ => None,
    }
}

fn as_cursor_icon(value: &SceneValue) -> Option<perro_ui::CursorIcon> {
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "default" | "arrow" => Some(perro_ui::CursorIcon::Default),
        "contextmenu" => Some(perro_ui::CursorIcon::ContextMenu),
        "help" => Some(perro_ui::CursorIcon::Help),
        "pointer" | "hand" => Some(perro_ui::CursorIcon::Pointer),
        "progress" => Some(perro_ui::CursorIcon::Progress),
        "wait" => Some(perro_ui::CursorIcon::Wait),
        "cell" => Some(perro_ui::CursorIcon::Cell),
        "crosshair" => Some(perro_ui::CursorIcon::Crosshair),
        "text" | "ibeam" => Some(perro_ui::CursorIcon::Text),
        "verticaltext" => Some(perro_ui::CursorIcon::VerticalText),
        "alias" => Some(perro_ui::CursorIcon::Alias),
        "copy" => Some(perro_ui::CursorIcon::Copy),
        "move" => Some(perro_ui::CursorIcon::Move),
        "nodrop" => Some(perro_ui::CursorIcon::NoDrop),
        "notallowed" => Some(perro_ui::CursorIcon::NotAllowed),
        "grab" => Some(perro_ui::CursorIcon::Grab),
        "grabbing" => Some(perro_ui::CursorIcon::Grabbing),
        "eresize" => Some(perro_ui::CursorIcon::EResize),
        "nresize" => Some(perro_ui::CursorIcon::NResize),
        "neresize" => Some(perro_ui::CursorIcon::NeResize),
        "nwresize" => Some(perro_ui::CursorIcon::NwResize),
        "sresize" => Some(perro_ui::CursorIcon::SResize),
        "seresize" => Some(perro_ui::CursorIcon::SeResize),
        "swresize" => Some(perro_ui::CursorIcon::SwResize),
        "wresize" => Some(perro_ui::CursorIcon::WResize),
        "ewresize" | "horizontalresize" => Some(perro_ui::CursorIcon::EwResize),
        "nsresize" | "verticalresize" => Some(perro_ui::CursorIcon::NsResize),
        "neswresize" => Some(perro_ui::CursorIcon::NeswResize),
        "nwseresize" => Some(perro_ui::CursorIcon::NwseResize),
        "colresize" => Some(perro_ui::CursorIcon::ColResize),
        "rowresize" => Some(perro_ui::CursorIcon::RowResize),
        "allscroll" => Some(perro_ui::CursorIcon::AllScroll),
        "zoomin" => Some(perro_ui::CursorIcon::ZoomIn),
        "zoomout" => Some(perro_ui::CursorIcon::ZoomOut),
        "dndask" => Some(perro_ui::CursorIcon::DndAsk),
        "allresize" => Some(perro_ui::CursorIcon::AllResize),
        _ => None,
    }
}

fn as_ui_anchor(value: &SceneValue) -> Option<perro_ui::UiAnchor> {
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "c" | "center" | "middle" => Some(perro_ui::UiAnchor::Center),
        "l" | "left" | "centerleft" | "middleleft" => Some(perro_ui::UiAnchor::Left),
        "r" | "right" | "centerright" | "middleright" => Some(perro_ui::UiAnchor::Right),
        "t" | "top" | "topcenter" | "topmiddle" => Some(perro_ui::UiAnchor::Top),
        "b" | "bottom" | "bottomcenter" | "bottommiddle" => Some(perro_ui::UiAnchor::Bottom),
        "tl" | "topleft" | "lefttop" => Some(perro_ui::UiAnchor::TopLeft),
        "tr" | "topright" | "righttop" => Some(perro_ui::UiAnchor::TopRight),
        "bl" | "bottomleft" | "leftbottom" => Some(perro_ui::UiAnchor::BottomLeft),
        "br" | "bottomright" | "rightbottom" => Some(perro_ui::UiAnchor::BottomRight),
        _ => None,
    }
}

fn as_ui_layout_mode(value: &SceneValue) -> Option<perro_ui::UiLayoutMode> {
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "h" | "horizontal" | "hlayout" | "hbox" | "row" => Some(perro_ui::UiLayoutMode::H),
        "v" | "vertical" | "vlayout" | "vbox" | "column" | "col" => {
            Some(perro_ui::UiLayoutMode::V)
        }
        "g" | "grid" => Some(perro_ui::UiLayoutMode::Grid),
        _ => None,
    }
}

fn as_ui_size_mode(value: &SceneValue) -> Option<perro_ui::UiSizeMode> {
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "fixed" | "f" => Some(perro_ui::UiSizeMode::Fixed),
        "fill" | "expand" => Some(perro_ui::UiSizeMode::Fill),
        "fit" | "fitchild" | "fitchildren" | "wrap" | "content" => {
            Some(perro_ui::UiSizeMode::FitChildren)
        }
        _ => None,
    }
}

fn as_ui_horizontal_align(value: &SceneValue) -> Option<perro_ui::UiHorizontalAlign> {
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "start" | "left" | "l" => Some(perro_ui::UiHorizontalAlign::Left),
        "center" | "c" | "middle" => Some(perro_ui::UiHorizontalAlign::Center),
        "end" | "right" | "r" => Some(perro_ui::UiHorizontalAlign::Right),
        "fill" | "stretch" => Some(perro_ui::UiHorizontalAlign::Fill),
        _ => None,
    }
}

fn as_ui_vertical_align(value: &SceneValue) -> Option<perro_ui::UiVerticalAlign> {
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "start" | "top" | "t" => Some(perro_ui::UiVerticalAlign::Top),
        "center" | "c" | "middle" => Some(perro_ui::UiVerticalAlign::Center),
        "end" | "bottom" | "b" => Some(perro_ui::UiVerticalAlign::Bottom),
        "fill" | "stretch" => Some(perro_ui::UiVerticalAlign::Fill),
        _ => None,
    }
}

fn as_ui_text_align(value: &SceneValue) -> Option<UiTextAlign> {
    match as_str(value)?.to_ascii_lowercase().as_str() {
        "start" | "left" | "top" => Some(UiTextAlign::Start),
        "center" | "middle" => Some(UiTextAlign::Center),
        "end" | "right" | "bottom" => Some(UiTextAlign::End),
        _ => None,
    }
}

fn as_ui_rect(value: &SceneValue) -> Option<perro_ui::UiRect> {
    match value {
        SceneValue::F32(v) => Some(perro_ui::UiRect::all(*v)),
        SceneValue::I32(v) => Some(perro_ui::UiRect::all(*v as f32)),
        SceneValue::Vec4 { x, y, z, w } => Some(perro_ui::UiRect::new(*x, *y, *z, *w)),
        SceneValue::Object(fields) => {
            let mut left = None;
            let mut top = None;
            let mut right = None;
            let mut bottom = None;
            for (key, value) in fields.iter() {
                match key.as_ref() {
                    "left" => left = as_f32(value),
                    "top" => top = as_f32(value),
                    "right" => right = as_f32(value),
                    "bottom" => bottom = as_f32(value),
                    _ => {}
                }
            }
            Some(perro_ui::UiRect::new(
                left.unwrap_or(0.0),
                top.unwrap_or(0.0),
                right.unwrap_or(0.0),
                bottom.unwrap_or(0.0),
            ))
        }
        _ => None,
    }
}

fn as_scene_color(value: &SceneValue) -> Option<Color> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some(Color::new(*x, *y, *z, *w)),
        SceneValue::Vec3 { x, y, z } => Some(Color::rgb(*x, *y, *z)),
        SceneValue::Str(v) => Color::from_hex(v.as_ref()),
        SceneValue::Key(v) => Color::from_hex(v.as_ref()),
        _ => None,
    }
}

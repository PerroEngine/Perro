use super::*;

pub(super) fn apply_ui_text_edit_fields(
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
        "input_type" | "text_input_type" => {
            if let Some(v) = as_str(value).and_then(as_text_input_type) {
                node.input_type = v;
            }
        }
        name if scene_key_in(name, HOVER_ENTER_SIGNAL_KEYS) => {
            node.hover_signals = as_signal_ids(value);
        }
        name if scene_key_in(name, HOVER_EXIT_SIGNAL_KEYS) => {
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

pub(super) fn as_text_input_type(value: &str) -> Option<perro_ui::UiTextInputType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "any" | "text" => Some(perro_ui::UiTextInputType::Any),
        "letters" | "alpha" => Some(perro_ui::UiTextInputType::Letters),
        "i32" | "int" | "integer" | "signed_integer" => {
            Some(perro_ui::UiTextInputType::SignedInteger)
        }
        "u32" | "uint" | "unsigned_integer" => Some(perro_ui::UiTextInputType::UnsignedInteger),
        "f32" | "float" | "number" | "signed_float" => Some(perro_ui::UiTextInputType::SignedFloat),
        "uf32" | "unsigned_float" | "positive_float" => {
            Some(perro_ui::UiTextInputType::UnsignedFloat)
        }
        _ => None,
    }
}

pub(super) fn decode_scene_text_literal(text: &str) -> String {
    if let Some(stripped) = text.strip_prefix("%%loc:") {
        return decode_text_escapes(&format!("%loc:{stripped}"));
    }
    if let Some(key) = parse_locale_text_key(text) {
        return key.to_string();
    }
    decode_text_escapes(text)
}

pub(super) fn decode_scene_text_edit_literal(text: &str, multiline: bool) -> String {
    let text = decode_scene_text_literal(text);
    if multiline {
        text
    } else {
        text.replace(['\r', '\n', '\t'], " ")
    }
}

pub(super) fn parse_locale_text_key(text: &str) -> Option<&str> {
    let raw = text.strip_prefix("%loc:")?.trim();
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        let key = raw[1..raw.len() - 1].trim();
        return (!key.is_empty()).then_some(key);
    }
    (!raw.is_empty()).then_some(raw)
}

pub(super) fn decode_text_escapes(text: &str) -> String {
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

pub(super) fn apply_ui_scroll_container_fields(node: &mut UiScrollContainer, fields: &[SceneObjectField]) {
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
        "scroll_dir" | "scroll_direction" | "direction" => {
            if let Some(v) = as_ui_scroll_direction(value) {
                node.scroll_dir = v;
            }
        }
        "scroll_bar_side" | "scrollbar_side" | "bar_side" | "side" => {
            if let Some(v) = as_ui_scroll_bar_side(value) {
                node.scroll_bar_side = v;
            }
        }
        "scroll_bar_padding" | "scrollbar_padding" | "bar_padding" => {
            if let Some(v) = as_f32(value) {
                node.scroll_bar_padding = v;
            }
        }
        _ => {}
    });
}

pub(super) fn as_ui_scroll_direction(value: &SceneValue) -> Option<perro_ui::UiScrollDirection> {
    match as_str(value)? {
        "h" | "horizontal" | "x" => Some(perro_ui::UiScrollDirection::Horizontal),
        "v" | "vertical" | "y" => Some(perro_ui::UiScrollDirection::Vertical),
        _ => None,
    }
}

pub(super) fn as_ui_scroll_bar_side(value: &SceneValue) -> Option<perro_ui::UiScrollBarSide> {
    match as_str(value)? {
        "left" => Some(perro_ui::UiScrollBarSide::Left),
        "right" => Some(perro_ui::UiScrollBarSide::Right),
        "top" => Some(perro_ui::UiScrollBarSide::Top),
        "bottom" => Some(perro_ui::UiScrollBarSide::Bottom),
        _ => None,
    }
}

pub(super) fn apply_ui_container_fields(
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
            if let Some(mode) = as_ui_layout_spacing_mode(value) {
                node.spacing_mode = mode;
            } else if let Some(v) = as_f32(value) {
                node.spacing = v;
                node.spacing_mode = perro_ui::UiLayoutSpacingMode::Fixed;
            }
        }
        "h_spacing" | "horizontal_spacing" => {
            if let Some(mode) = as_ui_layout_spacing_mode(value) {
                node.h_spacing_mode = mode;
            } else if let Some(v) = as_f32(value) {
                node.h_spacing = v;
                node.h_spacing_mode = perro_ui::UiLayoutSpacingMode::Fixed;
            }
        }
        "v_spacing" | "vertical_spacing" => {
            if let Some(mode) = as_ui_layout_spacing_mode(value) {
                node.v_spacing_mode = mode;
            } else if let Some(v) = as_f32(value) {
                node.v_spacing = v;
                node.v_spacing_mode = perro_ui::UiLayoutSpacingMode::Fixed;
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

pub(super) fn apply_ui_fixed_container_fields(
    node: &mut perro_ui::UiFixedLayoutContainer,
    fields: &[SceneObjectField],
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "spacing" => {
            if let Some(mode) = as_ui_layout_spacing_mode(value) {
                node.spacing_mode = mode;
            } else if let Some(v) = as_f32(value) {
                node.spacing = v;
                node.spacing_mode = perro_ui::UiLayoutSpacingMode::Fixed;
            }
        }
        "h_spacing" | "horizontal_spacing" => {
            if let Some(mode) = as_ui_layout_spacing_mode(value) {
                node.h_spacing_mode = mode;
            } else if let Some(v) = as_f32(value) {
                node.h_spacing = v;
                node.h_spacing_mode = perro_ui::UiLayoutSpacingMode::Fixed;
            }
        }
        "v_spacing" | "vertical_spacing" => {
            if let Some(mode) = as_ui_layout_spacing_mode(value) {
                node.v_spacing_mode = mode;
            } else if let Some(v) = as_f32(value) {
                node.v_spacing = v;
                node.v_spacing_mode = perro_ui::UiLayoutSpacingMode::Fixed;
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

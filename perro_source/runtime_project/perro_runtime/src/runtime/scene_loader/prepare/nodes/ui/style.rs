use super::*;

pub(super) fn parse_ui_style_source(source: &str) -> Option<perro_ui::UiStyle> {
    let text = source.trim();
    let wrapped;
    let parse_text = if ui_style_source_looks_like_object(text) {
        text
    } else {
        wrapped = format!("{{\n{text}\n}}");
        wrapped.as_str()
    };
    let parsed = std::panic::catch_unwind(|| Parser::new(parse_text).parse_value_literal()).ok()?;
    let SceneValue::Object(entries) = parsed else {
        return None;
    };
    let mut style = perro_ui::UiStyle::panel();
    apply_ui_style_fields(&mut style, entries.as_ref(), "");
    Some(style)
}

pub(super) fn ui_style_source_looks_like_object(text: &str) -> bool {
    text.lines()
        .map(strip_ui_style_line_comment)
        .map(str::trim)
        .find(|line| !line.is_empty())
        .is_some_and(|line| line.starts_with('{'))
}

pub(super) fn strip_ui_style_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut escape = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b == b'#' || (b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/') {
            return &line[..i];
        }
        i += 1;
    }
    line
}

pub(super) fn apply_ui_style_fields(
    style: &mut perro_ui::UiStyle,
    fields: &[SceneObjectField],
    prefix: &str,
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        let Some(field) = name.strip_prefix(prefix) else {
            return;
        };
        match field {
            "fill" | "color" => {
                if let Some(v) = as_scene_color(value) {
                    style.fill = v;
                    style.fill_kind = perro_ui::UiFillKind::Solid;
                }
            }
            "fill_kind" => {
                if let Some(v) = as_ui_fill_kind(value) {
                    style.fill_kind = v;
                }
            }
            "gradient" => {
                if let Some(v) = as_ui_linear_gradient(value) {
                    style.gradient = v;
                    style.fill_kind = perro_ui::UiFillKind::Linear;
                }
            }
            "gradient_start" | "gradient_start_color" => {
                if let Some(v) = as_scene_color(value) {
                    style.gradient.start_color = v;
                    style.fill_kind = perro_ui::UiFillKind::Linear;
                }
            }
            "gradient_end" | "gradient_end_color" => {
                if let Some(v) = as_scene_color(value) {
                    style.gradient.end_color = v;
                    style.fill_kind = perro_ui::UiFillKind::Linear;
                }
            }
            "gradient_vector" | "gradient_direction" => {
                if let Some(v) = as_vec2(value) {
                    style.gradient.vector = v;
                    style.fill_kind = perro_ui::UiFillKind::Linear;
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
                    style.set_corner_radius(v);
                }
            }
            "corner_radii" => {
                if let Some(v) = as_ui_corner_radii(value) {
                    style.corner_radii = v;
                }
            }
            "radius_tl" | "corner_radius_tl" => {
                if let Some(v) = as_ui_corner_radius(value) {
                    style.corner_radii.tl = v;
                }
            }
            "radius_tr" | "corner_radius_tr" => {
                if let Some(v) = as_ui_corner_radius(value) {
                    style.corner_radii.tr = v;
                }
            }
            "radius_br" | "corner_radius_br" => {
                if let Some(v) = as_ui_corner_radius(value) {
                    style.corner_radii.br = v;
                }
            }
            "radius_bl" | "corner_radius_bl" => {
                if let Some(v) = as_ui_corner_radius(value) {
                    style.corner_radii.bl = v;
                }
            }
            "shadow" => {
                if let Some(v) = as_ui_depth_effect(value) {
                    style.outer_shadow = v;
                }
            }
            "outer_shadow" => {
                if let Some(v) = as_ui_depth_effect(value) {
                    style.outer_shadow = v;
                }
            }
            "inner_shadow" => {
                if let Some(v) = as_ui_depth_effect(value) {
                    style.inner_shadow = v;
                }
            }
            "highlight" | "inner_highlight" => {
                if let Some(v) = as_ui_depth_effect(value) {
                    style.inner_highlight = v;
                }
            }
            "outer_highlight" => {
                if let Some(v) = as_ui_depth_effect(value) {
                    style.outer_highlight = v;
                }
            }
            "shadow_color" => {
                if let Some(v) = as_scene_color(value) {
                    style.outer_shadow.color = v;
                }
            }
            "shadow_distance" => {
                if let Some(v) = as_f32(value) {
                    style.outer_shadow.distance = v.max(0.0);
                }
            }
            "shadow_falloff" => {
                if let Some(v) = as_f32(value) {
                    style.outer_shadow.falloff = v.max(0.0);
                }
            }
            "shadow_vector" => {
                if let Some(v) = as_vec2(value) {
                    style.outer_shadow.vector = v;
                }
            }
            "shadow_size" => {
                if let Some(v) = as_f32(value) {
                    style.outer_shadow.size = v.max(0.0);
                }
            }
            "outer_shadow_color" => {
                if let Some(v) = as_scene_color(value) {
                    style.outer_shadow.color = v;
                }
            }
            "outer_shadow_distance" => {
                if let Some(v) = as_f32(value) {
                    style.outer_shadow.distance = v.max(0.0);
                }
            }
            "outer_shadow_falloff" => {
                if let Some(v) = as_f32(value) {
                    style.outer_shadow.falloff = v.max(0.0);
                }
            }
            "outer_shadow_vector" => {
                if let Some(v) = as_vec2(value) {
                    style.outer_shadow.vector = v;
                }
            }
            "outer_shadow_size" => {
                if let Some(v) = as_f32(value) {
                    style.outer_shadow.size = v.max(0.0);
                }
            }
            "inner_shadow_color" => {
                if let Some(v) = as_scene_color(value) {
                    style.inner_shadow.color = v;
                }
            }
            "inner_shadow_distance" => {
                if let Some(v) = as_f32(value) {
                    style.inner_shadow.distance = v.max(0.0);
                }
            }
            "inner_shadow_falloff" => {
                if let Some(v) = as_f32(value) {
                    style.inner_shadow.falloff = v.max(0.0);
                }
            }
            "inner_shadow_vector" => {
                if let Some(v) = as_vec2(value) {
                    style.inner_shadow.vector = v;
                }
            }
            "inner_shadow_size" => {
                if let Some(v) = as_f32(value) {
                    style.inner_shadow.size = v.max(0.0);
                }
            }
            "highlight_color" | "inner_highlight_color" => {
                if let Some(v) = as_scene_color(value) {
                    style.inner_highlight.color = v;
                }
            }
            "highlight_distance" | "inner_highlight_distance" => {
                if let Some(v) = as_f32(value) {
                    style.inner_highlight.distance = v.max(0.0);
                }
            }
            "highlight_falloff" | "inner_highlight_falloff" => {
                if let Some(v) = as_f32(value) {
                    style.inner_highlight.falloff = v.max(0.0);
                }
            }
            "highlight_vector" | "inner_highlight_vector" => {
                if let Some(v) = as_vec2(value) {
                    style.inner_highlight.vector = v;
                }
            }
            "highlight_size" | "inner_highlight_size" => {
                if let Some(v) = as_f32(value) {
                    style.inner_highlight.size = v.max(0.0);
                }
            }
            "outer_highlight_color" => {
                if let Some(v) = as_scene_color(value) {
                    style.outer_highlight.color = v;
                }
            }
            "outer_highlight_distance" => {
                if let Some(v) = as_f32(value) {
                    style.outer_highlight.distance = v.max(0.0);
                }
            }
            "outer_highlight_falloff" => {
                if let Some(v) = as_f32(value) {
                    style.outer_highlight.falloff = v.max(0.0);
                }
            }
            "outer_highlight_vector" => {
                if let Some(v) = as_vec2(value) {
                    style.outer_highlight.vector = v;
                }
            }
            "outer_highlight_size" => {
                if let Some(v) = as_f32(value) {
                    style.outer_highlight.size = v.max(0.0);
                }
            }
            _ => {}
        }
    });
}

pub(super) fn apply_ui_style_object_fields(
    style: &mut perro_ui::UiStyle,
    fields: &[SceneObjectField],
    object_name: &str,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if name != object_name {
            return;
        }
        match value {
            SceneValue::Object(entries) => apply_ui_style_fields(style, entries.as_ref(), ""),
            _ => {
                if let Some(source) = as_asset_source(value)
                    && source.ends_with(".uistyle")
                    && let Some(loaded) = load_ui_style_source(&source, static_ui_style_lookup)
                {
                    *style = loaded;
                }
            }
        }
    });
}

pub(super) fn apply_ui_button_state_fields(
    node: &mut UiButton,
    fields: &[SceneObjectField],
    state_name: &str,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) {
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
        apply_ui_style_object_fields(
            &mut style,
            entries.as_ref(),
            "style",
            static_ui_style_lookup,
        );

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

pub(super) fn ui_state_has_explicit_size_override(fields: &[SceneObjectField]) -> bool {
    let mut found = false;
    SceneFieldIterRef::new(fields).for_each(|name, _| {
        if matches!(
            name,
            "size_percent"
                | "size_ratio"
                | "h_size"
                | "horizontal_size"
                | "v_size"
                | "vertical_size"
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

pub(super) fn as_ui_corner_radius(value: &SceneValue) -> Option<f32> {
    if let Some(v) = as_f32(value) {
        return Some(v.clamp(0.0, 1.0));
    }
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "full" | "pill" | "round" | "rounded" => Some(1.0),
        _ => None,
    }
}

pub(super) fn as_ui_depth_effect(value: &SceneValue) -> Option<perro_ui::UiDepthEffect> {
    let SceneValue::Object(fields) = value else {
        return None;
    };
    let mut effect = perro_ui::UiDepthEffect::none();
    SceneFieldIterRef::new(fields.as_ref()).for_each(|name, value| match name {
        "color" => {
            if let Some(v) = as_scene_color(value) {
                effect.color = v;
            }
        }
        "distance" | "dist" => {
            if let Some(v) = as_f32(value) {
                effect.distance = v.max(0.0);
            }
        }
        "falloff" | "blur" => {
            if let Some(v) = as_f32(value) {
                effect.falloff = v.max(0.0);
            }
        }
        "vector" | "direction" | "dir" => {
            if let Some(v) = as_vec2(value) {
                effect.vector = v;
            }
        }
        "size" => {
            if let Some(v) = as_f32(value) {
                effect.size = v.max(0.0);
            }
        }
        _ => {}
    });
    Some(effect)
}

pub(super) fn as_ui_fill_kind(value: &SceneValue) -> Option<perro_ui::UiFillKind> {
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "solid" => Some(perro_ui::UiFillKind::Solid),
        "linear" | "gradient" | "lineargradient" => Some(perro_ui::UiFillKind::Linear),
        _ => None,
    }
}

pub(super) fn as_ui_linear_gradient(value: &SceneValue) -> Option<perro_ui::UiLinearGradient> {
    let SceneValue::Object(fields) = value else {
        return None;
    };
    let mut gradient = perro_ui::UiLinearGradient::none();
    SceneFieldIterRef::new(fields.as_ref()).for_each(|name, value| match name {
        "start" | "start_color" | "from" => {
            if let Some(v) = as_scene_color(value) {
                gradient.start_color = v;
            }
        }
        "end" | "end_color" | "to" => {
            if let Some(v) = as_scene_color(value) {
                gradient.end_color = v;
            }
        }
        "vector" | "direction" | "dir" => {
            if let Some(v) = as_vec2(value) {
                gradient.vector = v;
            }
        }
        _ => {}
    });
    Some(gradient)
}

pub(super) fn as_ui_corner_radii(value: &SceneValue) -> Option<perro_ui::UiCornerRadii> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some(perro_ui::UiCornerRadii::new(
            x.clamp(0.0, 1.0),
            y.clamp(0.0, 1.0),
            z.clamp(0.0, 1.0),
            w.clamp(0.0, 1.0),
        )),
        _ => as_ui_corner_radius(value).map(perro_ui::UiCornerRadii::all),
    }
}

pub(super) fn as_signal_id(value: &SceneValue) -> Option<perro_ids::SignalID> {
    if let Some(v) = as_str(value) {
        return Some(perro_ids::SignalID::from_string(v));
    }
    if let Some(v) = value.as_key() {
        return Some(perro_ids::SignalID::from_string(v));
    }
    value.as_hashed().map(perro_ids::SignalID::from_u64)
}

pub(super) fn as_signal_ids(value: &SceneValue) -> Vec<perro_ids::SignalID> {
    match value {
        SceneValue::Array(values) => values.iter().filter_map(as_signal_id).collect(),
        _ => as_signal_id(value).into_iter().collect(),
    }
}

pub(super) fn as_ui_mouse_filter(value: &SceneValue) -> Option<UiMouseFilter> {
    match as_str(value)?.to_ascii_lowercase().as_str() {
        "stop" => Some(UiMouseFilter::Stop),
        "pass" => Some(UiMouseFilter::Pass),
        "ignore" => Some(UiMouseFilter::Ignore),
        _ => None,
    }
}

pub(super) fn as_cursor_icon(value: &SceneValue) -> Option<perro_ui::CursorIcon> {
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

pub(super) fn as_ui_anchor(value: &SceneValue) -> Option<perro_ui::UiAnchor> {
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

pub(super) fn as_ui_layout_mode(value: &SceneValue) -> Option<perro_ui::UiLayoutMode> {
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "h" | "horizontal" | "hlayout" | "hbox" | "row" => Some(perro_ui::UiLayoutMode::H),
        "v" | "vertical" | "vlayout" | "vbox" | "column" | "col" => Some(perro_ui::UiLayoutMode::V),
        "g" | "grid" => Some(perro_ui::UiLayoutMode::Grid),
        _ => None,
    }
}

pub(super) fn as_ui_layout_spacing_mode(
    value: &SceneValue,
) -> Option<perro_ui::UiLayoutSpacingMode> {
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "fixed" | "f" => Some(perro_ui::UiLayoutSpacingMode::Fixed),
        "fill" | "spacebetween" | "spread" | "justify" => {
            Some(perro_ui::UiLayoutSpacingMode::Fill)
        }
        _ => None,
    }
}

pub(super) fn as_ui_size_mode(value: &SceneValue) -> Option<perro_ui::UiSizeMode> {
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

pub(super) fn as_ui_horizontal_align(value: &SceneValue) -> Option<perro_ui::UiHorizontalAlign> {
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

pub(super) fn as_ui_vertical_align(value: &SceneValue) -> Option<perro_ui::UiVerticalAlign> {
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

pub(super) fn as_ui_text_align(value: &SceneValue) -> Option<UiTextAlign> {
    match as_str(value)?.to_ascii_lowercase().as_str() {
        "start" | "left" | "top" => Some(UiTextAlign::Start),
        "center" | "middle" => Some(UiTextAlign::Center),
        "end" | "right" | "bottom" => Some(UiTextAlign::End),
        _ => None,
    }
}

pub(super) fn as_ui_image_scale_mode(value: &SceneValue) -> Option<UiImageScaleMode> {
    match as_str(value)?
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
        .as_str()
    {
        "stretch" | "fill" => Some(UiImageScaleMode::Stretch),
        "fit" | "contain" | "keepaspect" => Some(UiImageScaleMode::Fit),
        "cover" | "crop" => Some(UiImageScaleMode::Cover),
        _ => None,
    }
}

pub(super) fn as_vec4_array(value: &SceneValue) -> Option<[f32; 4]> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some([*x, *y, *z, *w]),
        SceneValue::Array(values) if values.len() == 4 => Some([
            as_f32(&values[0])?,
            as_f32(&values[1])?,
            as_f32(&values[2])?,
            as_f32(&values[3])?,
        ]),
        _ => None,
    }
}

pub(super) fn as_ui_rect(value: &SceneValue) -> Option<perro_ui::UiRect> {
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

pub(super) fn as_scene_color(value: &SceneValue) -> Option<Color> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some(Color::new(*x, *y, *z, *w)),
        SceneValue::Vec3 { x, y, z } => Some(Color::rgb(*x, *y, *z)),
        SceneValue::Str(v) => Color::from_hex(v.as_ref()),
        SceneValue::Key(v) => Color::from_hex(v.as_ref()),
        _ => None,
    }
}

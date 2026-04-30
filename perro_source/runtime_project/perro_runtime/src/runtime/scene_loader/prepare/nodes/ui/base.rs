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
        "anchor" => {
            if let Some(v) = as_ui_anchor(value) {
                node.layout.anchor = v;
            }
        }
        "position" => {
            if let Some(v) = as_vec2(value) {
                node.layout.position = v.into();
            }
        }
        "position_percent" | "position_pct" => {
            if let Some(v) = as_vec2(value) {
                node.layout.position = perro_ui::UiVector2::percent(v.x, v.y);
            }
        }
        "position_ratio" => {
            if let Some(v) = as_vec2(value) {
                node.layout.position = perro_ui::UiVector2::ratio(v.x, v.y);
            }
        }
        "size" => {
            if let Some(v) = as_vec2(value) {
                node.layout.size = v.into();
            }
        }
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
        "pivot" => {
            if let Some(v) = as_vec2(value) {
                node.layout.pivot = v.into();
            }
        }
        "pivot_percent" | "pivot_pct" => {
            if let Some(v) = as_vec2(value) {
                node.layout.pivot = perro_ui::UiVector2::percent(v.x, v.y);
            }
        }
        "pivot_ratio" => {
            if let Some(v) = as_vec2(value) {
                node.layout.pivot = perro_ui::UiVector2::ratio(v.x, v.y);
            }
        }
        "translation" => {
            if let Some(v) = as_vec2(value) {
                node.layout.translation = v;
            }
        }
        "scale" => {
            if let Some(v) = as_vec2(value) {
                node.layout.scale = v;
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
        "min_size" => {
            if let Some(v) = as_vec2(value) {
                node.layout.min_size = v;
            }
        }
        "max_size" => {
            if let Some(v) = as_vec2(value) {
                node.layout.max_size = v;
            }
        }
        "min_w" | "min_width" => {
            if let Some(v) = as_f32(value) {
                node.layout.min_size.x = v;
            }
        }
        "min_h" | "min_height" => {
            if let Some(v) = as_f32(value) {
                node.layout.min_size.y = v;
            }
        }
        "max_w" | "max_width" => {
            if let Some(v) = as_f32(value) {
                node.layout.max_size.x = v;
            }
        }
        "max_h" | "max_height" => {
            if let Some(v) = as_f32(value) {
                node.layout.max_size.y = v;
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
        "text" => {
            if let Some(v) = as_str(value) {
                node.text = Cow::Owned(v.to_string());
            }
        }
        "text_color" => {
            if let Some(v) = as_scene_color(value) {
                node.text_color = v;
            }
        }
        "disabled" => {
            if let Some(v) = as_bool(value) {
                node.disabled = v;
            }
        }
        _ => {}
    });
    apply_ui_style_fields(&mut node.style, fields, "");
    apply_ui_style_fields(&mut node.hover_style, fields, "hover_");
    apply_ui_style_fields(&mut node.pressed_style, fields, "pressed_");
}

fn apply_ui_label_fields(node: &mut UiLabel, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "text" => {
            if let Some(v) = as_str(value) {
                node.text = Cow::Owned(v.to_string());
            }
        }
        "color" | "text_color" => {
            if let Some(v) = as_scene_color(value) {
                node.color = v;
            }
        }
        "font_size" => {
            if let Some(v) = as_f32(value) {
                node.font_size = v;
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

fn as_ui_mouse_filter(value: &SceneValue) -> Option<UiMouseFilter> {
    match as_str(value)?.to_ascii_lowercase().as_str() {
        "stop" => Some(UiMouseFilter::Stop),
        "pass" => Some(UiMouseFilter::Pass),
        "ignore" => Some(UiMouseFilter::Ignore),
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

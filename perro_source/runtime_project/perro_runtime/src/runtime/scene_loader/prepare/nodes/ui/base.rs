fn build_ui_root(data: &SceneDefNodeData) -> UiRoot {
    let mut node = UiRoot::new();
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

fn build_ui_hbox(data: &SceneDefNodeData) -> UiHBox {
    let mut node = UiHBox::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_box_fields(&mut node.inner.spacing, &data.fields);
    node
}

fn build_ui_vbox(data: &SceneDefNodeData) -> UiVBox {
    let mut node = UiVBox::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_box_fields(&mut node.inner.spacing, &data.fields);
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

fn apply_ui_root_data(target: &mut UiRoot, data: &SceneDefNodeData) {
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(target, base);
    }
    apply_ui_root_fields(target, &data.fields);
}

fn apply_ui_root_fields(node: &mut UiRoot, fields: &[SceneObjectField]) {
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
        "translation" => {
            if let Some(v) = as_vec2(value) {
                node.layout.translation = v;
            }
        }
        "min_size" => {
            if let Some(v) = as_vec2(value) {
                node.layout.min_size = v;
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

fn apply_ui_box_fields(spacing: &mut f32, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if name == "spacing"
            && let Some(v) = as_f32(value)
        {
            *spacing = v;
        }
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
                if let Some(v) = as_f32(value) {
                    style.corner_radius = v;
                }
            }
            _ => {}
        }
    });
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

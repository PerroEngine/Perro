fn build_sprite_2d(data: &SceneDefNodeData) -> Sprite2D {
    let mut node = Sprite2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_sprite_2d_fields(&mut node, &data.fields);
    node
}

fn build_video_player_2d(data: &SceneDefNodeData) -> VideoPlayer2D {
    let mut node = VideoPlayer2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_video_player_fields(&mut node.video, &data.fields);
    apply_video_player_2d_fields(&mut node, &data.fields);
    node
}

fn build_label_2d(data: &SceneDefNodeData) -> Label2D {
    let mut node = Label2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_label_2d_fields(&mut node, &data.fields);
    node
}

fn build_button_2d(data: &SceneDefNodeData) -> Button2D {
    let mut node = Button2D::default();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_button_2d_fields(&mut node, &data.fields);
    node
}

fn build_image_button_2d(data: &SceneDefNodeData) -> ImageButton2D {
    let mut node = ImageButton2D::default();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_image_button_2d_fields(&mut node, &data.fields);
    node
}

fn build_nine_slice_button_2d(data: &SceneDefNodeData) -> NineSliceButton2D {
    let mut node = NineSliceButton2D::default();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_nine_slice_button_2d_fields(&mut node, &data.fields);
    node
}

fn build_nine_slice_2d(data: &SceneDefNodeData) -> NineSlice2D {
    let mut node = NineSlice2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_nine_slice_2d_fields(&mut node, &data.fields);
    node
}

fn build_animated_sprite_2d(data: &SceneDefNodeData) -> AnimatedSprite2D {
    let mut node = AnimatedSprite2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_animated_sprite_2d_fields(&mut node, &data.fields);
    node
}

fn build_water_body_2d(data: &SceneDefNodeData) -> WaterBody2D {
    let mut node = WaterBody2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_water_body_fields(&mut node.water, "WaterBody2D", &data.fields);
    node
}

fn build_tilemap_2d(data: &SceneDefNodeData) -> TileMap2D {
    let mut node = TileMap2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_tilemap_2d_fields(&mut node, &data.fields);
    node
}

fn apply_tilemap_2d_fields(node: &mut TileMap2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("TileMap2D", name) {
            Some(NodeField::TileMap2D(TileMap2DField::Tileset)) => {
                if let Some(v) = as_str(value) {
                    node.tileset = v.into();
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::Width)) => {
                if let Some(v) = as_u32(value) {
                    node.width = v;
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::Height)) => {
                if let Some(v) = as_u32(value) {
                    node.height = v;
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::EmptyTile)) => {
                if let Some(v) = as_i32(value) {
                    node.empty_tile = v;
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::Tiles)) => {
                if let SceneValue::Array(items) = value {
                    node.tiles = items.iter().filter_map(as_i32).collect();
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::CollisionEnabled)) => {
                if let Some(v) = as_bool(value) {
                    node.collision_enabled = v;
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::CollisionLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_layers = v;
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::CollisionMask)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_mask = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_sprite_2d_fields(node: &mut Sprite2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each_field(|field, value| {
        match field {
            SceneFieldName::TextureRegion => {
                if let Some((x, y, w, h)) = value.as_vec4()
                    && w > 0.0
                    && h > 0.0
                {
                    node.texture_region = Some([x, y, w, h]);
                }
            }
            SceneFieldName::FlipX => {
                if let Some(v) = value.as_bool() {
                    node.flip_x = v;
                }
            }
            SceneFieldName::FlipY => {
                if let Some(v) = value.as_bool() {
                    node.flip_y = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_label_2d_fields(node: &mut Label2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "text" => {
            if let Some(v) = as_str(value) {
                node.text = Cow::Owned(decode_scene_text_literal(v));
            }
        }
        "size" => {
            if let Some(v) = as_vec2(value) {
                node.size = Vector2::new(v.x.max(0.001), v.y.max(0.001));
            }
        }
        name if scene_key_in(name, TEXT_COLOR_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.color = v;
            }
        }
        "font_size" | "text_size" => {
            if let Some(v) = as_f32(value) {
                node.font_size = v.max(0.001);
            }
        }
        "font" => {
            if let Some(v) = as_str(value).and_then(perro_ui::UiFont::parse) {
                node.font = v;
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

fn apply_button_2d_fields(node: &mut Button2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if matches!(name, "size")
            && let Some((x, y)) = value.as_vec2()
        {
            node.size = Vector2::new(x.max(0.0), y.max(0.0));
        }
    });
    apply_button_2d_common(
        Button2DCommonFields {
            input_mask: &mut node.input_mask,
            mouse_filter: &mut node.mouse_filter,
            cursor_icon: &mut node.cursor_icon,
            input_enabled: &mut node.input_enabled,
            clicked_signals: &mut node.clicked_signals,
            hover_signals: &mut node.hover_signals,
            hover_exit_signals: &mut node.hover_exit_signals,
            pressed_signals: &mut node.pressed_signals,
            released_signals: &mut node.released_signals,
            web: &mut node.web,
        },
        fields,
    );
    apply_ui_style_fields(&mut node.style, fields, "");
    node.hover_style = node.style.clone();
    node.pressed_style = node.style.clone();
    apply_ui_style_fields(&mut node.hover_style, fields, "hover_");
    apply_ui_style_fields(&mut node.pressed_style, fields, "pressed_");
}

fn apply_image_button_2d_fields(node: &mut ImageButton2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "size" => {
            if let Some((x, y)) = value.as_vec2() {
                node.size = Vector2::new(x.max(0.0), y.max(0.0));
            }
        }
        name if scene_key_in(name, TEXTURE_REGION_KEYS) => {
            if let Some((x, y, w, h)) = value.as_vec4()
                && w > 0.0
                && h > 0.0
            {
                node.texture_region = Some([x, y, w, h]);
            }
        }
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        _ => {}
    });
    apply_button_2d_common(
        Button2DCommonFields {
            input_mask: &mut node.input_mask,
            mouse_filter: &mut node.mouse_filter,
            cursor_icon: &mut node.cursor_icon,
            input_enabled: &mut node.input_enabled,
            clicked_signals: &mut node.clicked_signals,
            hover_signals: &mut node.hover_signals,
            hover_exit_signals: &mut node.hover_exit_signals,
            pressed_signals: &mut node.pressed_signals,
            released_signals: &mut node.released_signals,
            web: &mut node.web,
        },
        fields,
    );
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
}

fn apply_nine_slice_2d_fields(node: &mut NineSlice2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "size" => {
            if let Some((x, y)) = value.as_vec2() {
                node.size = Vector2::new(x.max(0.0), y.max(0.0));
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
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        _ => {}
    });
}

fn apply_animated_sprite_2d_fields(node: &mut AnimatedSprite2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("AnimatedSprite2D", name) {
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Animations)) => {
                if let Some(animations) = parse_animated_sprite_list(value) {
                    node.animations = animations;
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FlipX)) => {
                if let Some(v) = value.as_bool() {
                    node.flip_x = v;
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FlipY)) => {
                if let Some(v) = value.as_bool() {
                    node.flip_y = v;
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::CurrentAnimation)) => {
                if let Some(v) = as_str(value) {
                    node.current_animation = std::borrow::Cow::Owned(v.to_string());
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::CurrentFrame)) => {
                if let Some(v) = as_i32(value) {
                    node.current_frame = u32::try_from(v.max(0)).unwrap_or(0);
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FpsScale)) => {
                if let Some(v) = value.as_f32() {
                    node.fps_scale = v.max(0.0);
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Playing)) => {
                if let Some(v) = value.as_bool() {
                    node.playing = v;
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Looping)) => {
                if let Some(v) = value.as_bool() {
                    node.looping = v;
                }
            }
            _ => {}
        }
    });
    if node.current_animation_data().is_none() {
        node.animations.push(AnimatedSprite::default());
    }
    let max_frame = node
        .current_animation_data()
        .map(|animation| animation.frame_count.max(1).saturating_sub(1))
        .unwrap_or(0);
    node.current_frame = node.current_frame.min(max_frame);
}

struct Button2DCommonFields<'a> {
    input_mask: &'a mut perro_ui::UiInputMask,
    mouse_filter: &'a mut UiMouseFilter,
    cursor_icon: &'a mut perro_ui::CursorIcon,
    input_enabled: &'a mut bool,
    clicked_signals: &'a mut Vec<perro_ids::SignalID>,
    hover_signals: &'a mut Vec<perro_ids::SignalID>,
    hover_exit_signals: &'a mut Vec<perro_ids::SignalID>,
    pressed_signals: &'a mut Vec<perro_ids::SignalID>,
    released_signals: &'a mut Vec<perro_ids::SignalID>,
    web: &'a mut Option<perro_ui::UiButtonWebAction>,
}

fn apply_button_2d_common(target: Button2DCommonFields<'_>, fields: &[SceneObjectField]) {
    apply_ui_input_mask_fields(target.input_mask, fields);
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "input_enabled" => {
            if let Some(v) = as_bool(value) {
                *target.input_enabled = v;
            }
        }
        "disabled" => {
            if let Some(v) = as_bool(value) {
                *target.input_enabled = !v;
            }
        }
        "mouse_filter" => {
            if let Some(v) = as_ui_mouse_filter(value) {
                *target.mouse_filter = v;
            }
        }
        "cursor_icon" | "hover_cursor_icon" => {
            if let Some(v) = as_cursor_icon(value) {
                *target.cursor_icon = v;
            }
        }
        name if scene_key_in(name, HOVER_ENTER_SIGNAL_KEYS) => {
            *target.hover_signals = as_signal_ids(value);
        }
        name if scene_key_in(name, HOVER_EXIT_SIGNAL_KEYS) => {
            *target.hover_exit_signals = as_signal_ids(value);
        }
        "pressed_signals" | "press_signals" => {
            *target.pressed_signals = as_signal_ids(value);
        }
        "released_signals" | "release_signals" => {
            *target.released_signals = as_signal_ids(value);
        }
        "clicked_signals" | "click_signals" => {
            *target.clicked_signals = as_signal_ids(value);
        }
        "web" => {
            *target.web = parse_ui_button_web_action(value);
        }
        _ => {}
    });
}

fn as_margins_4(value: &SceneValue) -> Option<[f32; 4]> {
    if let Some((x, y, z, w)) = value.as_vec4() {
        return Some([x.max(0.0), y.max(0.0), z.max(0.0), w.max(0.0)]);
    }
    if let Some((x, y)) = value.as_vec2() {
        return Some([x.max(0.0), y.max(0.0), x.max(0.0), y.max(0.0)]);
    }
    value.as_f32().map(|v| [v.max(0.0); 4])
}

fn parse_animated_sprite_list(value: &SceneValue) -> Option<Vec<AnimatedSprite>> {
    let SceneValue::Array(items) = value else {
        return None;
    };
    let mut out = Vec::new();
    for item in items.iter() {
        if let Some(animation) = parse_animated_sprite(item) {
            out.push(animation);
        }
    }
    (!out.is_empty()).then_some(out)
}

fn parse_animated_sprite(value: &SceneValue) -> Option<AnimatedSprite> {
    let SceneValue::Object(fields) = value else {
        return None;
    };

    let mut animation = AnimatedSprite::default();
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
                    animation.name = std::borrow::Cow::Owned(v.to_string());
                }
            }
            "start" | "offset" | "origin" => {
                if let Some((x, y)) = value.as_vec2() {
                    animation.start = [x, y];
                }
            }
            name if scene_key_in(name, TEXTURE_REGION_KEYS) => {
                if let Some((x, y, _, _)) = value.as_vec4() {
                    animation.start = [x, y];
                }
            }
            "frame_size" | "cell_size" => {
                if let Some((w, h)) = value.as_vec2()
                    && w > 0.0
                    && h > 0.0
                {
                    animation.frame_size = [w, h];
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
                if let Some(v) = value.as_f32() {
                    animation.fps = v.max(0.0);
                }
            }
            _ => {}
        }
    }
    animation.frame_count = animation.frame_count.max(1);
    Some(animation)
}

fn apply_nine_slice_button_2d_fields(node: &mut NineSliceButton2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "size" => {
            if let Some((x, y)) = value.as_vec2() {
                node.size = Vector2::new(x.max(0.0), y.max(0.0));
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
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        _ => {}
    });
    apply_button_2d_common(
        Button2DCommonFields {
            input_mask: &mut node.input_mask,
            mouse_filter: &mut node.mouse_filter,
            cursor_icon: &mut node.cursor_icon,
            input_enabled: &mut node.input_enabled,
            clicked_signals: &mut node.clicked_signals,
            hover_signals: &mut node.hover_signals,
            hover_exit_signals: &mut node.hover_exit_signals,
            pressed_signals: &mut node.pressed_signals,
            released_signals: &mut node.released_signals,
            web: &mut node.web,
        },
        fields,
    );
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
}

fn apply_video_player_2d_fields(node: &mut VideoPlayer2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "size" => {
            if let Some(v) = as_vec2(value) {
                node.size = Vector2::new(v.x.max(0.001), v.y.max(0.001));
            }
        }
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        name if scene_key_in(name, FLIP_X_KEYS) => {
            if let Some(v) = as_bool(value) {
                node.flip_x = v;
            }
        }
        name if scene_key_in(name, FLIP_Y_KEYS) => {
            if let Some(v) = as_bool(value) {
                node.flip_y = v;
            }
        }
        _ => {}
    });
}

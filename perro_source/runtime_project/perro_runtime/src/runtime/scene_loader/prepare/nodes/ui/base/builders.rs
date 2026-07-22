use super::*;

pub(super) fn build_ui_node(data: &SceneDefNodeData) -> UiNode {
    let mut node = UiNode::new();
    apply_ui_root_data(&mut node, data);
    node
}

pub(super) fn load_ui_style_source(
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

pub(super) fn build_ui_panel(
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

pub(super) fn build_ui_progress_bar(
    data: &SceneDefNodeData,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> UiProgressBar {
    let mut node = UiProgressBar::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_style_fields(&mut node.background_style, &data.fields, "background_");
    apply_ui_style_object_fields(&mut node.background_style, &data.fields, "background_style", static_ui_style_lookup);
    apply_ui_style_fields(&mut node.fill_style, &data.fields, "fill_");
    apply_ui_style_object_fields(&mut node.fill_style, &data.fields, "fill_style", static_ui_style_lookup);
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| {
        if matches!(name, "value" | "progress") && let Some(v) = as_f32(value) {
            node.set_value(v);
        }
        if matches!(name, "background" | "background_color") && let Some(v) = as_scene_color(value) {
            node.background_style.fill = v;
        }
        if matches!(name, "fill" | "fill_color" | "color_fill") && let Some(v) = as_scene_color(value) {
            node.fill_style.fill = v;
        }
        if matches!(name, "background_rounding" | "background_radius") && let Some(v) = as_f32(value) {
            node.background_style.set_corner_radius(v);
        }
        if matches!(name, "fill_rounding" | "fill_radius") && let Some(v) = as_f32(value) {
            node.fill_style.set_corner_radius(v);
        }
    });
    node
}

pub(super) fn build_ui_button(
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

pub(super) fn build_ui_dropdown(
    data: &SceneDefNodeData,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> UiDropdown {
    let mut node = UiDropdown::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.button.base, base);
    }
    apply_ui_root_fields(&mut node.button.base, &data.fields);
    apply_ui_dropdown_fields(&mut node, &data.fields, static_ui_style_lookup);
    node
}

pub(super) fn build_ui_shape(data: &SceneDefNodeData) -> UiShape {
    let mut node = UiShape::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_shape_fields(&mut node, &data.fields);
    node
}

pub(super) fn build_ui_checkbox(
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

pub(super) fn build_ui_color_picker(
    data: &SceneDefNodeData,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> UiColorPicker {
    let mut node = UiColorPicker::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.button.base, base);
    }
    apply_ui_root_fields(&mut node.button.base, &data.fields);
    apply_ui_color_picker_fields(&mut node, &data.fields, static_ui_style_lookup);
    node
}

pub(super) fn build_ui_label(data: &SceneDefNodeData) -> UiLabel {
    let mut node = UiLabel::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_label_fields(&mut node, &data.fields);
    node
}

pub(super) fn build_ui_image(data: &SceneDefNodeData) -> UiImage {
    let mut node = UiImage::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_image_fields(&mut node, &data.fields);
    node
}

pub(super) fn build_ui_video_player(data: &SceneDefNodeData) -> UiVideoPlayer {
    let mut node = UiVideoPlayer::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_video_player_fields(&mut node.video, &data.fields);
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| match name {
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        "scale_mode" | "aspect_mode" => {
            if let Some(v) = as_ui_image_scale_mode(value) {
                node.scale_mode = v;
            }
        }
        "aspect_ratio" => {
            if let Some(v) = as_f32(value) {
                node.aspect_ratio = v.max(0.0);
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

pub(super) fn build_ui_image_button(data: &SceneDefNodeData) -> UiImageButton {
    let mut node = UiImageButton::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_image_button_fields(&mut node, &data.fields);
    node
}

pub(super) fn build_ui_nine_slice_button(data: &SceneDefNodeData) -> UiNineSliceButton {
    let mut node = UiNineSliceButton::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_nine_slice_button_fields(&mut node, &data.fields);
    node
}

pub(super) fn build_ui_nine_slice(data: &SceneDefNodeData) -> UiNineSlice {
    let mut node = UiNineSlice::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_nine_slice_fields(&mut node, &data.fields);
    node
}

pub(super) fn build_ui_camera_stream(data: &SceneDefNodeData) -> UiCameraStream {
    let mut node = UiCameraStream::default();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_camera_stream_fields(&mut node.stream, &data.fields);
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| match name {
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
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

pub(super) fn build_ui_sub_view(data: &SceneDefNodeData) -> UiSubView {
    let mut node = UiSubView::default();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);

    let mut camera = Camera3D {
        projection: node.projection.clone(),
        post_processing: node.post_processing.clone(),
        ..Default::default()
    };
    apply_camera_3d_fields(&mut camera, &data.fields);
    node.projection = camera.projection;
    node.post_processing = camera.post_processing;

    SceneFieldIterRef::new(&data.fields).for_each(|name, value| match name {
        "resolution" => {
            if let Some(v) = as_vec2(value) {
                node.resolution = UVector2::new(
                    (v.x.max(0.0) as u32).min(8192),
                    (v.y.max(0.0) as u32).min(8192),
                );
            }
        }
        "width" => {
            if let Some(v) = as_u32(value) {
                node.resolution.x = v.min(8192);
            }
        }
        "height" => {
            if let Some(v) = as_u32(value) {
                node.resolution.y = v.min(8192);
            }
        }
        "aspect_ratio" | "ratio" => {
            if let Some(v) = as_f32(value) {
                node.aspect_ratio = v.max(0.0);
            }
        }
        "aspect_mode" | "scale_mode" | "image_scale" => {
            if let Some(v) = as_str(value) {
                node.aspect_mode = match v {
                    "stretch" | "fill" => UiImageScaleMode::Stretch,
                    "cover" | "crop" => UiImageScaleMode::Cover,
                    _ => UiImageScaleMode::Fit,
                };
            }
        }
        "view_position" | "camera_position" => {
            if let Some(v) = as_vec3(value) {
                node.view_position = v;
            }
        }
        "view_rotation" | "camera_rotation" => {
            if let Some(v) = as_quat(value) {
                node.view_rotation = v;
            }
        }
        "view_2d_position" => {
            if let Some(v) = as_vec2(value) {
                node.view_2d_position = v;
            }
        }
        "view_2d_rotation" => {
            if let Some(v) = as_f32(value) {
                node.view_2d_rotation = v;
            }
        }
        "view_2d_zoom" => {
            if let Some(v) = as_f32(value) {
                node.view_2d_zoom = v.max(0.001);
            }
        }
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        "background" | "clear_color" => {
            if let Some(v) = as_scene_color(value) {
                node.background = v;
            }
        }
        "corner_radius" | "radius" => {
            if let Some(v) = as_ui_corner_radius(value) {
                node.corner_radius = v;
            }
        }
        "enabled" | "active" => {
            if let Some(v) = as_bool(value) {
                node.enabled = v;
            }
        }
        "suspend_when_hidden" => {
            if let Some(v) = as_bool(value) {
                node.suspend_when_hidden = v;
            }
        }
        _ => {}
    });
    node
}

pub(super) fn build_ui_animated_image(data: &SceneDefNodeData) -> UiAnimatedImage {
    let mut node = UiAnimatedImage::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_animated_image_fields(&mut node, &data.fields);
    node
}

pub(super) fn build_ui_text_box(
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

pub(super) fn build_ui_text_block(
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

pub(super) fn build_ui_scroll_container(data: &SceneDefNodeData) -> UiScrollContainer {
    let mut node = UiScrollContainer::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.base, base);
    }
    apply_ui_root_fields(&mut node.base, &data.fields);
    apply_ui_scroll_container_fields(&mut node, &data.fields);
    node
}

pub(super) fn build_ui_layout(data: &SceneDefNodeData) -> UiLayout {
    let mut node = UiLayout::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_container_fields(&mut node.inner, &data.fields, true);
    node
}

pub(super) fn build_ui_hlayout(data: &SceneDefNodeData) -> UiHLayout {
    let mut node = UiHLayout::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_fixed_container_fields(&mut node.inner, &data.fields);
    node
}

pub(super) fn build_ui_vlayout(data: &SceneDefNodeData) -> UiVLayout {
    let mut node = UiVLayout::new();
    if let Some(base) = data.base_ref() {
        apply_ui_root_data(&mut node.inner.base, base);
    }
    apply_ui_root_fields(&mut node.inner.base, &data.fields);
    apply_ui_fixed_container_fields(&mut node.inner, &data.fields);
    node
}

pub(super) fn build_ui_grid(data: &SceneDefNodeData) -> UiGrid {
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
        "spacing" => {
            if let Some(mode) = as_ui_layout_spacing_mode(value) {
                node.h_spacing_mode = mode;
                node.v_spacing_mode = mode;
            } else if let Some(v) = as_f32(value) {
                node.h_spacing = v;
                node.v_spacing = v;
                node.h_spacing_mode = perro_ui::UiLayoutSpacingMode::Fixed;
                node.v_spacing_mode = perro_ui::UiLayoutSpacingMode::Fixed;
            }
        }
        _ => {}
    });
    node
}

pub(super) fn build_ui_tree_list(data: &SceneDefNodeData) -> UiTreeList {
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
        "row_height" | "height" => {
            if let Some(v) = as_f32(value) {
                node.row_height = v.max(1.0);
            }
        }
        "icon_size" => {
            if let Some(v) = as_f32(value) {
                node.icon_size = v.max(0.0);
            }
        }
        "toggle_size" => {
            if let Some(v) = as_f32(value) {
                node.toggle_size = v.max(0.0);
            }
        }
        "line_width" => {
            if let Some(v) = as_f32(value) {
                node.line_width = v.max(0.0);
            }
        }
        "v_spacing" | "vertical_spacing" | "spacing" => {
            if let Some(v) = as_f32(value) {
                node.v_spacing = v.max(0.0);
            }
        }
        "line_color" => {
            if let Some(v) = as_scene_color(value) {
                node.line_color = v;
            }
        }
        "triangle_color" => {
            if let Some(v) = as_scene_color(value) {
                node.triangle_color = v;
            }
        }
        "text_color" => {
            if let Some(v) = as_scene_color(value) {
                node.text_color = v;
            }
        }
        "selected_index" => {
            if let Some(v) = as_i32(value)
                && v >= 0
            {
                node.selected_index = Some(v as usize);
            }
        }
        "items" => {
            node.items = as_tree_list_items(value);
        }
        "selected_signals" | "changed_signals" | "value_changed_signals" => {
            node.selected_signals = as_signal_ids(value);
        }
        "toggled_signals" | "toggle_signals" => {
            node.toggled_signals = as_signal_ids(value);
        }
        _ => {}
    });
    node
}

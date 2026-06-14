use super::*;

#[derive(Clone, Copy)]
pub(super) struct UiAutoLayout {
    pub(super) mode: UiLayoutMode,
    pub(super) columns: u32,
    pub(super) h_spacing: f32,
    pub(super) v_spacing: f32,
}

#[derive(Clone, Copy)]
pub(super) struct UiChildrenLayoutCtx {
    pub(super) parent_layout_rect: ComputedUiRect,
    pub(super) content: ComputedUiRect,
    pub(super) parent_scale: Vector2,
}

#[derive(Clone, Copy)]
pub(super) struct UiCommandCtx {
    pub(super) node: NodeID,
    pub(super) rect: UiRectState,
    pub(super) clip_rect: [f32; 4],
    pub(super) scale: Vector2,
    pub(super) virtual_font_scale: f32,
    pub(super) modulate: Color,
}

#[derive(Clone, Copy)]
pub(super) struct TextEditCommandCtx<'a> {
    pub(super) command: UiCommandCtx,
    pub(super) edit: &'a UiTextEdit,
    pub(super) multiline: bool,
    pub(super) focused: bool,
}

pub(super) fn ui_root_from_data(data: &SceneNodeData) -> Option<&UiNode> {
    match data {
        SceneNodeData::UiNode(root) => Some(root),
        SceneNodeData::UiCameraStream(node) => Some(&node.base),
        SceneNodeData::UiPanel(node) => Some(&node.base),
        SceneNodeData::UiShape(node) => Some(&node.base),
        SceneNodeData::UiButton(node) => Some(&node.base),
        SceneNodeData::UiDropdown(node) => Some(&node.button.base),
        SceneNodeData::UiCheckbox(node) => Some(&node.button.base),
        SceneNodeData::UiColorPicker(node) => Some(&node.button.base),
        SceneNodeData::UiImage(node) => Some(&node.base),
        SceneNodeData::UiImageButton(node) => Some(&node.base),
        SceneNodeData::UiNineSlice(node) => Some(&node.base),
        SceneNodeData::UiAnimatedImage(node) => Some(&node.base),
        SceneNodeData::UiLabel(node) => Some(&node.base),
        SceneNodeData::UiTextBox(node) => Some(&node.inner.base),
        SceneNodeData::UiTextBlock(node) => Some(&node.inner.base),
        SceneNodeData::UiScrollContainer(node) => Some(&node.base),
        SceneNodeData::UiLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiHLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiVLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiGrid(node) => Some(&node.base),
        SceneNodeData::UiTreeList(node) => Some(&node.base),
        _ => None,
    }
}

pub(super) fn ui_root_mut_from_data(data: &mut SceneNodeData) -> Option<&mut UiNode> {
    match data {
        SceneNodeData::UiNode(root) => Some(root),
        SceneNodeData::UiCameraStream(node) => Some(&mut node.base),
        SceneNodeData::UiPanel(node) => Some(&mut node.base),
        SceneNodeData::UiShape(node) => Some(&mut node.base),
        SceneNodeData::UiButton(node) => Some(&mut node.base),
        SceneNodeData::UiDropdown(node) => Some(&mut node.button.base),
        SceneNodeData::UiCheckbox(node) => Some(&mut node.button.base),
        SceneNodeData::UiColorPicker(node) => Some(&mut node.button.base),
        SceneNodeData::UiImage(node) => Some(&mut node.base),
        SceneNodeData::UiImageButton(node) => Some(&mut node.base),
        SceneNodeData::UiNineSlice(node) => Some(&mut node.base),
        SceneNodeData::UiAnimatedImage(node) => Some(&mut node.base),
        SceneNodeData::UiLabel(node) => Some(&mut node.base),
        SceneNodeData::UiTextBox(node) => Some(&mut node.inner.base),
        SceneNodeData::UiTextBlock(node) => Some(&mut node.inner.base),
        SceneNodeData::UiScrollContainer(node) => Some(&mut node.base),
        SceneNodeData::UiLayout(node) => Some(&mut node.inner.base),
        SceneNodeData::UiHLayout(node) => Some(&mut node.inner.base),
        SceneNodeData::UiVLayout(node) => Some(&mut node.inner.base),
        SceneNodeData::UiGrid(node) => Some(&mut node.base),
        SceneNodeData::UiTreeList(node) => Some(&mut node.base),
        _ => None,
    }
}

pub(super) fn ui_scroll_content_rect(
    data: &SceneNodeData,
    content: ComputedUiRect,
) -> ComputedUiRect {
    let SceneNodeData::UiScrollContainer(node) = data else {
        return content;
    };
    ComputedUiRect::new(
        content.center + Vector2::new(-node.scroll.x, node.scroll.y),
        content.size,
    )
}

pub(super) fn ui_auto_layout_from_data(data: &SceneNodeData) -> Option<UiAutoLayout> {
    match data {
        SceneNodeData::UiLayout(node) => {
            let h_spacing = if node.inner.h_spacing != 0.0 {
                node.inner.h_spacing
            } else {
                node.inner.spacing
            };
            let v_spacing = if node.inner.v_spacing != 0.0 {
                node.inner.v_spacing
            } else {
                node.inner.spacing
            };
            Some(UiAutoLayout {
                mode: node.inner.mode,
                columns: node.inner.columns.max(1),
                h_spacing,
                v_spacing,
            })
        }
        SceneNodeData::UiHLayout(node) => Some(UiAutoLayout {
            mode: UiLayoutMode::H,
            columns: node.inner.columns.max(1),
            h_spacing: node.inner.h_spacing.max(node.inner.spacing),
            v_spacing: node.inner.v_spacing.max(node.inner.spacing),
        }),
        SceneNodeData::UiVLayout(node) => Some(UiAutoLayout {
            mode: UiLayoutMode::V,
            columns: node.inner.columns.max(1),
            h_spacing: node.inner.h_spacing.max(node.inner.spacing),
            v_spacing: node.inner.v_spacing.max(node.inner.spacing),
        }),
        SceneNodeData::UiGrid(node) => Some(UiAutoLayout {
            mode: UiLayoutMode::Grid,
            columns: node.columns.max(1),
            h_spacing: node.h_spacing,
            v_spacing: node.v_spacing,
        }),
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub(super) struct UiTreeListRow {
    pub(super) index: usize,
    pub(super) depth: u32,
    pub(super) has_children: bool,
    pub(super) last_child: bool,
}

pub(super) fn ui_fill_width(
    layout: &UiLayoutData,
    parent_layout: &UiLayoutData,
    available: f32,
) -> f32 {
    if layout.h_size == UiSizeMode::Fill || parent_layout.h_align == UiHorizontalAlign::Fill {
        (available - layout.margin.horizontal()).max(0.0)
    } else {
        0.0
    }
}

pub(super) fn ui_fill_height(
    layout: &UiLayoutData,
    parent_layout: &UiLayoutData,
    available: f32,
) -> f32 {
    if layout.v_size == UiSizeMode::Fill || parent_layout.v_align == UiVerticalAlign::Fill {
        (available - layout.margin.vertical()).max(0.0)
    } else {
        0.0
    }
}

pub(super) fn ui_h_spacing_amount(spacing_ratio: f32, container_width: f32) -> f32 {
    spacing_ratio.max(0.0) * container_width.max(0.0)
}

pub(super) fn ui_v_spacing_amount(spacing_ratio: f32, container_height: f32) -> f32 {
    spacing_ratio.max(0.0) * container_height.max(0.0)
}

pub(super) fn ui_padding_inset(
    rect: ComputedUiRect,
    padding: perro_ui::UiRect,
) -> perro_ui::UiRect {
    perro_ui::UiRect::new(
        padding.left.max(0.0) * rect.size.x,
        padding.top.max(0.0) * rect.size.y,
        padding.right.max(0.0) * rect.size.x,
        padding.bottom.max(0.0) * rect.size.y,
    )
}

pub(super) fn fit_size_with_padding_ratio(content_size: f32, start: f32, end: f32) -> f32 {
    let ratio = (start.max(0.0) + end.max(0.0)).min(0.999);
    content_size.max(0.0) / (1.0 - ratio)
}

pub(super) fn ui_translation_offset(
    transform: &UiTransform,
    parent_size: Vector2,
    size: Vector2,
) -> Vector2 {
    transform.translation_offset(parent_size, size)
}

pub(super) fn safe_ui_scale(scale: Vector2) -> Vector2 {
    Vector2::new(scale.x.max(0.0001), scale.y.max(0.0001))
}

pub(super) fn scale_ui_rect_from_parent(
    rect: ComputedUiRect,
    parent_rect: ComputedUiRect,
    parent_scale: Vector2,
) -> ComputedUiRect {
    ComputedUiRect::new(
        parent_rect.center + (rect.center - parent_rect.center) * parent_scale,
        rect.size * parent_scale,
    )
}

pub(super) fn insert_scaled_ui_child_rect(
    computed: &mut AHashMap<NodeID, ComputedUiRect>,
    computed_scales: &mut AHashMap<NodeID, Vector2>,
    parent_rect: ComputedUiRect,
    parent_scale: Vector2,
    child: NodeID,
    rect: ComputedUiRect,
    child_scale: Vector2,
) {
    computed.insert(
        child,
        scale_ui_rect_from_parent(rect, parent_rect, parent_scale),
    );
    computed_scales.insert(child, parent_scale * child_scale);
}

pub(super) fn ui_text_measure(data: &SceneNodeData) -> Vector2 {
    match data {
        SceneNodeData::UiLabel(label) => {
            measure_text(label.text.as_ref(), fallback_text_size(label.font_size))
        }
        SceneNodeData::UiTextBox(text_box) => measure_text(
            text_box.inner.text.as_ref(),
            fallback_text_size(text_box.inner.font_size),
        ),
        SceneNodeData::UiTextBlock(text_block) => measure_text(
            text_block.inner.text.as_ref(),
            fallback_text_size(text_block.inner.font_size),
        ),
        _ => Vector2::ZERO,
    }
}

pub(super) fn measure_text(text: &str, font_size: f32) -> Vector2 {
    let mut max_cols = 0_usize;
    let mut line_count = 0_usize;
    for line in text.lines() {
        max_cols = max_cols.max(line.chars().count());
        line_count += 1;
    }
    if line_count == 0 {
        line_count = 1;
    }
    Vector2::new(
        max_cols as f32 * font_size * 0.6,
        line_count as f32 * font_size * 1.2,
    )
}

pub(super) fn align_h_start(
    min_x: f32,
    available: f32,
    used: f32,
    align: UiHorizontalAlign,
) -> f32 {
    match align {
        UiHorizontalAlign::Left | UiHorizontalAlign::Fill => min_x,
        UiHorizontalAlign::Center => min_x + (available - used).max(0.0) * 0.5,
        UiHorizontalAlign::Right => min_x + (available - used).max(0.0),
    }
}

pub(super) fn align_v_top(max_y: f32, available: f32, used: f32, align: UiVerticalAlign) -> f32 {
    match align {
        UiVerticalAlign::Top | UiVerticalAlign::Fill => max_y,
        UiVerticalAlign::Center => max_y - (available - used).max(0.0) * 0.5,
        UiVerticalAlign::Bottom => max_y - (available - used).max(0.0),
    }
}

pub(super) fn align_h_center(
    min_x: f32,
    available: f32,
    width: f32,
    margin: perro_ui::UiRect,
    align: UiHorizontalAlign,
) -> f32 {
    match align {
        UiHorizontalAlign::Left | UiHorizontalAlign::Fill => min_x + margin.left + width * 0.5,
        UiHorizontalAlign::Center => min_x + available * 0.5 + (margin.left - margin.right) * 0.5,
        UiHorizontalAlign::Right => min_x + available - margin.right - width * 0.5,
    }
}

pub(super) fn align_v_center(
    top_y: f32,
    available: f32,
    height: f32,
    margin: perro_ui::UiRect,
    align: UiVerticalAlign,
) -> f32 {
    match align {
        UiVerticalAlign::Top | UiVerticalAlign::Fill => top_y - margin.top - height * 0.5,
        UiVerticalAlign::Center => top_y - available * 0.5 + (margin.bottom - margin.top) * 0.5,
        UiVerticalAlign::Bottom => top_y - available + margin.bottom + height * 0.5,
    }
}

pub(super) fn ui_command_from_node(
    data: &SceneNodeData,
    command_ctx: UiCommandCtx,
    button_state: UiButtonVisualState,
    focused_text_edit: Option<NodeID>,
) -> Option<UiCommand> {
    let UiCommandCtx {
        node,
        rect,
        clip_rect,
        scale,
        virtual_font_scale,
        modulate,
    } = command_ctx;
    match data {
        SceneNodeData::UiPanel(panel) => Some(panel_command(
            node,
            rect,
            clip_rect,
            scale,
            &panel.style,
            modulate,
        )),
        SceneNodeData::UiShape(shape) => Some(UiCommand::UpsertShape {
            node,
            rect,
            clip_rect,
            kind: shape.kind,
            fill: Runtime::color_modulate_rgba(shape.fill.to_rgba(), modulate),
            stroke: Runtime::color_modulate_rgba(shape.stroke.to_rgba(), modulate),
            stroke_width: shape.stroke_width * ui_style_scale(scale),
        }),
        SceneNodeData::UiButton(button) => {
            let style = button_style(button, button_state);
            let style_scale = ui_style_scale(scale);
            Some(UiCommand::UpsertButton {
                node,
                rect,
                clip_rect,
                fill: Runtime::color_modulate_rgba(style.fill.to_rgba(), modulate),
                stroke: Runtime::color_modulate_rgba(style.stroke.to_rgba(), modulate),
                stroke_width: style.stroke_width * style_scale,
                corner_radius: style.corner_radius,
                shadow: ui_depth_effect_state(style.shadow, style_scale),
                highlight: ui_depth_effect_state(style.highlight, style_scale),
                disabled: button.disabled,
            })
        }
        SceneNodeData::UiDropdown(dropdown) => {
            let style = button_style(&dropdown.button, button_state);
            let style_scale = ui_style_scale(scale);
            Some(UiCommand::UpsertButton {
                node,
                rect,
                clip_rect,
                fill: Runtime::color_modulate_rgba(style.fill.to_rgba(), modulate),
                stroke: Runtime::color_modulate_rgba(style.stroke.to_rgba(), modulate),
                stroke_width: style.stroke_width * style_scale,
                corner_radius: style.corner_radius,
                shadow: ui_depth_effect_state(style.shadow, style_scale),
                highlight: ui_depth_effect_state(style.highlight, style_scale),
                disabled: dropdown.disabled,
            })
        }
        SceneNodeData::UiCheckbox(checkbox) => {
            let style = checkbox_style(checkbox, button_state);
            let style_scale = ui_style_scale(scale);
            Some(UiCommand::UpsertCheckbox {
                node,
                rect,
                clip_rect,
                fill: Runtime::color_modulate_rgba(style.fill.to_rgba(), modulate),
                stroke: Runtime::color_modulate_rgba(style.stroke.to_rgba(), modulate),
                stroke_width: style.stroke_width * style_scale,
                corner_radius: style.corner_radius,
                shadow: ui_depth_effect_state(style.shadow, style_scale),
                highlight: ui_depth_effect_state(style.highlight, style_scale),
                checked: checkbox.checked,
                dot_fill: Runtime::color_modulate_rgba(checkbox.dot_fill.to_rgba(), modulate),
                disabled: checkbox.disabled,
            })
        }
        SceneNodeData::UiColorPicker(_) => None,
        SceneNodeData::UiLabel(label) => Some(UiCommand::UpsertLabel {
            node,
            rect,
            clip_rect,
            text: Cow::Owned(label.text.to_string()),
            color: Runtime::color_modulate(label.color, modulate),
            font_size: {
                let (base, node_scale) =
                    if let Some(px) = text_size_from_rect_ratio(rect.size, label.text_size_ratio) {
                        (px, 1.0)
                    } else {
                        (fallback_text_size(label.font_size), ui_font_scale(scale))
                    };
                resolve_font_size(base, node_scale, virtual_font_scale, label.font_sizing)
            },
            h_align: text_align_state(label.h_align),
            v_align: text_align_state(label.v_align),
        }),
        SceneNodeData::UiImage(image) => {
            if image.texture.is_nil() {
                return None;
            }
            let (uv_min, uv_max, aspect_ratio) =
                ui_image_region_uv(image.texture_region, image.aspect_ratio);
            Some(UiCommand::UpsertImage {
                node,
                rect,
                clip_rect,
                texture: image.texture,
                tint: Runtime::color_modulate(image.tint, modulate),
                uv_min,
                uv_max,
                scale_mode: ui_image_scale_state(image.scale_mode),
                h_align: text_align_state(image.h_align),
                v_align: text_align_state(image.v_align),
                aspect_ratio,
                corner_radius: 0.0,
            })
        }
        SceneNodeData::UiImageButton(image) => {
            if image.texture.is_nil() {
                return None;
            }
            let (uv_min, uv_max, aspect_ratio) =
                ui_image_region_uv(image.texture_region, image.aspect_ratio);
            Some(UiCommand::UpsertImage {
                node,
                rect,
                clip_rect,
                texture: image.texture,
                tint: Runtime::color_modulate(image_button_tint(image, button_state), modulate),
                uv_min,
                uv_max,
                scale_mode: ui_image_scale_state(image.scale_mode),
                h_align: text_align_state(image.h_align),
                v_align: text_align_state(image.v_align),
                aspect_ratio,
                corner_radius: 0.0,
            })
        }
        SceneNodeData::UiNineSlice(image) => {
            if image.texture.is_nil() {
                return None;
            }
            let (uv_min, uv_max, _) = ui_image_region_uv(image.texture_region, 0.0);
            Some(UiCommand::UpsertNineSlice {
                node,
                rect,
                clip_rect,
                texture: image.texture,
                tint: Runtime::color_modulate(image.tint, modulate),
                uv_min,
                uv_max,
                margins: image.margins,
            })
        }
        SceneNodeData::UiAnimatedImage(image) => {
            if image.texture.is_nil() {
                return None;
            }
            let (uv_min, uv_max, aspect_ratio) =
                ui_image_region_uv(image.current_texture_region(), image.aspect_ratio);
            Some(UiCommand::UpsertImage {
                node,
                rect,
                clip_rect,
                texture: image.texture,
                tint: Runtime::color_modulate(image.tint, modulate),
                uv_min,
                uv_max,
                scale_mode: ui_image_scale_state(image.scale_mode),
                h_align: text_align_state(image.h_align),
                v_align: text_align_state(image.v_align),
                aspect_ratio,
                corner_radius: 0.0,
            })
        }
        SceneNodeData::UiCameraStream(stream) => {
            if !stream.stream.enabled || stream.stream.camera.is_nil() {
                return None;
            }
            Some(UiCommand::UpsertImage {
                node,
                rect,
                clip_rect,
                texture: Runtime::camera_stream_texture_id(node),
                tint: Runtime::color_modulate(stream.tint, modulate),
                uv_min: [0.0, 0.0],
                uv_max: [1.0, 1.0],
                scale_mode: ui_image_scale_state(stream.stream.aspect_mode),
                h_align: UiTextAlignState::Center,
                v_align: UiTextAlignState::Center,
                aspect_ratio: camera_stream_aspect_ratio(
                    stream.stream.aspect_ratio,
                    stream.stream.resolution,
                ),
                corner_radius: stream.corner_radius,
            })
        }
        SceneNodeData::UiTextBox(text_box) => Some(text_edit_command(TextEditCommandCtx {
            command: command_ctx,
            edit: &text_box.inner,
            multiline: false,
            focused: focused_text_edit == Some(node),
        })),
        SceneNodeData::UiTextBlock(text_block) => Some(text_edit_command(TextEditCommandCtx {
            command: command_ctx,
            edit: &text_block.inner,
            multiline: true,
            focused: focused_text_edit == Some(node),
        })),
        _ => None,
    }
}

pub(super) fn ui_rect_state_from_node(
    data: &SceneNodeData,
    rect: ComputedUiRect,
    button_state: UiButtonVisualState,
    effective_z: i32,
) -> Option<UiRectState> {
    match data {
        SceneNodeData::UiButton(button) => {
            return Some(button_rect_state(button, rect, button_state, effective_z));
        }
        SceneNodeData::UiDropdown(dropdown) => {
            return Some(button_rect_state(
                &dropdown.button,
                rect,
                button_state,
                effective_z,
            ));
        }
        SceneNodeData::UiCheckbox(checkbox) => {
            return Some(button_rect_state(
                &checkbox.button,
                rect,
                button_state,
                effective_z,
            ));
        }
        SceneNodeData::UiColorPicker(picker) => {
            return Some(button_rect_state(
                &picker.button,
                rect,
                button_state,
                effective_z,
            ));
        }
        SceneNodeData::UiImageButton(button) => {
            return Some(image_button_rect_state(
                button,
                rect,
                button_state,
                effective_z,
            ));
        }
        _ => {}
    }
    let ui = ui_root_from_data(data)?;
    Some(UiRectState {
        center: [rect.center.x, rect.center.y],
        size: [rect.size.x, rect.size.y],
        pivot: ui_pivot_state(&ui.transform),
        rotation_radians: ui.transform.rotation,
        z_index: effective_z,
    })
}

pub(super) fn image_button_rect_state(
    button: &perro_ui::UiImageButton,
    base_rect: ComputedUiRect,
    state: UiButtonVisualState,
    effective_z: i32,
) -> UiRectState {
    let ui = image_button_state_base(button, state).unwrap_or(&button.base);
    let state_has_size_override = match state {
        UiButtonVisualState::Hover => button.hover_size_override,
        UiButtonVisualState::Pressed => button.pressed_size_override,
        UiButtonVisualState::Neutral => false,
    };
    let size = match state {
        UiButtonVisualState::Neutral => base_rect.size,
        UiButtonVisualState::Hover | UiButtonVisualState::Pressed => {
            if state_has_size_override {
                ui.transform
                    .scale_size(ui.layout.size.resolve(base_rect.size))
            } else {
                base_rect.size
            }
        }
    };
    let center = if state == UiButtonVisualState::Neutral {
        base_rect.center
    } else {
        base_rect.center + ui.transform.translation
    };
    UiRectState {
        center: [center.x, center.y],
        size: [size.x, size.y],
        pivot: ui_pivot_state(&ui.transform),
        rotation_radians: ui.transform.rotation,
        z_index: effective_z,
    }
}

pub(super) fn button_rect_state(
    button: &perro_ui::UiButton,
    base_rect: ComputedUiRect,
    state: UiButtonVisualState,
    effective_z: i32,
) -> UiRectState {
    let ui = button_state_base(button, state).unwrap_or(&button.base);
    let state_has_size_override = match state {
        UiButtonVisualState::Hover => button.hover_size_override,
        UiButtonVisualState::Pressed => button.pressed_size_override,
        UiButtonVisualState::Neutral => false,
    };
    let size = match state {
        UiButtonVisualState::Neutral => base_rect.size,
        UiButtonVisualState::Hover | UiButtonVisualState::Pressed => {
            if state_has_size_override {
                ui.transform
                    .scale_size(ui.layout.size.resolve(base_rect.size))
            } else {
                base_rect.size
            }
        }
    };
    let center = if state == UiButtonVisualState::Neutral {
        base_rect.center
    } else {
        base_rect.center + ui.transform.translation
    };
    UiRectState {
        center: [center.x, center.y],
        size: [size.x, size.y],
        pivot: ui_pivot_state(&ui.transform),
        rotation_radians: ui.transform.rotation,
        z_index: effective_z,
    }
}

pub(super) fn computed_rect_from_state(rect: &UiRectState) -> ComputedUiRect {
    ComputedUiRect::new(
        Vector2::new(rect.center[0], rect.center[1]),
        Vector2::new(rect.size[0], rect.size[1]),
    )
}

pub(super) fn ui_pivot_state(transform: &UiTransform) -> [f32; 2] {
    let pivot = transform.pivot.resolve(Vector2::new(1.0, 1.0));
    [pivot.x, pivot.y]
}

pub(super) fn ui_command_matches_node(
    command: &UiCommand,
    data: &SceneNodeData,
    command_ctx: UiCommandCtx,
    button_state: UiButtonVisualState,
    focused_text_edit: Option<NodeID>,
) -> bool {
    let node = match command {
        UiCommand::UpsertPanel { node, .. }
        | UiCommand::UpsertShape { node, .. }
        | UiCommand::UpsertColorWheel { node, .. }
        | UiCommand::UpsertButton { node, .. }
        | UiCommand::UpsertCheckbox { node, .. }
        | UiCommand::UpsertLabel { node, .. }
        | UiCommand::UpsertImage { node, .. }
        | UiCommand::UpsertNineSlice { node, .. }
        | UiCommand::UpsertTextEdit { node, .. }
        | UiCommand::RemoveNode { node } => *node,
        UiCommand::Clear => NodeID::nil(),
    };
    let Some(expected) = ui_command_from_node(
        data,
        UiCommandCtx {
            node,
            ..command_ctx
        },
        button_state,
        focused_text_edit,
    ) else {
        return false;
    };
    *command == expected
}

pub(super) fn ui_font_scale(scale: Vector2) -> f32 {
    scale.y.abs().max(0.0001)
}

pub(super) fn ui_style_scale(scale: Vector2) -> f32 {
    scale.x.abs().min(scale.y.abs()).max(0.0001)
}

pub(super) fn button_style(button: &perro_ui::UiButton, state: UiButtonVisualState) -> &UiStyle {
    if button_inactive(button) {
        return &button.style;
    }
    match state {
        UiButtonVisualState::Neutral => &button.style,
        UiButtonVisualState::Hover => &button.hover_style,
        UiButtonVisualState::Pressed => &button.pressed_style,
    }
}

pub(super) fn button_state_base(
    button: &perro_ui::UiButton,
    state: UiButtonVisualState,
) -> Option<&perro_ui::UiNode> {
    if button_inactive(button) {
        return None;
    }
    match state {
        UiButtonVisualState::Neutral => None,
        UiButtonVisualState::Hover => button.hover_base.as_ref(),
        UiButtonVisualState::Pressed => button.pressed_base.as_ref(),
    }
}

pub(super) fn button_inactive(button: &perro_ui::UiButton) -> bool {
    button.disabled || !button.input_enabled
}

pub(super) fn checkbox_style(
    checkbox: &perro_ui::UiCheckbox,
    state: UiButtonVisualState,
) -> &UiStyle {
    if checkbox_inactive(checkbox) {
        return if checkbox.checked {
            &checkbox.checked_style
        } else {
            &checkbox.button.style
        };
    }
    if !checkbox.checked {
        return button_style(&checkbox.button, state);
    }
    match state {
        UiButtonVisualState::Neutral => &checkbox.checked_style,
        UiButtonVisualState::Hover => &checkbox.checked_hover_style,
        UiButtonVisualState::Pressed => &checkbox.checked_pressed_style,
    }
}

pub(super) fn checkbox_inactive(checkbox: &perro_ui::UiCheckbox) -> bool {
    button_inactive(&checkbox.button)
}

pub(super) fn image_button_tint(
    button: &perro_ui::UiImageButton,
    state: UiButtonVisualState,
) -> Color {
    if image_button_inactive(button) {
        return button.tint;
    }
    match state {
        UiButtonVisualState::Neutral => button.tint,
        UiButtonVisualState::Hover => button.hover_tint,
        UiButtonVisualState::Pressed => button.pressed_tint,
    }
}

pub(super) fn image_button_state_base(
    button: &perro_ui::UiImageButton,
    state: UiButtonVisualState,
) -> Option<&perro_ui::UiNode> {
    if image_button_inactive(button) {
        return None;
    }
    match state {
        UiButtonVisualState::Neutral => None,
        UiButtonVisualState::Hover => button.hover_base.as_ref(),
        UiButtonVisualState::Pressed => button.pressed_base.as_ref(),
    }
}

pub(super) fn image_button_inactive(button: &perro_ui::UiImageButton) -> bool {
    button.disabled || !button.input_enabled
}

pub(super) fn button_custom_event_signals<'a>(
    button: &'a perro_ui::UiButton,
    event: &str,
) -> &'a [SignalID] {
    match event {
        "hover_enter" => &button.hover_signals,
        "hover_exit" => &button.hover_exit_signals,
        "pressed" => &button.pressed_signals,
        "released" => &button.released_signals,
        "click" => &button.clicked_signals,
        _ => &[],
    }
}

pub(super) fn image_button_custom_event_signals<'a>(
    button: &'a perro_ui::UiImageButton,
    event: &str,
) -> &'a [SignalID] {
    match event {
        "hover_enter" => &button.hover_signals,
        "hover_exit" => &button.hover_exit_signals,
        "pressed" => &button.pressed_signals,
        "released" => &button.released_signals,
        "click" => &button.clicked_signals,
        _ => &[],
    }
}

pub(super) fn text_edit_custom_event_signals<'a>(
    edit: &'a UiTextEdit,
    event: &str,
) -> &'a [SignalID] {
    match event {
        "hovered" => &edit.hover_signals,
        "unhovered" => &edit.hover_exit_signals,
        "focused" => &edit.focused_signals,
        "unfocused" => &edit.unfocused_signals,
        "text_changed" => &edit.text_changed_signals,
        _ => &[],
    }
}

pub(super) fn collect_button_events(
    node: NodeID,
    prev: UiButtonVisualState,
    next: UiButtonVisualState,
    out: &mut Vec<(NodeID, &'static str)>,
) {
    if prev == next {
        return;
    }

    if prev == UiButtonVisualState::Neutral && next != UiButtonVisualState::Neutral {
        out.push((node, "hover_enter"));
    }
    if prev != UiButtonVisualState::Neutral && next == UiButtonVisualState::Neutral {
        out.push((node, "hover_exit"));
    }
    if prev != UiButtonVisualState::Pressed && next == UiButtonVisualState::Pressed {
        out.push((node, "pressed"));
    }
    if prev == UiButtonVisualState::Pressed && next != UiButtonVisualState::Pressed {
        out.push((node, "released"));
    }
    if prev == UiButtonVisualState::Pressed && next == UiButtonVisualState::Hover {
        out.push((node, "click"));
    }
}

pub(super) fn text_align_state(align: perro_ui::UiTextAlign) -> UiTextAlignState {
    match align {
        perro_ui::UiTextAlign::Start => UiTextAlignState::Start,
        perro_ui::UiTextAlign::Center => UiTextAlignState::Center,
        perro_ui::UiTextAlign::End => UiTextAlignState::End,
    }
}

pub(super) fn ui_image_scale_state(mode: UiImageScaleMode) -> UiImageScaleState {
    match mode {
        UiImageScaleMode::Stretch => UiImageScaleState::Stretch,
        UiImageScaleMode::Fit => UiImageScaleState::Fit,
        UiImageScaleMode::Cover => UiImageScaleState::Cover,
    }
}

pub(super) fn ui_image_region_uv(
    region: Option<[f32; 4]>,
    aspect_ratio: f32,
) -> ([f32; 2], [f32; 2], f32) {
    let Some([x, y, w, h]) = region else {
        return ([0.0, 0.0], [1.0, 1.0], aspect_ratio.max(0.0));
    };
    if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return ([0.0, 0.0], [1.0, 1.0], aspect_ratio.max(0.0));
    }
    ([x, y], [x + w, y + h], aspect_ratio.max(w / h))
}

pub(super) fn camera_stream_aspect_ratio(aspect_ratio: f32, resolution: UVector2) -> f32 {
    if aspect_ratio.is_finite() && aspect_ratio > 0.0 {
        aspect_ratio
    } else {
        resolution.x.max(1) as f32 / resolution.y.max(1) as f32
    }
}

mod text_edit;
pub(super) use text_edit::*;

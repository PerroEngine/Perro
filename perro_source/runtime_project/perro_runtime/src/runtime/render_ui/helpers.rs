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
}

#[derive(Clone, Copy)]
pub(super) struct TextEditCommandCtx<'a> {
    pub(super) command: UiCommandCtx,
    pub(super) edit: &'a UiTextEdit,
    pub(super) multiline: bool,
    pub(super) focused: bool,
}

pub(super) fn ui_root_from_data(data: &SceneNodeData) -> Option<&UiBox> {
    match data {
        SceneNodeData::UiBox(root) => Some(root),
        SceneNodeData::UiPanel(node) => Some(&node.base),
        SceneNodeData::UiButton(node) => Some(&node.base),
        SceneNodeData::UiImage(node) => Some(&node.base),
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
pub(super) struct UiTreeRow {
    pub(super) node: NodeID,
    pub(super) depth: u32,
}

pub(super) fn ui_tree_visible_rows(tree: &perro_ui::UiTreeList) -> Vec<UiTreeRow> {
    let mut rows = Vec::new();
    let mut stack = Vec::new();
    let mut seen = Vec::new();
    for &root in tree.roots.iter().rev() {
        stack.push((root, 0_u32));
    }
    while let Some((node, depth)) = stack.pop() {
        if seen.contains(&node) {
            continue;
        }
        seen.push(node);
        rows.push(UiTreeRow { node, depth });
        if tree.is_collapsed(node) {
            continue;
        }
        for &child in tree.children_of(node).iter().rev() {
            stack.push((child, depth.saturating_add(1)));
        }
    }
    rows
}

pub(super) fn ui_tree_contains(tree: &perro_ui::UiTreeList, child: NodeID) -> bool {
    tree.roots.contains(&child)
        || tree
            .branches
            .iter()
            .any(|branch| branch.children.contains(&child))
}

pub(super) fn ui_tree_visible_contains(tree: &perro_ui::UiTreeList, child: NodeID) -> bool {
    ui_tree_visible_rows(tree)
        .into_iter()
        .any(|row| row.node == child)
}

pub(super) fn ui_tree_all_nodes(tree: &perro_ui::UiTreeList) -> Vec<NodeID> {
    let mut nodes = Vec::new();
    for &root in &tree.roots {
        if !nodes.contains(&root) {
            nodes.push(root);
        }
    }
    for branch in &tree.branches {
        if !nodes.contains(&branch.parent) {
            nodes.push(branch.parent);
        }
        for &child in &branch.children {
            if !nodes.contains(&child) {
                nodes.push(child);
            }
        }
    }
    nodes
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

pub(super) fn ui_translation_offset(transform: &UiTransform, size: Vector2) -> Vector2 {
    transform.translation_offset(size)
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
    } = command_ctx;
    match data {
        SceneNodeData::UiPanel(panel) => {
            Some(panel_command(node, rect, clip_rect, scale, &panel.style))
        }
        SceneNodeData::UiButton(button) => {
            let style = button_style(button, button_state);
            let style_scale = ui_style_scale(scale);
            Some(UiCommand::UpsertButton {
                node,
                rect,
                clip_rect,
                fill: style.fill.to_rgba(),
                stroke: style.stroke.to_rgba(),
                stroke_width: style.stroke_width * style_scale,
                corner_radius: style.corner_radius,
                shadow: ui_depth_effect_state(style.shadow, style_scale),
                highlight: ui_depth_effect_state(style.highlight, style_scale),
                disabled: button.disabled,
            })
        }
        SceneNodeData::UiLabel(label) => Some(UiCommand::UpsertLabel {
            node,
            rect,
            clip_rect,
            text: Cow::Owned(label.text.to_string()),
            color: label.color.to_rgba(),
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
                tint: image.tint.to_rgba(),
                uv_min,
                uv_max,
                scale_mode: ui_image_scale_state(image.scale_mode),
                h_align: text_align_state(image.h_align),
                v_align: text_align_state(image.v_align),
                aspect_ratio,
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
                tint: image.tint.to_rgba(),
                uv_min,
                uv_max,
                scale_mode: ui_image_scale_state(image.scale_mode),
                h_align: text_align_state(image.h_align),
                v_align: text_align_state(image.v_align),
                aspect_ratio,
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
    if let SceneNodeData::UiButton(button) = data {
        return Some(button_rect_state(button, rect, button_state, effective_z));
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
        | UiCommand::UpsertButton { node, .. }
        | UiCommand::UpsertLabel { node, .. }
        | UiCommand::UpsertImage { node, .. }
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
) -> Option<&perro_ui::UiBox> {
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

pub(super) fn button_custom_event_signals<'a>(
    button: &'a perro_ui::UiButton,
    event: &str,
) -> &'a [SignalID] {
    match event {
        "hover_enter" => &button.hover_signals,
        "hover_exit" => &button.hover_exit_signals,
        "pressed" => &button.pressed_signals,
        "released" => &button.released_signals,
        "click" => &button.click_signals,
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

pub(super) fn text_edit_command(ctx: TextEditCommandCtx<'_>) -> UiCommand {
    let TextEditCommandCtx {
        command:
            UiCommandCtx {
                node,
                rect,
                clip_rect,
                scale,
                virtual_font_scale,
            },
        edit,
        multiline,
        focused,
    } = ctx;
    let focused_style = &edit.focused_style;
    let style = &edit.style;
    let style_scale = ui_style_scale(scale);
    let font_scale = ui_font_scale(scale);
    UiCommand::UpsertTextEdit {
        node,
        rect,
        clip_rect,
        fill: if focused {
            focused_style.fill.to_rgba()
        } else {
            style.fill.to_rgba()
        },
        stroke: if focused {
            focused_style.stroke.to_rgba()
        } else {
            style.stroke.to_rgba()
        },
        stroke_width: if focused {
            focused_style.stroke_width
        } else {
            style.stroke_width
        } * style_scale,
        corner_radius: if focused {
            focused_style.corner_radius
        } else {
            style.corner_radius
        },
        shadow: ui_depth_effect_state(
            if focused {
                focused_style.shadow
            } else {
                style.shadow
            },
            style_scale,
        ),
        highlight: ui_depth_effect_state(
            if focused {
                focused_style.highlight
            } else {
                style.highlight
            },
            style_scale,
        ),
        text: Cow::Owned(edit.text.to_string()),
        placeholder: Cow::Owned(edit.placeholder.to_string()),
        color: edit.color.to_rgba(),
        placeholder_color: edit.placeholder_color.to_rgba(),
        selection_color: edit.selection_color.to_rgba(),
        caret_color: edit.caret_color.to_rgba(),
        font_size: {
            let (base, node_scale) =
                if let Some(px) = text_size_from_rect_ratio(rect.size, edit.text_size_ratio) {
                    (px, 1.0)
                } else {
                    (fallback_text_size(edit.font_size), font_scale)
                };
            resolve_font_size(base, node_scale, virtual_font_scale, edit.font_sizing)
        },
        padding: [
            edit.padding.left * scale.x.abs().max(0.0001),
            edit.padding.top * scale.y.abs().max(0.0001),
            edit.padding.right * scale.x.abs().max(0.0001),
            edit.padding.bottom * scale.y.abs().max(0.0001),
        ],
        scroll: [edit.h_scroll, edit.v_scroll],
        caret: edit.caret,
        anchor: edit.anchor,
        focused,
        multiline,
    }
}

pub(super) fn resolve_font_size(
    font_size: f32,
    node_font_scale: f32,
    virtual_font_scale: f32,
    sizing: UiFontSizing,
) -> f32 {
    let viewport_scale = if sizing.relative_to_virtual {
        sizing.clamp_scale(virtual_font_scale)
    } else {
        1.0
    };
    font_size * node_font_scale * viewport_scale
}

pub(super) fn fallback_text_size(font_size: f32) -> f32 {
    if font_size.is_finite() && font_size > 0.0 {
        font_size
    } else {
        16.0
    }
}

pub(super) fn text_size_from_rect_ratio(rect_size: [f32; 2], ratio: f32) -> Option<f32> {
    if !ratio.is_finite() || ratio <= 0.0 {
        return None;
    }
    Some((rect_size[1].max(1.0) * ratio).max(1.0))
}

pub(super) fn panel_command(
    node: NodeID,
    rect: UiRectState,
    clip_rect: [f32; 4],
    scale: Vector2,
    style: &UiStyle,
) -> UiCommand {
    let style_scale = ui_style_scale(scale);
    UiCommand::UpsertPanel {
        node,
        rect,
        clip_rect,
        fill: style.fill.to_rgba(),
        stroke: style.stroke.to_rgba(),
        stroke_width: style.stroke_width * style_scale,
        corner_radius: style.corner_radius,
        shadow: ui_depth_effect_state(style.shadow, style_scale),
        highlight: ui_depth_effect_state(style.highlight, style_scale),
    }
}

pub(super) fn ui_depth_effect_state(
    effect: perro_ui::UiDepthEffect,
    scale: f32,
) -> UiDepthEffectState {
    UiDepthEffectState {
        color: effect.color.to_rgba(),
        distance: effect.distance * scale,
        falloff: effect.falloff * scale,
        vector: [effect.vector.x, effect.vector.y],
        size: effect.size,
    }
}

pub(super) fn viewport_clip_rect(viewport: Vector2) -> [f32; 4] {
    [0.0, 0.0, viewport.x.max(1.0), viewport.y.max(1.0)]
}

pub(super) fn rect_to_screen_clip(rect: ComputedUiRect, viewport: Vector2) -> [f32; 4] {
    let cx = viewport.x * 0.5 + rect.center.x;
    let cy = viewport.y * 0.5 - rect.center.y;
    let hx = rect.size.x * 0.5;
    let hy = rect.size.y * 0.5;
    [cx - hx, cy - hy, cx + hx, cy + hy]
}

pub(super) fn intersect_clip_rect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let min_x = a[0].max(b[0]);
    let min_y = a[1].max(b[1]);
    let max_x = a[2].min(b[2]);
    let max_y = a[3].min(b[3]);
    [min_x, min_y, max_x.max(min_x), max_y.max(min_y)]
}

pub(super) fn ui_command_clip_rect(command: &UiCommand) -> [f32; 4] {
    match command {
        UiCommand::UpsertPanel { clip_rect, .. }
        | UiCommand::UpsertButton { clip_rect, .. }
        | UiCommand::UpsertLabel { clip_rect, .. }
        | UiCommand::UpsertImage { clip_rect, .. }
        | UiCommand::UpsertTextEdit { clip_rect, .. } => *clip_rect,
        UiCommand::RemoveNode { .. } | UiCommand::Clear => [0.0, 0.0, 1.0, 1.0],
    }
}

pub(super) fn text_edit_ref(data: &SceneNodeData) -> Option<&UiTextEdit> {
    match data {
        SceneNodeData::UiTextBox(node) => Some(&node.inner),
        SceneNodeData::UiTextBlock(node) => Some(&node.inner),
        _ => None,
    }
}

pub(super) fn text_edit_mut(data: &mut SceneNodeData) -> Option<&mut UiTextEdit> {
    match data {
        SceneNodeData::UiTextBox(node) => Some(&mut node.inner),
        SceneNodeData::UiTextBlock(node) => Some(&mut node.inner),
        _ => None,
    }
}

pub(super) fn text_edit_keys() -> &'static [KeyCode] {
    &[
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Enter,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::KeyA,
        KeyCode::KeyC,
        KeyCode::KeyV,
        KeyCode::KeyX,
    ]
}

pub(super) fn repeatable_text_edit_keys() -> &'static [KeyCode] {
    &[
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::Home,
        KeyCode::End,
    ]
}

pub(super) fn insert_text_input(edit: &mut UiTextEdit, text: &str) -> bool {
    if !edit.editable || text.is_empty() {
        return false;
    }
    let filtered = normalize_text_input(text, edit.multiline);
    if filtered.is_empty() {
        return false;
    }
    replace_selection(edit, &filtered);
    true
}

pub(super) fn apply_text_edit_key_input(
    edit: &mut UiTextEdit,
    shift: bool,
    ctrl: bool,
    repeat_key: Option<KeyCode>,
    input: &perro_input::InputSnapshot,
) -> bool {
    let mut changed = false;
    if ctrl && input.is_key_pressed(KeyCode::KeyA) {
        edit.anchor = 0;
        edit.caret = edit.text.len();
        return true;
    }
    if ctrl && input.is_key_pressed(KeyCode::KeyC) {
        copy_selection_to_clipboard(edit);
        return false;
    }
    if ctrl && input.is_key_pressed(KeyCode::KeyX) {
        if edit.editable && copy_selection_to_clipboard(edit) {
            replace_selection(edit, "");
            return true;
        }
        return false;
    }
    if ctrl && input.is_key_pressed(KeyCode::KeyV) {
        if edit.editable
            && let Some(text) = read_clipboard_text(edit.multiline)
        {
            replace_selection(edit, &text);
            return true;
        }
        return false;
    }
    if repeat_key == Some(KeyCode::Backspace) && edit.editable {
        changed |= backspace(edit);
    }
    if repeat_key == Some(KeyCode::Delete) && edit.editable {
        changed |= delete(edit);
    }
    if input.is_key_pressed(KeyCode::Enter) && edit.editable && edit.multiline {
        replace_selection(edit, "\n");
        changed = true;
    }
    if repeat_key == Some(KeyCode::ArrowLeft) {
        move_caret(edit, prev_char(edit.text.as_ref(), edit.caret), shift);
        changed = true;
    }
    if repeat_key == Some(KeyCode::ArrowRight) {
        move_caret(edit, next_char(edit.text.as_ref(), edit.caret), shift);
        changed = true;
    }
    if repeat_key == Some(KeyCode::Home) {
        let line = line_for_index(edit.text.as_ref(), edit.caret);
        move_caret(edit, line.start, shift);
        changed = true;
    }
    if repeat_key == Some(KeyCode::End) {
        let line = line_for_index(edit.text.as_ref(), edit.caret);
        move_caret(edit, line.end, shift);
        changed = true;
    }
    if edit.multiline && repeat_key == Some(KeyCode::ArrowUp) {
        move_vertical(edit, -1, shift);
        changed = true;
    }
    if edit.multiline && repeat_key == Some(KeyCode::ArrowDown) {
        move_vertical(edit, 1, shift);
        changed = true;
    }
    changed
}

pub(super) fn copy_selection_to_clipboard(edit: &UiTextEdit) -> bool {
    let (start, end) = selection_range(edit);
    if start == end {
        return false;
    }
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return false;
    };
    clipboard
        .set_text(edit.text[start..end].to_string())
        .is_ok()
}

pub(super) fn read_clipboard_text(multiline: bool) -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let text = clipboard.get_text().ok()?;
    let text = normalize_text_input(&text, multiline);
    (!text.is_empty()).then_some(text)
}

pub(super) fn replace_selection(edit: &mut UiTextEdit, replacement: &str) {
    let mut text = edit.text.to_string();
    let (start, end) = selection_range(edit);
    text.replace_range(start..end, replacement);
    let caret = start + replacement.len();
    edit.text = Cow::Owned(text);
    edit.caret = caret;
    edit.anchor = caret;
}

pub(super) fn selection_range(edit: &UiTextEdit) -> (usize, usize) {
    let text = edit.text.as_ref();
    let a = clamp_char_boundary(text, edit.anchor);
    let b = clamp_char_boundary(text, edit.caret);
    if a <= b { (a, b) } else { (b, a) }
}

pub(super) fn backspace(edit: &mut UiTextEdit) -> bool {
    if edit.caret != edit.anchor {
        replace_selection(edit, "");
        return true;
    }
    let prev = prev_char(edit.text.as_ref(), edit.caret);
    if prev == edit.caret {
        return false;
    }
    let mut text = edit.text.to_string();
    text.replace_range(prev..edit.caret, "");
    edit.text = Cow::Owned(text);
    edit.caret = prev;
    edit.anchor = prev;
    true
}

pub(super) fn delete(edit: &mut UiTextEdit) -> bool {
    if edit.caret != edit.anchor {
        replace_selection(edit, "");
        return true;
    }
    let next = next_char(edit.text.as_ref(), edit.caret);
    if next == edit.caret {
        return false;
    }
    let mut text = edit.text.to_string();
    text.replace_range(edit.caret..next, "");
    edit.text = Cow::Owned(text);
    edit.anchor = edit.caret;
    true
}

pub(super) fn move_caret(edit: &mut UiTextEdit, index: usize, extend: bool) {
    let index = clamp_char_boundary(edit.text.as_ref(), index);
    edit.caret = index;
    if !extend {
        edit.anchor = index;
    }
}

pub(super) fn move_vertical(edit: &mut UiTextEdit, delta: i32, extend: bool) {
    let text = edit.text.as_ref();
    let lines = text_line_ranges(text);
    let Some(current_line) = lines
        .iter()
        .position(|line| edit.caret >= line.start && edit.caret <= line.end)
    else {
        return;
    };
    let target_line = (current_line as i32 + delta).clamp(0, lines.len() as i32 - 1) as usize;
    let col = text[lines[current_line].start..edit.caret].chars().count();
    let target = index_at_col(text, lines[target_line], col);
    move_caret(edit, target, extend);
}

pub(super) fn ensure_caret_visible(edit: &mut UiTextEdit, rect: Option<ComputedUiRect>) {
    let Some(rect) = rect else {
        return;
    };
    let content_w = (rect.size.x - edit.padding.horizontal()).max(1.0);
    let content_h = (rect.size.y - edit.padding.vertical()).max(1.0);
    let caret_pos = caret_text_pos(edit);
    let line_h = text_line_height(edit);
    if caret_pos.x < edit.h_scroll {
        edit.h_scroll = caret_pos.x.max(0.0);
    } else if caret_pos.x + 2.0 > edit.h_scroll + content_w {
        edit.h_scroll = (caret_pos.x + 2.0 - content_w).max(0.0);
    }
    if edit.multiline {
        if caret_pos.y < edit.v_scroll {
            edit.v_scroll = caret_pos.y.max(0.0);
        } else if caret_pos.y + line_h > edit.v_scroll + content_h {
            edit.v_scroll = (caret_pos.y + line_h - content_h).max(0.0);
        }
    } else {
        edit.v_scroll = 0.0;
    }
}

pub(super) fn text_index_from_local(edit: &UiTextEdit, local: Vector2) -> usize {
    let lines = text_line_ranges(edit.text.as_ref());
    let line_h = text_line_height(edit);
    let char_w = text_char_width(edit);
    let line_idx = if edit.multiline {
        ((local.y / line_h).floor() as isize).clamp(0, lines.len() as isize - 1) as usize
    } else {
        0
    };
    let col = ((local.x / char_w).round() as isize).max(0) as usize;
    index_at_col(edit.text.as_ref(), lines[line_idx], col)
}

pub(super) fn caret_text_pos(edit: &UiTextEdit) -> Vector2 {
    let text = edit.text.as_ref();
    let lines = text_line_ranges(text);
    let mut line_idx = 0usize;
    let mut line = lines[0];
    for (idx, candidate) in lines.iter().copied().enumerate() {
        if edit.caret >= candidate.start && edit.caret <= candidate.end {
            line_idx = idx;
            line = candidate;
            break;
        }
    }
    let col = text[line.start..edit.caret.min(line.end)].chars().count();
    Vector2::new(
        col as f32 * text_char_width(edit),
        line_idx as f32 * text_line_height(edit),
    )
}

#[derive(Clone, Copy)]
pub(super) struct TextRange {
    pub(super) start: usize,
    pub(super) end: usize,
}

pub(super) fn line_for_index(text: &str, index: usize) -> TextRange {
    text_line_ranges(text)
        .into_iter()
        .find(|line| index >= line.start && index <= line.end)
        .unwrap_or(TextRange {
            start: 0,
            end: text.len(),
        })
}

pub(super) fn text_line_ranges(text: &str) -> Vec<TextRange> {
    if text.is_empty() {
        return vec![TextRange { start: 0, end: 0 }];
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            out.push(TextRange { start, end: idx });
            start = idx + ch.len_utf8();
        }
    }
    out.push(TextRange {
        start,
        end: text.len(),
    });
    out
}

pub(super) fn normalize_text_input(text: &str, multiline: bool) -> String {
    if multiline {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.replace(['\r', '\n', '\t'], " ")
    }
}

pub(super) fn index_at_col(text: &str, line: TextRange, col: usize) -> usize {
    for (count, (idx, _)) in text[line.start..line.end].char_indices().enumerate() {
        if count == col {
            return line.start + idx;
        }
    }
    line.end
}

pub(super) fn prev_char(text: &str, index: usize) -> usize {
    let index = clamp_char_boundary(text, index);
    text[..index]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(index)
}

pub(super) fn next_char(text: &str, index: usize) -> usize {
    let index = clamp_char_boundary(text, index);
    text[index..]
        .chars()
        .next()
        .map(|ch| index + ch.len_utf8())
        .unwrap_or(index)
}

pub(super) fn clamp_char_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

pub(super) fn text_char_width(edit: &UiTextEdit) -> f32 {
    (edit.font_size * 0.6).max(1.0)
}

pub(super) fn text_line_height(edit: &UiTextEdit) -> f32 {
    (edit.font_size * 1.25).max(1.0)
}

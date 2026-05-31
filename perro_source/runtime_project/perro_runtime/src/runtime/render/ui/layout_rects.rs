use super::*;

impl Runtime {
    pub(super) fn ui_effective_z(&self, node: NodeID) -> i32 {
        let mut cur = node;
        let mut out = 0_i32;
        let mut guard = 0_u32;
        while !cur.is_nil() && guard < 4096 {
            guard += 1;
            let Some(scene_node) = self.nodes.get(cur) else {
                break;
            };
            if let Some(ui) = ui_root_from_data(&scene_node.data) {
                out = out.saturating_add(ui.layout.z_index);
            }
            cur = scene_node.parent;
        }
        out
    }

    pub(super) fn compute_ui_child_rect(
        &self,
        parent: NodeID,
        child: NodeID,
        parent_rect: ComputedUiRect,
        child_layout: &UiLayoutData,
        child_transform: &UiTransform,
    ) -> Option<ComputedUiRect> {
        let parent_node = self.nodes.get(parent)?;
        let parent_ui = ui_root_from_data(&parent_node.data)?;
        let layout_children = self.ui_layout_children(parent);
        let content_rect = ui_scroll_content_rect(
            &parent_node.data,
            parent_rect.inset(ui_padding_inset(parent_rect, parent_ui.layout.padding)),
        );
        let auto_rect = ui_auto_layout_from_data(&parent_node.data).and_then(|auto_layout| {
            match auto_layout.mode {
                UiLayoutMode::H => self.compute_ui_h_child_rect(
                    &parent_ui.layout,
                    &layout_children,
                    child,
                    content_rect,
                    auto_layout.h_spacing,
                ),
                UiLayoutMode::V => self.compute_ui_v_child_rect(
                    &parent_ui.layout,
                    &layout_children,
                    child,
                    content_rect,
                    auto_layout.v_spacing,
                ),
                UiLayoutMode::Grid => self.compute_ui_grid_child_rect(
                    &parent_ui.layout,
                    &layout_children,
                    child,
                    content_rect,
                    auto_layout,
                ),
            }
        });
        auto_rect.or_else(|| {
            let child_content = content_rect.inset(child_layout.margin);
            let fill_size = Vector2::new(
                if child_layout.h_size == UiSizeMode::Fill {
                    child_content.size.x
                } else {
                    0.0
                },
                if child_layout.v_size == UiSizeMode::Fill {
                    child_content.size.y
                } else {
                    0.0
                },
            );
            let size = self.resolve_ui_size(child, child_content.size, Some(fill_size));
            Some(child_layout.compute_rect_with_size(child_transform, child_content, size))
        })
    }

    pub(super) fn compute_ui_h_child_rect(
        &self,
        parent_layout: &UiLayoutData,
        children: &[NodeID],
        child: NodeID,
        content: ComputedUiRect,
        spacing: f32,
    ) -> Option<ComputedUiRect> {
        let spacing = ui_h_spacing_amount(spacing, content.size.x);
        let fill_width = self.h_fill_width(children, content.size.x, spacing);
        let used_width = self.h_used_width(children, content.size, spacing, fill_width);
        let min = content.min();
        let max = content.max();
        let mut x = align_h_start(min.x, content.size.x, used_width, parent_layout.h_align);
        for sibling in children.iter().copied() {
            let Some((layout, transform)) = self
                .nodes
                .get(sibling)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some((&ui.layout, &ui.transform)))
            else {
                continue;
            };
            let fill_size = Vector2::new(
                if layout.h_size == UiSizeMode::Fill {
                    fill_width
                } else {
                    0.0
                },
                ui_fill_height(layout, parent_layout, content.size.y),
            );
            let size = self.resolve_ui_size(sibling, content.size, Some(fill_size));
            if sibling == child {
                let y = align_v_center(
                    max.y,
                    content.size.y,
                    size.y,
                    layout.margin,
                    parent_layout.v_align,
                );
                let center = Vector2::new(x + layout.margin.left + size.x * 0.5, y)
                    + ui_translation_offset(transform, content.size, size);
                return Some(ComputedUiRect::new(center, size));
            }
            x += size.x + layout.margin.horizontal() + spacing;
        }
        None
    }

    pub(super) fn compute_ui_h_children_rects(
        &self,
        parent_layout: &UiLayoutData,
        children: &[NodeID],
        layout_ctx: UiChildrenLayoutCtx,
        spacing: f32,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &mut AHashMap<NodeID, Vector2>,
    ) {
        let UiChildrenLayoutCtx {
            parent_layout_rect,
            content,
            parent_scale,
        } = layout_ctx;
        let spacing = ui_h_spacing_amount(spacing, content.size.x);
        let fill_width = self.h_fill_width(children, content.size.x, spacing);
        let used_width = self.h_used_width(children, content.size, spacing, fill_width);
        let min = content.min();
        let max = content.max();
        let mut x = align_h_start(min.x, content.size.x, used_width, parent_layout.h_align);
        for sibling in children.iter().copied() {
            let Some((layout, transform)) = self
                .nodes
                .get(sibling)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some((&ui.layout, &ui.transform)))
            else {
                continue;
            };
            let fill_size = Vector2::new(
                if layout.h_size == UiSizeMode::Fill {
                    fill_width
                } else {
                    0.0
                },
                ui_fill_height(layout, parent_layout, content.size.y),
            );
            let size = self.resolve_ui_size(sibling, content.size, Some(fill_size));
            let y = align_v_center(
                max.y,
                content.size.y,
                size.y,
                layout.margin,
                parent_layout.v_align,
            );
            let center = Vector2::new(x + layout.margin.left + size.x * 0.5, y)
                + ui_translation_offset(transform, content.size, size);
            insert_scaled_ui_child_rect(
                computed,
                computed_scales,
                parent_layout_rect,
                parent_scale,
                sibling,
                ComputedUiRect::new(center, size),
                transform.scale,
            );
            x += size.x + layout.margin.horizontal() + spacing;
        }
    }

    pub(super) fn compute_ui_v_child_rect(
        &self,
        parent_layout: &UiLayoutData,
        children: &[NodeID],
        child: NodeID,
        content: ComputedUiRect,
        spacing: f32,
    ) -> Option<ComputedUiRect> {
        let spacing = ui_v_spacing_amount(spacing, content.size.y);
        let fill_height = self.v_fill_height(children, content.size.y, spacing);
        let used_height = self.v_used_height(children, content.size, spacing, fill_height);
        let min = content.min();
        let max = content.max();
        let mut y = align_v_top(max.y, content.size.y, used_height, parent_layout.v_align);
        for sibling in children.iter().copied() {
            let Some((layout, transform)) = self
                .nodes
                .get(sibling)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some((&ui.layout, &ui.transform)))
            else {
                continue;
            };
            let fill_size = Vector2::new(
                ui_fill_width(layout, parent_layout, content.size.x),
                if layout.v_size == UiSizeMode::Fill {
                    fill_height
                } else {
                    0.0
                },
            );
            let size = self.resolve_ui_size(sibling, content.size, Some(fill_size));
            if sibling == child {
                let x = align_h_center(
                    min.x,
                    content.size.x,
                    size.x,
                    layout.margin,
                    parent_layout.h_align,
                );
                let center = Vector2::new(x, y - layout.margin.top - size.y * 0.5)
                    + ui_translation_offset(transform, content.size, size);
                return Some(ComputedUiRect::new(center, size));
            }
            y -= size.y + layout.margin.vertical() + spacing;
        }
        None
    }

    pub(super) fn compute_ui_v_children_rects(
        &self,
        parent_layout: &UiLayoutData,
        children: &[NodeID],
        layout_ctx: UiChildrenLayoutCtx,
        spacing: f32,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &mut AHashMap<NodeID, Vector2>,
    ) {
        let UiChildrenLayoutCtx {
            parent_layout_rect,
            content,
            parent_scale,
        } = layout_ctx;
        let spacing = ui_v_spacing_amount(spacing, content.size.y);
        let fill_height = self.v_fill_height(children, content.size.y, spacing);
        let used_height = self.v_used_height(children, content.size, spacing, fill_height);
        let min = content.min();
        let max = content.max();
        let mut y = align_v_top(max.y, content.size.y, used_height, parent_layout.v_align);
        for sibling in children.iter().copied() {
            let Some((layout, transform)) = self
                .nodes
                .get(sibling)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some((&ui.layout, &ui.transform)))
            else {
                continue;
            };
            let fill_size = Vector2::new(
                ui_fill_width(layout, parent_layout, content.size.x),
                if layout.v_size == UiSizeMode::Fill {
                    fill_height
                } else {
                    0.0
                },
            );
            let size = self.resolve_ui_size(sibling, content.size, Some(fill_size));
            let x = align_h_center(
                min.x,
                content.size.x,
                size.x,
                layout.margin,
                parent_layout.h_align,
            );
            let center = Vector2::new(x, y - layout.margin.top - size.y * 0.5)
                + ui_translation_offset(transform, content.size, size);
            insert_scaled_ui_child_rect(
                computed,
                computed_scales,
                parent_layout_rect,
                parent_scale,
                sibling,
                ComputedUiRect::new(center, size),
                transform.scale,
            );
            y -= size.y + layout.margin.vertical() + spacing;
        }
    }

    pub(super) fn compute_ui_grid_child_rect(
        &self,
        parent_layout: &UiLayoutData,
        children: &[NodeID],
        child: NodeID,
        content: ComputedUiRect,
        auto: UiAutoLayout,
    ) -> Option<ComputedUiRect> {
        let columns = auto.columns.max(1) as usize;
        let mut child_index = None;
        let mut ui_index = 0_usize;
        let ui_count = children
            .iter()
            .filter(|&&node| {
                self.nodes
                    .get(node)
                    .and_then(|node| ui_root_from_data(&node.data))
                    .is_some_and(|ui| ui.visible)
            })
            .count();
        if ui_count == 0 {
            return None;
        }
        let used_columns = columns.min(ui_count);
        let row_count = ui_count.div_ceil(columns);
        let mut cell_width = 0.0_f32;
        let mut cell_height = 0.0_f32;
        for sibling in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(sibling)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some(&ui.layout))
            else {
                continue;
            };
            if sibling == child {
                child_index = Some(ui_index);
            }
            let size = self.resolve_ui_size(sibling, content.size, None);
            cell_width = cell_width.max(size.x + layout.margin.horizontal());
            cell_height = cell_height.max(size.y + layout.margin.vertical());
            ui_index += 1;
        }
        let index = child_index?;
        let h_spacing = ui_h_spacing_amount(auto.h_spacing, content.size.x);
        let v_spacing = ui_v_spacing_amount(auto.v_spacing, content.size.y);
        let used_width = cell_width * used_columns as f32 + h_spacing * (used_columns - 1) as f32;
        let used_height = cell_height * row_count as f32 + v_spacing * (row_count - 1) as f32;
        let (layout, transform) = self
            .nodes
            .get(child)
            .and_then(|node| ui_root_from_data(&node.data))
            .and_then(|ui| ui.visible.then_some((&ui.layout, &ui.transform)))?;
        let col = index % columns;
        let row = index / columns;
        let fill_size = Vector2::new(
            ui_fill_width(layout, parent_layout, cell_width),
            ui_fill_height(layout, parent_layout, cell_height),
        );
        let size = self.resolve_ui_size(
            child,
            Vector2::new(cell_width, cell_height),
            Some(fill_size),
        );
        let min = content.min();
        let max = content.max();
        let grid_min_x = align_h_start(min.x, content.size.x, used_width, parent_layout.h_align);
        let grid_top_y = align_v_top(max.y, content.size.y, used_height, parent_layout.v_align);
        let cell_min_x = grid_min_x + col as f32 * (cell_width + h_spacing);
        let cell_top_y = grid_top_y - row as f32 * (cell_height + v_spacing);
        let center =
            Vector2::new(
                align_h_center(
                    cell_min_x,
                    cell_width,
                    size.x,
                    layout.margin,
                    parent_layout.h_align,
                ),
                align_v_center(
                    cell_top_y,
                    cell_height,
                    size.y,
                    layout.margin,
                    parent_layout.v_align,
                ),
            ) + ui_translation_offset(transform, Vector2::new(cell_width, cell_height), size);
        Some(ComputedUiRect::new(center, size))
    }

    pub(super) fn compute_ui_grid_children_rects(
        &self,
        parent_layout: &UiLayoutData,
        children: &[NodeID],
        layout_ctx: UiChildrenLayoutCtx,
        auto: UiAutoLayout,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &mut AHashMap<NodeID, Vector2>,
    ) {
        let UiChildrenLayoutCtx {
            parent_layout_rect,
            content,
            parent_scale,
        } = layout_ctx;
        let columns = auto.columns.max(1) as usize;
        let mut ui_count = 0_usize;
        let mut cell_width = 0.0_f32;
        let mut cell_height = 0.0_f32;
        for sibling in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(sibling)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some(&ui.layout))
            else {
                continue;
            };
            let size = self.resolve_ui_size(sibling, content.size, None);
            cell_width = cell_width.max(size.x + layout.margin.horizontal());
            cell_height = cell_height.max(size.y + layout.margin.vertical());
            ui_count += 1;
        }
        if ui_count == 0 {
            return;
        }

        let used_columns = columns.min(ui_count);
        let row_count = ui_count.div_ceil(columns);
        let h_spacing = ui_h_spacing_amount(auto.h_spacing, content.size.x);
        let v_spacing = ui_v_spacing_amount(auto.v_spacing, content.size.y);
        let used_width = cell_width * used_columns as f32 + h_spacing * (used_columns - 1) as f32;
        let used_height = cell_height * row_count as f32 + v_spacing * (row_count - 1) as f32;
        let min = content.min();
        let max = content.max();
        let grid_min_x = align_h_start(min.x, content.size.x, used_width, parent_layout.h_align);
        let grid_top_y = align_v_top(max.y, content.size.y, used_height, parent_layout.v_align);

        let mut index = 0_usize;
        for child in children.iter().copied() {
            let Some((layout, transform)) = self
                .nodes
                .get(child)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some((&ui.layout, &ui.transform)))
            else {
                continue;
            };
            let col = index % columns;
            let row = index / columns;
            let fill_size = Vector2::new(
                ui_fill_width(layout, parent_layout, cell_width),
                ui_fill_height(layout, parent_layout, cell_height),
            );
            let size = self.resolve_ui_size(
                child,
                Vector2::new(cell_width, cell_height),
                Some(fill_size),
            );
            let cell_min_x = grid_min_x + col as f32 * (cell_width + h_spacing);
            let cell_top_y = grid_top_y - row as f32 * (cell_height + v_spacing);
            let center =
                Vector2::new(
                    align_h_center(
                        cell_min_x,
                        cell_width,
                        size.x,
                        layout.margin,
                        parent_layout.h_align,
                    ),
                    align_v_center(
                        cell_top_y,
                        cell_height,
                        size.y,
                        layout.margin,
                        parent_layout.v_align,
                    ),
                ) + ui_translation_offset(transform, Vector2::new(cell_width, cell_height), size);
            insert_scaled_ui_child_rect(
                computed,
                computed_scales,
                parent_layout_rect,
                parent_scale,
                child,
                ComputedUiRect::new(center, size),
                transform.scale,
            );
            index += 1;
        }
    }
}

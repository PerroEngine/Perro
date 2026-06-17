use super::*;

impl Runtime {
    pub(super) fn ui_effective_z(&self, node: NodeID) -> i32 {
        let mut cur = node;
        let mut z_sum = 0_i64;
        let mut ui_depth = 0_i64;
        let mut guard = 0_u32;
        while !cur.is_nil() && guard < 4096 {
            guard += 1;
            let Some(scene_node) = self.nodes.get(cur) else {
                break;
            };
            if let Some(ui) = ui_root_from_data(&scene_node.data) {
                z_sum = z_sum.saturating_add(ui.layout.z_index as i64);
                ui_depth = ui_depth.saturating_add(1);
            }
            cur = scene_node.parent;
        }
        z_sum
            .saturating_mul(4096)
            .saturating_add(ui_depth.saturating_sub(1))
            .clamp(i32::MIN as i64, i32::MAX as i64) as i32
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
                    auto_layout.h_spacing_mode,
                ),
                UiLayoutMode::V => self.compute_ui_v_child_rect(
                    &parent_ui.layout,
                    &layout_children,
                    child,
                    content_rect,
                    auto_layout.v_spacing,
                    auto_layout.v_spacing_mode,
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
        spacing_mode: UiLayoutSpacingMode,
    ) -> Option<ComputedUiRect> {
        let spacing = self.h_layout_spacing(children, content.size, spacing, spacing_mode);
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
        spacing: UiAxisLayoutSpacing,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &mut AHashMap<NodeID, Vector2>,
    ) {
        let UiChildrenLayoutCtx { content, .. } = layout_ctx;
        let spacing = self.h_layout_spacing(children, content.size, spacing.amount, spacing.mode);
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
                layout_ctx,
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
        spacing_mode: UiLayoutSpacingMode,
    ) -> Option<ComputedUiRect> {
        let spacing = self.v_layout_spacing(children, content.size, spacing, spacing_mode);
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
        spacing: UiAxisLayoutSpacing,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &mut AHashMap<NodeID, Vector2>,
    ) {
        let UiChildrenLayoutCtx { content, .. } = layout_ctx;
        let spacing = self.v_layout_spacing(children, content.size, spacing.amount, spacing.mode);
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
                layout_ctx,
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
        let h_spacing = self.grid_h_spacing(
            content.size.x,
            cell_width,
            used_columns,
            auto.h_spacing,
            auto.h_spacing_mode,
        );
        let v_spacing = self.grid_v_spacing(
            content.size.y,
            cell_height,
            row_count,
            auto.v_spacing,
            auto.v_spacing_mode,
        );
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
        let UiChildrenLayoutCtx { content, .. } = layout_ctx;
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
        let h_spacing = self.grid_h_spacing(
            content.size.x,
            cell_width,
            used_columns,
            auto.h_spacing,
            auto.h_spacing_mode,
        );
        let v_spacing = self.grid_v_spacing(
            content.size.y,
            cell_height,
            row_count,
            auto.v_spacing,
            auto.v_spacing_mode,
        );
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
                layout_ctx,
                child,
                ComputedUiRect::new(center, size),
                transform.scale,
            );
            index += 1;
        }
    }

    fn h_layout_spacing(
        &self,
        children: &[NodeID],
        available: Vector2,
        spacing: f32,
        spacing_mode: UiLayoutSpacingMode,
    ) -> f32 {
        if spacing_mode != UiLayoutSpacingMode::Fill {
            return ui_h_spacing_amount(spacing, available.x);
        }
        let fill_width = self.h_fill_width(children, available.x, 0.0);
        let used_width = self.h_used_width(children, available, 0.0, fill_width);
        fill_spacing_amount(
            available.x,
            used_width,
            self.visible_ui_child_count(children),
        )
    }

    fn v_layout_spacing(
        &self,
        children: &[NodeID],
        available: Vector2,
        spacing: f32,
        spacing_mode: UiLayoutSpacingMode,
    ) -> f32 {
        if spacing_mode != UiLayoutSpacingMode::Fill {
            return ui_v_spacing_amount(spacing, available.y);
        }
        let fill_height = self.v_fill_height(children, available.y, 0.0);
        let used_height = self.v_used_height(children, available, 0.0, fill_height);
        fill_spacing_amount(
            available.y,
            used_height,
            self.visible_ui_child_count(children),
        )
    }

    fn grid_h_spacing(
        &self,
        width: f32,
        cell_width: f32,
        used_columns: usize,
        spacing: f32,
        spacing_mode: UiLayoutSpacingMode,
    ) -> f32 {
        if spacing_mode != UiLayoutSpacingMode::Fill {
            return ui_h_spacing_amount(spacing, width);
        }
        fill_spacing_amount(width, cell_width * used_columns as f32, used_columns)
    }

    fn grid_v_spacing(
        &self,
        height: f32,
        cell_height: f32,
        row_count: usize,
        spacing: f32,
        spacing_mode: UiLayoutSpacingMode,
    ) -> f32 {
        if spacing_mode != UiLayoutSpacingMode::Fill {
            return ui_v_spacing_amount(spacing, height);
        }
        fill_spacing_amount(height, cell_height * row_count as f32, row_count)
    }

    fn visible_ui_child_count(&self, children: &[NodeID]) -> usize {
        children
            .iter()
            .filter(|&&node| {
                self.nodes
                    .get(node)
                    .and_then(|node| ui_root_from_data(&node.data))
                    .is_some_and(|ui| ui.visible)
            })
            .count()
    }
}

fn fill_spacing_amount(axis: f32, used: f32, count: usize) -> f32 {
    if count <= 1 {
        0.0
    } else {
        ((axis - used) / (count - 1) as f32).max(0.0)
    }
}

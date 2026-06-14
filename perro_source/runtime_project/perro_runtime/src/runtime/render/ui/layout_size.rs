use super::*;

impl Runtime {
    pub(super) fn ui_tree_list_rows(&self, list: &perro_ui::UiTreeList) -> Vec<UiTreeListRow> {
        list.visible_items()
            .into_iter()
            .map(|item| UiTreeListRow {
                index: item.index,
                depth: item.depth,
                has_children: item.has_children,
                last_child: item.last_child,
            })
            .collect()
    }

    pub(super) fn is_effectively_visible_for_ui(&self, node: NodeID) -> bool {
        self.is_effectively_visible(node)
    }

    pub(super) fn ui_effective_clip_rect_screen(
        &self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        viewport: Vector2,
    ) -> [f32; 4] {
        let mut clip = viewport_clip_rect(viewport);
        let mut current = Some(node);
        while let Some(id) = current {
            let Some(scene_node) = self.nodes.get(id) else {
                break;
            };
            if ui_root_from_data(&scene_node.data).is_some_and(|ui| ui.clip_children)
                && let Some(rect) = computed.get(&id).copied()
            {
                clip = intersect_clip_rect(clip, rect_to_screen_clip(rect, viewport));
            }
            current = (!scene_node.parent.is_nil()).then_some(scene_node.parent);
        }
        clip
    }

    pub(super) fn resolve_ui_size(
        &self,
        node: NodeID,
        available: Vector2,
        fill_size: Option<Vector2>,
    ) -> Vector2 {
        let Some(scene_node) = self.nodes.get(node) else {
            return Vector2::ZERO;
        };
        let Some(ui) = ui_root_from_data(&scene_node.data) else {
            return Vector2::ZERO;
        };
        if !ui.visible {
            return Vector2::ZERO;
        }
        let layout = ui.layout;
        let transform = ui.transform;
        let mut size = layout.size.resolve(available);
        if ui.layout.h_size == UiSizeMode::FitChildren
            || ui.layout.v_size == UiSizeMode::FitChildren
        {
            let fit = self.fit_children_size(node, available);
            if layout.h_size == UiSizeMode::FitChildren {
                size.x = fit.x;
            }
            if layout.v_size == UiSizeMode::FitChildren {
                size.y = fit.y;
            }
        }
        if let Some(fill) = fill_size {
            if layout.h_size == UiSizeMode::Fill {
                size.x = fill.x;
            }
            if layout.v_size == UiSizeMode::Fill {
                size.y = fill.y;
            }
        }
        let baseline_size = {
            let mut baselines = self.render_ui.size_clamp_baselines.borrow_mut();
            let baseline = baselines
                .entry(node)
                .and_modify(|baseline| {
                    if baseline.size_def != layout.size
                        || baseline.h_mode != layout.h_size
                        || baseline.v_mode != layout.v_size
                    {
                        baseline.size = size;
                        baseline.size_def = layout.size;
                        baseline.h_mode = layout.h_size;
                        baseline.v_mode = layout.v_size;
                    }
                })
                .or_insert_with(|| super::super::state::UiSizeClampBaseline {
                    size,
                    size_def: layout.size,
                    h_mode: layout.h_size,
                    v_mode: layout.v_size,
                });
            baseline.size
        };
        let min_size = Vector2::new(
            layout
                .min_size
                .x
                .max(baseline_size.x * layout.min_size_scale.x.max(0.0)),
            layout
                .min_size
                .y
                .max(baseline_size.y * layout.min_size_scale.y.max(0.0)),
        );
        let max_x_scale = if layout.max_size_scale.x.is_finite() {
            layout.max_size_scale.x.max(0.0)
        } else {
            f32::INFINITY
        };
        let max_y_scale = if layout.max_size_scale.y.is_finite() {
            layout.max_size_scale.y.max(0.0)
        } else {
            f32::INFINITY
        };
        let max_size = Vector2::new(
            layout.max_size.x.min(baseline_size.x * max_x_scale),
            layout.max_size.y.min(baseline_size.y * max_y_scale),
        );
        size = Vector2::new(
            size.x.clamp(min_size.x, max_size.x.max(min_size.x)),
            size.y.clamp(min_size.y, max_size.y.max(min_size.y)),
        );
        transform.scale_size(size)
    }

    pub(super) fn fit_children_size(&self, node: NodeID, available: Vector2) -> Vector2 {
        let Some(scene_node) = self.nodes.get(node) else {
            return Vector2::ZERO;
        };
        let Some(ui) = ui_root_from_data(&scene_node.data) else {
            return Vector2::ZERO;
        };
        let text = ui_text_measure(&scene_node.data);
        let children = scene_node.get_children_ids();
        let child_size = match &scene_node.data {
            SceneNodeData::UiTreeList(list) => self.ui_tree_list_content_size(list, available),
            _ if ui_auto_layout_from_data(&scene_node.data).is_some() => self
                .auto_layout_content_size(
                    children,
                    available,
                    ui_auto_layout_from_data(&scene_node.data).unwrap(),
                ),
            _ => self.absolute_children_content_size(children, available),
        };
        let content = text.x.max(child_size.x);
        let content_h = text.y.max(child_size.y);
        Vector2::new(
            fit_size_with_padding_ratio(content, ui.layout.padding.left, ui.layout.padding.right),
            fit_size_with_padding_ratio(content_h, ui.layout.padding.top, ui.layout.padding.bottom),
        )
    }

    pub(super) fn auto_layout_content_size(
        &self,
        children: &[NodeID],
        available: Vector2,
        auto: UiAutoLayout,
    ) -> Vector2 {
        match auto.mode {
            UiLayoutMode::H => {
                let h_spacing = ui_h_spacing_amount(auto.h_spacing, available.x);
                let mut width = 0.0_f32;
                let mut height = 0.0_f32;
                let mut count = 0_u32;
                for child in children.iter().copied() {
                    let Some(layout) = self
                        .nodes
                        .get(child)
                        .and_then(|node| ui_root_from_data(&node.data))
                        .and_then(|ui| ui.visible.then_some(&ui.layout))
                    else {
                        continue;
                    };
                    let size = self.resolve_ui_size(child, available, None);
                    width += size.x + layout.margin.horizontal();
                    height = height.max(size.y + layout.margin.vertical());
                    count += 1;
                }
                if count > 1 {
                    width += h_spacing * (count - 1) as f32;
                }
                Vector2::new(width, height)
            }
            UiLayoutMode::V => {
                let v_spacing = ui_v_spacing_amount(auto.v_spacing, available.y);
                let mut width = 0.0_f32;
                let mut height = 0.0_f32;
                let mut count = 0_u32;
                for child in children.iter().copied() {
                    let Some(layout) = self
                        .nodes
                        .get(child)
                        .and_then(|node| ui_root_from_data(&node.data))
                        .and_then(|ui| ui.visible.then_some(&ui.layout))
                    else {
                        continue;
                    };
                    let size = self.resolve_ui_size(child, available, None);
                    width = width.max(size.x + layout.margin.horizontal());
                    height += size.y + layout.margin.vertical();
                    count += 1;
                }
                if count > 1 {
                    height += v_spacing * (count - 1) as f32;
                }
                Vector2::new(width, height)
            }
            UiLayoutMode::Grid => {
                let columns = auto.columns.max(1);
                let h_spacing = ui_h_spacing_amount(auto.h_spacing, available.x);
                let v_spacing = ui_v_spacing_amount(auto.v_spacing, available.y);
                let mut width = 0.0_f32;
                let mut row_width = 0.0_f32;
                let mut row_height = 0.0_f32;
                let mut total_height = 0.0_f32;
                let mut col = 0_u32;
                let mut rows = 0_u32;
                for child in children.iter().copied() {
                    let Some(layout) = self
                        .nodes
                        .get(child)
                        .and_then(|node| ui_root_from_data(&node.data))
                        .and_then(|ui| ui.visible.then_some(&ui.layout))
                    else {
                        continue;
                    };
                    let size = self.resolve_ui_size(child, available, None);
                    if col > 0 {
                        row_width += h_spacing;
                    }
                    row_width += size.x + layout.margin.horizontal();
                    row_height = row_height.max(size.y + layout.margin.vertical());
                    col += 1;
                    if col >= columns {
                        width = width.max(row_width);
                        total_height += row_height;
                        rows += 1;
                        row_width = 0.0;
                        row_height = 0.0;
                        col = 0;
                    }
                }
                if col > 0 {
                    width = width.max(row_width);
                    total_height += row_height;
                    rows += 1;
                }
                if rows > 1 {
                    total_height += v_spacing * (rows - 1) as f32;
                }
                Vector2::new(width, total_height)
            }
        }
    }

    pub(super) fn absolute_children_content_size(
        &self,
        children: &[NodeID],
        available: Vector2,
    ) -> Vector2 {
        let mut size = Vector2::ZERO;
        for child in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(child)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some(&ui.layout))
            else {
                continue;
            };
            let child_size = self.resolve_ui_size(child, available, None);
            size.x = size.x.max(child_size.x + layout.margin.horizontal());
            size.y = size.y.max(child_size.y + layout.margin.vertical());
        }
        size
    }

    pub(super) fn ui_tree_list_content_size(
        &self,
        list: &perro_ui::UiTreeList,
        available: Vector2,
    ) -> Vector2 {
        let rows = self.ui_tree_list_rows(list);
        if rows.is_empty() {
            return Vector2::ZERO;
        }
        let max_depth = rows.iter().fold(0, |max_depth, row| {
            let _ = (row.index, row.has_children, row.last_child);
            max_depth.max(row.depth)
        });
        let height = rows.len() as f32 * list.row_height
            + rows.len().saturating_sub(1) as f32
                * ui_v_spacing_amount(list.v_spacing, available.y);
        let width = available
            .x
            .max(list.indent * max_depth as f32 + list.toggle_size + list.icon_size + 96.0);
        Vector2::new(width, height)
    }

    pub(super) fn h_fill_width(&self, children: &[NodeID], width: f32, spacing: f32) -> f32 {
        let mut fixed = 0.0_f32;
        let mut fill_count = 0_u32;
        let mut ui_count = 0_u32;
        for child in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(child)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some(&ui.layout))
            else {
                continue;
            };
            ui_count += 1;
            fixed += layout.margin.horizontal();
            if layout.h_size == UiSizeMode::Fill {
                fill_count += 1;
            } else {
                fixed += self
                    .resolve_ui_size(child, Vector2::new(width, 0.0), None)
                    .x;
            }
        }
        if ui_count > 1 {
            fixed += spacing * (ui_count - 1) as f32;
        }
        if fill_count == 0 {
            0.0
        } else {
            ((width - fixed) / fill_count as f32).max(0.0)
        }
    }

    pub(super) fn h_used_width(
        &self,
        children: &[NodeID],
        available: Vector2,
        spacing: f32,
        fill_width: f32,
    ) -> f32 {
        let mut width = 0.0_f32;
        let mut count = 0_u32;
        for child in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(child)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some(&ui.layout))
            else {
                continue;
            };
            let fill_size = Vector2::new(
                if layout.h_size == UiSizeMode::Fill {
                    fill_width
                } else {
                    0.0
                },
                0.0,
            );
            width += self.resolve_ui_size(child, available, Some(fill_size)).x
                + layout.margin.horizontal();
            count += 1;
        }
        if count > 1 {
            width += spacing * (count - 1) as f32;
        }
        width
    }

    pub(super) fn v_fill_height(&self, children: &[NodeID], height: f32, spacing: f32) -> f32 {
        let mut fixed = 0.0_f32;
        let mut fill_count = 0_u32;
        let mut ui_count = 0_u32;
        for child in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(child)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some(&ui.layout))
            else {
                continue;
            };
            ui_count += 1;
            fixed += layout.margin.vertical();
            if layout.v_size == UiSizeMode::Fill {
                fill_count += 1;
            } else {
                fixed += self
                    .resolve_ui_size(child, Vector2::new(0.0, height), None)
                    .y;
            }
        }
        if ui_count > 1 {
            fixed += spacing * (ui_count - 1) as f32;
        }
        if fill_count == 0 {
            0.0
        } else {
            ((height - fixed) / fill_count as f32).max(0.0)
        }
    }

    pub(super) fn v_used_height(
        &self,
        children: &[NodeID],
        available: Vector2,
        spacing: f32,
        fill_height: f32,
    ) -> f32 {
        let mut height = 0.0_f32;
        let mut count = 0_u32;
        for child in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(child)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some(&ui.layout))
            else {
                continue;
            };
            let fill_size = Vector2::new(
                0.0,
                if layout.v_size == UiSizeMode::Fill {
                    fill_height
                } else {
                    0.0
                },
            );
            height += self.resolve_ui_size(child, available, Some(fill_size)).y
                + layout.margin.vertical();
            count += 1;
        }
        if count > 1 {
            height += spacing * (count - 1) as f32;
        }
        height
    }
}

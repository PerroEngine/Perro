use super::Runtime;
use ahash::AHashMap;
use perro_ids::NodeID;
use perro_nodes::SceneNodeData;
use perro_render_bridge::{RenderCommand, UiCommand, UiRectState};
use perro_structs::Vector2;
use perro_ui::{
    ComputedUiRect, UiBox, UiHorizontalAlign, UiLayoutData, UiLayoutMode, UiSizeMode, UiStyle,
    UiVerticalAlign,
};
use std::borrow::Cow;

impl Runtime {
    pub fn extract_render_ui_commands(&mut self) {
        let viewport = self.input.viewport_size();
        let root_rect = ComputedUiRect::new(Vector2::ZERO, viewport);
        let mut computed = AHashMap::<NodeID, ComputedUiRect>::default();
        let node_ids: Vec<NodeID> = self.nodes.iter().map(|(id, _)| id).collect();

        self.queue_render_command(RenderCommand::Ui(UiCommand::Clear));
        for node in node_ids.iter().copied() {
            self.compute_ui_rect(node, root_rect, &mut computed);
        }

        for node in node_ids {
            let Some(rect) = computed.get(&node).copied() else {
                continue;
            };
            let effective_visible = self.is_effectively_visible(node);
            let Some(command) = self.nodes.get(node).and_then(|scene_node| {
                ui_command_from_node(node, &scene_node.data, rect, effective_visible)
            }) else {
                continue;
            };
            self.queue_render_command(RenderCommand::Ui(command));
        }
    }

    fn compute_ui_rect(
        &self,
        node: NodeID,
        root_rect: ComputedUiRect,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<ComputedUiRect> {
        if let Some(rect) = computed.get(&node).copied() {
            return Some(rect);
        }

        let scene_node = self.nodes.get(node)?;
        let ui_root = ui_root_from_data(&scene_node.data)?;
        let parent_rect = if scene_node.parent.is_nil() {
            root_rect
        } else {
            self.compute_ui_rect(scene_node.parent, root_rect, computed)
                .unwrap_or(root_rect)
        };
        let rect = if scene_node.parent.is_nil() {
            let size = self.resolve_ui_size(node, parent_rect.size, None);
            ui_root.layout.compute_rect_with_size(parent_rect, size)
        } else {
            self.compute_ui_child_rect(scene_node.parent, node, parent_rect, &ui_root.layout)
                .unwrap_or_else(|| {
                    let parent_content = self
                        .nodes
                        .get(scene_node.parent)
                        .and_then(|parent| ui_root_from_data(&parent.data))
                        .map(|parent| parent_rect.inset(parent.layout.padding))
                        .unwrap_or(parent_rect);
                    let parent_content = parent_content.inset(ui_root.layout.margin);
                    let size = self.resolve_ui_size(node, parent_content.size, None);
                    ui_root.layout.compute_rect_with_size(parent_content, size)
                })
        };
        computed.insert(node, rect);
        Some(rect)
    }

    fn compute_ui_child_rect(
        &self,
        parent: NodeID,
        child: NodeID,
        parent_rect: ComputedUiRect,
        child_layout: &UiLayoutData,
    ) -> Option<ComputedUiRect> {
        let parent_node = self.nodes.get(parent)?;
        let parent_ui = ui_root_from_data(&parent_node.data)?;
        let content_rect = parent_rect.inset(parent_ui.layout.padding);
        let auto_layout = ui_auto_layout_from_data(&parent_node.data)?;
        match auto_layout.mode {
            UiLayoutMode::H => self.compute_ui_h_child_rect(
                &parent_ui.layout,
                parent_node.get_children_ids(),
                child,
                content_rect,
                auto_layout.h_spacing,
            ),
            UiLayoutMode::V => self.compute_ui_v_child_rect(
                &parent_ui.layout,
                parent_node.get_children_ids(),
                child,
                content_rect,
                auto_layout.v_spacing,
            ),
            UiLayoutMode::Grid => self.compute_ui_grid_child_rect(
                &parent_ui.layout,
                parent_node.get_children_ids(),
                child,
                content_rect,
                auto_layout.columns,
                auto_layout.h_spacing,
                auto_layout.v_spacing,
            ),
        }
        .or_else(|| {
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
            Some(child_layout.compute_rect_with_size(child_content, size))
        })
    }

    fn compute_ui_h_child_rect(
        &self,
        parent_layout: &UiLayoutData,
        children: &[NodeID],
        child: NodeID,
        content: ComputedUiRect,
        spacing: f32,
    ) -> Option<ComputedUiRect> {
        let fill_width = self.h_fill_width(children, content.size.x, spacing);
        let used_width = self.h_used_width(children, content.size, spacing, fill_width);
        let min = content.min();
        let max = content.max();
        let mut x = align_h_start(min.x, content.size.x, used_width, parent_layout.h_align);
        for sibling in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(sibling)
                .and_then(|node| ui_root_from_data(&node.data))
                .map(|ui| &ui.layout)
            else {
                continue;
            };
            let fill_size = Vector2::new(
                if layout.h_size == UiSizeMode::Fill {
                    fill_width
                } else {
                    0.0
                },
                if layout.v_size == UiSizeMode::Fill {
                    (content.size.y - layout.margin.vertical()).max(0.0)
                } else if parent_layout.v_align == UiVerticalAlign::Fill {
                    (content.size.y - layout.margin.vertical()).max(0.0)
                } else {
                    0.0
                },
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
                let center =
                    Vector2::new(x + layout.margin.left + size.x * 0.5, y) + layout.translation;
                return Some(ComputedUiRect::new(center, size));
            }
            x += size.x + layout.margin.horizontal() + spacing;
        }
        None
    }

    fn compute_ui_v_child_rect(
        &self,
        parent_layout: &UiLayoutData,
        children: &[NodeID],
        child: NodeID,
        content: ComputedUiRect,
        spacing: f32,
    ) -> Option<ComputedUiRect> {
        let fill_height = self.v_fill_height(children, content.size.y, spacing);
        let used_height = self.v_used_height(children, content.size, spacing, fill_height);
        let min = content.min();
        let max = content.max();
        let mut y = align_v_top(max.y, content.size.y, used_height, parent_layout.v_align);
        for sibling in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(sibling)
                .and_then(|node| ui_root_from_data(&node.data))
                .map(|ui| &ui.layout)
            else {
                continue;
            };
            let fill_size = Vector2::new(
                if layout.h_size == UiSizeMode::Fill {
                    (content.size.x - layout.margin.horizontal()).max(0.0)
                } else if parent_layout.h_align == UiHorizontalAlign::Fill {
                    (content.size.x - layout.margin.horizontal()).max(0.0)
                } else {
                    0.0
                },
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
                let center =
                    Vector2::new(x, y - layout.margin.top - size.y * 0.5) + layout.translation;
                return Some(ComputedUiRect::new(center, size));
            }
            y -= size.y + layout.margin.vertical() + spacing;
        }
        None
    }

    fn compute_ui_grid_child_rect(
        &self,
        parent_layout: &UiLayoutData,
        children: &[NodeID],
        child: NodeID,
        content: ComputedUiRect,
        columns: u32,
        h_spacing: f32,
        v_spacing: f32,
    ) -> Option<ComputedUiRect> {
        let columns = columns.max(1) as usize;
        let cell_width = ((content.size.x - h_spacing * (columns.saturating_sub(1) as f32))
            / columns as f32)
            .max(0.0);
        let mut child_index = None;
        let mut max_row_height = 0.0_f32;
        let mut ui_index = 0_usize;
        for sibling in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(sibling)
                .and_then(|node| ui_root_from_data(&node.data))
                .map(|ui| &ui.layout)
            else {
                continue;
            };
            if sibling == child {
                child_index = Some(ui_index);
            }
            let fill_size = Vector2::new(
                if layout.h_size == UiSizeMode::Fill {
                    (cell_width - layout.margin.horizontal()).max(0.0)
                } else if parent_layout.h_align == UiHorizontalAlign::Fill {
                    (cell_width - layout.margin.horizontal()).max(0.0)
                } else {
                    0.0
                },
                0.0,
            );
            let size = self.resolve_ui_size(
                sibling,
                Vector2::new(cell_width, content.size.y),
                Some(fill_size),
            );
            max_row_height = max_row_height.max(size.y + layout.margin.vertical());
            ui_index += 1;
        }
        let index = child_index?;
        let layout = self
            .nodes
            .get(child)
            .and_then(|node| ui_root_from_data(&node.data))
            .map(|ui| &ui.layout)?;
        let fill_size = Vector2::new(
            if layout.h_size == UiSizeMode::Fill {
                (cell_width - layout.margin.horizontal()).max(0.0)
            } else if parent_layout.h_align == UiHorizontalAlign::Fill {
                (cell_width - layout.margin.horizontal()).max(0.0)
            } else {
                0.0
            },
            if layout.v_size == UiSizeMode::Fill {
                (max_row_height - layout.margin.vertical()).max(0.0)
            } else if parent_layout.v_align == UiVerticalAlign::Fill {
                (max_row_height - layout.margin.vertical()).max(0.0)
            } else {
                0.0
            },
        );
        let size = self.resolve_ui_size(
            child,
            Vector2::new(cell_width, content.size.y),
            Some(fill_size),
        );
        let col = index % columns;
        let row = index / columns;
        let min = content.min();
        let max = content.max();
        let cell_min_x = min.x + col as f32 * (cell_width + h_spacing);
        let cell_top_y = max.y - row as f32 * (max_row_height + v_spacing);
        let center = Vector2::new(
            align_h_center(
                cell_min_x,
                cell_width,
                size.x,
                layout.margin,
                parent_layout.h_align,
            ),
            align_v_center(
                cell_top_y,
                max_row_height,
                size.y,
                layout.margin,
                parent_layout.v_align,
            ),
        ) + layout.translation;
        Some(ComputedUiRect::new(center, size))
    }

    fn resolve_ui_size(
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
        let mut size = ui.layout.size.resolve(available);
        if ui.layout.h_size == UiSizeMode::FitChildren
            || ui.layout.v_size == UiSizeMode::FitChildren
        {
            let fit = self.fit_children_size(node, available);
            if ui.layout.h_size == UiSizeMode::FitChildren {
                size.x = fit.x;
            }
            if ui.layout.v_size == UiSizeMode::FitChildren {
                size.y = fit.y;
            }
        }
        if let Some(fill) = fill_size {
            if ui.layout.h_size == UiSizeMode::Fill {
                size.x = fill.x;
            }
            if ui.layout.v_size == UiSizeMode::Fill {
                size.y = fill.y;
            }
        }
        ui.layout.scale_size(ui.layout.clamp_size(size))
    }

    fn fit_children_size(&self, node: NodeID, available: Vector2) -> Vector2 {
        let Some(scene_node) = self.nodes.get(node) else {
            return Vector2::ZERO;
        };
        let Some(ui) = ui_root_from_data(&scene_node.data) else {
            return Vector2::ZERO;
        };
        let text = ui_text_measure(&scene_node.data);
        let children = scene_node.get_children_ids();
        let child_size = if let Some(auto) = ui_auto_layout_from_data(&scene_node.data) {
            self.auto_layout_content_size(children, available, auto)
        } else {
            self.absolute_children_content_size(children, available)
        };
        Vector2::new(
            text.x.max(child_size.x) + ui.layout.padding.horizontal(),
            text.y.max(child_size.y) + ui.layout.padding.vertical(),
        )
    }

    fn auto_layout_content_size(
        &self,
        children: &[NodeID],
        available: Vector2,
        auto: UiAutoLayout,
    ) -> Vector2 {
        match auto.mode {
            UiLayoutMode::H => {
                let mut width = 0.0_f32;
                let mut height = 0.0_f32;
                let mut count = 0_u32;
                for child in children.iter().copied() {
                    let Some(layout) = self
                        .nodes
                        .get(child)
                        .and_then(|node| ui_root_from_data(&node.data))
                        .map(|ui| &ui.layout)
                    else {
                        continue;
                    };
                    let size = self.resolve_ui_size(child, available, None);
                    width += size.x + layout.margin.horizontal();
                    height = height.max(size.y + layout.margin.vertical());
                    count += 1;
                }
                if count > 1 {
                    width += auto.h_spacing * (count - 1) as f32;
                }
                Vector2::new(width, height)
            }
            UiLayoutMode::V => {
                let mut width = 0.0_f32;
                let mut height = 0.0_f32;
                let mut count = 0_u32;
                for child in children.iter().copied() {
                    let Some(layout) = self
                        .nodes
                        .get(child)
                        .and_then(|node| ui_root_from_data(&node.data))
                        .map(|ui| &ui.layout)
                    else {
                        continue;
                    };
                    let size = self.resolve_ui_size(child, available, None);
                    width = width.max(size.x + layout.margin.horizontal());
                    height += size.y + layout.margin.vertical();
                    count += 1;
                }
                if count > 1 {
                    height += auto.v_spacing * (count - 1) as f32;
                }
                Vector2::new(width, height)
            }
            UiLayoutMode::Grid => {
                let columns = auto.columns.max(1);
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
                        .map(|ui| &ui.layout)
                    else {
                        continue;
                    };
                    let size = self.resolve_ui_size(child, available, None);
                    if col > 0 {
                        row_width += auto.h_spacing;
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
                    total_height += auto.v_spacing * (rows - 1) as f32;
                }
                Vector2::new(width, total_height)
            }
        }
    }

    fn absolute_children_content_size(&self, children: &[NodeID], available: Vector2) -> Vector2 {
        let mut size = Vector2::ZERO;
        for child in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(child)
                .and_then(|node| ui_root_from_data(&node.data))
                .map(|ui| &ui.layout)
            else {
                continue;
            };
            let child_size = self.resolve_ui_size(child, available, None);
            size.x = size.x.max(child_size.x + layout.margin.horizontal());
            size.y = size.y.max(child_size.y + layout.margin.vertical());
        }
        size
    }

    fn h_fill_width(&self, children: &[NodeID], width: f32, spacing: f32) -> f32 {
        let mut fixed = 0.0_f32;
        let mut fill_count = 0_u32;
        let mut ui_count = 0_u32;
        for child in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(child)
                .and_then(|node| ui_root_from_data(&node.data))
                .map(|ui| &ui.layout)
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

    fn h_used_width(
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
                .map(|ui| &ui.layout)
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

    fn v_fill_height(&self, children: &[NodeID], height: f32, spacing: f32) -> f32 {
        let mut fixed = 0.0_f32;
        let mut fill_count = 0_u32;
        let mut ui_count = 0_u32;
        for child in children.iter().copied() {
            let Some(layout) = self
                .nodes
                .get(child)
                .and_then(|node| ui_root_from_data(&node.data))
                .map(|ui| &ui.layout)
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

    fn v_used_height(
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
                .map(|ui| &ui.layout)
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

#[derive(Clone, Copy)]
struct UiAutoLayout {
    mode: UiLayoutMode,
    columns: u32,
    h_spacing: f32,
    v_spacing: f32,
}

fn ui_root_from_data(data: &SceneNodeData) -> Option<&UiBox> {
    match data {
        SceneNodeData::UiBox(root) => Some(root),
        SceneNodeData::UiPanel(node) => Some(&node.base),
        SceneNodeData::UiButton(node) => Some(&node.base),
        SceneNodeData::UiLabel(node) => Some(&node.base),
        SceneNodeData::UiLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiHLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiVLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiGrid(node) => Some(&node.base),
        _ => None,
    }
}

fn ui_auto_layout_from_data(data: &SceneNodeData) -> Option<UiAutoLayout> {
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

fn ui_text_measure(data: &SceneNodeData) -> Vector2 {
    match data {
        SceneNodeData::UiLabel(label) => measure_text(label.text.as_ref(), label.font_size),
        SceneNodeData::UiButton(button) => {
            let text = measure_text(button.text.as_ref(), 16.0);
            Vector2::new(text.x + 24.0, text.y + 12.0)
        }
        _ => Vector2::ZERO,
    }
}

fn measure_text(text: &str, font_size: f32) -> Vector2 {
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

fn align_h_start(min_x: f32, available: f32, used: f32, align: UiHorizontalAlign) -> f32 {
    match align {
        UiHorizontalAlign::Left | UiHorizontalAlign::Fill => min_x,
        UiHorizontalAlign::Center => min_x + (available - used).max(0.0) * 0.5,
        UiHorizontalAlign::Right => min_x + (available - used).max(0.0),
    }
}

fn align_v_top(max_y: f32, available: f32, used: f32, align: UiVerticalAlign) -> f32 {
    match align {
        UiVerticalAlign::Top | UiVerticalAlign::Fill => max_y,
        UiVerticalAlign::Center => max_y - (available - used).max(0.0) * 0.5,
        UiVerticalAlign::Bottom => max_y - (available - used).max(0.0),
    }
}

fn align_h_center(
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

fn align_v_center(
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

fn ui_command_from_node(
    node: NodeID,
    data: &SceneNodeData,
    rect: ComputedUiRect,
    effective_visible: bool,
) -> Option<UiCommand> {
    if !effective_visible {
        return None;
    }

    let rect = UiRectState {
        center: [rect.center.x, rect.center.y],
        size: [rect.size.x, rect.size.y],
        z_index: ui_root_from_data(data)?.layout.z_index,
    };
    match data {
        SceneNodeData::UiPanel(panel) => Some(panel_command(node, rect, &panel.style)),
        SceneNodeData::UiButton(button) => {
            let style = if button.disabled {
                button.style.clone()
            } else {
                button.style.clone()
            };
            Some(UiCommand::UpsertButton {
                node,
                rect,
                text: Cow::Owned(button.text.to_string()),
                text_color: button.text_color.to_rgba(),
                fill: style.fill.to_rgba(),
                stroke: style.stroke.to_rgba(),
                stroke_width: style.stroke_width,
                corner_radius: style.corner_radius,
                disabled: button.disabled,
            })
        }
        SceneNodeData::UiLabel(label) => Some(UiCommand::UpsertLabel {
            node,
            rect,
            text: Cow::Owned(label.text.to_string()),
            color: label.color.to_rgba(),
            font_size: label.font_size,
        }),
        _ => None,
    }
}

fn panel_command(node: NodeID, rect: UiRectState, style: &UiStyle) -> UiCommand {
    UiCommand::UpsertPanel {
        node,
        rect,
        fill: style.fill.to_rgba(),
        stroke: style.stroke.to_rgba(),
        stroke_width: style.stroke_width,
        corner_radius: style.corner_radius,
    }
}

use super::*;

impl Runtime {
    pub(super) fn compute_ui_rect(
        &self,
        node: NodeID,
        root_rect: ComputedUiRect,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &mut AHashMap<NodeID, Vector2>,
        auto_layout_computed: &mut ahash::AHashSet<NodeID>,
    ) -> Option<ComputedUiRect> {
        if let Some(rect) = computed.get(&node).copied() {
            return Some(rect);
        }

        let scene_node = self.nodes.get(node)?;
        let ui_root = ui_root_from_data(&scene_node.data)?;
        if !matches!(scene_node.data, SceneNodeData::UiTreeList(_))
            && let Some(tree_parent) = self.ui_tree_owner(node)
        {
            self.compute_ui_rect(
                tree_parent,
                root_rect,
                computed,
                computed_scales,
                auto_layout_computed,
            )?;
            return computed.get(&node).copied();
        }
        let (ui_parent, parent_rect) = self.resolve_ui_parent_rect(
            scene_node.parent,
            root_rect,
            computed,
            computed_scales,
            auto_layout_computed,
        );
        let rect = if scene_node.parent.is_nil() {
            let size = self.resolve_ui_size(node, parent_rect.size, None);
            let rect = ui_root
                .layout
                .compute_rect_with_size(&ui_root.transform, parent_rect, size);
            computed_scales.insert(node, ui_root.transform.scale);
            rect
        } else {
            let parent_scale = ui_parent
                .and_then(|id| computed_scales.get(&id).copied())
                .unwrap_or(Vector2::ONE);
            let parent_layout_rect = ComputedUiRect::new(
                parent_rect.center,
                parent_rect.size / safe_ui_scale(parent_scale),
            );
            if ui_parent
                .and_then(|id| {
                    self.nodes
                        .get(id)
                        .and_then(|parent| ui_auto_layout_from_data(&parent.data))
                })
                .is_some()
            {
                let ui_parent_id = ui_parent.unwrap_or(scene_node.parent);
                if auto_layout_computed.insert(ui_parent_id) {
                    self.compute_ui_auto_children_rects(
                        ui_parent_id,
                        parent_rect,
                        parent_scale,
                        parent_layout_rect,
                        computed,
                        computed_scales,
                    );
                }
                if let Some(rect) = computed.get(&node).copied() {
                    return Some(rect);
                }
            }
            let child_layout_rect = self
                .compute_ui_child_rect(
                    ui_parent.unwrap_or(scene_node.parent),
                    node,
                    parent_layout_rect,
                    &ui_root.layout,
                    &ui_root.transform,
                )
                .unwrap_or_else(|| {
                    let parent_content = ui_parent
                        .and_then(|id| self.nodes.get(id))
                        .and_then(|parent| ui_root_from_data(&parent.data))
                        .map(|parent| {
                            parent_layout_rect
                                .inset(ui_padding_inset(parent_layout_rect, parent.layout.padding))
                        })
                        .unwrap_or(parent_layout_rect);
                    let parent_content = parent_content.inset(ui_root.layout.margin);
                    let size = self.resolve_ui_size(node, parent_content.size, None);
                    ui_root
                        .layout
                        .compute_rect_with_size(&ui_root.transform, parent_content, size)
                });
            let rect =
                scale_ui_rect_from_parent(child_layout_rect, parent_layout_rect, parent_scale);
            computed_scales.insert(node, parent_scale * ui_root.transform.scale);
            rect
        };
        computed.insert(node, rect);
        if let SceneNodeData::UiTreeList(tree) = &scene_node.data {
            self.compute_ui_tree_rows(tree, rect, computed);
        }
        Some(rect)
    }

    pub(super) fn ui_virtual_font_scale(&self, viewport: Vector2) -> f32 {
        let (vw, vh) = self
            .project()
            .map(|project| {
                (
                    project.config.virtual_width.max(1) as f32,
                    project.config.virtual_height.max(1) as f32,
                )
            })
            .unwrap_or((viewport.x.max(1.0), viewport.y.max(1.0)));
        let sx = viewport.x.max(1.0) / vw;
        let sy = viewport.y.max(1.0) / vh;
        sx.min(sy).max(0.0001)
    }

    pub(super) fn resolve_ui_parent_rect(
        &self,
        mut parent: NodeID,
        root_rect: ComputedUiRect,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &mut AHashMap<NodeID, Vector2>,
        auto_layout_computed: &mut ahash::AHashSet<NodeID>,
    ) -> (Option<NodeID>, ComputedUiRect) {
        while !parent.is_nil() {
            let Some(parent_node) = self.nodes.get(parent) else {
                break;
            };
            if ui_root_from_data(&parent_node.data).is_some() {
                let rect = self
                    .compute_ui_rect(
                        parent,
                        root_rect,
                        computed,
                        computed_scales,
                        auto_layout_computed,
                    )
                    .unwrap_or(root_rect);
                return (Some(parent), rect);
            }
            parent = parent_node.parent;
        }
        (None, root_rect)
    }

    pub(super) fn closest_ui_parent(&self, mut parent: NodeID) -> Option<NodeID> {
        while !parent.is_nil() {
            let parent_node = self.nodes.get(parent)?;
            if ui_root_from_data(&parent_node.data).is_some() {
                return Some(parent);
            }
            parent = parent_node.parent;
        }
        None
    }

    pub(super) fn ui_layout_children(&self, parent: NodeID) -> Vec<NodeID> {
        let mut out = Vec::new();
        let Some(parent_node) = self.nodes.get(parent) else {
            return out;
        };
        let mut stack: Vec<NodeID> = parent_node.get_children_ids().iter().rev().copied().collect();
        while let Some(node_id) = stack.pop() {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            if ui_root_from_data(&node.data).is_some() {
                out.push(node_id);
                continue;
            }
            stack.extend(node.get_children_ids().iter().rev().copied());
        }
        out
    }

    pub(super) fn compute_ui_auto_children_rects(
        &self,
        parent: NodeID,
        _parent_rect: ComputedUiRect,
        parent_scale: Vector2,
        parent_layout_rect: ComputedUiRect,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &mut AHashMap<NodeID, Vector2>,
    ) -> Option<()> {
        let parent_node = self.nodes.get(parent)?;
        let parent_ui = ui_root_from_data(&parent_node.data)?;
        let auto_layout = ui_auto_layout_from_data(&parent_node.data)?;
        let layout_children = self.ui_layout_children(parent);
        let content_rect = ui_scroll_content_rect(
            &parent_node.data,
            parent_layout_rect.inset(ui_padding_inset(
                parent_layout_rect,
                parent_ui.layout.padding,
            )),
        );
        let layout_ctx = UiChildrenLayoutCtx {
            parent_layout_rect,
            content: content_rect,
            parent_scale,
        };
        match auto_layout.mode {
            UiLayoutMode::H => self.compute_ui_h_children_rects(
                &parent_ui.layout,
                &layout_children,
                layout_ctx,
                auto_layout.h_spacing,
                computed,
                computed_scales,
            ),
            UiLayoutMode::V => self.compute_ui_v_children_rects(
                &parent_ui.layout,
                &layout_children,
                layout_ctx,
                auto_layout.v_spacing,
                computed,
                computed_scales,
            ),
            UiLayoutMode::Grid => self.compute_ui_grid_children_rects(
                &parent_ui.layout,
                &layout_children,
                layout_ctx,
                auto_layout,
                computed,
                computed_scales,
            ),
        }
        Some(())
    }

    pub(super) fn remove_retained_ui_node(&mut self, node: NodeID) {
        self.render_ui.retained_rects.remove(&node);
        self.render_ui.button_states.remove(&node);
        if self.render_ui.hovered_text_edit == Some(node) {
            self.render_ui.hovered_text_edit = None;
        }
        if self.render_ui.focused_text_edit == Some(node) {
            self.render_ui.focused_text_edit = None;
        }
        if self.render_ui.focused_ui_node == Some(node) {
            self.render_ui.focused_ui_node = None;
        }
        if self.render_ui.nav_pressed_button == Some(node) {
            self.render_ui.nav_pressed_button = None;
        }
        if self.render_ui.pressed_text_edit == Some(node) {
            self.render_ui.pressed_text_edit = None;
        }
        if self.render_ui.retained_commands.remove(&node).is_some() {
            self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode { node }));
        }
    }

    pub(super) fn remove_no_longer_visible_ui_nodes(
        &mut self,
        visible_now: &ahash::AHashSet<NodeID>,
    ) {
        let mut to_remove = Vec::new();
        for node in self.render_ui.prev_visible.iter().copied() {
            if !visible_now.contains(&node) {
                to_remove.push(node);
            }
        }
        for node in to_remove {
            self.remove_retained_ui_node(node);
        }
    }
}

use super::*;

impl Runtime {
    pub(super) fn ui_pixel_snapping_enabled(&self) -> bool {
        self.project()
            .map(|project| project.config.rendering.ui.pixel_snapping)
            .unwrap_or(false)
    }

    pub(super) fn ui_pixel_snap_scale_factor(&self) -> f32 {
        1.0
    }

    pub(crate) fn compute_ui_rect(
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
        let (ui_parent, parent_rect) = self.resolve_ui_parent_rect(
            scene_node.parent,
            root_rect,
            computed,
            computed_scales,
            auto_layout_computed,
        );
        let rect = if scene_node.parent.is_nil() {
            // Layout root: anchors/positions resolve against the real root
            // rect, but percent SIZES resolve against the aspect-fit virtual
            // canvas so authored proportions survive non-16:9 windows.
            let size_basis = self.ui_root_size_basis(parent_rect.size);
            let size =
                self.resolve_ui_size_with_basis(node, parent_rect.size, None, Some(size_basis));
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
                        parent_scale,
                        parent_layout_rect,
                        root_rect.size,
                        computed,
                        computed_scales,
                    );
                }
                if let Some(rect) = computed.get(&node).copied() {
                    return Some(rect);
                }
            }
            // A UI tree rooted against a sub-view target (the owner rect
            // seeded by `nested_ui_sub_view_rect`) treats the owner like the
            // window: its first-level UI descendants are layout roots, so
            // their percent sizes aspect-fit the target rect too. In the main
            // window pass every processed node's world is nil, so this never
            // fires there.
            let sub_view_root_basis = ui_parent.and_then(|parent| {
                (!parent.is_nil() && self.node_world(node) == Some(parent))
                    .then(|| self.ui_root_size_basis(parent_layout_rect.size))
            });
            let child_layout_rect = self
                .compute_ui_child_rect(
                    ui_parent.unwrap_or(scene_node.parent),
                    node,
                    parent_layout_rect,
                    &ui_root.layout,
                    &ui_root.transform,
                    sub_view_root_basis,
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
                    let parent_content = parent_content.inset(ui_margin_scaled(
                        ui_root.layout.margin,
                        self.ui_content_scale(),
                    ));
                    // No resolved UI parent means this node roots a UI tree
                    // under a non-UI scene parent: same size-basis rule as
                    // the nil-parent branch.
                    let size_basis = if ui_parent.is_none() {
                        Some(self.ui_root_size_basis(parent_content.size))
                    } else {
                        None
                    };
                    let size = self.resolve_ui_size_with_basis(
                        node,
                        parent_content.size,
                        None,
                        size_basis,
                    );
                    ui_root
                        .layout
                        .compute_rect_with_size(&ui_root.transform, parent_content, size)
                });
            let rect =
                scale_ui_rect_from_parent(child_layout_rect, parent_layout_rect, parent_scale);
            computed_scales.insert(node, parent_scale * ui_root.transform.scale);
            rect
        };
        let rect = if self.ui_pixel_snapping_enabled() {
            snap_computed_ui_rect(rect, root_rect.size, self.ui_pixel_snap_scale_factor())
        } else {
            rect
        };
        computed.insert(node, rect);
        Some(rect)
    }

    /// Uniform content scale: min(viewport / project virtual canvas). 1.0 at
    /// the design resolution (and when no project config is loaded). Used for
    /// every absolute-px UI quantity (fonts, margins, strokes, min/max sizes)
    /// so authored pixel values stay proportional on any window.
    pub(crate) fn ui_virtual_font_scale(&self, viewport: Vector2) -> f32 {
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

    /// `ui_virtual_font_scale` for the current window viewport.
    pub(crate) fn ui_content_scale(&self) -> f32 {
        self.ui_virtual_font_scale(self.input.viewport_size())
    }

    /// SIZE basis for layout roots: the project virtual canvas aspect-fit
    /// into `root_size`. Percent sizes on root nodes resolve against this
    /// basis so a ratio-authored node keeps its designed shape on any window
    /// aspect, while anchors/positions keep resolving against the real
    /// `root_size` (corner HUD stays in real corners; extra width becomes
    /// breathing room, not stretch). Returns `root_size` unchanged when the
    /// aspect matches the canvas (bit-identical layout at the design aspect)
    /// or when no project config is loaded.
    pub(crate) fn ui_root_size_basis(&self, root_size: Vector2) -> Vector2 {
        let Some((vw, vh)) = self.project().map(|project| {
            (
                project.config.virtual_width.max(1) as f32,
                project.config.virtual_height.max(1) as f32,
            )
        }) else {
            return root_size;
        };
        let sx = root_size.x.max(1.0) / vw;
        let sy = root_size.y.max(1.0) / vh;
        if sx == sy {
            return root_size;
        }
        let s = sx.min(sy).max(0.0001);
        Vector2::new(vw * s, vh * s)
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

    /// Appends `parent`'s ui layout children to `out` (no clear); scratch vecs
    /// come from the render-ui pool so repeated calls allocate nothing.
    pub(super) fn ui_layout_children_into(&self, parent: NodeID, out: &mut Vec<NodeID>) {
        let Some(parent_children) = self.nodes.children(parent) else {
            return;
        };
        let mut stack = self.render_ui.acquire_node_vec();
        stack.extend(parent_children.iter().rev().copied());
        while let Some(node_id) = stack.pop() {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            if ui_root_from_data(&node.data).is_some() {
                out.push(node_id);
                continue;
            }
            if let Some(children) = self.nodes.children(node_id) {
                stack.extend(children.iter().rev().copied());
            }
        }
        self.render_ui.release_node_vec(stack);
    }

    pub(super) fn compute_ui_auto_children_rects(
        &self,
        parent: NodeID,
        parent_scale: Vector2,
        parent_layout_rect: ComputedUiRect,
        viewport: Vector2,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &mut AHashMap<NodeID, Vector2>,
    ) -> Option<()> {
        let parent_node = self.nodes.get(parent)?;
        let parent_ui = ui_root_from_data(&parent_node.data)?;
        let auto_layout = ui_auto_layout_from_data(&parent_node.data)?;
        let mut layout_children = self.render_ui.acquire_node_vec();
        self.ui_layout_children_into(parent, &mut layout_children);
        let content_rect = ui_scroll_content_rect(
            &parent_node.data,
            parent_layout_rect.inset(ui_padding_inset(
                parent_layout_rect,
                parent_ui.layout.padding,
            )),
            self.ui_content_scale(),
        );
        let layout_ctx = UiChildrenLayoutCtx {
            parent_layout_rect,
            content: content_rect,
            parent_scale,
            viewport,
            snap: self.ui_pixel_snapping_enabled(),
            snap_scale: self.ui_pixel_snap_scale_factor(),
        };
        match auto_layout.mode {
            UiLayoutMode::H => self.compute_ui_h_children_rects(
                &parent_ui.layout,
                &layout_children,
                layout_ctx,
                UiAxisLayoutSpacing {
                    amount: auto_layout.h_spacing,
                    mode: auto_layout.h_spacing_mode,
                },
                computed,
                computed_scales,
            ),
            UiLayoutMode::V => self.compute_ui_v_children_rects(
                &parent_ui.layout,
                &layout_children,
                layout_ctx,
                UiAxisLayoutSpacing {
                    amount: auto_layout.v_spacing,
                    mode: auto_layout.v_spacing_mode,
                },
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
        self.render_ui.release_node_vec(layout_children);
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
        if self.render_ui.active_scrollbar == Some(node) {
            self.render_ui.active_scrollbar = None;
            self.render_ui.scrollbar_drag_offset = 0.0;
        }
        if self.render_ui.retained_commands.remove(&node).is_some() {
            self.queue_render_command(RenderCommand::Ui(Box::new(UiCommand::RemoveNode { node })));
        }
        // A deleted color picker never reaches the wheel emit pass again, so its
        // synthetic wheel node would stay retained (and drawn) forever. Only the
        // gone-from-arena case clears it: a merely hidden picker keeps its
        // retained wheel so the next pass compares equal instead of churning.
        if self.nodes.get(node).is_none() {
            self.remove_retained_color_wheel(color_picker_wheel_render_node(node));
        }
    }

    pub(super) fn remove_no_longer_visible_ui_nodes(
        &mut self,
        visible_now: &ahash::AHashSet<NodeID>,
    ) {
        let mut to_remove = std::mem::take(&mut self.render_ui.removed_visible_scratch);
        to_remove.clear();
        for node in self.render_ui.prev_visible.iter().copied() {
            if !visible_now.contains(&node) {
                to_remove.push(node);
            }
        }
        for node in to_remove.iter().copied() {
            self.remove_retained_ui_node(node);
        }
        to_remove.clear();
        self.render_ui.removed_visible_scratch = to_remove;
    }
}

//! Runtime UI layout, retained command extraction, and text input handling.

use super::state::{DirtyState, UiButtonVisualState};
use super::{Runtime, RuntimeUiTiming};
use ahash::AHashMap;
use perro_ids::{NodeID, SignalID, TextureID};
use perro_input::{KeyCode, MouseButton};
use perro_nodes::SceneNodeData;
use perro_render_bridge::{
    RenderCommand, RenderRequestID, ResourceCommand, UiCommand, UiDepthEffectState,
    UiImageScaleState, UiRectState, UiTextAlignState,
};
use perro_runtime_context::sub_apis::SignalAPI;
use perro_structs::Vector2;
use perro_ui::{
    ComputedUiRect, UiBox, UiFontSizing, UiHorizontalAlign, UiImageScaleMode, UiLayoutData,
    UiLayoutMode, UiSizeMode, UiStyle, UiTextEdit, UiTransform, UiVerticalAlign,
};
use perro_variant::Variant;
use std::borrow::Cow;

mod locale;

const TEXT_EDIT_REPEAT_DELAY: f32 = 0.35;
const TEXT_EDIT_REPEAT_RATE: f32 = 0.035;

fn ui_image_texture_request(node: NodeID) -> RenderRequestID {
    RenderRequestID::new((node.as_u64() << 8) | 0xE9)
}

impl Runtime {
    pub(crate) fn mark_ui_viewport_dirty(&mut self) {
        let ids: Vec<NodeID> = self
            .nodes
            .iter()
            .filter_map(|(id, node)| ui_root_from_data(&node.data).is_some().then_some(id))
            .collect();
        for id in ids {
            self.mark_ui_dirty(
                id,
                Runtime::UI_DIRTY_LAYOUT_SELF
                    | Runtime::UI_DIRTY_LAYOUT_PARENT
                    | Runtime::UI_DIRTY_COMMANDS,
            );
        }
    }

    fn resolve_ui_image_texture(&mut self, node: NodeID) -> Option<TextureID> {
        let mut texture = self
            .nodes
            .get(node)
            .and_then(|scene_node| match &scene_node.data {
                SceneNodeData::UiImage(image) => Some(image.texture),
                SceneNodeData::UiAnimatedImage(image) => Some(image.texture),
                _ => None,
            })?;

        if texture.is_nil() {
            let request = ui_image_texture_request(node);
            if let Some(crate::RuntimeRenderResult::Texture(id)) = self.take_render_result(request)
            {
                texture = id;
            }
        }

        if texture.is_nil() {
            let request = ui_image_texture_request(node);
            if !self.render.is_inflight(request) {
                let source = self
                    .render_2d
                    .texture_sources
                    .get(&node)
                    .cloned()
                    .unwrap_or_else(|| "__default__".to_string());
                self.render.mark_inflight(request);
                self.queue_render_command(RenderCommand::Resource(
                    ResourceCommand::CreateTexture {
                        request,
                        id: TextureID::nil(),
                        source,
                        reserved: false,
                    },
                ));
            }
            return None;
        }

        Some(texture)
    }

    pub fn extract_render_ui_commands(&mut self) {
        self.extract_render_ui_commands_inner(None);
    }

    pub fn extract_render_ui_commands_timed(&mut self) -> RuntimeUiTiming {
        let mut timing = RuntimeUiTiming::default();
        self.extract_render_ui_commands_inner(Some(&mut timing));
        timing
    }

    fn extract_render_ui_commands_inner(&mut self, timing: Option<&mut RuntimeUiTiming>) {
        self.refresh_locale_text_bindings();
        let total_start = timing.as_ref().map(|_| std::time::Instant::now());
        let bootstrap_scan = self.render_ui.prev_visible.is_empty()
            && self.render_ui.retained_commands.is_empty()
            && self.render_ui.computed_rects.is_empty();
        let input_changed = self.ui_pointer_changed();
        let text_input_changed =
            self.render_ui.focused_text_edit.is_some() && self.ui_text_input_changed();
        let has_extraction_work = self.dirty.has_any_dirty()
            || self.dirty.has_pending_transform_roots()
            || !self.render_ui.removed_nodes.is_empty()
            || bootstrap_scan
            || input_changed
            || text_input_changed;
        if !has_extraction_work {
            if let Some(timing) = timing {
                timing.total = total_start.expect("ui timing total start exists").elapsed();
            }
            return;
        }
        let mut timing = timing;
        let dirty_node_count = self.dirty.dirty_indices().len();

        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();

        let viewport = self.input.viewport_size();
        let virtual_font_scale = self.ui_virtual_font_scale(viewport);
        let root_rect = ComputedUiRect::new(Vector2::ZERO, viewport);
        let mut traversal_ids = std::mem::take(&mut self.render_ui.traversal_ids);
        let mut traversal_seen = std::mem::take(&mut self.render_ui.traversal_seen);
        let mut command_ids = std::mem::take(&mut self.render_ui.command_ids);
        let mut command_seen = std::mem::take(&mut self.render_ui.command_seen);
        traversal_ids.clear();
        traversal_seen.clear();
        command_ids.clear();
        command_seen.clear();
        for &raw_index in self.dirty.dirty_indices() {
            let index = raw_index as usize;
            let Some((node, _)) = self.nodes.slot_get(index) else {
                continue;
            };
            let mut flags = self.dirty.ui_flags_at(index);
            if flags == 0 {
                flags = DirtyState::UI_LAYOUT_MASK | DirtyState::DIRTY_COMMANDS;
            }
            if (flags & DirtyState::UI_LAYOUT_MASK) != 0 && traversal_seen.insert(node) {
                traversal_ids.push(node);
            }
            if (flags & DirtyState::DIRTY_COMMANDS) != 0 && command_seen.insert(node) {
                command_ids.push(node);
            }
            if (flags & DirtyState::DIRTY_LAYOUT_PARENT) != 0
                && let Some(parent) = self.nodes.get(node).map(|node| node.parent)
                && let Some(ui_parent) = self.closest_ui_parent(parent)
                && self
                    .nodes
                    .get(ui_parent)
                    .and_then(|parent_node| ui_auto_layout_from_data(&parent_node.data))
                    .is_some()
            {
                let siblings = self.ui_layout_children(ui_parent);
                for &sibling in &siblings {
                    if traversal_seen.insert(sibling) {
                        traversal_ids.push(sibling);
                    }
                    if command_seen.insert(sibling) {
                        command_ids.push(sibling);
                    }
                }
            }
        }
        if traversal_ids.is_empty() && bootstrap_scan {
            traversal_ids.extend(self.nodes.iter().map(|(id, _)| id));
        }
        traversal_seen.extend(traversal_ids.iter().copied());
        let mut traversal_cursor = 0usize;
        while traversal_cursor < traversal_ids.len() {
            let node = traversal_ids[traversal_cursor];
            traversal_cursor += 1;
            if let Some(node_ref) = self.nodes.get(node) {
                for &child in node_ref.get_children_ids() {
                    if traversal_seen.insert(child) {
                        traversal_ids.push(child);
                    }
                }
                if let SceneNodeData::UiTreeList(tree) = &node_ref.data {
                    for child in ui_tree_all_nodes(tree) {
                        if traversal_seen.insert(child) {
                            traversal_ids.push(child);
                        }
                    }
                }
            }
        }
        for &node in &traversal_ids {
            if command_seen.insert(node) {
                command_ids.push(node);
            }
        }
        if input_changed || bootstrap_scan {
            self.collect_retained_command_ids(&mut command_ids, &mut command_seen);
        }
        traversal_seen.clear();
        self.render_ui.traversal_seen = traversal_seen;
        if let Some(timing) = timing.as_deref_mut() {
            timing.dirty_nodes = dirty_node_count.min(u32::MAX as usize) as u32;
            timing.affected_nodes = traversal_ids.len().min(u32::MAX as usize) as u32;
        }

        let mut visible_now = std::mem::take(&mut self.render_ui.visible_now);
        visible_now.clear();
        visible_now.extend(self.render_ui.prev_visible.iter().copied());
        let mut removed_nodes = std::mem::take(&mut self.render_ui.removed_nodes);
        for node in removed_nodes.drain(..) {
            if self.render_ui.focused_text_edit == Some(node) {
                self.render_ui.focused_text_edit = None;
            }
            if self.render_ui.hovered_text_edit == Some(node) {
                self.render_ui.hovered_text_edit = None;
            }
            if self.render_ui.pressed_text_edit == Some(node) {
                self.render_ui.pressed_text_edit = None;
            }
            visible_now.remove(&node);
            self.render_ui.computed_rects.remove(&node);
            self.render_ui
                .size_clamp_baselines
                .borrow_mut()
                .remove(&node);
            self.render_ui.computed_scales.remove(&node);
            self.render_ui.retained_rects.remove(&node);
            self.render_ui.button_states.remove(&node);
            if self.render_ui.retained_commands.remove(&node).is_some() {
                self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode { node }));
            }
        }
        self.render_ui.removed_nodes = removed_nodes;

        let mut computed = std::mem::take(&mut self.render_ui.computed_rects);
        let mut computed_scales = std::mem::take(&mut self.render_ui.computed_scales);
        for node in traversal_ids.iter() {
            computed.remove(node);
            computed_scales.remove(node);
        }
        let mut auto_layout_computed = std::mem::take(&mut self.render_ui.auto_layout_computed);
        auto_layout_computed.clear();
        let layout_start = timing.as_ref().map(|_| std::time::Instant::now());
        for node in traversal_ids.iter().copied() {
            let was_cached = computed.contains_key(&node);
            let before_len = computed.len();
            self.compute_ui_rect(
                node,
                root_rect,
                &mut computed,
                &mut computed_scales,
                &mut auto_layout_computed,
            );
            if let Some(timing) = timing.as_deref_mut() {
                if was_cached {
                    timing.cached_rects = timing.cached_rects.saturating_add(1);
                } else if computed.len() > before_len {
                    let added = (computed.len() - before_len).min(u32::MAX as usize) as u32;
                    timing.recalculated_rects = timing.recalculated_rects.saturating_add(added);
                }
            }
        }
        if let Some(timing) = timing.as_deref_mut() {
            timing.auto_layout_batches = auto_layout_computed.len().min(u32::MAX as usize) as u32;
        }
        self.render_ui.auto_layout_computed = auto_layout_computed;
        if let Some(timing) = timing.as_deref_mut() {
            timing.layout += layout_start
                .expect("ui layout timing start exists")
                .elapsed();
        }

        self.process_text_edit_input(&computed, &mut command_ids, &mut command_seen);
        self.refresh_button_visual_states(&computed, &mut command_ids, &mut command_seen);

        let commands_start = timing.as_ref().map(|_| std::time::Instant::now());
        for node in command_ids.iter().copied() {
            if let Some(timing) = timing.as_deref_mut() {
                timing.command_nodes = timing.command_nodes.saturating_add(1);
            }
            visible_now.remove(&node);
            let effective_visible = self.is_effectively_visible_for_ui(node);
            if let Some(texture) = self.resolve_ui_image_texture(node)
                && let Some(scene_node) = self.nodes.get_mut(node)
            {
                match &mut scene_node.data {
                    SceneNodeData::UiImage(image) => image.texture = texture,
                    SceneNodeData::UiAnimatedImage(image) => image.texture = texture,
                    _ => {}
                }
            }
            let Some(scene_node) = self.nodes.get(node) else {
                self.remove_retained_ui_node(node);
                if let Some(timing) = timing.as_deref_mut() {
                    timing.removed_nodes = timing.removed_nodes.saturating_add(1);
                }
                continue;
            };
            let state = self
                .render_ui
                .button_states
                .get(&node)
                .copied()
                .unwrap_or_default();
            let effective_z = self.ui_effective_z(node);
            let rect_state = if let Some(rect) = computed.get(&node).copied() {
                ui_rect_state_from_node(&scene_node.data, rect, state, effective_z)
            } else {
                self.render_ui.retained_rects.get(&node).copied()
            };
            let Some(rect_state) = rect_state else {
                self.remove_retained_ui_node(node);
                if let Some(timing) = timing.as_deref_mut() {
                    timing.removed_nodes = timing.removed_nodes.saturating_add(1);
                }
                continue;
            };
            if !effective_visible {
                self.remove_retained_ui_node(node);
                if let Some(timing) = timing.as_deref_mut() {
                    timing.removed_nodes = timing.removed_nodes.saturating_add(1);
                }
                continue;
            }
            let scale = computed_scales.get(&node).copied().unwrap_or(Vector2::ONE);
            let clip_rect = if computed.contains_key(&node) {
                self.ui_effective_clip_rect_screen(node, &computed, viewport)
            } else {
                self.render_ui
                    .retained_commands
                    .get(&node)
                    .map(ui_command_clip_rect)
                    .unwrap_or_else(|| viewport_clip_rect(viewport))
            };
            let retained_matches =
                self.render_ui
                    .retained_commands
                    .get(&node)
                    .is_some_and(|command| {
                        let command_ctx = UiCommandCtx {
                            node,
                            rect: rect_state,
                            clip_rect,
                            scale,
                            virtual_font_scale,
                        };
                        ui_command_matches_node(
                            command,
                            &scene_node.data,
                            command_ctx,
                            state,
                            self.render_ui.focused_text_edit,
                        )
                    });
            if !retained_matches {
                let command_ctx = UiCommandCtx {
                    node,
                    rect: rect_state,
                    clip_rect,
                    scale,
                    virtual_font_scale,
                };
                let Some(command) = ui_command_from_node(
                    &scene_node.data,
                    command_ctx,
                    state,
                    self.render_ui.focused_text_edit,
                ) else {
                    self.remove_retained_ui_node(node);
                    if let Some(timing) = timing.as_deref_mut() {
                        timing.removed_nodes = timing.removed_nodes.saturating_add(1);
                    }
                    continue;
                };
                self.queue_render_command(RenderCommand::Ui(command.clone()));
                self.render_ui.retained_commands.insert(node, command);
                if let Some(timing) = timing.as_deref_mut() {
                    timing.command_emitted = timing.command_emitted.saturating_add(1);
                }
            } else if let Some(timing) = timing.as_deref_mut() {
                timing.command_skipped = timing.command_skipped.saturating_add(1);
            }
            self.render_ui.retained_rects.insert(node, rect_state);
            visible_now.insert(node);
        }
        self.remove_no_longer_visible_ui_nodes(&visible_now);
        if let Some(timing) = timing.as_deref_mut() {
            timing.commands += commands_start
                .expect("ui commands timing start exists")
                .elapsed();
        }

        self.render_ui.computed_rects = computed;
        self.render_ui.computed_scales = computed_scales;
        std::mem::swap(&mut self.render_ui.prev_visible, &mut visible_now);
        visible_now.clear();
        self.render_ui.visible_now = visible_now;

        traversal_ids.clear();
        self.render_ui.traversal_ids = traversal_ids;
        command_ids.clear();
        command_seen.clear();
        self.render_ui.command_ids = command_ids;
        self.render_ui.command_seen = command_seen;

        if let Some(timing) = timing {
            timing.total = total_start.expect("ui timing total start exists").elapsed();
        }
    }

    fn compute_ui_rect(
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
                        .map(|parent| parent_layout_rect.inset(parent.layout.padding))
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

    fn ui_virtual_font_scale(&self, viewport: Vector2) -> f32 {
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

    fn resolve_ui_parent_rect(
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

    fn closest_ui_parent(&self, mut parent: NodeID) -> Option<NodeID> {
        while !parent.is_nil() {
            let parent_node = self.nodes.get(parent)?;
            if ui_root_from_data(&parent_node.data).is_some() {
                return Some(parent);
            }
            parent = parent_node.parent;
        }
        None
    }

    fn ui_layout_children(&self, parent: NodeID) -> Vec<NodeID> {
        let mut out = Vec::new();
        let Some(parent_node) = self.nodes.get(parent) else {
            return out;
        };
        let mut stack: Vec<NodeID> = parent_node.get_children_ids().to_vec();
        while let Some(node_id) = stack.pop() {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            if ui_root_from_data(&node.data).is_some() {
                out.push(node_id);
                continue;
            }
            stack.extend(node.get_children_ids().iter().copied());
        }
        out
    }

    fn compute_ui_auto_children_rects(
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
            parent_layout_rect.inset(parent_ui.layout.padding),
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

    fn remove_retained_ui_node(&mut self, node: NodeID) {
        self.render_ui.retained_rects.remove(&node);
        self.render_ui.button_states.remove(&node);
        if self.render_ui.hovered_text_edit == Some(node) {
            self.render_ui.hovered_text_edit = None;
        }
        if self.render_ui.focused_text_edit == Some(node) {
            self.render_ui.focused_text_edit = None;
        }
        if self.render_ui.pressed_text_edit == Some(node) {
            self.render_ui.pressed_text_edit = None;
        }
        if self.render_ui.retained_commands.remove(&node).is_some() {
            self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode { node }));
        }
    }

    fn remove_no_longer_visible_ui_nodes(&mut self, visible_now: &ahash::AHashSet<NodeID>) {
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

    fn ui_pointer_changed(&self) -> bool {
        let pointer = (
            self.input.mouse_position(),
            self.input.is_mouse_down(MouseButton::Left),
        );
        self.render_ui.last_ui_pointer != Some(pointer)
    }

    fn collect_retained_command_ids(
        &self,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        for node in self.render_ui.retained_commands.keys().copied() {
            if command_seen.insert(node) {
                command_ids.push(node);
            }
        }
    }

    fn ui_text_input_changed(&self) -> bool {
        !self.input.text_inputs().is_empty()
            || self.input.mouse_wheel() != Vector2::ZERO
            || text_edit_keys()
                .iter()
                .any(|&key| self.input.is_key_pressed(key) || self.input.is_key_down(key))
    }

    fn process_text_edit_input(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        let mouse_pos = self.input.mouse_position();
        let mouse_pressed = self.input.is_mouse_pressed(MouseButton::Left);
        let mouse_down = self.input.is_mouse_down(MouseButton::Left);
        let hovered = self.hovered_text_edit(computed);
        if self.render_ui.hovered_text_edit != hovered {
            if let Some(prev) = self.render_ui.hovered_text_edit {
                self.emit_text_edit_event(prev, "unhovered", None);
            }
            if let Some(next) = hovered {
                self.emit_text_edit_event(next, "hovered", None);
            }
            self.render_ui.hovered_text_edit = hovered;
        }
        if mouse_pressed {
            let hit = hovered;
            if self.render_ui.focused_text_edit != hit {
                if let Some(prev) = self.render_ui.focused_text_edit
                    && command_seen.insert(prev)
                {
                    command_ids.push(prev);
                }
                if let Some(prev) = self.render_ui.focused_text_edit {
                    self.emit_text_edit_event(prev, "unfocused", None);
                }
                if let Some(next) = hit
                    && command_seen.insert(next)
                {
                    command_ids.push(next);
                }
                if let Some(next) = hit {
                    self.emit_text_edit_event(next, "focused", None);
                }
                self.render_ui.focused_text_edit = hit;
            }
            self.render_ui.pressed_text_edit = hit;
            if let Some(node) = hit {
                self.seek_text_edit_at_mouse(node, computed, mouse_pos, false);
                if command_seen.insert(node) {
                    command_ids.push(node);
                }
            }
        } else if mouse_down
            && let Some(node) = self.render_ui.pressed_text_edit
            && self.render_ui.focused_text_edit == Some(node)
        {
            self.seek_text_edit_at_mouse(node, computed, mouse_pos, true);
            if command_seen.insert(node) {
                command_ids.push(node);
            }
        } else if self.input.is_mouse_released(MouseButton::Left) {
            self.render_ui.pressed_text_edit = None;
        }

        let Some(focused) = self.render_ui.focused_text_edit else {
            return;
        };
        if self
            .nodes
            .get(focused)
            .is_none_or(|node| text_edit_ref(&node.data).is_none())
        {
            self.render_ui.focused_text_edit = None;
            self.render_ui.pressed_text_edit = None;
            self.render_ui.text_edit_repeat_key = None;
            self.render_ui.text_edit_repeat_timer = 0.0;
            return;
        }

        let mut changed = false;
        let mut text_changed = false;
        let mut changed_text = None;
        let text_inputs: Vec<String> = self.input.text_inputs().to_vec();
        let shift = self.input.is_key_down(KeyCode::ShiftLeft)
            || self.input.is_key_down(KeyCode::ShiftRight);
        let ctrl = self.input.is_key_down(KeyCode::ControlLeft)
            || self.input.is_key_down(KeyCode::ControlRight);
        let wheel = self.input.mouse_wheel();
        let repeat_key = self.text_edit_repeat_key(ctrl);
        if let Some(scene_node) = self.nodes.get_mut(focused)
            && let Some(edit) = text_edit_mut(&mut scene_node.data)
        {
            let old_text = edit.text.to_string();
            if !ctrl {
                for text in text_inputs {
                    changed |= insert_text_input(edit, &text);
                }
            }
            changed |= apply_text_edit_key_input(edit, shift, ctrl, repeat_key, &self.input);
            if edit.multiline && wheel.y != 0.0 {
                edit.v_scroll = (edit.v_scroll - wheel.y * edit.font_size * 2.0).max(0.0);
                changed = true;
            }
            ensure_caret_visible(edit, computed.get(&focused).copied());
            if edit.text.as_ref() != old_text {
                text_changed = true;
                changed_text = Some(edit.text.to_string());
            }
        }
        if changed && command_seen.insert(focused) {
            command_ids.push(focused);
        }
        if changed {
            self.mark_ui_dirty(focused, Runtime::UI_DIRTY_COMMANDS | Runtime::UI_DIRTY_TEXT);
        }
        if text_changed {
            self.emit_text_edit_event(focused, "text_changed", changed_text.as_deref());
        }
    }

    fn text_edit_repeat_key(&mut self, ctrl: bool) -> Option<KeyCode> {
        if ctrl {
            self.render_ui.text_edit_repeat_key = None;
            self.render_ui.text_edit_repeat_timer = 0.0;
            return None;
        }
        let active = repeatable_text_edit_keys()
            .iter()
            .copied()
            .find(|&key| self.input.is_key_down(key));
        let Some(key) = active else {
            self.render_ui.text_edit_repeat_key = None;
            self.render_ui.text_edit_repeat_timer = 0.0;
            return None;
        };
        if self.input.is_key_pressed(key) || self.render_ui.text_edit_repeat_key != Some(key) {
            self.render_ui.text_edit_repeat_key = Some(key);
            self.render_ui.text_edit_repeat_timer = TEXT_EDIT_REPEAT_DELAY;
            return Some(key);
        }
        self.render_ui.text_edit_repeat_timer -= self.time.delta.max(0.0);
        if self.render_ui.text_edit_repeat_timer > 0.0 {
            return None;
        }
        while self.render_ui.text_edit_repeat_timer <= 0.0 {
            self.render_ui.text_edit_repeat_timer += TEXT_EDIT_REPEAT_RATE;
        }
        Some(key)
    }

    fn hovered_text_edit(&self, computed: &AHashMap<NodeID, ComputedUiRect>) -> Option<NodeID> {
        let mouse = self.input.mouse_position();
        let viewport = self.input.viewport_size();
        let point = Vector2::new((mouse.x - 0.5) * viewport.x, (mouse.y - 0.5) * viewport.y);
        let mut best = None;
        let mut best_z = i32::MIN;
        for (node, scene_node) in self.nodes.iter() {
            let Some(edit) = text_edit_ref(&scene_node.data) else {
                continue;
            };
            if !edit.base.visible
                || !edit.base.input_enabled
                || !self.is_effectively_visible_for_ui(node)
            {
                continue;
            }
            let Some(rect) = computed.get(&node).copied().or_else(|| {
                self.render_ui
                    .retained_rects
                    .get(&node)
                    .copied()
                    .map(|rect| computed_rect_from_state(&rect))
            }) else {
                continue;
            };
            let z = self.ui_effective_z(node);
            if rect.contains(point) && z >= best_z {
                best = Some(node);
                best_z = z;
            }
        }
        best
    }

    fn seek_text_edit_at_mouse(
        &mut self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        mouse_pos: Vector2,
        extend: bool,
    ) {
        let viewport = self.input.viewport_size();
        let Some(rect) = computed.get(&node).copied() else {
            return;
        };
        if let Some(scene_node) = self.nodes.get_mut(node)
            && let Some(edit) = text_edit_mut(&mut scene_node.data)
        {
            let point = Vector2::new(
                (mouse_pos.x - 0.5) * viewport.x,
                (mouse_pos.y - 0.5) * viewport.y,
            );
            let min = rect.min();
            let local = Vector2::new(
                point.x - min.x - edit.padding.left + edit.h_scroll,
                rect.max().y - point.y - edit.padding.top + edit.v_scroll,
            );
            let index = text_index_from_local(edit, local);
            edit.caret = index;
            if !extend {
                edit.anchor = index;
            }
            ensure_caret_visible(edit, Some(rect));
        }
    }

    fn refresh_button_visual_states(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        let hovered = self.hovered_button(computed);
        let mouse_down = self.input.is_mouse_down(MouseButton::Left);
        let mut next_states = std::mem::take(&mut self.render_ui.button_states);
        next_states.retain(|node, _| self.nodes.get(*node).is_some());
        let mut events = Vec::new();

        for (node, scene_node) in self.nodes.iter() {
            let SceneNodeData::UiButton(button) = &scene_node.data else {
                continue;
            };
            let inactive = button_inactive(button);
            let next = if inactive || Some(node) != hovered {
                UiButtonVisualState::Neutral
            } else if mouse_down {
                UiButtonVisualState::Pressed
            } else {
                UiButtonVisualState::Hover
            };
            let prev = next_states.insert(node, next).unwrap_or_default();
            if !inactive {
                collect_button_events(node, prev, next, &mut events);
            }
            if prev != next && command_seen.insert(node) {
                command_ids.push(node);
            }
        }

        self.render_ui.button_states = next_states;
        let text_hovered = self.hovered_text_edit(computed);
        let cursor_icon = text_hovered
            .map(|_| perro_ui::CursorIcon::Text)
            .or_else(|| {
                hovered
                    .and_then(|node| self.nodes.get(node))
                    .and_then(|scene_node| match &scene_node.data {
                        SceneNodeData::UiButton(button) => Some(button.cursor_icon),
                        _ => None,
                    })
            })
            .unwrap_or(perro_ui::CursorIcon::Default);
        if self.render_ui.cursor_icon != cursor_icon {
            self.render_ui.cursor_icon = cursor_icon;
            self.set_cursor_icon_request(cursor_icon);
        }
        self.render_ui.last_ui_pointer = Some((
            self.input.mouse_position(),
            self.input.is_mouse_down(MouseButton::Left),
        ));
        self.emit_button_events(&events);
    }

    fn emit_button_events(&mut self, events: &[(NodeID, &'static str)]) {
        for &(node, event) in events {
            self.collect_button_event_signals(node, event);
            if self.render_ui.event_signal_scratch.is_empty() {
                continue;
            }
            let mut signals = std::mem::take(&mut self.render_ui.event_signal_scratch);
            let params = [Variant::from(node)];
            for signal in signals.iter().copied() {
                let _ = SignalAPI::signal_emit(self, signal, &params);
            }
            signals.clear();
            self.render_ui.event_signal_scratch = signals;
        }
    }

    fn emit_text_edit_event(&mut self, node: NodeID, event: &str, text: Option<&str>) {
        self.collect_text_edit_event_signals(node, event);
        if self.render_ui.event_signal_scratch.is_empty() {
            return;
        }
        let mut signals = std::mem::take(&mut self.render_ui.event_signal_scratch);
        if let Some(text) = text {
            let params = [Variant::from(node), Variant::from(text)];
            for signal in signals.iter().copied() {
                let _ = SignalAPI::signal_emit(self, signal, &params);
            }
        } else {
            let params = [Variant::from(node)];
            for signal in signals.iter().copied() {
                let _ = SignalAPI::signal_emit(self, signal, &params);
            }
        }
        signals.clear();
        self.render_ui.event_signal_scratch = signals;
    }

    fn collect_button_event_signals(&mut self, node: NodeID, event: &str) {
        self.render_ui.event_signal_scratch.clear();
        let Some(scene_node) = self.nodes.get(node) else {
            return;
        };
        let SceneNodeData::UiButton(button) = &scene_node.data else {
            return;
        };
        if button_inactive(button) {
            return;
        }
        let custom = button_custom_event_signals(button, event);
        self.render_ui
            .event_signal_scratch
            .reserve(1 + custom.len());
        let name = scene_node.name.as_ref();
        if !name.is_empty() {
            self.render_ui.event_signal_name_scratch.clear();
            self.render_ui.event_signal_name_scratch.push_str(name);
            self.render_ui.event_signal_name_scratch.push('_');
            self.render_ui.event_signal_name_scratch.push_str(event);
            self.render_ui
                .event_signal_scratch
                .push(SignalID::from_string(
                    &self.render_ui.event_signal_name_scratch,
                ));
        }
        self.render_ui
            .event_signal_scratch
            .extend(custom.iter().copied());
    }

    fn collect_text_edit_event_signals(&mut self, node: NodeID, event: &str) {
        self.render_ui.event_signal_scratch.clear();
        let Some(scene_node) = self.nodes.get(node) else {
            return;
        };
        let Some(edit) = text_edit_ref(&scene_node.data) else {
            return;
        };
        let custom = text_edit_custom_event_signals(edit, event);
        self.render_ui
            .event_signal_scratch
            .reserve(1 + custom.len());
        let name = scene_node.name.as_ref();
        if !name.is_empty() {
            self.render_ui.event_signal_name_scratch.clear();
            self.render_ui.event_signal_name_scratch.push_str(name);
            self.render_ui.event_signal_name_scratch.push('_');
            self.render_ui.event_signal_name_scratch.push_str(event);
            self.render_ui
                .event_signal_scratch
                .push(SignalID::from_string(
                    &self.render_ui.event_signal_name_scratch,
                ));
        }
        self.render_ui
            .event_signal_scratch
            .extend(custom.iter().copied());
    }

    #[cfg(test)]
    fn button_event_signals(&self, node: NodeID, event: &str) -> Vec<SignalID> {
        let Some(scene_node) = self.nodes.get(node) else {
            return Vec::new();
        };
        let SceneNodeData::UiButton(button) = &scene_node.data else {
            return Vec::new();
        };
        if button_inactive(button) {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(1 + button_custom_event_signals(button, event).len());
        let name = scene_node.name.as_ref();
        if !name.is_empty() {
            out.push(SignalID::from_string(&format!("{name}_{event}")));
        }
        out.extend(button_custom_event_signals(button, event).iter().copied());
        out
    }

    #[cfg(test)]
    fn text_edit_event_signals(&self, node: NodeID, event: &str) -> Vec<SignalID> {
        let Some(scene_node) = self.nodes.get(node) else {
            return Vec::new();
        };
        let Some(edit) = text_edit_ref(&scene_node.data) else {
            return Vec::new();
        };
        let custom = text_edit_custom_event_signals(edit, event);
        let mut out = Vec::with_capacity(1 + custom.len());
        let name = scene_node.name.as_ref();
        if !name.is_empty() {
            out.push(SignalID::from_string(&format!("{name}_{event}")));
        }
        out.extend(custom.iter().copied());
        out
    }

    fn hovered_button(&self, computed: &AHashMap<NodeID, ComputedUiRect>) -> Option<NodeID> {
        let viewport = self.input.viewport_size();
        let mouse = self.input.mouse_position();
        let point = Vector2::new((mouse.x - 0.5) * viewport.x, (mouse.y - 0.5) * viewport.y);

        let mut best: Option<(NodeID, i32)> = None;
        for (node, scene_node) in self.nodes.iter() {
            let SceneNodeData::UiButton(button) = &scene_node.data else {
                continue;
            };
            if button.disabled
                || !button.input_enabled
                || !self.is_effectively_visible_for_ui(node)
                || !matches!(
                    button.mouse_filter,
                    perro_ui::UiMouseFilter::Stop | perro_ui::UiMouseFilter::Pass
                )
            {
                continue;
            }
            let base_rect = computed.get(&node).copied().or_else(|| {
                self.render_ui
                    .retained_rects
                    .get(&node)
                    .map(computed_rect_from_state)
            });
            let Some(base_rect) = base_rect else {
                continue;
            };
            let state = self
                .render_ui
                .button_states
                .get(&node)
                .copied()
                .unwrap_or_default();
            let z = self.ui_effective_z(node);
            let hit_rect = if computed.contains_key(&node) {
                computed_rect_from_state(&button_rect_state(button, base_rect, state, z))
            } else {
                base_rect
            };
            if !hit_rect.contains_rounded(point, button_style(button, state).corner_radius) {
                continue;
            }
            match best {
                Some((best_node, best_z))
                    if best_z > z || (best_z == z && best_node.as_u64() > node.as_u64()) => {}
                _ => best = Some((node, z)),
            }
        }
        best.map(|(node, _)| node)
    }

    fn ui_effective_z(&self, node: NodeID) -> i32 {
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

    fn compute_ui_child_rect(
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
            parent_rect.inset(parent_ui.layout.padding),
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

    fn compute_ui_h_child_rect(
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
                    + ui_translation_offset(transform, size);
                return Some(ComputedUiRect::new(center, size));
            }
            x += size.x + layout.margin.horizontal() + spacing;
        }
        None
    }

    fn compute_ui_h_children_rects(
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
                + ui_translation_offset(transform, size);
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

    fn compute_ui_v_child_rect(
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
                    + ui_translation_offset(transform, size);
                return Some(ComputedUiRect::new(center, size));
            }
            y -= size.y + layout.margin.vertical() + spacing;
        }
        None
    }

    fn compute_ui_v_children_rects(
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
                + ui_translation_offset(transform, size);
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

    fn compute_ui_grid_child_rect(
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
                cell_height,
                size.y,
                layout.margin,
                parent_layout.v_align,
            ),
        ) + ui_translation_offset(transform, size);
        Some(ComputedUiRect::new(center, size))
    }

    fn compute_ui_grid_children_rects(
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
                    cell_height,
                    size.y,
                    layout.margin,
                    parent_layout.v_align,
                ),
            ) + ui_translation_offset(transform, size);
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

    fn compute_ui_tree_rows(
        &self,
        tree: &perro_ui::UiTreeList,
        tree_rect: ComputedUiRect,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
    ) {
        let content = tree_rect.inset(tree.base.layout.padding);
        let rows = ui_tree_visible_rows(tree);
        if rows.is_empty() {
            return;
        }
        let max = content.max();
        let mut y = max.y;
        for row in rows {
            let Some((layout, transform)) = self
                .nodes
                .get(row.node)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some((&ui.layout, &ui.transform)))
            else {
                continue;
            };
            let indent = tree.indent * row.depth as f32;
            let row_content = ComputedUiRect::new(
                Vector2::new(content.center.x + indent * 0.5, content.center.y),
                Vector2::new((content.size.x - indent).max(0.0), content.size.y),
            )
            .inset(layout.margin);
            let fill_size = Vector2::new(
                if layout.h_size == UiSizeMode::Fill {
                    row_content.size.x
                } else {
                    0.0
                },
                0.0,
            );
            let size = self.resolve_ui_size(row.node, row_content.size, Some(fill_size));
            let center = Vector2::new(
                row_content.min().x + size.x * 0.5,
                y - layout.margin.top - size.y * 0.5,
            ) + ui_translation_offset(transform, size);
            computed.insert(row.node, ComputedUiRect::new(center, size));
            y -= size.y
                + layout.margin.vertical()
                + ui_v_spacing_amount(tree.v_spacing, content.size.y);
        }
    }

    fn ui_tree_owner(&self, child: NodeID) -> Option<NodeID> {
        self.nodes.iter().find_map(|(id, node)| {
            let SceneNodeData::UiTreeList(tree) = &node.data else {
                return None;
            };
            ui_tree_contains(tree, child).then_some(id)
        })
    }

    fn is_effectively_visible_for_ui(&self, node: NodeID) -> bool {
        if let Some(tree) = self.ui_tree_owner(node) {
            return self
                .nodes
                .get(node)
                .is_some_and(|scene_node| Self::node_local_visible(&scene_node.data))
                && self
                    .nodes
                    .get(tree)
                    .and_then(|scene_node| match &scene_node.data {
                        SceneNodeData::UiTreeList(tree) => {
                            Some(ui_tree_visible_contains(tree, node))
                        }
                        _ => None,
                    })
                    .unwrap_or(false)
                && self.is_effectively_visible(tree);
        }
        self.is_effectively_visible(node)
    }

    fn ui_effective_clip_rect_screen(
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
                .or_insert_with(|| super::state::UiSizeClampBaseline {
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

    fn fit_children_size(&self, node: NodeID, available: Vector2) -> Vector2 {
        let Some(scene_node) = self.nodes.get(node) else {
            return Vector2::ZERO;
        };
        let Some(ui) = ui_root_from_data(&scene_node.data) else {
            return Vector2::ZERO;
        };
        let text = ui_text_measure(&scene_node.data);
        let children = scene_node.get_children_ids();
        let child_size = if let SceneNodeData::UiTreeList(tree) = &scene_node.data {
            self.ui_tree_content_size(tree, available)
        } else if let Some(auto) = ui_auto_layout_from_data(&scene_node.data) {
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

    fn absolute_children_content_size(&self, children: &[NodeID], available: Vector2) -> Vector2 {
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

    fn ui_tree_content_size(&self, tree: &perro_ui::UiTreeList, available: Vector2) -> Vector2 {
        let mut width = 0.0_f32;
        let mut height = 0.0_f32;
        let mut count = 0_u32;
        for row in ui_tree_visible_rows(tree) {
            let Some(layout) = self
                .nodes
                .get(row.node)
                .and_then(|node| ui_root_from_data(&node.data))
                .and_then(|ui| ui.visible.then_some(&ui.layout))
            else {
                continue;
            };
            let indent = tree.indent * row.depth as f32;
            let child_available = Vector2::new((available.x - indent).max(0.0), available.y);
            let child_size = self.resolve_ui_size(row.node, child_available, None);
            width = width.max(indent + child_size.x + layout.margin.horizontal());
            height += child_size.y + layout.margin.vertical();
            count += 1;
        }
        if count > 1 {
            height += ui_v_spacing_amount(tree.v_spacing, available.y) * (count - 1) as f32;
        }
        Vector2::new(width, height)
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

    fn v_fill_height(&self, children: &[NodeID], height: f32, spacing: f32) -> f32 {
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

#[path = "render_ui/helpers.rs"]
mod helpers;

use helpers::*;

#[cfg(test)]
mod tests;

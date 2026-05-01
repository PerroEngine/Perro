use super::state::{DirtyState, UiButtonVisualState};
use super::{Runtime, RuntimeUiTiming};
use ahash::AHashMap;
use perro_ids::{NodeID, SignalID};
use perro_input::{KeyCode, MouseButton};
use perro_nodes::SceneNodeData;
use perro_render_bridge::{RenderCommand, UiCommand, UiRectState, UiTextAlignState};
use perro_runtime_context::sub_apis::SignalAPI;
use perro_structs::Vector2;
use perro_ui::{
    ComputedUiRect, UiBox, UiHorizontalAlign, UiLayoutData, UiLayoutMode, UiSizeMode, UiStyle,
    UiTextEdit, UiTransform, UiVerticalAlign,
};
use perro_variant::Variant;
use std::borrow::Cow;

const TEXT_EDIT_REPEAT_DELAY: f32 = 0.35;
const TEXT_EDIT_REPEAT_RATE: f32 = 0.035;

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

    pub fn extract_render_ui_commands(&mut self) {
        self.extract_render_ui_commands_inner(None);
    }

    pub fn extract_render_ui_commands_timed(&mut self) -> RuntimeUiTiming {
        let mut timing = RuntimeUiTiming::default();
        self.extract_render_ui_commands_inner(Some(&mut timing));
        timing
    }

    fn extract_render_ui_commands_inner(&mut self, timing: Option<&mut RuntimeUiTiming>) {
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
                && !parent.is_nil()
                && let Some(parent_node) = self.nodes.get(parent)
                && ui_auto_layout_from_data(&parent_node.data).is_some()
            {
                for &sibling in parent_node.get_children_ids() {
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
            }
        }
        for &node in &traversal_ids {
            if command_seen.insert(node) {
                command_ids.push(node);
            }
        }
        if input_changed || bootstrap_scan {
            self.collect_retained_button_command_ids(&mut command_ids, &mut command_seen);
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
            self.render_ui.retained_rects.remove(&node);
            self.render_ui.button_states.remove(&node);
            if self.render_ui.retained_commands.remove(&node).is_some() {
                self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode { node }));
            }
        }
        self.render_ui.removed_nodes = removed_nodes;

        let mut computed = std::mem::take(&mut self.render_ui.computed_rects);
        for node in traversal_ids.iter() {
            computed.remove(node);
        }
        let mut auto_layout_computed = std::mem::take(&mut self.render_ui.auto_layout_computed);
        auto_layout_computed.clear();
        let layout_start = timing.as_ref().map(|_| std::time::Instant::now());
        for node in traversal_ids.iter().copied() {
            let was_cached = computed.contains_key(&node);
            let before_len = computed.len();
            self.compute_ui_rect(node, root_rect, &mut computed, &mut auto_layout_computed);
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
            let effective_visible = self.is_effectively_visible(node);
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
            let rect_state = if let Some(rect) = computed.get(&node).copied() {
                ui_rect_state_from_node(&scene_node.data, rect, state)
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
            let retained_matches =
                self.render_ui
                    .retained_commands
                    .get(&node)
                    .is_some_and(|command| {
                        ui_command_matches_node(
                            command,
                            &scene_node.data,
                            rect_state,
                            state,
                            self.render_ui.focused_text_edit,
                        )
                    });
            if !retained_matches {
                let Some(command) = ui_command_from_node(
                    node,
                    &scene_node.data,
                    rect_state,
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
        auto_layout_computed: &mut ahash::AHashSet<NodeID>,
    ) -> Option<ComputedUiRect> {
        if let Some(rect) = computed.get(&node).copied() {
            return Some(rect);
        }

        let scene_node = self.nodes.get(node)?;
        let ui_root = ui_root_from_data(&scene_node.data)?;
        let parent_rect = if scene_node.parent.is_nil() {
            root_rect
        } else {
            self.compute_ui_rect(scene_node.parent, root_rect, computed, auto_layout_computed)
                .unwrap_or(root_rect)
        };
        let rect = if scene_node.parent.is_nil() {
            let size = self.resolve_ui_size(node, parent_rect.size, None);
            ui_root
                .layout
                .compute_rect_with_size(&ui_root.transform, parent_rect, size)
        } else {
            if self
                .nodes
                .get(scene_node.parent)
                .and_then(|parent| ui_auto_layout_from_data(&parent.data))
                .is_some()
            {
                if auto_layout_computed.insert(scene_node.parent) {
                    self.compute_ui_auto_children_rects(scene_node.parent, parent_rect, computed);
                }
                if let Some(rect) = computed.get(&node).copied() {
                    return Some(rect);
                }
            }
            self.compute_ui_child_rect(
                scene_node.parent,
                node,
                parent_rect,
                &ui_root.layout,
                &ui_root.transform,
            )
            .unwrap_or_else(|| {
                let parent_content = self
                    .nodes
                    .get(scene_node.parent)
                    .and_then(|parent| ui_root_from_data(&parent.data))
                    .map(|parent| parent_rect.inset(parent.layout.padding))
                    .unwrap_or(parent_rect);
                let parent_content = parent_content.inset(ui_root.layout.margin);
                let size = self.resolve_ui_size(node, parent_content.size, None);
                ui_root
                    .layout
                    .compute_rect_with_size(&ui_root.transform, parent_content, size)
            })
        };
        computed.insert(node, rect);
        Some(rect)
    }

    fn compute_ui_auto_children_rects(
        &self,
        parent: NodeID,
        parent_rect: ComputedUiRect,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<()> {
        let parent_node = self.nodes.get(parent)?;
        let parent_ui = ui_root_from_data(&parent_node.data)?;
        let auto_layout = ui_auto_layout_from_data(&parent_node.data)?;
        let content_rect = parent_rect.inset(parent_ui.layout.padding);
        match auto_layout.mode {
            UiLayoutMode::H => self.compute_ui_h_children_rects(
                &parent_ui.layout,
                parent_node.get_children_ids(),
                content_rect,
                auto_layout.h_spacing,
                computed,
            ),
            UiLayoutMode::V => self.compute_ui_v_children_rects(
                &parent_ui.layout,
                parent_node.get_children_ids(),
                content_rect,
                auto_layout.v_spacing,
                computed,
            ),
            UiLayoutMode::Grid => self.compute_ui_grid_children_rects(
                &parent_ui.layout,
                parent_node.get_children_ids(),
                content_rect,
                auto_layout,
                computed,
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

    fn collect_retained_button_command_ids(
        &self,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        for node in self.render_ui.retained_commands.keys().copied() {
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            if matches!(scene_node.data, SceneNodeData::UiButton(_)) && command_seen.insert(node) {
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
            if !edit.base.visible || !edit.base.input_enabled {
                continue;
            }
            let Some(rect) = computed.get(&node).copied() else {
                continue;
            };
            if rect.contains(point) && edit.base.layout.z_index >= best_z {
                best = Some(node);
                best_z = edit.base.layout.z_index;
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
            let next = if button.disabled || Some(node) != hovered {
                UiButtonVisualState::Neutral
            } else if mouse_down {
                UiButtonVisualState::Pressed
            } else {
                UiButtonVisualState::Hover
            };
            let prev = next_states.insert(node, next).unwrap_or_default();
            collect_button_events(node, prev, next, &mut events);
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
            let signals = self.button_event_signals(node, event);
            let params = [Variant::from(node)];
            for signal in signals {
                let _ = SignalAPI::signal_emit(self, signal, &params);
            }
        }
    }

    fn emit_text_edit_event(&mut self, node: NodeID, event: &str, text: Option<&str>) {
        let signals = self.text_edit_event_signals(node, event);
        if signals.is_empty() {
            return;
        }
        if let Some(text) = text {
            let params = [Variant::from(node), Variant::from(text)];
            for signal in signals {
                let _ = SignalAPI::signal_emit(self, signal, &params);
            }
        } else {
            let params = [Variant::from(node)];
            for signal in signals {
                let _ = SignalAPI::signal_emit(self, signal, &params);
            }
        }
    }

    fn button_event_signals(&self, node: NodeID, event: &str) -> Vec<SignalID> {
        let Some(scene_node) = self.nodes.get(node) else {
            return Vec::new();
        };
        let SceneNodeData::UiButton(button) = &scene_node.data else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(1 + button_custom_event_signals(button, event).len());
        let name = scene_node.name.as_ref();
        if !name.is_empty() {
            out.push(SignalID::from_string(&format!("{name}_{event}")));
        }
        out.extend(button_custom_event_signals(button, event).iter().copied());
        out
    }

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
                || !self.is_effectively_visible(node)
                || !matches!(
                    button.mouse_filter,
                    perro_ui::UiMouseFilter::Stop | perro_ui::UiMouseFilter::Pass
                )
            {
                continue;
            }
            let rect = computed.get(&node).copied().or_else(|| {
                self.render_ui
                    .retained_rects
                    .get(&node)
                    .map(computed_rect_from_state)
            });
            let Some(rect) = rect else {
                continue;
            };
            if !rect.contains(point) {
                continue;
            }
            match best {
                Some((best_node, best_z))
                    if best_z > button.layout.z_index
                        || (best_z == button.layout.z_index
                            && best_node.as_u64() > node.as_u64()) => {}
                _ => best = Some((node, button.layout.z_index)),
            }
        }
        best.map(|(node, _)| node)
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
                auto_layout,
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
                let center =
                    Vector2::new(x + layout.margin.left + size.x * 0.5, y) + transform.translation;
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
        content: ComputedUiRect,
        spacing: f32,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
    ) {
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
            let center =
                Vector2::new(x + layout.margin.left + size.x * 0.5, y) + transform.translation;
            computed.insert(sibling, ComputedUiRect::new(center, size));
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
                let center =
                    Vector2::new(x, y - layout.margin.top - size.y * 0.5) + transform.translation;
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
        content: ComputedUiRect,
        spacing: f32,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
    ) {
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
            let center =
                Vector2::new(x, y - layout.margin.top - size.y * 0.5) + transform.translation;
            computed.insert(sibling, ComputedUiRect::new(center, size));
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
        let used_width =
            cell_width * used_columns as f32 + auto.h_spacing * (used_columns - 1) as f32;
        let used_height = cell_height * row_count as f32 + auto.v_spacing * (row_count - 1) as f32;
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
        let cell_min_x = grid_min_x + col as f32 * (cell_width + auto.h_spacing);
        let cell_top_y = grid_top_y - row as f32 * (cell_height + auto.v_spacing);
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
        ) + transform.translation;
        Some(ComputedUiRect::new(center, size))
    }

    fn compute_ui_grid_children_rects(
        &self,
        parent_layout: &UiLayoutData,
        children: &[NodeID],
        content: ComputedUiRect,
        auto: UiAutoLayout,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
    ) {
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
        let used_width =
            cell_width * used_columns as f32 + auto.h_spacing * (used_columns - 1) as f32;
        let used_height = cell_height * row_count as f32 + auto.v_spacing * (row_count - 1) as f32;
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
            let cell_min_x = grid_min_x + col as f32 * (cell_width + auto.h_spacing);
            let cell_top_y = grid_top_y - row as f32 * (cell_height + auto.v_spacing);
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
            ) + transform.translation;
            computed.insert(child, ComputedUiRect::new(center, size));
            index += 1;
        }
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
        ui.transform.scale_size(ui.layout.clamp_size(size))
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
                        .and_then(|ui| ui.visible.then_some(&ui.layout))
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
        SceneNodeData::UiTextBox(node) => Some(&node.inner.base),
        SceneNodeData::UiTextBlock(node) => Some(&node.inner.base),
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

fn ui_fill_width(layout: &UiLayoutData, parent_layout: &UiLayoutData, available: f32) -> f32 {
    if layout.h_size == UiSizeMode::Fill || parent_layout.h_align == UiHorizontalAlign::Fill {
        (available - layout.margin.horizontal()).max(0.0)
    } else {
        0.0
    }
}

fn ui_fill_height(layout: &UiLayoutData, parent_layout: &UiLayoutData, available: f32) -> f32 {
    if layout.v_size == UiSizeMode::Fill || parent_layout.v_align == UiVerticalAlign::Fill {
        (available - layout.margin.vertical()).max(0.0)
    } else {
        0.0
    }
}

fn ui_text_measure(data: &SceneNodeData) -> Vector2 {
    match data {
        SceneNodeData::UiLabel(label) => measure_text(label.text.as_ref(), label.font_size),
        SceneNodeData::UiTextBox(text_box) => {
            measure_text(text_box.inner.text.as_ref(), text_box.inner.font_size)
        }
        SceneNodeData::UiTextBlock(text_block) => {
            measure_text(text_block.inner.text.as_ref(), text_block.inner.font_size)
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
    rect: UiRectState,
    button_state: UiButtonVisualState,
    focused_text_edit: Option<NodeID>,
) -> Option<UiCommand> {
    match data {
        SceneNodeData::UiPanel(panel) => Some(panel_command(node, rect, &panel.style)),
        SceneNodeData::UiButton(button) => {
            let style = button_style(button, button_state);
            Some(UiCommand::UpsertButton {
                node,
                rect,
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
            h_align: text_align_state(label.h_align),
            v_align: text_align_state(label.v_align),
        }),
        SceneNodeData::UiTextBox(text_box) => Some(text_edit_command(
            node,
            rect,
            &text_box.inner,
            false,
            focused_text_edit == Some(node),
        )),
        SceneNodeData::UiTextBlock(text_block) => Some(text_edit_command(
            node,
            rect,
            &text_block.inner,
            true,
            focused_text_edit == Some(node),
        )),
        _ => None,
    }
}

fn ui_rect_state_from_node(
    data: &SceneNodeData,
    rect: ComputedUiRect,
    button_state: UiButtonVisualState,
) -> Option<UiRectState> {
    if let SceneNodeData::UiButton(button) = data {
        return Some(button_rect_state(button, rect, button_state));
    }
    let ui = ui_root_from_data(data)?;
    Some(UiRectState {
        center: [rect.center.x, rect.center.y],
        size: [rect.size.x, rect.size.y],
        pivot: ui_pivot_state(&ui.transform),
        rotation_radians: ui.transform.rotation,
        z_index: ui.layout.z_index,
    })
}

fn button_rect_state(
    button: &perro_ui::UiButton,
    base_rect: ComputedUiRect,
    state: UiButtonVisualState,
) -> UiRectState {
    let ui = button_state_base(button, state).unwrap_or(&button.base);
    let size = match state {
        UiButtonVisualState::Neutral => base_rect.size,
        UiButtonVisualState::Hover | UiButtonVisualState::Pressed => ui
            .transform
            .scale_size(ui.layout.clamp_size(ui.layout.size.resolve(base_rect.size))),
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
        z_index: ui.layout.z_index,
    }
}

fn computed_rect_from_state(rect: &UiRectState) -> ComputedUiRect {
    ComputedUiRect::new(
        Vector2::new(rect.center[0], rect.center[1]),
        Vector2::new(rect.size[0], rect.size[1]),
    )
}

fn ui_pivot_state(transform: &UiTransform) -> [f32; 2] {
    let pivot = transform.pivot.resolve(Vector2::new(1.0, 1.0));
    [pivot.x, pivot.y]
}

fn ui_command_matches_node(
    command: &UiCommand,
    data: &SceneNodeData,
    rect: UiRectState,
    button_state: UiButtonVisualState,
    focused_text_edit: Option<NodeID>,
) -> bool {
    match (command, data) {
        (
            UiCommand::UpsertPanel {
                rect: command_rect,
                fill,
                stroke,
                stroke_width,
                corner_radius,
                ..
            },
            SceneNodeData::UiPanel(panel),
        ) => {
            *command_rect == rect
                && *fill == panel.style.fill.to_rgba()
                && *stroke == panel.style.stroke.to_rgba()
                && *stroke_width == panel.style.stroke_width
                && *corner_radius == panel.style.corner_radius
        }
        (
            UiCommand::UpsertButton {
                rect: command_rect,
                fill,
                stroke,
                stroke_width,
                corner_radius,
                disabled,
                ..
            },
            SceneNodeData::UiButton(button),
        ) => {
            let style = button_style(button, button_state);
            *command_rect == rect
                && *fill == style.fill.to_rgba()
                && *stroke == style.stroke.to_rgba()
                && *stroke_width == style.stroke_width
                && *corner_radius == style.corner_radius
                && *disabled == button.disabled
        }
        (
            UiCommand::UpsertLabel {
                rect: command_rect,
                text,
                color,
                font_size,
                h_align,
                v_align,
                ..
            },
            SceneNodeData::UiLabel(label),
        ) => {
            *command_rect == rect
                && text.as_ref() == label.text.as_ref()
                && *color == label.color.to_rgba()
                && *font_size == label.font_size
                && *h_align == text_align_state(label.h_align)
                && *v_align == text_align_state(label.v_align)
        }
        (UiCommand::UpsertTextEdit { .. }, SceneNodeData::UiTextBox(text_box)) => {
            *command
                == text_edit_command(
                    match command {
                        UiCommand::UpsertTextEdit { node, .. } => *node,
                        _ => NodeID::nil(),
                    },
                    rect,
                    &text_box.inner,
                    false,
                    focused_text_edit
                        == match command {
                            UiCommand::UpsertTextEdit { node, .. } => Some(*node),
                            _ => None,
                        },
                )
        }
        (UiCommand::UpsertTextEdit { .. }, SceneNodeData::UiTextBlock(text_block)) => {
            *command
                == text_edit_command(
                    match command {
                        UiCommand::UpsertTextEdit { node, .. } => *node,
                        _ => NodeID::nil(),
                    },
                    rect,
                    &text_block.inner,
                    true,
                    focused_text_edit
                        == match command {
                            UiCommand::UpsertTextEdit { node, .. } => Some(*node),
                            _ => None,
                        },
                )
        }
        _ => false,
    }
}

fn button_style(button: &perro_ui::UiButton, state: UiButtonVisualState) -> &UiStyle {
    if button.disabled {
        return &button.style;
    }
    match state {
        UiButtonVisualState::Neutral => &button.style,
        UiButtonVisualState::Hover => &button.hover_style,
        UiButtonVisualState::Pressed => &button.pressed_style,
    }
}

fn button_state_base(
    button: &perro_ui::UiButton,
    state: UiButtonVisualState,
) -> Option<&perro_ui::UiBox> {
    if button.disabled {
        return None;
    }
    match state {
        UiButtonVisualState::Neutral => None,
        UiButtonVisualState::Hover => button.hover_base.as_ref(),
        UiButtonVisualState::Pressed => button.pressed_base.as_ref(),
    }
}

fn button_custom_event_signals<'a>(button: &'a perro_ui::UiButton, event: &str) -> &'a [SignalID] {
    match event {
        "hover_enter" => &button.hover_signals,
        "hover_exit" => &button.hover_exit_signals,
        "pressed" => &button.pressed_signals,
        "released" => &button.released_signals,
        "click" => &button.click_signals,
        _ => &[],
    }
}

fn text_edit_custom_event_signals<'a>(edit: &'a UiTextEdit, event: &str) -> &'a [SignalID] {
    match event {
        "hovered" => &edit.hover_signals,
        "unhovered" => &edit.hover_exit_signals,
        "focused" => &edit.focused_signals,
        "unfocused" => &edit.unfocused_signals,
        "text_changed" => &edit.text_changed_signals,
        _ => &[],
    }
}

fn collect_button_events(
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

fn text_align_state(align: perro_ui::UiTextAlign) -> UiTextAlignState {
    match align {
        perro_ui::UiTextAlign::Start => UiTextAlignState::Start,
        perro_ui::UiTextAlign::Center => UiTextAlignState::Center,
        perro_ui::UiTextAlign::End => UiTextAlignState::End,
    }
}

fn text_edit_command(
    node: NodeID,
    rect: UiRectState,
    edit: &UiTextEdit,
    multiline: bool,
    focused: bool,
) -> UiCommand {
    let focused_style = &edit.focused_style;
    let style = &edit.style;
    UiCommand::UpsertTextEdit {
        node,
        rect,
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
        },
        corner_radius: if focused {
            focused_style.corner_radius
        } else {
            style.corner_radius
        },
        text: Cow::Owned(edit.text.to_string()),
        placeholder: Cow::Owned(edit.placeholder.to_string()),
        color: edit.color.to_rgba(),
        placeholder_color: edit.placeholder_color.to_rgba(),
        selection_color: edit.selection_color.to_rgba(),
        caret_color: edit.caret_color.to_rgba(),
        font_size: edit.font_size,
        padding: [
            edit.padding.left,
            edit.padding.top,
            edit.padding.right,
            edit.padding.bottom,
        ],
        scroll: [edit.h_scroll, edit.v_scroll],
        caret: edit.caret,
        anchor: edit.anchor,
        focused,
        multiline,
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

fn text_edit_ref(data: &SceneNodeData) -> Option<&UiTextEdit> {
    match data {
        SceneNodeData::UiTextBox(node) => Some(&node.inner),
        SceneNodeData::UiTextBlock(node) => Some(&node.inner),
        _ => None,
    }
}

fn text_edit_mut(data: &mut SceneNodeData) -> Option<&mut UiTextEdit> {
    match data {
        SceneNodeData::UiTextBox(node) => Some(&mut node.inner),
        SceneNodeData::UiTextBlock(node) => Some(&mut node.inner),
        _ => None,
    }
}

fn text_edit_keys() -> &'static [KeyCode] {
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

fn repeatable_text_edit_keys() -> &'static [KeyCode] {
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

fn insert_text_input(edit: &mut UiTextEdit, text: &str) -> bool {
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

fn apply_text_edit_key_input(
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

fn copy_selection_to_clipboard(edit: &UiTextEdit) -> bool {
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

fn read_clipboard_text(multiline: bool) -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let text = clipboard.get_text().ok()?;
    let text = normalize_text_input(&text, multiline);
    (!text.is_empty()).then_some(text)
}

fn replace_selection(edit: &mut UiTextEdit, replacement: &str) {
    let mut text = edit.text.to_string();
    let (start, end) = selection_range(edit);
    text.replace_range(start..end, replacement);
    let caret = start + replacement.len();
    edit.text = Cow::Owned(text);
    edit.caret = caret;
    edit.anchor = caret;
}

fn selection_range(edit: &UiTextEdit) -> (usize, usize) {
    let text = edit.text.as_ref();
    let a = clamp_char_boundary(text, edit.anchor);
    let b = clamp_char_boundary(text, edit.caret);
    if a <= b { (a, b) } else { (b, a) }
}

fn backspace(edit: &mut UiTextEdit) -> bool {
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

fn delete(edit: &mut UiTextEdit) -> bool {
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

fn move_caret(edit: &mut UiTextEdit, index: usize, extend: bool) {
    let index = clamp_char_boundary(edit.text.as_ref(), index);
    edit.caret = index;
    if !extend {
        edit.anchor = index;
    }
}

fn move_vertical(edit: &mut UiTextEdit, delta: i32, extend: bool) {
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

fn ensure_caret_visible(edit: &mut UiTextEdit, rect: Option<ComputedUiRect>) {
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

fn text_index_from_local(edit: &UiTextEdit, local: Vector2) -> usize {
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

fn caret_text_pos(edit: &UiTextEdit) -> Vector2 {
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
struct TextRange {
    start: usize,
    end: usize,
}

fn line_for_index(text: &str, index: usize) -> TextRange {
    text_line_ranges(text)
        .into_iter()
        .find(|line| index >= line.start && index <= line.end)
        .unwrap_or(TextRange {
            start: 0,
            end: text.len(),
        })
}

fn text_line_ranges(text: &str) -> Vec<TextRange> {
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

fn normalize_text_input(text: &str, multiline: bool) -> String {
    if multiline {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.replace(['\r', '\n', '\t'], " ")
    }
}

fn index_at_col(text: &str, line: TextRange, col: usize) -> usize {
    for (count, (idx, _)) in text[line.start..line.end].char_indices().enumerate() {
        if count == col {
            return line.start + idx;
        }
    }
    line.end
}

fn prev_char(text: &str, index: usize) -> usize {
    let index = clamp_char_boundary(text, index);
    text[..index]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(index)
}

fn next_char(text: &str, index: usize) -> usize {
    let index = clamp_char_boundary(text, index);
    text[index..]
        .chars()
        .next()
        .map(|ch| index + ch.len_utf8())
        .unwrap_or(index)
}

fn clamp_char_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn text_char_width(edit: &UiTextEdit) -> f32 {
    (edit.font_size * 0.6).max(1.0)
}

fn text_line_height(edit: &UiTextEdit) -> f32 {
    (edit.font_size * 1.25).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_nodes::{SceneNode, SceneNodeData};
    use perro_structs::Color;
    use perro_ui::{UiAnchor, UiGrid, UiHLayout, UiPanel, UiVector2};

    #[test]
    fn unchanged_ui_skips_redundant_upsert() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_panel(&mut runtime, [120.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert_eq!(commands.iter().filter(|cmd| matches!(cmd, RenderCommand::Ui(UiCommand::UpsertPanel { node: n, .. }) if *n == node)).count(), 1);

        runtime.clear_dirty_flags();
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.is_empty());
    }

    #[test]
    fn viewport_resize_recomputes_percent_ui_rects() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut panel = UiPanel::new();
        panel.layout.anchor = UiAnchor::TopRight;
        panel.layout.size = UiVector2::ratio(0.5, 0.25);
        panel.style.fill = Color::new(0.1, 0.2, 0.3, 1.0);
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(panel));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_viewport_size(1200, 900);
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node: n, rect, .. })
                if *n == node
                    && rect.size == [600.0, 225.0]
                    && rect.center == [300.0, 337.5]
        )));
    }

    #[test]
    fn dirty_ui_node_emits_changed_upsert_only() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_panel(&mut runtime, [120.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        runtime.clear_dirty_flags();

        if let Some(scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.style.fill = Color::new(0.8, 0.1, 0.1, 1.0);
        }
        runtime.mark_needs_rerender(node);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);

        assert_eq!(commands.len(), 1);
        assert!(
            matches!(&commands[0], RenderCommand::Ui(UiCommand::UpsertPanel { node: n, fill, .. }) if *n == node && *fill == [0.8, 0.1, 0.1, 1.0])
        );
    }

    #[test]
    fn button_uses_hover_and_pressed_styles_from_mouse_state() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_button(&mut runtime, [120.0, 40.0]);

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == [0.1, 0.2, 0.3, 1.0]
        )));
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == [0.2, 0.3, 0.4, 1.0]
        )));

        runtime.clear_dirty_flags();
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == [0.3, 0.4, 0.5, 1.0]
        )));
    }

    #[test]
    fn button_hover_requests_cursor_icon_and_unhover_restores_default() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_button(&mut runtime, [120.0, 40.0]);
        if let Some(scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.cursor_icon = perro_ui::CursorIcon::Grab;
        }

        runtime.extract_render_ui_commands();
        let _ = runtime.take_cursor_icon_request();
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.take_cursor_icon_request(),
            Some(perro_ui::CursorIcon::Grab)
        );
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(0.0, 0.0);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.take_cursor_icon_request(),
            Some(perro_ui::CursorIcon::Default)
        );
    }

    #[test]
    fn text_box_focus_accepts_committed_text_and_backspace() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let mut text_box = perro_ui::UiTextBox::new();
        text_box.inner.base.layout.size = UiVector2::pixels(200.0, 40.0);
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiTextBox(text_box));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();
        runtime.set_mouse_button_state(MouseButton::Left, false);
        runtime.begin_input_frame();

        runtime.push_text_input("abc");
        runtime.extract_render_ui_commands();
        let text = runtime.nodes.get(node).and_then(|scene_node| {
            if let SceneNodeData::UiTextBox(text_box) = &scene_node.data {
                Some(text_box.inner.text.as_ref())
            } else {
                None
            }
        });
        assert_eq!(text, Some("abc"));

        runtime.clear_dirty_flags();
        runtime.begin_input_frame();
        runtime.set_key_state(KeyCode::Backspace, true);
        runtime.extract_render_ui_commands();
        let text = runtime.nodes.get(node).and_then(|scene_node| {
            if let SceneNodeData::UiTextBox(text_box) = &scene_node.data {
                Some(text_box.inner.text.as_ref())
            } else {
                None
            }
        });
        assert_eq!(text, Some("ab"));
    }

    #[test]
    fn text_box_ctrl_shortcut_does_not_insert_key_text() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let mut text_box = perro_ui::UiTextBox::new();
        text_box.inner.base.layout.size = UiVector2::pixels(200.0, 40.0);
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiTextBox(text_box));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();
        runtime.set_mouse_button_state(MouseButton::Left, false);
        runtime.begin_input_frame();

        runtime.set_key_state(KeyCode::ControlLeft, true);
        runtime.set_key_state(KeyCode::KeyA, true);
        runtime.push_text_input("a");
        runtime.extract_render_ui_commands();

        let text = runtime.nodes.get(node).and_then(|scene_node| {
            if let SceneNodeData::UiTextBox(text_box) = &scene_node.data {
                Some(text_box.inner.text.as_ref())
            } else {
                None
            }
        });
        assert_eq!(text, Some(""));
    }

    #[test]
    fn held_backspace_repeats_in_text_box() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let mut text_box = perro_ui::UiTextBox::new();
        text_box.inner.base.layout.size = UiVector2::pixels(200.0, 40.0);
        text_box.inner.text = Cow::Borrowed("abcd");
        text_box.inner.caret = 4;
        text_box.inner.anchor = 4;
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiTextBox(text_box));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();
        runtime.set_mouse_button_state(MouseButton::Left, false);
        runtime.begin_input_frame();

        runtime.set_key_state(KeyCode::Backspace, true);
        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();
        runtime.begin_input_frame();
        runtime.update(0.36);
        runtime.extract_render_ui_commands();

        let text = runtime.nodes.get(node).and_then(|scene_node| {
            if let SceneNodeData::UiTextBox(text_box) = &scene_node.data {
                Some(text_box.inner.text.as_ref())
            } else {
                None
            }
        });
        assert_eq!(text, Some("ab"));
    }

    #[test]
    fn button_state_base_overrides_rect_transform() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut button = perro_ui::UiButton::new();
        button.layout.size = UiVector2::pixels(120.0, 40.0);
        let mut hover_base = button.base.clone();
        hover_base.layout.size = UiVector2::pixels(150.0, 48.0);
        hover_base.transform.translation = Vector2::new(6.0, -3.0);
        hover_base.transform.rotation = 0.25;
        button.hover_base = Some(hover_base);
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiButton(button));

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, rect, .. })
                if *n == node
                    && rect.center == [6.0, -3.0]
                    && rect.size == [150.0, 48.0]
                    && rect.rotation_radians == 0.25
        )));
    }

    #[test]
    fn button_event_signals_include_named_and_custom_signals() {
        let mut runtime = Runtime::new();
        let named = insert_button(&mut runtime, [120.0, 40.0]);
        runtime.nodes.get_mut(named).expect("named button").name = Cow::Borrowed("play");
        assert_eq!(
            runtime.button_event_signals(named, "click"),
            vec![SignalID::from_string("play_click")]
        );

        let mut button = perro_ui::UiButton::new();
        button
            .pressed_signals
            .push(SignalID::from_string("custom_a"));
        button
            .pressed_signals
            .push(SignalID::from_string("custom_b"));
        let custom = insert_ui_node(&mut runtime, SceneNodeData::UiButton(button));
        runtime.nodes.get_mut(custom).expect("custom button").name = Cow::Borrowed("fire");
        assert_eq!(
            runtime.button_event_signals(custom, "pressed"),
            vec![
                SignalID::from_string("fire_pressed"),
                SignalID::from_string("custom_a"),
                SignalID::from_string("custom_b"),
            ]
        );
    }

    #[test]
    fn text_edit_event_signals_include_named_and_custom_signals() {
        let mut runtime = Runtime::new();
        let named = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiTextBox(perro_ui::UiTextBox::new()),
        );
        runtime.nodes.get_mut(named).expect("named text box").name = Cow::Borrowed("name");
        assert_eq!(
            runtime.text_edit_event_signals(named, "focused"),
            vec![SignalID::from_string("name_focused")]
        );
        assert_eq!(
            runtime.text_edit_event_signals(named, "text_changed"),
            vec![SignalID::from_string("name_text_changed")]
        );

        let mut text_block = perro_ui::UiTextBlock::new();
        text_block
            .inner
            .hover_signals
            .push(SignalID::from_string("custom_hover"));
        text_block
            .inner
            .text_changed_signals
            .push(SignalID::from_string("custom_text"));
        let custom = insert_ui_node(&mut runtime, SceneNodeData::UiTextBlock(text_block));
        runtime
            .nodes
            .get_mut(custom)
            .expect("custom text block")
            .name = Cow::Borrowed("bio");
        assert_eq!(
            runtime.text_edit_event_signals(custom, "hovered"),
            vec![
                SignalID::from_string("bio_hovered"),
                SignalID::from_string("custom_hover"),
            ]
        );
        assert_eq!(
            runtime.text_edit_event_signals(custom, "text_changed"),
            vec![
                SignalID::from_string("bio_text_changed"),
                SignalID::from_string("custom_text"),
            ]
        );
    }

    #[test]
    fn default_hlayout_centers_child_group() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiHLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 100.0);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(layout));
        let child = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, parent, child);

        runtime.extract_render_ui_commands();

        let child_rect = runtime
            .render_ui
            .computed_rects
            .get(&child)
            .expect("child rect exists");
        assert_eq!(child_rect.center, Vector2::ZERO);
    }

    #[test]
    fn hlayout_ignores_invisible_child_space() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiHLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 100.0);
        layout.inner.spacing = 10.0;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(layout));
        let first = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let middle = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        let last = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.3, 0.4, 0.5, 1.0));
        attach_child(&mut runtime, parent, first);
        attach_child(&mut runtime, parent, middle);
        attach_child(&mut runtime, parent, last);

        set_panel_visible(&mut runtime, middle, false);
        runtime.mark_ui_dirty(
            middle,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        runtime.extract_render_ui_commands();

        let first_rect = runtime
            .render_ui
            .computed_rects
            .get(&first)
            .expect("first rect exists");
        let last_rect = runtime
            .render_ui
            .computed_rects
            .get(&last)
            .expect("last rect exists");
        assert_eq!(first_rect.center.x, -35.0);
        assert_eq!(last_rect.center.x, 35.0);
    }

    #[test]
    fn default_grid_centers_rows_in_parent() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut grid = UiGrid::new();
        grid.layout.size = UiVector2::pixels(300.0, 200.0);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiGrid(grid));
        let child = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, parent, child);

        runtime.extract_render_ui_commands();

        let child_rect = runtime
            .render_ui
            .computed_rects
            .get(&child)
            .expect("child rect exists");
        assert_eq!(child_rect.center, Vector2::ZERO);
    }

    #[test]
    fn grid_columns_auto_wrap_into_centered_rows() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut grid = UiGrid::new();
        grid.layout.size = UiVector2::pixels(300.0, 200.0);
        grid.columns = 3;
        grid.h_spacing = 10.0;
        grid.v_spacing = 10.0;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiGrid(grid));

        let mut children = Vec::new();
        for _ in 0..6 {
            let child = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
            attach_child(&mut runtime, parent, child);
            children.push(child);
        }

        runtime.extract_render_ui_commands();

        let first = runtime
            .render_ui
            .computed_rects
            .get(&children[0])
            .expect("first rect exists");
        let fourth = runtime
            .render_ui
            .computed_rects
            .get(&children[3])
            .expect("fourth rect exists");
        assert_eq!(first.center, Vector2::new(-70.0, 25.0));
        assert_eq!(fourth.center, Vector2::new(-70.0, -25.0));
    }

    #[test]
    fn grid_uses_uniform_cells_for_even_column_spacing() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut grid = UiGrid::new();
        grid.layout.size = UiVector2::pixels(400.0, 200.0);
        grid.columns = 3;
        grid.h_spacing = 10.0;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiGrid(grid));

        let first = insert_panel(&mut runtime, [80.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let middle = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let last = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, parent, first);
        attach_child(&mut runtime, parent, middle);
        attach_child(&mut runtime, parent, last);

        runtime.extract_render_ui_commands();

        let first = runtime
            .render_ui
            .computed_rects
            .get(&first)
            .expect("first rect exists");
        let middle = runtime
            .render_ui
            .computed_rects
            .get(&middle)
            .expect("middle rect exists");
        let last = runtime
            .render_ui
            .computed_rects
            .get(&last)
            .expect("last rect exists");
        assert_eq!(
            middle.center.x - first.center.x,
            last.center.x - middle.center.x
        );
    }

    #[test]
    fn grid_ignores_invisible_child_index() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut grid = UiGrid::new();
        grid.layout.size = UiVector2::pixels(300.0, 200.0);
        grid.columns = 3;
        grid.h_spacing = 10.0;
        grid.v_spacing = 10.0;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiGrid(grid));

        let first = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let hidden = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        let third = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.3, 0.4, 0.5, 1.0));
        let fourth = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.4, 0.5, 0.6, 1.0));
        attach_child(&mut runtime, parent, first);
        attach_child(&mut runtime, parent, hidden);
        attach_child(&mut runtime, parent, third);
        attach_child(&mut runtime, parent, fourth);

        set_panel_visible(&mut runtime, hidden, false);
        runtime.mark_ui_dirty(
            hidden,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        runtime.extract_render_ui_commands();

        let first_rect = runtime
            .render_ui
            .computed_rects
            .get(&first)
            .expect("first rect exists");
        let third_rect = runtime
            .render_ui
            .computed_rects
            .get(&third)
            .expect("third rect exists");
        let fourth_rect = runtime
            .render_ui
            .computed_rects
            .get(&fourth)
            .expect("fourth rect exists");
        assert_eq!(first_rect.center, Vector2::new(-70.0, 0.0));
        assert_eq!(third_rect.center, Vector2::ZERO);
        assert_eq!(fourth_rect.center, Vector2::new(70.0, 0.0));
    }

    #[test]
    fn ui_transform_dirty_updates_only_changed_branch() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiHLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 100.0);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(layout));
        let child = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let sibling = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        attach_child(&mut runtime, parent, child);
        attach_child(&mut runtime, parent, sibling);

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        runtime.clear_dirty_flags();

        if let Some(scene_node) = runtime.nodes.get_mut(child)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.transform.translation.x = 24.0;
        }
        runtime.mark_ui_dirty(
            child,
            Runtime::UI_DIRTY_TRANSFORM | Runtime::UI_DIRTY_COMMANDS,
        );
        let timing = runtime.extract_render_ui_commands_timed();
        commands.clear();
        runtime.drain_render_commands(&mut commands);

        assert_eq!(timing.affected_nodes, 1);
        assert_eq!(timing.command_nodes, 1);
        assert_eq!(
            commands
                .iter()
                .filter(|cmd| matches!(cmd, RenderCommand::Ui(UiCommand::UpsertPanel { node, .. }) if *node == child))
                .count(),
            1
        );
        assert!(
            !commands
                .iter()
                .any(|cmd| matches!(cmd, RenderCommand::Ui(UiCommand::UpsertPanel { node, .. }) if *node == sibling))
        );
    }

    #[test]
    fn ui_layout_parent_dirty_updates_auto_layout_siblings() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiHLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 100.0);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(layout));
        let child = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let sibling = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        attach_child(&mut runtime, parent, child);
        attach_child(&mut runtime, parent, sibling);

        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();

        if let Some(scene_node) = runtime.nodes.get_mut(child)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.size = UiVector2::pixels(90.0, 40.0);
        }
        runtime.mark_ui_dirty(
            child,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        let timing = runtime.extract_render_ui_commands_timed();

        assert_eq!(timing.affected_nodes, 2);
        assert_eq!(timing.command_nodes, 2);
    }

    fn insert_panel(runtime: &mut Runtime, size: [f32; 2], fill: Color) -> NodeID {
        let mut panel = UiPanel::new();
        panel.layout.size = UiVector2::pixels(size[0], size[1]);
        panel.style.fill = fill;
        insert_ui_node(runtime, SceneNodeData::UiPanel(panel))
    }

    fn insert_button(runtime: &mut Runtime, size: [f32; 2]) -> NodeID {
        let mut button = perro_ui::UiButton::new();
        button.layout.size = UiVector2::pixels(size[0], size[1]);
        button.style.fill = Color::new(0.1, 0.2, 0.3, 1.0);
        button.hover_style.fill = Color::new(0.2, 0.3, 0.4, 1.0);
        button.pressed_style.fill = Color::new(0.3, 0.4, 0.5, 1.0);
        insert_ui_node(runtime, SceneNodeData::UiButton(button))
    }

    fn set_panel_visible(runtime: &mut Runtime, node: NodeID, visible: bool) {
        if let Some(scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.visible = visible;
        }
    }

    fn insert_ui_node(runtime: &mut Runtime, data: SceneNodeData) -> NodeID {
        let node = runtime.nodes.insert(SceneNode::new(data));
        runtime
            .nodes
            .get_mut(node)
            .expect("inserted node exists")
            .id = node;
        runtime.mark_needs_rerender(node);
        node
    }

    fn attach_child(runtime: &mut Runtime, parent: NodeID, child: NodeID) {
        runtime
            .nodes
            .get_mut(parent)
            .expect("parent exists")
            .add_child(child);
        runtime.nodes.get_mut(child).expect("child exists").parent = parent;
        runtime.mark_needs_rerender(parent);
        runtime.mark_needs_rerender(child);
    }
}

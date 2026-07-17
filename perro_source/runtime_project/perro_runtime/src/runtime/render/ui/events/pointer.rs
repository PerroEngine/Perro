use super::*;

impl Runtime {
    pub(super) fn ui_pointer_screen_point(&mut self) -> Vector2 {
        if let Some(point) = self.render_ui.pointer_screen_point {
            return point;
        }
        let mouse = self.input.mouse_position();
        let viewport = self.input.viewport_size();
        let point = Vector2::new((mouse.x - 0.5) * viewport.x, (mouse.y - 0.5) * viewport.y);
        self.render_ui.pointer_screen_point = Some(point);
        point
    }

    pub(in super::super) fn ui_pointer_changed(&self) -> bool {
        let pointer = (
            self.input.mouse_position(),
            self.input.is_mouse_down(MouseButton::Left),
        );
        self.render_ui.last_ui_pointer != Some(pointer)
    }

    pub(in super::super) fn ui_nav_input_changed(&self) -> bool {
        self.input.is_key_pressed(KeyCode::Tab)
            || self.input.is_key_pressed(KeyCode::Enter)
            || self.input.is_key_pressed(KeyCode::Space)
            || self.input.is_key_released(KeyCode::Enter)
            || self.input.is_key_released(KeyCode::Space)
            || self.ui_gamepad_dpad_direction().is_some()
            || self.ui_action_pressed().is_some()
            || self.ui_action_released().is_some()
            || self.ui_stick_nav_direction().is_some()
            || self.render_ui.ui_nav_repeat_dir.is_some()
    }

    pub(in super::super) fn ui_scroll_input_changed(&self) -> bool {
        self.input.mouse_wheel() != Vector2::ZERO
            || ui_scroll_keys()
                .iter()
                .any(|&key| self.input.is_key_pressed(key) || self.input.is_key_down(key))
    }

    pub(in super::super) fn ui_text_input_changed(&self) -> bool {
        !self.input.text_inputs().is_empty()
            || self.input.mouse_wheel() != Vector2::ZERO
            || text_edit_keys()
                .iter()
                .any(|&key| self.input.is_key_pressed(key) || self.input.is_key_down(key))
    }

    pub(in super::super) fn process_ui_focus_input(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        let pointer_point = self.ui_pointer_screen_point();
        if self.input.is_mouse_pressed(MouseButton::Left) {
            let hit = self
                .hovered_focusable(computed, UiInputSource::Kbm, pointer_point)
                .map(|hit| hit.node);
            self.set_ui_focus(hit, command_ids, command_seen);
        }

        if self.input.is_key_pressed(KeyCode::Tab) {
            let reverse = self.input.is_key_down(KeyCode::ShiftLeft)
                || self.input.is_key_down(KeyCode::ShiftRight);
            if let Some(next) = self.next_tab_focus(computed, reverse, UiInputSource::Kbm) {
                self.set_ui_focus(Some(next), command_ids, command_seen);
            }
        }

        let dir = self
            .ui_gamepad_dpad_direction()
            .or_else(|| self.ui_repeating_stick_nav_direction());
        if let Some(nav) = dir
            && let Some(next) = self.next_directional_focus(computed, nav.dir, nav.source)
        {
            self.set_ui_focus(Some(next), command_ids, command_seen);
        }

        self.process_focused_button_action();
    }

    pub(in super::super) fn process_text_edit_input(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &AHashMap<NodeID, Vector2>,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        let virtual_font_scale = self.ui_virtual_font_scale(self.input.viewport_size());
        let mouse_pos = self.input.mouse_position();
        let pointer_point = self.ui_pointer_screen_point();
        let mouse_pressed = self.input.is_mouse_pressed(MouseButton::Left);
        let mouse_down = self.input.is_mouse_down(MouseButton::Left);
        let hovered = self.hovered_text_edit(computed, UiInputSource::Kbm, pointer_point);
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
            self.render_ui.pressed_text_edit = hit;
            if let Some(node) = hit {
                self.seek_text_edit_at_mouse(node, computed, computed_scales, mouse_pos, false);
                if command_seen.insert(node) {
                    command_ids.push(node);
                }
            }
        } else if mouse_down
            && let Some(node) = self.render_ui.pressed_text_edit
            && self.render_ui.focused_text_edit == Some(node)
        {
            self.seek_text_edit_at_mouse(node, computed, computed_scales, mouse_pos, true);
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
            if self.render_ui.focused_ui_node == Some(focused) {
                self.render_ui.focused_ui_node = None;
            }
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
        if let Some(scene_node) = self.nodes.get_mut_untracked(focused)
            && let Some(edit) = text_edit_mut(&mut scene_node.data)
        {
            let old_text = edit.text.to_string();
            if !ctrl {
                for text in text_inputs {
                    changed |= insert_text_input(edit, &text);
                }
            }
            changed |= apply_text_edit_key_input(edit, shift, ctrl, repeat_key, &self.input);
            let node_scale = computed_scales
                .get(&focused)
                .copied()
                .unwrap_or(Vector2::ONE);
            // Caret clamp only after edits; wheel scroll must stay free.
            if changed {
                ensure_caret_visible(
                    edit,
                    computed.get(&focused).copied(),
                    node_scale,
                    virtual_font_scale,
                );
            }
            if edit.multiline && wheel.y != 0.0 {
                let font_size = computed
                    .get(&focused)
                    .map(|rect| {
                        text_edit_effective_font_size(
                            edit,
                            [rect.size.x, rect.size.y],
                            node_scale,
                            virtual_font_scale,
                        )
                    })
                    .unwrap_or(edit.font_size);
                edit.v_scroll = (edit.v_scroll - wheel.y * font_size * 2.0).max(0.0);
                changed = true;
            }
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
            if let Some(text) = changed_text.as_deref() {
                self.process_color_picker_text_edit(focused, text, command_ids, command_seen);
            }
            self.emit_text_edit_event(focused, "text_changed", changed_text.as_deref());
        }
    }

    pub(in super::super) fn hovered_text_edit(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        source: UiInputSource,
        point: Vector2,
    ) -> Option<NodeID> {
        let mut best = None;
        let mut best_z = i32::MIN;
        for &node in &self.render_ui.visible_text_edits {
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            let Some(edit) = text_edit_ref(&scene_node.data) else {
                continue;
            };
            if !edit.base.visible
                || !edit.base.input_enabled
                || !self.ui_input_mask_accepts(&edit.input_mask, source)
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
            if rect.contains(point)
                && self.ui_point_in_effective_clip(node, computed, point)
                && z >= best_z
            {
                best = Some(node);
                best_z = z;
            }
        }
        best
    }

    pub(in super::super) fn seek_text_edit_at_mouse(
        &mut self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &AHashMap<NodeID, Vector2>,
        mouse_pos: Vector2,
        extend: bool,
    ) {
        let viewport = self.input.viewport_size();
        let virtual_font_scale = self.ui_virtual_font_scale(viewport);
        let Some(rect) = computed.get(&node).copied() else {
            return;
        };
        let node_scale = computed_scales.get(&node).copied().unwrap_or(Vector2::ONE);
        if let Some(scene_node) = self.nodes.get_mut_untracked(node)
            && let Some(edit) = text_edit_mut(&mut scene_node.data)
        {
            let point = Vector2::new(
                (mouse_pos.x - 0.5) * viewport.x,
                (mouse_pos.y - 0.5) * viewport.y,
            );
            let font_size = text_edit_effective_font_size(
                edit,
                [rect.size.x, rect.size.y],
                node_scale,
                virtual_font_scale,
            );
            let pad = scaled_text_edit_padding(edit, node_scale);
            let min = rect.min();
            let local = Vector2::new(
                point.x - min.x - pad[0] + edit.h_scroll,
                rect.max().y - point.y - pad[1] + edit.v_scroll,
            );
            let index = text_index_from_local(edit, local, font_size);
            edit.caret = index;
            if !extend {
                edit.anchor = index;
            }
            ensure_caret_visible(edit, Some(rect), node_scale, virtual_font_scale);
        }
    }
}

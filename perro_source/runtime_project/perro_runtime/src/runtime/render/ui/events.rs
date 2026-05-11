use super::*;

impl Runtime {
    pub(super) fn ui_pointer_changed(&self) -> bool {
        let pointer = (
            self.input.mouse_position(),
            self.input.is_mouse_down(MouseButton::Left),
        );
        self.render_ui.last_ui_pointer != Some(pointer)
    }

    pub(super) fn ui_text_input_changed(&self) -> bool {
        !self.input.text_inputs().is_empty()
            || self.input.mouse_wheel() != Vector2::ZERO
            || text_edit_keys()
                .iter()
                .any(|&key| self.input.is_key_pressed(key) || self.input.is_key_down(key))
    }

    pub(super) fn process_text_edit_input(
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

    pub(super) fn text_edit_repeat_key(&mut self, ctrl: bool) -> Option<KeyCode> {
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

    pub(super) fn hovered_text_edit(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<NodeID> {
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

    pub(super) fn seek_text_edit_at_mouse(
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

    pub(super) fn refresh_button_visual_states(
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

    pub(super) fn emit_button_events(&mut self, events: &[(NodeID, &'static str)]) {
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

    pub(super) fn emit_text_edit_event(&mut self, node: NodeID, event: &str, text: Option<&str>) {
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

    pub(super) fn collect_button_event_signals(&mut self, node: NodeID, event: &str) {
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

    pub(super) fn collect_text_edit_event_signals(&mut self, node: NodeID, event: &str) {
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
    pub(super) fn button_event_signals(&self, node: NodeID, event: &str) -> Vec<SignalID> {
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
    pub(super) fn text_edit_event_signals(&self, node: NodeID, event: &str) -> Vec<SignalID> {
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

    pub(super) fn hovered_button(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<NodeID> {
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
}

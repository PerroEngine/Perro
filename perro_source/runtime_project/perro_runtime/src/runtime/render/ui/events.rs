use super::*;

#[derive(Clone, Copy, Debug)]
struct UiFocusCandidate {
    node: NodeID,
    rect: ComputedUiRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UiInputSource {
    Kbm,
    Gamepad(usize),
    JoyCon(usize),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct UiDirectionalNav {
    source: UiInputSource,
    dir: [i8; 2],
}

impl Runtime {
    pub(super) fn ui_pointer_changed(&self) -> bool {
        let pointer = (
            self.input.mouse_position(),
            self.input.is_mouse_down(MouseButton::Left),
        );
        self.render_ui.last_ui_pointer != Some(pointer)
    }

    pub(super) fn ui_nav_input_changed(&self) -> bool {
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

    pub(super) fn ui_text_input_changed(&self) -> bool {
        !self.input.text_inputs().is_empty()
            || self.input.mouse_wheel() != Vector2::ZERO
            || text_edit_keys()
                .iter()
                .any(|&key| self.input.is_key_pressed(key) || self.input.is_key_down(key))
    }

    pub(super) fn process_ui_focus_input(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        if self.input.is_mouse_pressed(MouseButton::Left) {
            let hit = self
                .hovered_focusable(computed, UiInputSource::Kbm)
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

    pub(super) fn process_text_edit_input(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        let mouse_pos = self.input.mouse_position();
        let mouse_pressed = self.input.is_mouse_pressed(MouseButton::Left);
        let mouse_down = self.input.is_mouse_down(MouseButton::Left);
        let hovered = self.hovered_text_edit(computed, UiInputSource::Kbm);
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
        source: UiInputSource,
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
        let hovered = self.hovered_button(computed, UiInputSource::Kbm);
        let mouse_down = self.input.is_mouse_down(MouseButton::Left);
        let mut next_states = std::mem::take(&mut self.render_ui.button_states);
        next_states.retain(|node, _| self.nodes.get(*node).is_some());
        let mut events = Vec::new();

        for (node, scene_node) in self.nodes.iter() {
            let SceneNodeData::UiButton(button) = &scene_node.data else {
                continue;
            };
            let inactive = button_inactive(button);
            let focused_without_hover =
                hovered.is_none() && self.render_ui.focused_ui_node == Some(node);
            let next = if inactive {
                UiButtonVisualState::Neutral
            } else if self.render_ui.nav_pressed_button == Some(node) {
                UiButtonVisualState::Pressed
            } else if Some(node) != hovered && !focused_without_hover {
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
        let text_hovered = self.hovered_text_edit(computed, UiInputSource::Kbm);
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
        source: UiInputSource,
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
                || !self.ui_input_mask_accepts(&button.input_mask, source)
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

    fn set_ui_focus(
        &mut self,
        next: Option<NodeID>,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        if self.render_ui.focused_ui_node == next {
            return;
        }
        let prev = self.render_ui.focused_ui_node;
        if let Some(prev) = prev
            && command_seen.insert(prev)
        {
            command_ids.push(prev);
        }
        if let Some(prev) = self.render_ui.focused_text_edit {
            self.emit_text_edit_event(prev, "unfocused", None);
        }

        self.render_ui.focused_ui_node = next;
        self.render_ui.focused_text_edit = next.and_then(|node| {
            self.nodes
                .get(node)
                .and_then(|scene_node| text_edit_ref(&scene_node.data).map(|_| node))
        });
        self.render_ui.nav_pressed_button = None;

        if let Some(next) = next
            && command_seen.insert(next)
        {
            command_ids.push(next);
        }
        if let Some(next) = self.render_ui.focused_text_edit {
            self.emit_text_edit_event(next, "focused", None);
        }
    }

    fn hovered_focusable(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        source: UiInputSource,
    ) -> Option<UiFocusCandidate> {
        let text = self.hovered_text_edit(computed, source);
        let button = self.hovered_button(computed, source);
        match (text, button) {
            (Some(text), Some(button)) => {
                let text_z = self.ui_effective_z(text);
                let button_z = self.ui_effective_z(button);
                let picked = if text_z >= button_z { text } else { button };
                self.focus_candidate(computed, picked, source)
            }
            (Some(node), None) | (None, Some(node)) => self.focus_candidate(computed, node, source),
            (None, None) => None,
        }
    }

    fn collect_focus_candidates(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        source: UiInputSource,
    ) -> Vec<UiFocusCandidate> {
        let mut out = Vec::new();
        for (node, _) in self.nodes.iter() {
            if let Some(candidate) = self.focus_candidate(computed, node, source) {
                out.push(candidate);
            }
        }
        out.sort_by(compare_focus_visual_order);
        out
    }

    fn focus_candidate(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        node: NodeID,
        source: UiInputSource,
    ) -> Option<UiFocusCandidate> {
        let scene_node = self.nodes.get(node)?;
        match &scene_node.data {
            SceneNodeData::UiButton(button) => {
                if button.disabled
                    || !button.input_enabled
                    || !button.visible
                    || !self.ui_input_mask_accepts(&button.input_mask, source)
                {
                    return None;
                }
            }
            data => {
                let edit = text_edit_ref(data)?;
                if !edit.base.visible
                    || !edit.base.input_enabled
                    || !self.ui_input_mask_accepts(&edit.input_mask, source)
                {
                    return None;
                }
            }
        };
        if !self.is_effectively_visible_for_ui(node) {
            return None;
        }
        let rect = computed.get(&node).copied().or_else(|| {
            self.render_ui
                .retained_rects
                .get(&node)
                .map(computed_rect_from_state)
        })?;
        self.ui_focus_rect_visible(node, rect, computed)
            .then_some(UiFocusCandidate { node, rect })
    }

    fn ui_focus_rect_visible(
        &self,
        node: NodeID,
        rect: ComputedUiRect,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> bool {
        let viewport = self.input.viewport_size();
        let bounds = rect_to_screen_clip(rect, viewport);
        let clip = self.ui_effective_clip_rect_screen(node, computed, viewport);
        let visible = intersect_clip_rect(bounds, clip);
        visible[2] > visible[0] && visible[3] > visible[1]
    }

    fn next_tab_focus(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        reverse: bool,
        source: UiInputSource,
    ) -> Option<NodeID> {
        let candidates = self.collect_focus_candidates(computed, source);
        if candidates.is_empty() {
            return None;
        }
        let current = self.render_ui.focused_ui_node;
        let Some(index) = current.and_then(|node| candidates.iter().position(|c| c.node == node))
        else {
            return Some(if reverse {
                candidates[candidates.len() - 1].node
            } else {
                candidates[0].node
            });
        };
        let next = if reverse {
            index.checked_sub(1).unwrap_or(candidates.len() - 1)
        } else {
            (index + 1) % candidates.len()
        };
        Some(candidates[next].node)
    }

    fn next_directional_focus(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        dir: [i8; 2],
        source: UiInputSource,
    ) -> Option<NodeID> {
        let candidates = self.collect_focus_candidates(computed, source);
        if candidates.is_empty() {
            return None;
        }
        let current = self
            .render_ui
            .focused_ui_node
            .and_then(|node| candidates.iter().find(|c| c.node == node).copied());
        let Some(current) = current else {
            return Some(candidates[0].node);
        };
        let axis = Vector2::new(dir[0] as f32, dir[1] as f32);
        candidates
            .iter()
            .copied()
            .filter(|candidate| candidate.node != current.node)
            .filter_map(|candidate| {
                let delta = candidate.rect.center - current.rect.center;
                let forward = delta.x * axis.x + delta.y * axis.y;
                if forward <= 0.0 {
                    return None;
                }
                let lateral = if dir[0] != 0 {
                    delta.y.abs()
                } else {
                    delta.x.abs()
                };
                let score = forward + lateral * 2.0;
                Some((candidate.node, score))
            })
            .min_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.as_u64().cmp(&b.0.as_u64()))
            })
            .map(|(node, _)| node)
    }

    fn ui_gamepad_dpad_direction(&self) -> Option<UiDirectionalNav> {
        for (index, gamepad) in self.input.gamepads().iter().enumerate() {
            let source = UiInputSource::Gamepad(index);
            if gamepad.is_button_pressed(GamepadButton::DpadUp) {
                return Some(UiDirectionalNav {
                    source,
                    dir: [0, 1],
                });
            }
            if gamepad.is_button_pressed(GamepadButton::DpadDown) {
                return Some(UiDirectionalNav {
                    source,
                    dir: [0, -1],
                });
            }
            if gamepad.is_button_pressed(GamepadButton::DpadLeft) {
                return Some(UiDirectionalNav {
                    source,
                    dir: [-1, 0],
                });
            }
            if gamepad.is_button_pressed(GamepadButton::DpadRight) {
                return Some(UiDirectionalNav {
                    source,
                    dir: [1, 0],
                });
            }
        }
        None
    }

    fn ui_repeating_stick_nav_direction(&mut self) -> Option<UiDirectionalNav> {
        let nav = self.ui_stick_nav_direction();
        let Some(nav) = nav else {
            if !self.ui_any_nav_stick_held() {
                self.render_ui.ui_nav_repeat_dir = None;
                self.render_ui.ui_nav_repeat_timer = 0.0;
            }
            return None;
        };
        if self.render_ui.ui_nav_repeat_dir != Some(nav.dir) {
            self.render_ui.ui_nav_repeat_dir = Some(nav.dir);
            self.render_ui.ui_nav_repeat_timer = UI_NAV_REPEAT_DELAY;
            return Some(nav);
        }
        self.render_ui.ui_nav_repeat_timer -= self.time.delta.max(0.0);
        if self.render_ui.ui_nav_repeat_timer > 0.0 {
            return None;
        }
        while self.render_ui.ui_nav_repeat_timer <= 0.0 {
            self.render_ui.ui_nav_repeat_timer += UI_NAV_REPEAT_RATE;
        }
        Some(nav)
    }

    fn ui_stick_nav_direction(&self) -> Option<UiDirectionalNav> {
        for (index, gamepad) in self.input.gamepads().iter().enumerate() {
            if let Some(dir) = stick_nav_direction(gamepad.left_stick(), UI_NAV_STICK_ON) {
                return Some(UiDirectionalNav {
                    source: UiInputSource::Gamepad(index),
                    dir,
                });
            }
        }
        for (index, joycon) in self.input.joycons().iter().enumerate() {
            if let Some(dir) = stick_nav_direction(joycon.stick(), UI_NAV_STICK_ON) {
                return Some(UiDirectionalNav {
                    source: UiInputSource::JoyCon(index),
                    dir,
                });
            }
        }
        None
    }

    fn ui_any_nav_stick_held(&self) -> bool {
        self.input
            .gamepads()
            .iter()
            .any(|gamepad| stick_nav_direction(gamepad.left_stick(), UI_NAV_STICK_OFF).is_some())
            || self
                .input
                .joycons()
                .iter()
                .any(|joycon| stick_nav_direction(joycon.stick(), UI_NAV_STICK_OFF).is_some())
    }

    fn ui_action_pressed(&self) -> Option<UiInputSource> {
        if self.input.is_key_pressed(KeyCode::Enter) || self.input.is_key_pressed(KeyCode::Space) {
            return Some(UiInputSource::Kbm);
        }
        for (index, gamepad) in self.input.gamepads().iter().enumerate() {
            if gamepad.is_button_pressed(GamepadButton::Bottom) {
                return Some(UiInputSource::Gamepad(index));
            }
        }
        for (index, joycon) in self.input.joycons().iter().enumerate() {
            if joycon.is_button_pressed(JoyConButton::Right) {
                return Some(UiInputSource::JoyCon(index));
            }
        }
        None
    }

    fn ui_action_released(&self) -> Option<UiInputSource> {
        if self.input.is_key_released(KeyCode::Enter) || self.input.is_key_released(KeyCode::Space)
        {
            return Some(UiInputSource::Kbm);
        }
        for (index, gamepad) in self.input.gamepads().iter().enumerate() {
            if gamepad.is_button_released(GamepadButton::Bottom) {
                return Some(UiInputSource::Gamepad(index));
            }
        }
        for (index, joycon) in self.input.joycons().iter().enumerate() {
            if joycon.is_button_released(JoyConButton::Right) {
                return Some(UiInputSource::JoyCon(index));
            }
        }
        None
    }

    fn process_focused_button_action(&mut self) {
        let focused_button = self.render_ui.focused_ui_node.and_then(|node| {
            self.nodes
                .get(node)
                .and_then(|scene_node| match &scene_node.data {
                    SceneNodeData::UiButton(button) if !button_inactive(button) => Some(node),
                    _ => None,
                })
        });
        if let Some(source) = self.ui_action_pressed()
            && let Some(node) = focused_button
            && self.ui_node_accepts_input_source(node, source)
        {
            self.render_ui.nav_pressed_button = Some(node);
        }
        if let Some(source) = self.ui_action_released()
            && let Some(node) = self.render_ui.nav_pressed_button
            && self.ui_node_accepts_input_source(node, source)
        {
            self.render_ui.nav_pressed_button = None;
        }
    }

    fn ui_node_accepts_input_source(&self, node: NodeID, source: UiInputSource) -> bool {
        let Some(scene_node) = self.nodes.get(node) else {
            return false;
        };
        match &scene_node.data {
            SceneNodeData::UiButton(button) => {
                !button_inactive(button) && self.ui_input_mask_accepts(&button.input_mask, source)
            }
            data => text_edit_ref(data)
                .is_some_and(|edit| self.ui_input_mask_accepts(&edit.input_mask, source)),
        }
    }

    fn ui_input_mask_accepts(&self, mask: &perro_ui::UiInputMask, source: UiInputSource) -> bool {
        if self.ui_input_mask_matches_kbm(mask.deny_kbm, source)
            || self.ui_input_mask_matches_ids(&mask.deny_gamepads, source, UiInputSource::Gamepad)
            || self.ui_input_mask_matches_ids(&mask.deny_joycons, source, UiInputSource::JoyCon)
            || mask
                .deny_players
                .iter()
                .any(|&player| self.ui_input_source_matches_player(player, source))
        {
            return false;
        }
        if !mask.has_allow_filter() {
            return true;
        }
        self.ui_input_mask_matches_kbm(mask.allow_kbm, source)
            || self.ui_input_mask_matches_ids(&mask.allow_gamepads, source, UiInputSource::Gamepad)
            || self.ui_input_mask_matches_ids(&mask.allow_joycons, source, UiInputSource::JoyCon)
            || mask
                .allow_players
                .iter()
                .any(|&player| self.ui_input_source_matches_player(player, source))
    }

    fn ui_input_mask_matches_kbm(&self, enabled: bool, source: UiInputSource) -> bool {
        enabled && source == UiInputSource::Kbm
    }

    fn ui_input_mask_matches_ids(
        &self,
        ids: &[usize],
        source: UiInputSource,
        make_source: fn(usize) -> UiInputSource,
    ) -> bool {
        ids.iter().any(|&id| make_source(id) == source)
    }

    fn ui_input_source_matches_player(&self, player: usize, source: UiInputSource) -> bool {
        let Some(player) = self.input.players().get(player) else {
            return false;
        };
        match (player.get_binding(), source) {
            (PlayerBinding::Kbm, UiInputSource::Kbm) => true,
            (PlayerBinding::Gamepad { index }, UiInputSource::Gamepad(source_index)) => {
                index == source_index
            }
            (PlayerBinding::JoyConSingle { index }, UiInputSource::JoyCon(source_index)) => {
                index == source_index
            }
            (PlayerBinding::JoyConPair { left, right }, UiInputSource::JoyCon(source_index)) => {
                left == source_index || right == source_index
            }
            _ => false,
        }
    }
}

fn compare_focus_visual_order(a: &UiFocusCandidate, b: &UiFocusCandidate) -> std::cmp::Ordering {
    b.rect
        .center
        .y
        .partial_cmp(&a.rect.center.y)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| {
            a.rect
                .center
                .x
                .partial_cmp(&b.rect.center.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| a.node.as_u64().cmp(&b.node.as_u64()))
}

fn stick_nav_direction(stick: Vector2, threshold: f32) -> Option<[i8; 2]> {
    let ax = stick.x.abs();
    let ay = stick.y.abs();
    if ax < threshold && ay < threshold {
        return None;
    }
    if ax > ay {
        Some(if stick.x < 0.0 { [-1, 0] } else { [1, 0] })
    } else {
        Some(if stick.y < 0.0 { [0, -1] } else { [0, 1] })
    }
}

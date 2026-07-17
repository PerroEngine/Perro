use super::*;

impl Runtime {
    pub(super) fn set_ui_focus(
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

    pub(super) fn hovered_focusable(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        source: UiInputSource,
        point: Vector2,
    ) -> Option<UiFocusCandidate> {
        let text = self.hovered_text_edit(computed, source, point);
        let button = self.hovered_button(computed, source, point);
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

    pub(super) fn collect_focus_candidates(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        source: UiInputSource,
    ) -> Vec<UiFocusCandidate> {
        let mut out = Vec::new();
        for &node in &self.render_ui.focusable_nodes {
            if let Some(candidate) = self.focus_candidate(computed, node, source) {
                out.push(candidate);
            }
        }
        out.sort_by(compare_focus_visual_order);
        out
    }

    pub(super) fn focus_candidate(
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
            SceneNodeData::UiCheckbox(checkbox) => {
                if checkbox.disabled
                    || !checkbox.input_enabled
                    || !checkbox.visible
                    || !self.ui_input_mask_accepts(&checkbox.input_mask, source)
                {
                    return None;
                }
            }
            SceneNodeData::UiImageButton(button) => {
                if button.disabled
                    || !button.input_enabled
                    || !button.visible
                    || !self.ui_input_mask_accepts(&button.input_mask, source)
                {
                    return None;
                }
            }
            SceneNodeData::UiNineSliceButton(button) => {
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

    pub(super) fn ui_focus_rect_visible(
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

    pub(super) fn ui_point_in_effective_clip(
        &self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        point: Vector2,
    ) -> bool {
        let viewport = self.input.viewport_size();
        let clip = self.ui_effective_clip_rect_screen(node, computed, viewport);
        let screen_x = viewport.x * 0.5 + point.x;
        let screen_y = viewport.y * 0.5 - point.y;
        screen_x >= clip[0] && screen_x <= clip[2] && screen_y >= clip[1] && screen_y <= clip[3]
    }

    pub(super) fn ui_point_blocked_by_stop_node(
        &self,
        target: NodeID,
        target_z: i32,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        point: Vector2,
    ) -> bool {
        for (node, scene_node) in self.nodes.iter() {
            if node == target || self.ui_nodes_related(node, target) {
                continue;
            }
            let Some(ui) = ui_root_from_data(&scene_node.data) else {
                continue;
            };
            if !ui.visible
                || !ui.input_enabled
                || ui.mouse_filter != perro_ui::UiMouseFilter::Stop
                || !self.is_effectively_visible_for_ui(node)
            {
                continue;
            }
            let blocker_z = self.ui_effective_z(node);
            if blocker_z < target_z || (blocker_z == target_z && node.as_u64() < target.as_u64()) {
                continue;
            }
            let Some(rect) = computed.get(&node).copied().or_else(|| {
                self.render_ui
                    .retained_rects
                    .get(&node)
                    .map(computed_rect_from_state)
            }) else {
                continue;
            };
            if rect.contains(point) && self.ui_point_in_effective_clip(node, computed, point) {
                return true;
            }
        }
        false
    }

    pub(super) fn ui_nodes_related(&self, a: NodeID, b: NodeID) -> bool {
        self.ui_node_is_ancestor(a, b) || self.ui_node_is_ancestor(b, a)
    }

    pub(super) fn ui_node_is_ancestor(&self, ancestor: NodeID, mut node: NodeID) -> bool {
        while let Some(scene_node) = self.nodes.get(node) {
            let parent = scene_node.parent;
            if parent.is_nil() {
                return false;
            }
            if parent == ancestor {
                return true;
            }
            node = parent;
        }
        false
    }

    pub(super) fn next_tab_focus(
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

    pub(super) fn next_directional_focus(
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
}

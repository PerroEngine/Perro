use super::*;

impl Runtime {
    pub(in super::super) fn process_ui_scroll_input(
        &mut self,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        computed_scales: &mut AHashMap<NodeID, Vector2>,
        root_rect: ComputedUiRect,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        let mut changed = Vec::new();
        let mut changed_seen = ahash::AHashSet::default();
        let wheel = self.input.mouse_wheel();

        for scroller in self.visible_scroll_containers(computed) {
            if self.advance_scroll_container_animation(scroller, computed)
                && changed_seen.insert(scroller)
            {
                changed.push(scroller);
            }
        }

        if let Some(scroller) = self.process_scrollbar_pointer_input(computed)
            && changed_seen.insert(scroller)
        {
            changed.push(scroller);
        }

        if wheel.y != 0.0 && !self.focused_multiline_text_edit_consumes_wheel() {
            let pointer = self.ui_pointer_screen_point();
            if let Some(scroller) = self.hovered_scroll_container(computed, pointer) {
                let step = computed
                    .get(&scroller)
                    .map(|rect| {
                        (if self.scroll_container_is_horizontal(scroller) {
                            rect.size.x
                        } else {
                            rect.size.y
                        }) * 0.12
                    })
                    .unwrap_or(0.0);
                if step > 0.0
                    && self.adjust_scroll_container(scroller, computed, -(wheel.y * step))
                    && changed_seen.insert(scroller)
                {
                    changed.push(scroller);
                }
            }
        }

        if self.render_ui.focused_text_edit.is_none()
            && let Some(scroller) = self.keyboard_scroll_container_target(computed)
            && let Some(delta) = self.scroll_container_keyboard_delta(scroller, computed)
            && self.adjust_scroll_container(scroller, computed, delta)
            && changed_seen.insert(scroller)
        {
            changed.push(scroller);
        }

        for scroller in self.visible_scroll_containers(computed) {
            if self.clamp_scroll_container(scroller, computed) && changed_seen.insert(scroller) {
                changed.push(scroller);
            }
        }

        if changed.is_empty() {
            return;
        }

        let mut recompute = Vec::new();
        let mut recompute_seen = ahash::AHashSet::default();
        for node in changed {
            self.collect_ui_subtree_nodes(node, &mut recompute, &mut recompute_seen);
        }
        for &node in &recompute {
            computed.remove(&node);
            computed_scales.remove(&node);
        }
        let mut auto_layout_computed = ahash::AHashSet::default();
        for &node in &recompute {
            self.compute_ui_rect(
                node,
                root_rect,
                computed,
                computed_scales,
                &mut auto_layout_computed,
            );
            if command_seen.insert(node) {
                command_ids.push(node);
            }
        }
    }

    pub(super) fn advance_scroll_container_animation(
        &mut self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> bool {
        let max_scroll = self.scroll_container_max(node, computed);
        let delta = self.time.delta.max(0.0);
        let Some(scene_node) = self.nodes.get_mut_untracked(node) else {
            return false;
        };
        let SceneNodeData::UiScrollContainer(scroller) = &mut scene_node.data else {
            return false;
        };
        let Some(mut animation) = scroller.scroll_animation else {
            return false;
        };
        animation.elapsed = (animation.elapsed + delta).max(0.0);
        let t = if animation.duration <= 0.0 {
            1.0
        } else {
            (animation.elapsed / animation.duration).clamp(0.0, 1.0)
        };
        let target = match scroller.scroll_dir {
            perro_ui::UiScrollDirection::Horizontal => {
                Vector2::new(max_scroll.x * animation.target_part, 0.0)
            }
            perro_ui::UiScrollDirection::Vertical => {
                Vector2::new(0.0, max_scroll.y * animation.target_part)
            }
        };
        let old = scroller.scroll;
        scroller.scroll = Vector2::new(
            animation.start.x + (target.x - animation.start.x) * t,
            animation.start.y + (target.y - animation.start.y) * t,
        );
        if t >= 1.0 {
            scroller.scroll_animation = None;
        } else {
            scroller.scroll_animation = Some(animation);
        }
        if scroller.scroll == old {
            return false;
        }
        self.mark_ui_dirty(
            node,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        true
    }

    pub(super) fn process_scrollbar_pointer_input(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<NodeID> {
        if self.input.is_mouse_released(MouseButton::Left) {
            self.render_ui.active_scrollbar = None;
            self.render_ui.scrollbar_drag_offset = 0.0;
        }
        let pointer = self.ui_pointer_screen_point();
        if self.input.is_mouse_pressed(MouseButton::Left)
            && let Some((node, offset)) = self.hit_scrollbar(pointer, computed)
        {
            self.render_ui.active_scrollbar = Some(node);
            self.render_ui.scrollbar_drag_offset = offset;
            return self.set_scroll_from_scrollbar_pointer(node, pointer, computed, offset);
        }
        if self.input.is_mouse_down(MouseButton::Left)
            && let Some(node) = self.render_ui.active_scrollbar
        {
            if !self.is_effectively_visible_for_ui(node) {
                self.render_ui.active_scrollbar = None;
                self.render_ui.scrollbar_drag_offset = 0.0;
                return None;
            }
            return self.set_scroll_from_scrollbar_pointer(
                node,
                pointer,
                computed,
                self.render_ui.scrollbar_drag_offset,
            );
        }
        None
    }

    pub(super) fn hit_scrollbar(
        &self,
        point: Vector2,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<(NodeID, f32)> {
        let mut best: Option<(NodeID, i32, f32)> = None;
        for node in self.visible_scroll_containers(computed) {
            let Some((track, thumb, scroller)) = self.scrollbar_hit_rects(node, computed) else {
                continue;
            };
            if !track.contains(point) {
                continue;
            }
            let offset = if thumb.contains(point) {
                match scroller.scroll_dir {
                    perro_ui::UiScrollDirection::Horizontal => point.x - thumb.center.x,
                    perro_ui::UiScrollDirection::Vertical => point.y - thumb.center.y,
                }
            } else {
                0.0
            };
            let z = self.ui_effective_z(node);
            match best {
                Some((best_node, best_z, _))
                    if best_z > z || (best_z == z && best_node.as_u64() > node.as_u64()) => {}
                _ => best = Some((node, z, offset)),
            }
        }
        best.map(|(node, _, offset)| (node, offset))
    }

    pub(super) fn scrollbar_hit_rects(
        &self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<(ComputedUiRect, ComputedUiRect, perro_ui::UiScrollContainer)> {
        let scene_node = self.nodes.get(node)?;
        let SceneNodeData::UiScrollContainer(scroller) = &scene_node.data else {
            return None;
        };
        let rect = computed.get(&node).copied().or_else(|| {
            self.render_ui
                .retained_rects
                .get(&node)
                .map(computed_rect_from_state)
        })?;
        let max_scroll = self.scroll_container_max(node, computed);
        let thumb = ui_scrollbar_rect(scroller, rect, max_scroll)?;
        let track = ui_scrollbar_track_rect(scroller, rect, max_scroll)?;
        Some((track, thumb, *scroller.clone()))
    }

    pub(super) fn set_scroll_from_scrollbar_pointer(
        &mut self,
        node: NodeID,
        point: Vector2,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        drag_offset: f32,
    ) -> Option<NodeID> {
        let (track, thumb, scroller_copy) = self.scrollbar_hit_rects(node, computed)?;
        let max_scroll = self.scroll_container_max(node, computed);
        let Some(scene_node) = self.nodes.get_mut_untracked(node) else {
            self.render_ui.active_scrollbar = None;
            return None;
        };
        let SceneNodeData::UiScrollContainer(scroller) = &mut scene_node.data else {
            self.render_ui.active_scrollbar = None;
            return None;
        };
        let old = scroller.scroll;
        scroller.scroll_animation = None;
        match scroller_copy.scroll_dir {
            perro_ui::UiScrollDirection::Horizontal => {
                let travel = (track.size.x - thumb.size.x).max(0.0);
                let center = point.x - drag_offset;
                let pos = center - track.min().x - thumb.size.x * 0.5;
                let progress = if travel > 0.0 {
                    (pos / travel).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                scroller.scroll.x = max_scroll.x * progress;
                scroller.scroll.y = 0.0;
            }
            perro_ui::UiScrollDirection::Vertical => {
                let travel = (track.size.y - thumb.size.y).max(0.0);
                let center = point.y - drag_offset;
                let pos = track.max().y - thumb.size.y * 0.5 - center;
                let progress = if travel > 0.0 {
                    (pos / travel).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                scroller.scroll.x = 0.0;
                scroller.scroll.y = max_scroll.y * progress;
            }
        }
        if scroller.scroll == old {
            return None;
        }
        self.mark_ui_dirty(
            node,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        Some(node)
    }

    pub(in super::super) fn text_edit_repeat_key(&mut self, ctrl: bool) -> Option<KeyCode> {
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

    pub(super) fn hovered_scroll_container(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        point: Vector2,
    ) -> Option<NodeID> {
        let mut best: Option<(NodeID, i32)> = None;
        for node in self.visible_scroll_containers(computed) {
            if !self.scroll_container_can_scroll(node, computed) {
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
            if !rect.contains(point) {
                continue;
            }
            let z = self.ui_effective_z(node);
            match best {
                Some((best_node, best_z))
                    if best_z > z || (best_z == z && best_node.as_u64() > node.as_u64()) => {}
                _ => best = Some((node, z)),
            }
        }
        best.map(|(node, _)| node)
    }

    pub(super) fn visible_scroll_containers(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Vec<NodeID> {
        let mut out = Vec::new();
        for (node, scene_node) in self.nodes.iter() {
            let SceneNodeData::UiScrollContainer(scroller) = &scene_node.data else {
                continue;
            };
            if !scroller.visible
                || !scroller.input_enabled
                || !self.is_effectively_visible_for_ui(node)
                || !matches!(
                    scroller.mouse_filter,
                    perro_ui::UiMouseFilter::Stop | perro_ui::UiMouseFilter::Pass
                )
                || (!computed.contains_key(&node)
                    && !self.render_ui.retained_rects.contains_key(&node))
            {
                continue;
            }
            out.push(node);
        }
        out
    }

    pub(super) fn scroll_container_is_horizontal(&self, node: NodeID) -> bool {
        self.nodes
            .get(node)
            .and_then(|scene_node| match &scene_node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll_dir),
                _ => None,
            })
            == Some(perro_ui::UiScrollDirection::Horizontal)
    }

    pub(super) fn focused_multiline_text_edit_consumes_wheel(&self) -> bool {
        self.render_ui
            .focused_text_edit
            .and_then(|node| self.nodes.get(node))
            .and_then(|scene_node| text_edit_ref(&scene_node.data))
            .is_some_and(|edit| edit.multiline)
    }

    pub(super) fn keyboard_scroll_container_target(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<NodeID> {
        self.render_ui
            .focused_ui_node
            .and_then(|node| self.closest_scroll_container_ancestor(node, computed))
            .or_else(|| self.sole_root_scroll_container(computed))
    }

    pub(super) fn closest_scroll_container_ancestor(
        &self,
        mut node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<NodeID> {
        loop {
            let scene_node = self.nodes.get(node)?;
            if matches!(scene_node.data, SceneNodeData::UiScrollContainer(_))
                && self.scroll_container_can_scroll(node, computed)
            {
                return Some(node);
            }
            if scene_node.parent.is_nil() {
                return None;
            }
            node = scene_node.parent;
        }
    }

    pub(super) fn sole_root_scroll_container(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<NodeID> {
        let mut found = None;
        for node in self.visible_scroll_containers(computed) {
            if !self.scroll_container_can_scroll(node, computed) {
                continue;
            }
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            let Some(parent) = (!scene_node.parent.is_nil()).then_some(scene_node.parent) else {
                if found.replace(node).is_some() {
                    return None;
                }
                continue;
            };
            if self
                .closest_scroll_container_ancestor(parent, computed)
                .is_some()
            {
                continue;
            }
            if found.replace(node).is_some() {
                return None;
            }
        }
        found
    }

    pub(super) fn scroll_container_can_scroll(
        &self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> bool {
        let max_scroll = self.scroll_container_max(node, computed);
        self.nodes
            .get(node)
            .and_then(|scene_node| match &scene_node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll_dir),
                _ => None,
            })
            .is_some_and(|dir| match dir {
                perro_ui::UiScrollDirection::Horizontal => max_scroll.x > 0.0,
                perro_ui::UiScrollDirection::Vertical => max_scroll.y > 0.0,
            })
    }

    pub(super) fn scroll_container_keyboard_delta(
        &self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<f32> {
        let view = computed.get(&node)?.size;
        let axis = if self.scroll_container_is_horizontal(node) {
            view.x
        } else {
            view.y
        };
        let line = axis * 0.10;
        let page = axis * 0.90;
        if self.input.is_key_pressed(KeyCode::Home) {
            return Some(f32::NEG_INFINITY);
        }
        if self.input.is_key_pressed(KeyCode::End) {
            return Some(f32::INFINITY);
        }
        if self.input.is_key_pressed(KeyCode::PageDown) {
            return Some(page);
        }
        if self.input.is_key_pressed(KeyCode::PageUp) {
            return Some(-page);
        }
        if self.input.is_key_pressed(KeyCode::ArrowDown) {
            return Some(line);
        }
        if self.input.is_key_pressed(KeyCode::ArrowUp) {
            return Some(-line);
        }
        None
    }

    pub(super) fn adjust_scroll_container(
        &mut self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        delta: f32,
    ) -> bool {
        let max_scroll = self.scroll_container_max(node, computed);
        let Some(scene_node) = self.nodes.get_mut_untracked(node) else {
            return false;
        };
        let SceneNodeData::UiScrollContainer(scroller) = &mut scene_node.data else {
            return false;
        };
        let old = scroller.scroll;
        scroller.scroll_animation = None;
        match scroller.scroll_dir {
            perro_ui::UiScrollDirection::Horizontal => {
                scroller.scroll.y = 0.0;
                scroller.scroll.x = apply_scroll_delta(scroller.scroll.x, delta, max_scroll.x);
            }
            perro_ui::UiScrollDirection::Vertical => {
                scroller.scroll.x = 0.0;
                scroller.scroll.y = apply_scroll_delta(scroller.scroll.y, delta, max_scroll.y);
            }
        }
        if scroller.scroll == old {
            return false;
        }
        self.mark_ui_dirty(
            node,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        true
    }

    pub(super) fn clamp_scroll_container(
        &mut self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> bool {
        let max_scroll = self.scroll_container_max(node, computed);
        let Some(scene_node) = self.nodes.get_mut_untracked(node) else {
            return false;
        };
        let SceneNodeData::UiScrollContainer(scroller) = &mut scene_node.data else {
            return false;
        };
        let old = scroller.scroll;
        match scroller.scroll_dir {
            perro_ui::UiScrollDirection::Horizontal => {
                scroller.scroll.x = scroller.scroll.x.clamp(0.0, max_scroll.x);
                scroller.scroll.y = 0.0;
            }
            perro_ui::UiScrollDirection::Vertical => {
                scroller.scroll.x = 0.0;
                scroller.scroll.y = scroller.scroll.y.clamp(0.0, max_scroll.y);
            }
        }
        if scroller.scroll == old {
            return false;
        }
        self.mark_ui_dirty(
            node,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        true
    }

    pub(in super::super) fn scroll_container_max(
        &self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Vector2 {
        Vector2::new(
            self.scroll_container_max_x(node, computed).unwrap_or(0.0),
            self.scroll_container_max_y(node, computed).unwrap_or(0.0),
        )
    }

    pub(super) fn scroll_container_max_x(
        &self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<f32> {
        let scene_node = self.nodes.get(node)?;
        let SceneNodeData::UiScrollContainer(scroller) = &scene_node.data else {
            return None;
        };
        let rect = computed.get(&node).copied().or_else(|| {
            self.render_ui
                .retained_rects
                .get(&node)
                .map(computed_rect_from_state)
        })?;
        let content = rect.inset(ui_padding_inset(rect, scroller.layout.padding));
        let (content_min, content_max) = self.scroll_container_unscrolled_bounds(node, computed)?;
        Some(
            (content_max.x - content.max().x)
                .max(0.0)
                .max((content.min().x - content_min.x).max(0.0)),
        )
    }

    pub(super) fn scroll_container_max_y(
        &self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<f32> {
        let scene_node = self.nodes.get(node)?;
        let SceneNodeData::UiScrollContainer(scroller) = &scene_node.data else {
            return None;
        };
        let rect = computed.get(&node).copied().or_else(|| {
            self.render_ui
                .retained_rects
                .get(&node)
                .map(computed_rect_from_state)
        })?;
        let content = rect.inset(ui_padding_inset(rect, scroller.layout.padding));
        let (content_min, _) = self.scroll_container_unscrolled_bounds(node, computed)?;
        Some((content.min().y - content_min.y).max(0.0))
    }

    pub(in super::super) fn scroll_container_unscrolled_bounds(
        &self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<(Vector2, Vector2)> {
        let scene_node = self.nodes.get(node)?;
        let SceneNodeData::UiScrollContainer(scroller) = &scene_node.data else {
            return None;
        };
        let offset = Vector2::new(scroller.scroll.x, -scroller.scroll.y);
        let mut min = Vector2::new(f32::INFINITY, f32::INFINITY);
        let mut max = Vector2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
        let mut found = false;
        let mut nodes = Vec::new();
        let mut seen = ahash::AHashSet::default();
        for child in self.ui_layout_children(node) {
            self.collect_ui_subtree_nodes(child, &mut nodes, &mut seen);
        }
        for child in nodes {
            let Some(rect) = computed.get(&child).copied().or_else(|| {
                self.render_ui
                    .retained_rects
                    .get(&child)
                    .map(computed_rect_from_state)
            }) else {
                continue;
            };
            let unscrolled = ComputedUiRect::new(rect.center + offset, rect.size);
            min = min.min(unscrolled.min());
            max = max.max(unscrolled.max());
            found = true;
        }
        found.then_some((min, max))
    }

    pub(super) fn collect_ui_subtree_nodes(
        &self,
        root: NodeID,
        out: &mut Vec<NodeID>,
        seen: &mut ahash::AHashSet<NodeID>,
    ) {
        if !seen.insert(root) {
            return;
        }
        out.push(root);
        let Some(children) = self.nodes.children(root) else {
            return;
        };
        let mut stack = children.to_vec();
        while let Some(node) = stack.pop() {
            let Some(child) = self.nodes.get(node) else {
                continue;
            };
            if ui_root_from_data(&child.data).is_some() {
                if seen.insert(node) {
                    out.push(node);
                }
                if let Some(children) = self.nodes.children(node) {
                    stack.extend(children.iter().copied());
                }
                continue;
            }
            if let Some(children) = self.nodes.children(node) {
                stack.extend(children.iter().copied());
            }
        }
    }
}

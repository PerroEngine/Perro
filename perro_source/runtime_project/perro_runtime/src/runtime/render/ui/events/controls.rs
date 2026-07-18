use super::*;

impl Runtime {
    pub(in super::super) fn refresh_button_visual_states(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        let pointer_point = self.ui_pointer_screen_point();
        if self.input.is_mouse_down(MouseButton::Left) {
            self.process_color_picker_popup_input(
                computed,
                pointer_point,
                command_ids,
                command_seen,
            );
        }
        let hovered = self.hovered_button(computed, UiInputSource::Kbm, pointer_point);
        let mouse_down = self.input.is_mouse_down(MouseButton::Left);
        if self.input.is_mouse_pressed(MouseButton::Left) {
            self.render_ui.pressed_ui_button = hovered;
        } else if !mouse_down {
            self.render_ui.pressed_ui_button = None;
        }
        let mut next_states = std::mem::take(&mut self.render_ui.button_states);
        next_states.retain(|node, _| self.nodes.get(*node).is_some());
        let mut motions = std::mem::take(&mut self.render_ui.button_motions);
        motions.retain(|node, _| self.nodes.get(*node).is_some());
        let mut events = Vec::new();
        if self
            .render_ui
            .focused_ui_node
            .is_some_and(|node| !self.is_effectively_visible_for_ui(node))
        {
            self.render_ui.focused_ui_node = None;
            self.render_ui.focused_text_edit = None;
        }
        if self
            .render_ui
            .nav_pressed_button
            .is_some_and(|node| !self.is_effectively_visible_for_ui(node))
        {
            self.render_ui.nav_pressed_button = None;
        }

        for (node, scene_node) in self.nodes.iter() {
            let effectively_visible = self.is_effectively_visible_for_ui(node);
            let Some(inactive) = ui_button_like_inactive(&scene_node.data) else {
                continue;
            };
            let inactive = inactive || !effectively_visible;
            let focused_without_hover =
                hovered.is_none() && self.render_ui.focused_ui_node == Some(node);
            let next = if inactive {
                UiButtonVisualState::Neutral
            } else if self.render_ui.nav_pressed_button == Some(node) {
                UiButtonVisualState::Pressed
            } else if Some(node) != hovered && !focused_without_hover {
                UiButtonVisualState::Neutral
            } else if mouse_down && self.render_ui.pressed_ui_button == Some(node) {
                UiButtonVisualState::Pressed
            } else {
                UiButtonVisualState::Hover
            };
            let prev = next_states.insert(node, next).unwrap_or_default();
            if !inactive {
                collect_button_events(node, prev, next, &mut events);
            }
            let hover_target = if next == UiButtonVisualState::Neutral {
                0.0
            } else {
                1.0
            };
            let press_target = if next == UiButtonVisualState::Pressed {
                1.0
            } else {
                0.0
            };
            let needs_motion = prev != next || motions.contains_key(&node);
            if needs_motion {
                let motion = motions.entry(node).or_insert(UiButtonMotion {
                    hover: if prev == UiButtonVisualState::Neutral {
                        0.0
                    } else {
                        1.0
                    },
                    press: if prev == UiButtonVisualState::Pressed {
                        1.0
                    } else {
                        0.0
                    },
                    wiggle_time: 1.0,
                    wiggle_sign: 1.0,
                });
                if prev != next
                    && (prev == UiButtonVisualState::Neutral
                        || next == UiButtonVisualState::Neutral)
                {
                    motion.wiggle_time = 0.0;
                    motion.wiggle_sign = if next == UiButtonVisualState::Neutral {
                        -1.0
                    } else {
                        1.0
                    };
                }
                let dt = self.time.delta.clamp(0.0, 0.05);
                let hover_alpha = 1.0 - (-dt * 22.0).exp();
                let press_alpha = 1.0 - (-dt * 38.0).exp();
                motion.hover += (hover_target - motion.hover) * hover_alpha;
                motion.press += (press_target - motion.press) * press_alpha;
                motion.wiggle_time += dt;
                let settled = (motion.hover - hover_target).abs() < 0.001
                    && (motion.press - press_target).abs() < 0.001
                    && motion.wiggle_time >= 0.14;
                if settled {
                    motions.remove(&node);
                }
            }
            if needs_motion && command_seen.insert(node) {
                command_ids.push(node);
            }
        }

        self.render_ui.button_states = next_states;
        self.render_ui.button_motions = motions;
        let text_hovered = self.hovered_text_edit(computed, UiInputSource::Kbm, pointer_point);
        let scrollbar_hovered = self.render_ui.active_scrollbar.is_some()
            || self.hit_scrollbar(pointer_point, computed).is_some();
        let cursor_icon = text_hovered
            .map(|_| perro_ui::CursorIcon::Text)
            .or_else(|| scrollbar_hovered.then_some(perro_ui::CursorIcon::Pointer))
            .or_else(|| {
                hovered
                    .and_then(|node| self.nodes.get(node))
                    .and_then(|scene_node| match &scene_node.data {
                        SceneNodeData::UiButton(button) => Some(button.cursor_icon),
                        SceneNodeData::UiDropdown(dropdown) => Some(dropdown.cursor_icon),
                        SceneNodeData::UiCheckbox(checkbox) => Some(checkbox.cursor_icon),
                        SceneNodeData::UiImageButton(button) => Some(button.cursor_icon),
                        SceneNodeData::UiNineSliceButton(button) => Some(button.cursor_icon),
                        _ => None,
                    })
            })
            .unwrap_or(perro_ui::CursorIcon::Default);
        self.set_render_cursor_icon_ui(cursor_icon);
        self.render_ui.last_ui_pointer = Some((
            self.input.mouse_position(),
            self.input.is_mouse_down(MouseButton::Left),
        ));
        self.emit_button_events(&events);
    }

    pub(in super::super) fn emit_button_events(&mut self, events: &[(NodeID, &'static str)]) {
        for &(node, event) in events {
            if event == "click" {
                self.toggle_checkbox(node);
                self.toggle_color_picker_from_child(node);
                self.process_tree_list_click(node);
                self.process_dropdown_click(node);
                self.process_button_web_action(node);
            }
            self.collect_button_event_signals(node, event);
            if self.render_ui.event_signal_scratch.is_empty() {
                continue;
            }
            let mut signals = std::mem::take(&mut self.render_ui.event_signal_scratch);
            let params = [Variant::from(node)];
            for signal in signals.iter().copied() {
                self.queue_ui_signal(signal, &params);
            }
            signals.clear();
            self.render_ui.event_signal_scratch = signals;
        }
    }

    pub(super) fn process_button_web_action(&mut self, node: NodeID) {
        let Some(scene_node) = self.nodes.get(node) else {
            return;
        };
        let web = match &scene_node.data {
            SceneNodeData::UiButton(button) => button.web.as_ref(),
            SceneNodeData::UiDropdown(dropdown) => dropdown.web.as_ref(),
            SceneNodeData::UiCheckbox(checkbox) => checkbox.web.as_ref(),
            SceneNodeData::UiImageButton(button) => button.web.as_ref(),
            SceneNodeData::UiNineSliceButton(button) => button.web.as_ref(),
            _ => None,
        };
        let Some(web) = web else {
            return;
        };
        let _ = perro_web::push_route(web.href.as_ref());
    }

    pub(super) fn toggle_checkbox(&mut self, node: NodeID) {
        let Some(scene_node) = self.nodes.get_mut_untracked(node) else {
            return;
        };
        let SceneNodeData::UiCheckbox(checkbox) = &mut scene_node.data else {
            return;
        };
        checkbox.checked = !checkbox.checked;
        self.mark_ui_dirty(node, Runtime::UI_DIRTY_COMMANDS);
    }

    pub(super) fn process_tree_list_click(&mut self, node: NodeID) {
        let Some((tree_id, row_idx, toggle)) = self.tree_list_parent_for_internal(node) else {
            return;
        };
        let Some(scene_node) = self.nodes.get_mut_untracked(tree_id) else {
            return;
        };
        let SceneNodeData::UiTreeList(tree) = &mut scene_node.data else {
            return;
        };
        let visible = tree.visible_items();
        let Some(row) = visible.get(row_idx).copied() else {
            return;
        };
        let Some(item) = tree.items.get_mut(row.index) else {
            return;
        };
        // Row clicks expand/collapse parents in the same click as selection;
        // dedicated toggle arrows never change selection.
        let mut toggled = None;
        if row.has_children {
            item.open = !item.open;
            toggled = Some((
                tree.toggled_signals.clone(),
                [
                    Variant::from(tree_id),
                    Variant::from(row.index as i32),
                    Variant::from(item.open),
                    item.value.clone(),
                ],
            ));
        }
        let mut selected = None;
        if !toggle && item.selectable {
            let value = item.value.clone();
            tree.selected_index = Some(row.index);
            selected = Some((
                tree.selected_signals.clone(),
                [
                    Variant::from(tree_id),
                    Variant::from(row.index as i32),
                    value,
                ],
            ));
        }
        if toggled.is_none() && selected.is_none() {
            return;
        }
        self.sync_tree_list_internal_nodes(tree_id);
        if let Some((signals, params)) = toggled {
            for signal in signals {
                self.queue_ui_signal(signal, &params);
            }
        }
        if let Some((signals, params)) = selected {
            for signal in signals {
                self.queue_ui_signal(signal, &params);
            }
        }
    }

    pub(super) fn tree_list_parent_for_internal(
        &self,
        internal_id: NodeID,
    ) -> Option<(NodeID, usize, bool)> {
        self.nodes.iter().find_map(|(id, scene_node)| {
            let SceneNodeData::UiTreeList(tree) = &scene_node.data else {
                return None;
            };
            if let Some(idx) = tree
                .internal_toggles
                .iter()
                .position(|item| *item == internal_id)
            {
                return Some((id, idx, true));
            }
            tree.internal_rows
                .iter()
                .position(|item| *item == internal_id)
                .map(|idx| (id, idx, false))
        })
    }

    pub(super) fn process_dropdown_click(&mut self, node: NodeID) {
        if let Some((dropdown_id, option_idx)) = self.dropdown_parent_for_option(node) {
            let Some(scene_node) = self.nodes.get_mut_untracked(dropdown_id) else {
                return;
            };
            let SceneNodeData::UiDropdown(dropdown) = &mut scene_node.data else {
                return;
            };
            if option_idx >= dropdown.options.len() {
                return;
            }
            dropdown.selected_index = option_idx;
            dropdown.open = false;
            let value = dropdown.options[option_idx].value.clone();
            let signals = dropdown.selected_signals.clone();
            self.sync_dropdown_internal_nodes(dropdown_id);
            let params = [
                Variant::from(dropdown_id),
                Variant::from(option_idx as i32),
                value,
            ];
            for signal in signals {
                self.queue_ui_signal(signal, &params);
            }
            return;
        }

        let Some(scene_node) = self.nodes.get_mut_untracked(node) else {
            return;
        };
        let SceneNodeData::UiDropdown(dropdown) = &mut scene_node.data else {
            return;
        };
        dropdown.open = !dropdown.open;
        self.sync_dropdown_internal_nodes(node);
    }

    pub(super) fn dropdown_parent_for_option(&self, option_id: NodeID) -> Option<(NodeID, usize)> {
        self.nodes.iter().find_map(|(id, scene_node)| {
            let SceneNodeData::UiDropdown(dropdown) = &scene_node.data else {
                return None;
            };
            dropdown
                .internal_option_buttons
                .iter()
                .position(|item| *item == option_id)
                .map(|idx| (id, idx))
        })
    }

    pub(super) fn toggle_color_picker_from_child(&mut self, node: NodeID) {
        let Some(parent) = self.color_picker_parent_for_swatch(node) else {
            return;
        };
        let Some(scene_node) = self.nodes.get_mut_untracked(parent) else {
            return;
        };
        let SceneNodeData::UiColorPicker(picker) = &mut scene_node.data else {
            return;
        };
        picker.popup_open = !picker.popup_open;
        self.sync_color_picker_internal_nodes(parent);
    }

    pub(super) fn process_color_picker_popup_input(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        point: Vector2,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        let mut updates = Vec::new();
        for (node, scene_node) in self.nodes.iter() {
            let SceneNodeData::UiColorPicker(picker) = &scene_node.data else {
                continue;
            };
            if !picker.popup_open || button_inactive(&picker.button) {
                continue;
            }
            let popup_id = picker.internal_popup_panel;
            let Some(popup_rect) = computed.get(&popup_id).copied().or_else(|| {
                self.render_ui
                    .retained_rects
                    .get(&popup_id)
                    .map(computed_rect_from_state)
            }) else {
                continue;
            };
            if !picker.show_selector {
                continue;
            }
            let layout = color_picker_layout(
                picker.popup_size,
                picker.wheel_radius,
                picker.show_selector,
                picker.show_rgba,
                picker.show_hsl,
                picker.show_hex,
            );
            let rect = color_picker_wheel_rect(popup_rect, picker.wheel_radius, layout.selector_y);
            let Some(color) =
                color_picker_color_at_point(rect, picker.picker_mode, picker.color.a(), point)
            else {
                continue;
            };
            updates.push((node, color));
        }
        for (node, color) in updates {
            let Some(scene_node) = self.nodes.get_mut_untracked(node) else {
                continue;
            };
            let SceneNodeData::UiColorPicker(picker) = &mut scene_node.data else {
                continue;
            };
            let changed = picker.color != color;
            let signals = picker.color_changed_signals.clone();
            picker.color = color;
            self.sync_color_picker_internal_nodes(node);
            if command_seen.insert(node) {
                command_ids.push(node);
            }
            if !changed {
                continue;
            }
            let params = [
                Variant::from(node),
                Variant::from(color.r()),
                Variant::from(color.g()),
                Variant::from(color.b()),
                Variant::from(color.a()),
            ];
            for signal in signals {
                self.queue_ui_signal(signal, &params);
            }
        }
    }

    pub(in super::super) fn emit_text_edit_event(
        &mut self,
        node: NodeID,
        event: &str,
        text: Option<&str>,
    ) {
        self.collect_text_edit_event_signals(node, event);
        if self.render_ui.event_signal_scratch.is_empty() {
            return;
        }
        let mut signals = std::mem::take(&mut self.render_ui.event_signal_scratch);
        if let Some(text) = text {
            let params = [Variant::from(node), Variant::from(text)];
            for signal in signals.iter().copied() {
                self.queue_ui_signal(signal, &params);
            }
        } else {
            let params = [Variant::from(node)];
            for signal in signals.iter().copied() {
                self.queue_ui_signal(signal, &params);
            }
        }
        signals.clear();
        self.render_ui.event_signal_scratch = signals;
    }

    pub(in super::super) fn collect_button_event_signals(&mut self, node: NodeID, event: &str) {
        self.render_ui.event_signal_scratch.clear();
        let Some(scene_node) = self.nodes.get(node) else {
            return;
        };
        let Some(custom) = ui_button_like_custom_event_signals(&scene_node.data, event) else {
            return;
        };
        self.render_ui
            .event_signal_scratch
            .reserve(1 + custom.len());
        let name = scene_node.name.as_ref();
        if !name.is_empty() {
            self.render_ui.event_signal_name_scratch.clear();
            self.render_ui.event_signal_name_scratch.push_str(name);
            self.render_ui.event_signal_name_scratch.push('_');
            self.render_ui
                .event_signal_name_scratch
                .push_str(button_named_event(event));
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

    pub(in super::super) fn collect_text_edit_event_signals(&mut self, node: NodeID, event: &str) {
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
    pub(in super::super) fn button_event_signals(
        &self,
        node: NodeID,
        event: &str,
    ) -> Vec<SignalID> {
        let Some(scene_node) = self.nodes.get(node) else {
            return Vec::new();
        };
        let Some(custom) = ui_button_like_custom_event_signals(&scene_node.data, event) else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(1 + custom.len());
        let name = scene_node.name.as_ref();
        if !name.is_empty() {
            out.push(SignalID::from_string(&format!(
                "{name}_{}",
                button_named_event(event)
            )));
        }
        out.extend(custom.iter().copied());
        out
    }

    #[cfg(test)]
    pub(in super::super) fn text_edit_event_signals(
        &self,
        node: NodeID,
        event: &str,
    ) -> Vec<SignalID> {
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

    pub(in super::super) fn hovered_button(
        &self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        source: UiInputSource,
        point: Vector2,
    ) -> Option<NodeID> {
        let mut best: Option<(NodeID, i32)> = None;
        for &node in &self.render_ui.visible_buttons {
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
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
            let Some(hit) = ui_button_like_hit_data(&scene_node.data, state) else {
                continue;
            };
            if hit.disabled
                || !hit.input_enabled
                || !self.ui_input_mask_accepts(hit.input_mask, source)
                || !self.is_effectively_visible_for_ui(node)
                || !matches!(
                    hit.mouse_filter,
                    perro_ui::UiMouseFilter::Stop | perro_ui::UiMouseFilter::Pass
                )
            {
                continue;
            }
            let z = self.ui_effective_z(node);
            let hit_rect = if computed.contains_key(&node) {
                match &scene_node.data {
                    SceneNodeData::UiButton(button) => {
                        computed_rect_from_state(&button_rect_state(button, base_rect, state, z))
                    }
                    SceneNodeData::UiCheckbox(checkbox) => computed_rect_from_state(
                        &button_rect_state(&checkbox.button, base_rect, state, z),
                    ),
                    SceneNodeData::UiImageButton(button) => computed_rect_from_state(
                        &image_button_rect_state(button, base_rect, state, z),
                    ),
                    SceneNodeData::UiNineSliceButton(button) => computed_rect_from_state(
                        &nine_slice_button_rect_state(button, base_rect, state, z),
                    ),
                    _ => base_rect,
                }
            } else {
                base_rect
            };
            if !hit_rect.contains_rounded(point, hit.corner_radius) {
                continue;
            }
            if !self.ui_point_in_effective_clip(node, computed, point) {
                continue;
            }
            if self.ui_point_blocked_by_stop_node(node, z, computed, point) {
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

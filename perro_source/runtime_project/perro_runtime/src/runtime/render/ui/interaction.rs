use super::*;

impl Runtime {
    pub(super) fn rebuild_visible_interactive_ui_cache(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
    ) {
        let mut scan_seen = std::mem::take(&mut self.render_ui.interactive_scan_seen);
        let mut buttons = std::mem::take(&mut self.render_ui.visible_buttons);
        let mut text_edits = std::mem::take(&mut self.render_ui.visible_text_edits);
        let mut focusables = std::mem::take(&mut self.render_ui.focusable_nodes);
        scan_seen.clear();
        buttons.clear();
        text_edits.clear();
        focusables.clear();

        for node in self.render_ui.prev_visible.iter().copied() {
            if !scan_seen.insert(node) {
                continue;
            }
            self.collect_visible_interactive_ui_node(
                node,
                computed,
                &mut buttons,
                &mut text_edits,
                &mut focusables,
            );
        }
        for node in computed.keys().copied() {
            if !scan_seen.insert(node) {
                continue;
            }
            self.collect_visible_interactive_ui_node(
                node,
                computed,
                &mut buttons,
                &mut text_edits,
                &mut focusables,
            );
        }

        self.render_ui.interactive_scan_seen = scan_seen;
        self.render_ui.visible_buttons = buttons;
        self.render_ui.visible_text_edits = text_edits;
        self.render_ui.focusable_nodes = focusables;
    }

    pub(super) fn collect_visible_interactive_ui_node(
        &self,
        node: NodeID,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        buttons: &mut Vec<NodeID>,
        text_edits: &mut Vec<NodeID>,
        focusables: &mut Vec<NodeID>,
    ) {
        if !self.is_effectively_visible_for_ui(node) {
            return;
        }
        let has_rect =
            computed.contains_key(&node) || self.render_ui.retained_rects.contains_key(&node);
        if !has_rect {
            return;
        }
        let Some(scene_node) = self.nodes.get(node) else {
            return;
        };
        match &scene_node.data {
            SceneNodeData::UiButton(button) => {
                if !button.visible || button.disabled || !button.input_enabled {
                    return;
                }
                buttons.push(node);
                focusables.push(node);
            }
            SceneNodeData::UiDropdown(dropdown) => {
                if !dropdown.visible || dropdown.disabled || !dropdown.input_enabled {
                    return;
                }
                buttons.push(node);
                focusables.push(node);
            }
            SceneNodeData::UiCheckbox(checkbox) => {
                if !checkbox.visible || checkbox.disabled || !checkbox.input_enabled {
                    return;
                }
                buttons.push(node);
                focusables.push(node);
            }
            SceneNodeData::UiColorPicker(_) => {}
            SceneNodeData::UiImageButton(button) => {
                if !button.visible || button.disabled || !button.input_enabled {
                    return;
                }
                buttons.push(node);
                focusables.push(node);
            }
            SceneNodeData::UiNineSliceButton(button) => {
                if !button.visible || button.disabled || !button.input_enabled {
                    return;
                }
                buttons.push(node);
                focusables.push(node);
            }
            SceneNodeData::UiNineSlice(_) => {}
            data => {
                let Some(edit) = text_edit_ref(data) else {
                    return;
                };
                if !edit.base.visible || !edit.base.input_enabled {
                    return;
                }
                text_edits.push(node);
                focusables.push(node);
            }
        }
    }

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

    pub(super) fn resolve_ui_image_texture(&mut self, node: NodeID) -> Option<TextureID> {
        let mut texture = self
            .nodes
            .get(node)
            .and_then(|scene_node| match &scene_node.data {
                SceneNodeData::UiImage(image) => Some(image.texture),
                SceneNodeData::UiImageButton(image) => Some(image.texture),
                SceneNodeData::UiNineSliceButton(image) => Some(image.texture),
                SceneNodeData::UiNineSlice(image) => Some(image.texture),
                SceneNodeData::UiAnimatedImage(image) => Some(image.texture),
                SceneNodeData::UiVideoPlayer(video) => Some(video.video.texture),
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

        if self.resource_api.is_texture_id_pending(texture) {
            return None;
        }

        Some(texture)
    }

    pub(super) fn ui_image_has_pending_texture(&self, node: NodeID) -> bool {
        self.nodes
            .get(node)
            .is_some_and(|scene_node| match &scene_node.data {
                SceneNodeData::UiImage(image) => {
                    !image.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(image.texture)
                }
                SceneNodeData::UiImageButton(image) => {
                    !image.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(image.texture)
                }
                SceneNodeData::UiNineSliceButton(image) => {
                    !image.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(image.texture)
                }
                SceneNodeData::UiNineSlice(image) => {
                    !image.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(image.texture)
                }
                SceneNodeData::UiAnimatedImage(image) => {
                    !image.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(image.texture)
                }
                SceneNodeData::UiVideoPlayer(video) => {
                    !video.video.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(video.video.texture)
                }
                _ => false,
            })
    }

    pub fn extract_render_ui_commands(&mut self) {
        self.extract_render_ui_commands_inner(None);
    }

    pub fn extract_render_ui_commands_timed(&mut self) -> RuntimeUiTiming {
        let mut timing = RuntimeUiTiming::default();
        self.extract_render_ui_commands_inner(Some(&mut timing));
        timing
    }

    pub(super) fn extract_render_ui_commands_inner(
        &mut self,
        timing: Option<&mut RuntimeUiTiming>,
    ) {
        self.refresh_locale_text_bindings();
        self.render_ui.pointer_screen_point = None;
        let total_start = timing.as_ref().map(|_| Instant::now());
        let dropdown_animation_changed = self.update_dropdown_open_animations();
        let bootstrap_scan = self.render_ui.prev_visible.is_empty()
            && self.render_ui.retained_commands.is_empty()
            && self.render_ui.computed_rects.is_empty();
        let input_changed = self.ui_pointer_changed() || self.ui_nav_input_changed();
        let scroll_input_changed = self.ui_scroll_input_changed();
        let text_input_changed =
            self.render_ui.focused_text_edit.is_some() && self.ui_text_input_changed();
        // The scroll-animation probe walks every node; keep it last so the
        // common dirty/input cases short-circuit past it.
        let has_extraction_work = self.dirty.has_any_dirty()
            || self.dirty.has_pending_transform_roots()
            || !self.render_ui.removed_nodes.is_empty()
            || bootstrap_scan
            || input_changed
            || scroll_input_changed
            || text_input_changed
            || dropdown_animation_changed
            || !self.render_ui.button_motions.is_empty()
            || self.has_active_scroll_container_animation();
        if !has_extraction_work {
            if let (Some(timing), Some(total_start)) = (timing, total_start) {
                timing.total = total_start.elapsed();
            }
            return;
        }
        let mut timing = timing;
        self.ensure_color_picker_internal_nodes();
        self.ensure_tree_list_internal_nodes();
        self.ensure_dropdown_internal_nodes();

        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();

        let viewport = self.input.viewport_size();
        let virtual_font_scale = self.ui_virtual_font_scale(viewport);
        let root_rect = ComputedUiRect::new(Vector2::ZERO, viewport);
        let mut dirty_entries = std::mem::take(&mut self.render_ui.dirty_entries_scratch);
        dirty_entries.clear();
        dirty_entries.extend(self.dirty.dirty_indices().iter().filter_map(|&raw_index| {
            let index = raw_index as usize;
            self.nodes
                .slot_get(index)
                .map(|(node, _)| (node, self.dirty.ui_flags_at(index)))
        }));
        let dirty_node_count = dirty_entries.len();
        let mut all_ids = std::mem::take(&mut self.render_ui.all_ids_scratch);
        all_ids.clear();
        all_ids.extend(self.nodes.iter().map(|(id, _)| id));
        let mut parent_siblings = std::mem::take(&mut self.render_ui.parent_siblings_scratch);
        parent_siblings.clear();
        // dedup the layout-children DFS per ui_parent: when a container changes,
        // all its children get DIRTY_LAYOUT_PARENT, so many dirty nodes resolve
        // to the same ui_parent + would rescan the same subtree otherwise.
        let mut layout_children_memo: AHashMap<NodeID, Vec<NodeID>> = AHashMap::new();
        for &(node, flags) in &dirty_entries {
            let flags = if flags == 0 {
                DirtyState::UI_LAYOUT_MASK | DirtyState::DIRTY_COMMANDS
            } else {
                flags
            };
            if (flags & DirtyState::DIRTY_LAYOUT_PARENT) == 0 {
                continue;
            }
            if let Some(parent) = self.nodes.get(node).map(|node| node.parent)
                && let Some(ui_parent) = self.closest_ui_parent(parent)
                && self
                    .nodes
                    .get(ui_parent)
                    .and_then(|parent_node| ui_auto_layout_from_data(&parent_node.data))
                    .is_some()
            {
                if !layout_children_memo.contains_key(&ui_parent) {
                    let computed = self.ui_layout_children(ui_parent);
                    layout_children_memo.insert(ui_parent, computed);
                }
                if let Some(siblings) = layout_children_memo.get(&ui_parent) {
                    parent_siblings.insert(node, siblings.clone());
                }
            }
        }
        let nodes = &self.nodes;
        let plan = self.render_ui.collect_extraction_plan(
            dirty_entries.iter().copied(),
            all_ids.iter().copied(),
            UiExtractionOptions {
                mask: UiDirtyMask {
                    layout_mask: DirtyState::UI_LAYOUT_MASK,
                    layout_parent: DirtyState::DIRTY_LAYOUT_PARENT,
                    commands: DirtyState::DIRTY_COMMANDS,
                    default_flags: DirtyState::UI_LAYOUT_MASK | DirtyState::DIRTY_COMMANDS,
                },
                bootstrap_scan,
                input_changed,
            },
            |node, out| {
                if let Some(siblings) = parent_siblings.get(&node) {
                    out.extend(siblings.iter().copied());
                }
            },
            |node, out| {
                if let Some(node_ref) = nodes.get(node) {
                    out.extend(node_ref.get_children_ids().iter().copied());
                }
            },
        );
        dirty_entries.clear();
        all_ids.clear();
        parent_siblings.clear();
        self.render_ui.dirty_entries_scratch = dirty_entries;
        self.render_ui.all_ids_scratch = all_ids;
        self.render_ui.parent_siblings_scratch = parent_siblings;
        let traversal_ids = plan.traversal_ids;
        let mut command_ids = plan.command_ids;
        let mut command_seen = plan.command_seen;
        for (node, scene_node) in self.nodes.iter() {
            if matches!(
                scene_node.data,
                SceneNodeData::UiCameraStream(_) | SceneNodeData::UiViewport(_)
            ) && command_seen.insert(node)
            {
                command_ids.push(node);
            }
        }
        if let Some(timing) = timing.as_deref_mut() {
            timing.dirty_nodes = dirty_node_count.min(u32::MAX as usize) as u32;
            timing.affected_nodes = plan.affected_nodes;
        }
        let mut visible_now = std::mem::take(&mut self.render_ui.visible_now);
        visible_now.clear();
        visible_now.extend(self.render_ui.prev_visible.iter().copied());
        let mut removed_nodes = std::mem::take(&mut self.render_ui.removed_nodes);
        for node in removed_nodes.drain(..) {
            if self.render_ui.focused_text_edit == Some(node) {
                self.render_ui.focused_text_edit = None;
            }
            if self.render_ui.focused_ui_node == Some(node) {
                self.render_ui.focused_ui_node = None;
            }
            if self.render_ui.nav_pressed_button == Some(node) {
                self.render_ui.nav_pressed_button = None;
            }
            if self.render_ui.hovered_text_edit == Some(node) {
                self.render_ui.hovered_text_edit = None;
            }
            if self.render_ui.pressed_text_edit == Some(node) {
                self.render_ui.pressed_text_edit = None;
            }
            if self.render_ui.pressed_ui_button == Some(node) {
                self.render_ui.pressed_ui_button = None;
            }
            if self.render_ui.active_scrollbar == Some(node) {
                self.render_ui.active_scrollbar = None;
                self.render_ui.scrollbar_drag_offset = 0.0;
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
            self.render_ui.button_motions.remove(&node);
            self.render_ui.interactive_scan_seen.remove(&node);
            self.render_ui.visible_buttons.retain(|id| *id != node);
            self.render_ui.visible_text_edits.retain(|id| *id != node);
            self.render_ui.focusable_nodes.retain(|id| *id != node);
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
        let layout_start = timing.as_ref().map(|_| Instant::now());
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
        self.rebuild_visible_interactive_ui_cache(&computed);
        if let (Some(timing), Some(layout_start)) = (timing.as_deref_mut(), layout_start) {
            timing.layout += layout_start.elapsed();
        }

        // Layout already ran; dirty marks made by these input handlers
        // (dropdown open, tree toggle, checkbox) would be wiped by the
        // frame-end dirty clear before the next layout pass sees them.
        // Collect them so the bridge can re-apply after the clear.
        self.render_ui.defer_dirty_marks = true;
        self.process_ui_focus_input(&computed, &mut command_ids, &mut command_seen);
        self.process_text_edit_input(
            &computed,
            &computed_scales,
            &mut command_ids,
            &mut command_seen,
        );
        self.process_ui_scroll_input(
            &mut computed,
            &mut computed_scales,
            root_rect,
            &mut command_ids,
            &mut command_seen,
        );
        self.refresh_button_visual_states(&computed, &mut command_ids, &mut command_seen);
        self.render_ui.defer_dirty_marks = false;

        let commands_start = timing.as_ref().map(|_| Instant::now());
        for node in command_ids.iter().copied() {
            if let Some(timing) = timing.as_deref_mut() {
                timing.command_nodes = timing.command_nodes.saturating_add(1);
            }
            visible_now.remove(&node);
            let effective_visible = self.is_effectively_visible_for_ui(node);
            if let Some(texture) = self.resolve_ui_image_texture(node)
                && let Some(scene_node) = self.nodes.get_mut_untracked(node)
            {
                match &mut scene_node.data {
                    SceneNodeData::UiImage(image) => image.texture = texture,
                    SceneNodeData::UiImageButton(image) => image.texture = texture,
                    SceneNodeData::UiNineSliceButton(image) => image.texture = texture,
                    SceneNodeData::UiNineSlice(image) => image.texture = texture,
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
            if effective_visible
                && self.render_ui.retained_commands.contains_key(&node)
                && self.ui_image_has_pending_texture(node)
            {
                visible_now.insert(node);
                continue;
            }
            let effective_z = self.ui_effective_z(node);
            let rect_state = if let Some(rect) = computed.get(&node).copied() {
                let target = ui_rect_state_from_node(&scene_node.data, rect, state, effective_z);
                if let (Some(target), Some(motion)) =
                    (target, self.render_ui.button_motions.get(&node).copied())
                {
                    Some(animated_button_rect_state(
                        &scene_node.data,
                        rect,
                        effective_z,
                        target,
                        motion,
                    ))
                } else {
                    target
                }
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
                if matches!(
                    scene_node.data,
                    SceneNodeData::UiCameraStream(_) | SceneNodeData::UiViewport(_)
                ) {
                    self.queue_render_command(RenderCommand::CameraStream(
                        CameraStreamCommand::RemoveNode { node },
                    ));
                }
                self.remove_retained_ui_node(node);
                if let Some(timing) = timing.as_deref_mut() {
                    timing.removed_nodes = timing.removed_nodes.saturating_add(1);
                }
                continue;
            }
            let ui_stream = match &scene_node.data {
                SceneNodeData::UiCameraStream(stream_node) => Some(stream_node.stream.clone()),
                _ => None,
            };
            let ui_viewport = match &scene_node.data {
                SceneNodeData::UiViewport(viewport) => Some((**viewport).clone()),
                _ => None,
            };
            let mut camera_stream_texture = None;
            let mut camera_stream_resolution = None;
            if let Some(stream) = ui_stream {
                if let Some(state) = self.camera_stream_state(node, &stream) {
                    camera_stream_texture = Some(state.output_texture);
                    camera_stream_resolution = match &state.source {
                        CameraStreamSourceState::Webcam { resolution, .. } => Some(*resolution),
                        _ => Some(state.resolution),
                    };
                    self.queue_render_command(RenderCommand::CameraStream(
                        CameraStreamCommand::Upsert {
                            node,
                            state: Box::new(state),
                        },
                    ));
                } else {
                    self.queue_render_command(RenderCommand::CameraStream(
                        CameraStreamCommand::RemoveNode { node },
                    ));
                }
            }
            if let Some(viewport) = ui_viewport {
                if let Some(state) = self.ui_viewport_state(node, &viewport, rect_state.size) {
                    camera_stream_texture = Some(state.output_texture);
                    camera_stream_resolution = Some(state.resolution);
                    self.queue_render_command(RenderCommand::CameraStream(
                        CameraStreamCommand::Upsert {
                            node,
                            state: Box::new(state),
                        },
                    ));
                } else {
                    self.queue_render_command(RenderCommand::CameraStream(
                        CameraStreamCommand::RemoveNode { node },
                    ));
                }
            }
            let Some(scene_node) = self.nodes.get(node) else {
                self.remove_retained_ui_node(node);
                if let Some(timing) = timing.as_deref_mut() {
                    timing.removed_nodes = timing.removed_nodes.saturating_add(1);
                }
                continue;
            };
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
            if let SceneNodeData::UiScrollContainer(scroller) = &scene_node.data {
                let rect = computed_rect_from_state(&rect_state);
                let command = ui_scrollbar_command(
                    node,
                    scroller,
                    rect,
                    clip_rect,
                    self.scroll_container_max(node, &computed),
                    effective_z,
                );
                match command {
                    Some(command) => {
                        if self.render_ui.retained_commands.get(&node) != Some(&command) {
                            self.queue_render_command(RenderCommand::Ui(command.clone()));
                            self.render_ui.retained_commands.insert(node, command);
                            if let Some(timing) = timing.as_deref_mut() {
                                timing.command_emitted = timing.command_emitted.saturating_add(1);
                            }
                        } else if let Some(timing) = timing.as_deref_mut() {
                            timing.command_skipped = timing.command_skipped.saturating_add(1);
                        }
                    }
                    None => {
                        if self.render_ui.retained_commands.remove(&node).is_some() {
                            self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode {
                                node,
                            }));
                        }
                    }
                }
                self.render_ui.retained_rects.insert(node, rect_state);
                visible_now.insert(node);
                continue;
            }
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
                            modulate: self.effective_self_modulate(node),
                            camera_stream_texture,
                            camera_stream_resolution,
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
                    modulate: self.effective_self_modulate(node),
                    camera_stream_texture,
                    camera_stream_resolution,
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
        self.emit_color_picker_wheel_commands(&computed, viewport);
        for node in self.render_ui.prev_visible.iter().copied() {
            if !visible_now.contains(&node)
                && self.render_ui.retained_commands.contains_key(&node)
                && self.ui_image_has_pending_texture(node)
            {
                visible_now.insert(node);
            }
        }
        self.remove_no_longer_visible_ui_nodes(&visible_now);
        if let (Some(timing), Some(commands_start)) = (timing.as_deref_mut(), commands_start) {
            timing.commands += commands_start.elapsed();
        }

        self.render_ui.computed_rects = computed;
        self.render_ui.computed_scales = computed_scales;
        std::mem::swap(&mut self.render_ui.prev_visible, &mut visible_now);
        visible_now.clear();
        self.render_ui.visible_now = visible_now;

        self.render_ui
            .restore_extraction_plan(traversal_ids, command_ids, command_seen);

        if let (Some(timing), Some(total_start)) = (timing, total_start) {
            timing.total = total_start.elapsed();
        }
    }

    pub(super) fn has_active_scroll_container_animation(&self) -> bool {
        self.nodes.iter().any(|(_, node)| {
            matches!(
                &node.data,
                SceneNodeData::UiScrollContainer(scroller)
                    if scroller.scroll_animation.is_some()
            )
        })
    }
}

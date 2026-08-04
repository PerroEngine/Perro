use super::*;

/// `PERRO_STREAM_LOG=1` diagnostic gate, read once. The sub-view log site sits
/// on a per-frame rebuild path, so it must not re-read the environment.
#[cfg(not(target_arch = "wasm32"))]
static STREAM_LOG_ENABLED: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| std::env::var_os("PERRO_STREAM_LOG").is_some());

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
                self.queue_render_command(RenderCommand::Resource(Box::new(
                    ResourceCommand::CreateTexture {
                        request,
                        id: TextureID::nil(),
                        source,
                        reserved: false,
                    },
                )));
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
        // Raw field edits may bypass runtime dirty hooks. Check only retained UI
        // nodes + their parent chains, not every changed game node. The global
        // revision makes mouse-only frames skip this walk entirely.
        let arena_revision = self.nodes.mutation_revision();
        let covered_arena_revision = self.render_ui.arena_mutation_revision;
        let mut arena_changed_ids = std::mem::take(&mut self.render_ui.all_ids_scratch);
        arena_changed_ids.clear();
        if covered_arena_revision != arena_revision {
            let mut changed_seen = std::mem::take(&mut self.render_ui.traversal_seen);
            changed_seen.clear();
            for visible in self.render_ui.prev_visible.iter().copied() {
                let mut cursor = Some(visible);
                while let Some(node) = cursor {
                    let Some(scene_node) = self.nodes.get(node) else {
                        break;
                    };
                    if self
                        .nodes
                        .node_change_stamp(node)
                        .is_some_and(|stamp| stamp > covered_arena_revision)
                        && changed_seen.insert(node)
                    {
                        arena_changed_ids.push(node);
                    }
                    cursor = (scene_node.parent != NodeID::nil()).then_some(scene_node.parent);
                }
            }
            changed_seen.clear();
            self.render_ui.traversal_seen = changed_seen;
            self.render_ui.arena_mutation_revision = arena_revision;
        }
        let retained_arena_mutation_changed = !arena_changed_ids.is_empty();
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
            || retained_arena_mutation_changed
            || input_changed
            || scroll_input_changed
            || text_input_changed
            || dropdown_animation_changed
            || !self.render_ui.button_motions.is_empty()
            || self.has_active_scroll_container_animation();
        if !has_extraction_work {
            arena_changed_ids.clear();
            self.render_ui.all_ids_scratch = arena_changed_ids;
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
        // Raw/field-level node edits move per-node arena stamps even when they
        // bypass runtime UI dirty hooks. Scan that compact lane only when the
        // global arena revision moved, and expand changed roots through the
        // normal traversal. Mouse-only frames never enter this path.
        if retained_arena_mutation_changed {
            dirty_entries.extend(
                arena_changed_ids
                    .iter()
                    .copied()
                    .filter(|&node| !self.dirty.is_node_dirty(node))
                    .map(|node| {
                        (
                            node,
                            DirtyState::UI_LAYOUT_MASK | DirtyState::DIRTY_COMMANDS,
                        )
                    }),
            );
        }
        arena_changed_ids.clear();
        self.render_ui.all_ids_scratch = arena_changed_ids;
        dirty_entries.retain(|(node, _)| self.node_world(*node) == Some(NodeID::nil()));
        let dirty_node_count = dirty_entries.len();
        // shared member view: refcount clone, no per-pass Vec copy.
        let all_ids = self.world_members_arc(NodeID::nil());
        let mut layout_parents = std::mem::take(&mut self.render_ui.layout_parent_scratch);
        layout_parents.clear();
        // dedup the layout-children DFS per ui_parent: when a container changes,
        // all its children get DIRTY_LAYOUT_PARENT, so many dirty nodes resolve
        // to the same ui_parent + would rescan the same subtree otherwise.
        // The memo stores ranges into one flat scratch vec (both persist in
        // RenderUiState), so this pass allocates nothing at steady state.
        let mut layout_children_memo =
            std::mem::take(&mut self.render_ui.layout_children_memo_scratch);
        layout_children_memo.clear();
        let mut layout_children_flat =
            std::mem::take(&mut self.render_ui.layout_children_flat_scratch);
        layout_children_flat.clear();
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
                    let start = layout_children_flat.len() as u32;
                    self.ui_layout_children_into(ui_parent, &mut layout_children_flat);
                    layout_children_memo
                        .insert(ui_parent, (start, layout_children_flat.len() as u32));
                }
                layout_parents.insert(node, ui_parent);
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
            },
            |node, out| {
                if let Some(ui_parent) = layout_parents.get(&node)
                    && let Some(&(start, end)) = layout_children_memo.get(ui_parent)
                {
                    out.extend(
                        layout_children_flat[start as usize..end as usize]
                            .iter()
                            .copied(),
                    );
                }
            },
            |node, out| {
                if let Some(node_ref) = nodes.get(node)
                    && !matches!(
                        node_ref.data,
                        SceneNodeData::UiSubView(_)
                            | SceneNodeData::SubView2D(_)
                            | SceneNodeData::SubView3D(_)
                    )
                {
                    out.extend(node_ref.get_children_ids().iter().copied());
                }
            },
        );
        dirty_entries.clear();
        layout_parents.clear();
        layout_children_memo.clear();
        layout_children_flat.clear();
        self.render_ui.dirty_entries_scratch = dirty_entries;
        self.render_ui.layout_parent_scratch = layout_parents;
        self.render_ui.layout_children_memo_scratch = layout_children_memo;
        self.render_ui.layout_children_flat_scratch = layout_children_flat;
        let traversal_ids = plan.traversal_ids;
        let mut command_ids = plan.command_ids;
        let mut command_seen = plan.command_seen;
        // Stream/sub-view rebuild is dirty-world gated: a full state rebuild
        // walks the watched world through every collector and its Upsert
        // wakes a full gpu frame, so input-only passes (pointer move, button
        // motion) must not touch clean streams. Rebuild when the node itself
        // is dirty, its watched world holds a dirty node, or the source is a
        // webcam (frames + probed resolution resolve async, O(1) state path).
        let mut dirty_worlds = std::mem::take(&mut self.dirty_world_scratch);
        self.collect_dirty_worlds(&mut dirty_worlds);
        let mut stream_nodes = std::mem::take(&mut self.stream_node_scratch);
        self.fill_stream_nodes(&mut stream_nodes);
        for node in stream_nodes.drain(..) {
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            let rebuild = match &scene_node.data {
                SceneNodeData::UiSubView(_) => {
                    bootstrap_scan || dirty_worlds.contains(&node) || self.dirty.is_node_dirty(node)
                }
                SceneNodeData::UiCameraStream(stream) => {
                    let camera = stream.stream.camera;
                    bootstrap_scan
                        || self.dirty.is_node_dirty(node)
                        || self.nodes.get(camera).is_some_and(|camera_node| {
                            matches!(camera_node.data, SceneNodeData::Webcam(_))
                        })
                        || self
                            .node_world(camera)
                            .is_some_and(|world| dirty_worlds.contains(&world))
                }
                _ => continue,
            };
            if rebuild && self.node_world(node) == Some(NodeID::nil()) && command_seen.insert(node)
            {
                command_ids.push(node);
            }
        }
        self.stream_node_scratch = stream_nodes;
        // dirty_worlds stays live: the command loop re-checks it per stream
        // visit (input passes re-add every retained command node); restored
        // to the scratch slot after the loop.
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
                self.queue_render_command(RenderCommand::Ui(Box::new(UiCommand::RemoveNode {
                    node,
                })));
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
        // Resolve pointer hover once for all text/button handlers in this pass.
        // The screen point itself is also cached by `ui_pointer_screen_point`.
        let pointer_point = self.ui_pointer_screen_point();
        let hovered_text_edit =
            self.hovered_text_edit(&computed, UiInputSource::Kbm, pointer_point);
        self.process_ui_focus_input(&computed, &mut command_ids, &mut command_seen);
        self.process_text_edit_input(
            &computed,
            &computed_scales,
            hovered_text_edit,
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
        self.refresh_button_visual_states(
            &computed,
            hovered_text_edit,
            &mut command_ids,
            &mut command_seen,
        );
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
                    SceneNodeData::UiCameraStream(_) | SceneNodeData::UiSubView(_)
                ) {
                    self.ui_stream_render_info.remove(&node);
                    self.queue_camera_stream_remove(node);
                }
                self.remove_retained_ui_node(node);
                if let Some(timing) = timing.as_deref_mut() {
                    timing.removed_nodes = timing.removed_nodes.saturating_add(1);
                }
                continue;
            }
            // Clone gating: the stream/sub-view deep clones only happen inside
            // the rebuild branch; input-only visits read the cached info from
            // borrowed data without copying any stream state.
            let mut camera_stream_texture = None;
            let mut camera_stream_resolution = None;
            if let SceneNodeData::UiCameraStream(stream_node) = &scene_node.data {
                // dirty-world gate: rebuilding re-collects the watched world
                // through every collector; input-refresh visits reuse the
                // cached output texture/resolution instead.
                let camera = stream_node.stream.camera;
                let rebuild = bootstrap_scan
                    || self.dirty.is_node_dirty(node)
                    || self.nodes.get(camera).is_some_and(|camera_node| {
                        matches!(camera_node.data, SceneNodeData::Webcam(_))
                    })
                    || self
                        .node_world(camera)
                        .is_some_and(|world| dirty_worlds.contains(&world))
                    || !self.ui_stream_render_info.contains_key(&node);
                if rebuild {
                    let stream = stream_node.stream.clone();
                    if let Some(state) = self.camera_stream_state(node, &stream) {
                        camera_stream_texture = Some(state.output_texture);
                        camera_stream_resolution = match &state.source {
                            CameraStreamSourceState::Webcam { resolution, .. } => Some(*resolution),
                            _ => Some(state.resolution),
                        };
                        self.ui_stream_render_info.insert(
                            node,
                            (
                                state.output_texture,
                                camera_stream_resolution.unwrap_or(state.resolution),
                                [0.0, 0.0],
                            ),
                        );
                        self.queue_camera_stream_upsert(node, std::sync::Arc::new(state));
                    } else {
                        self.ui_stream_render_info.remove(&node);
                        self.queue_camera_stream_remove(node);
                    }
                } else if let Some((texture, resolution, _)) = self.ui_stream_render_info.get(&node)
                {
                    camera_stream_texture = Some(*texture);
                    camera_stream_resolution = Some(*resolution);
                }
            }
            // Re-borrow: the camera-stream rebuild above may call `&mut self`.
            if let Some(SceneNodeData::UiSubView(viewport)) =
                self.nodes.get(node).map(|scene_node| &scene_node.data)
            {
                let rebuild = bootstrap_scan
                    || self.dirty.is_node_dirty(node)
                    || dirty_worlds.contains(&node)
                    || self
                        .ui_stream_render_info
                        .get(&node)
                        .is_none_or(|(_, _, rect_size)| *rect_size != rect_state.size);
                if rebuild {
                    let sub_view = perro_nodes::SubView::from(viewport.as_ref());
                    if let Some(state) = self.sub_view_state(node, &sub_view, Some(rect_state.size))
                    {
                        // PERRO_STREAM_LOG=1: the UI-layout rect a depth-0 sub
                        // view actually occupies, in the units auto-resolution
                        // consumes. This rebuild path runs per frame for a live
                        // stream, so the env read is resolved once, not per
                        // frame.
                        #[cfg(not(target_arch = "wasm32"))]
                        if *STREAM_LOG_ENABLED {
                            eprintln!(
                                "[perro][runtime] sub view node={} ui_rect={}x{} -> target={}x{}",
                                node.as_u64(),
                                rect_state.size[0],
                                rect_state.size[1],
                                state.resolution[0],
                                state.resolution[1],
                            );
                        }
                        camera_stream_texture = Some(state.output_texture);
                        camera_stream_resolution = Some(state.resolution);
                        self.ui_stream_render_info.insert(
                            node,
                            (state.output_texture, state.resolution, rect_state.size),
                        );
                        self.queue_camera_stream_upsert(node, std::sync::Arc::new(state));
                    } else {
                        self.ui_stream_render_info.remove(&node);
                        self.queue_camera_stream_remove(node);
                    }
                } else if let Some((texture, resolution, _)) = self.ui_stream_render_info.get(&node)
                {
                    camera_stream_texture = Some(*texture);
                    camera_stream_resolution = Some(*resolution);
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
                    virtual_font_scale,
                );
                match command {
                    Some(command) => {
                        if self.render_ui.retained_commands.get(&node) != Some(&command) {
                            self.queue_render_command(RenderCommand::Ui(Box::new(command.clone())));
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
                            self.queue_render_command(RenderCommand::Ui(Box::new(
                                UiCommand::RemoveNode { node },
                            )));
                        }
                    }
                }
                self.render_ui.retained_rects.insert(node, rect_state);
                visible_now.insert(node);
                continue;
            }
            // Build once: with Arc-backed text/font fields this is allocation
            // free (stack struct + refcount bumps), so the unchanged path costs
            // one build + compare, and the changed path reuses the same value.
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
            if self.render_ui.retained_commands.get(&node) != Some(&command) {
                self.queue_render_command(RenderCommand::Ui(Box::new(command.clone())));
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
        dirty_worlds.clear();
        self.dirty_world_scratch = dirty_worlds;
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
        self.render_ui.arena_mutation_revision = self.nodes.mutation_revision();

        if let (Some(timing), Some(total_start)) = (timing, total_start) {
            timing.total = total_start.elapsed();
        }
    }

    pub(super) fn has_active_scroll_container_animation(&mut self) -> bool {
        // The animation flag itself cannot be counted: `UiScrollContainer::
        // scroll_to` is a pub field-level API scripts reach through
        // `with_node_mut` (no runtime hook, no ui-payload dirty fingerprint), so
        // a start/stop counter would miss script-started animations and stall
        // them. The *set of scroll containers* only moves on structural arena
        // changes, so cache that instead: an idle frame then derefs K scroll
        // containers rather than scanning every slot.
        let revision = self.nodes.structural_revision();
        if self.render_ui.scroll_container_ids_revision != Some(revision) {
            let mut ids = std::mem::take(&mut self.render_ui.scroll_container_ids);
            ids.clear();
            self.nodes
                .append_type_ids(perro_nodes::NodeType::UiScrollContainer, &mut ids);
            self.render_ui.scroll_container_ids = ids;
            self.render_ui.scroll_container_ids_revision = Some(revision);
        }
        self.render_ui.scroll_container_ids.iter().any(|&id| {
            matches!(
                self.nodes.get(id).map(|node| &node.data),
                Some(SceneNodeData::UiScrollContainer(scroller))
                    if scroller.scroll_animation.is_some()
            )
        })
    }
}

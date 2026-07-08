//! Runtime UI layout, retained command extraction, and text input handling.

use super::state::{DirtyState, UiButtonVisualState};
use super::{Runtime, RuntimeUiTiming};
use ahash::AHashMap;
use perro_ids::{NodeID, SignalID, TextureID};
#[cfg(test)]
use perro_input_api::GamepadAxis;
use perro_input_api::{GamepadButton, JoyConButton, KeyCode, MouseButton, PlayerBinding};
use perro_nodes::{SceneNode, SceneNodeData};
use perro_render_bridge::{
    CameraStreamCommand, RenderCommand, ResourceCommand, UiCommand, UiCornerRadiiState,
    UiDepthEffectState, UiFillKindState, UiImageScaleState, UiLinearGradientState, UiRectState,
    UiTextAlignState,
};
use perro_runtime_render::{UiDirtyMask, UiExtractionOptions, ui_image_texture_request};
use perro_structs::{Color, UVector2, Vector2};
use perro_ui::{
    ComputedUiRect, UiAnchor, UiButton, UiFontSizing, UiHorizontalAlign, UiImageScaleMode,
    UiLayoutData, UiLayoutMode, UiLayoutSpacingMode, UiNode, UiPanel, UiSizeMode, UiStyle,
    UiTextBox, UiTextEdit, UiTransform, UiVector2, UiVerticalAlign,
};
use perro_variant::Variant;
use std::borrow::Cow;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[path = "ui/color_picker.rs"]
mod color_picker;
#[path = "ui/locale.rs"]
mod locale;

use color_picker::*;

const TEXT_EDIT_REPEAT_DELAY: f32 = 0.35;
const TEXT_EDIT_REPEAT_RATE: f32 = 0.035;
const UI_NAV_REPEAT_DELAY: f32 = 0.35;
const UI_NAV_REPEAT_RATE: f32 = 0.15;
const UI_NAV_STICK_ON: f32 = 0.55;
const UI_NAV_STICK_OFF: f32 = 0.35;

impl Runtime {
    fn rebuild_visible_interactive_ui_cache(
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

    fn collect_visible_interactive_ui_node(
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

    fn resolve_ui_image_texture(&mut self, node: NodeID) -> Option<TextureID> {
        let mut texture = self
            .nodes
            .get(node)
            .and_then(|scene_node| match &scene_node.data {
                SceneNodeData::UiImage(image) => Some(image.texture),
                SceneNodeData::UiImageButton(image) => Some(image.texture),
                SceneNodeData::UiNineSlice(image) => Some(image.texture),
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

        if self.resource_api.is_texture_id_pending(texture) {
            return None;
        }

        Some(texture)
    }

    fn ui_image_has_pending_texture(&self, node: NodeID) -> bool {
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
                SceneNodeData::UiNineSlice(image) => {
                    !image.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(image.texture)
                }
                SceneNodeData::UiAnimatedImage(image) => {
                    !image.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(image.texture)
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

    fn extract_render_ui_commands_inner(&mut self, timing: Option<&mut RuntimeUiTiming>) {
        self.refresh_locale_text_bindings();
        self.render_ui.pointer_screen_point = None;
        let total_start = timing.as_ref().map(|_| Instant::now());
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
        let dirty_entries = self
            .dirty
            .dirty_indices()
            .iter()
            .filter_map(|&raw_index| {
                let index = raw_index as usize;
                self.nodes
                    .slot_get(index)
                    .map(|(node, _)| (node, self.dirty.ui_flags_at(index)))
            })
            .collect::<Vec<_>>();
        let dirty_node_count = dirty_entries.len();
        let all_ids = self.nodes.iter().map(|(id, _)| id).collect::<Vec<_>>();
        let mut parent_siblings = AHashMap::<NodeID, Vec<NodeID>>::default();
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
                parent_siblings.insert(node, self.ui_layout_children(ui_parent));
            }
        }
        let nodes = &self.nodes;
        let plan = self.render_ui.collect_extraction_plan(
            dirty_entries,
            all_ids,
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
            |node| parent_siblings.get(&node).cloned().unwrap_or_default(),
            |node, out| {
                if let Some(node_ref) = nodes.get(node) {
                    out.extend(node_ref.get_children_ids().iter().copied());
                }
            },
        );
        let traversal_ids = plan.traversal_ids;
        let mut command_ids = plan.command_ids;
        let mut command_seen = plan.command_seen;
        for (node, scene_node) in self.nodes.iter() {
            if matches!(scene_node.data, SceneNodeData::UiCameraStream(_))
                && command_seen.insert(node)
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
                && let Some(scene_node) = self.nodes.get_mut(node)
            {
                match &mut scene_node.data {
                    SceneNodeData::UiImage(image) => image.texture = texture,
                    SceneNodeData::UiImageButton(image) => image.texture = texture,
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
                if matches!(scene_node.data, SceneNodeData::UiCameraStream(_)) {
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
            if let Some(stream) = ui_stream {
                if let Some(state) = self.camera_stream_state(node, &stream) {
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

    fn has_active_scroll_container_animation(&self) -> bool {
        self.nodes.iter().any(|(_, node)| {
            matches!(
                &node.data,
                SceneNodeData::UiScrollContainer(scroller)
                    if scroller.scroll_animation.is_some()
            )
        })
    }
}

impl Runtime {
    fn ensure_tree_list_internal_nodes(&mut self) {
        // Full sync clones the item list and re-marks every internal row
        // dirty, so it must not run on unrelated extraction work (pointer
        // moves, other widgets): gate it on the tree node itself being
        // dirty. Engine-side mutations (row clicks) call
        // sync_tree_list_internal_nodes directly.
        let tree_ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| match &node.data {
                SceneNodeData::UiTreeList(tree) => {
                    let never_synced = tree.internal_rows.is_empty() && !tree.items.is_empty();
                    Some((id, never_synced))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for (tree_id, never_synced) in tree_ids {
            if never_synced || self.dirty.ui_flags_at(tree_id.index() as usize) != 0 {
                self.sync_tree_list_internal_nodes(tree_id);
            }
        }
    }

    fn ensure_tree_list_internal_nodes_for(&mut self, tree_id: NodeID) {
        let Some((mut rows, mut toggles, mut icons, mut labels, mut lines, row_count)) =
            self.nodes.get(tree_id).and_then(|node| match &node.data {
                SceneNodeData::UiTreeList(tree) => Some((
                    tree.internal_rows.clone(),
                    tree.internal_toggles.clone(),
                    tree.internal_icons.clone(),
                    tree.internal_labels.clone(),
                    tree.internal_lines.clone(),
                    tree.visible_items().len(),
                )),
                _ => None,
            })
        else {
            return;
        };

        for id in rows.iter().copied().skip(row_count) {
            self.hide_tree_list_internal_node(id);
        }
        for id in toggles.iter().copied().skip(row_count) {
            self.hide_tree_list_internal_node(id);
        }
        for id in icons.iter().copied().skip(row_count) {
            self.hide_tree_list_internal_node(id);
        }
        for id in labels.iter().copied().skip(row_count) {
            self.hide_tree_list_internal_node(id);
        }
        for pair in lines.iter().copied().skip(row_count) {
            self.hide_tree_list_internal_node(pair[0]);
            self.hide_tree_list_internal_node(pair[1]);
        }

        if rows.len() < row_count {
            rows.resize(row_count, NodeID::nil());
        }
        if toggles.len() < row_count {
            toggles.resize(row_count, NodeID::nil());
        }
        if icons.len() < row_count {
            icons.resize(row_count, NodeID::nil());
        }
        if labels.len() < row_count {
            labels.resize(row_count, NodeID::nil());
        }
        if lines.len() < row_count {
            lines.resize(row_count, [NodeID::nil(); 2]);
        }

        for idx in 0..row_count {
            if !self.tree_list_internal_valid(rows[idx], tree_id, "button") {
                rows[idx] = self.insert_tree_list_row(tree_id, idx);
            }
            if !self.tree_list_internal_valid(toggles[idx], rows[idx], "shape") {
                toggles[idx] = self.insert_tree_list_toggle(rows[idx], idx);
            }
            if !self.tree_list_internal_valid(icons[idx], rows[idx], "image") {
                icons[idx] = self.insert_tree_list_icon(rows[idx], idx);
            }
            if !self.tree_list_internal_valid(labels[idx], rows[idx], "label") {
                labels[idx] = self.insert_tree_list_label(rows[idx], idx);
            }
            for (line_idx, line) in lines[idx].iter_mut().enumerate() {
                if !self.tree_list_internal_valid(*line, rows[idx], "panel") {
                    *line = self.insert_tree_list_line(rows[idx], idx, line_idx);
                }
            }
        }

        if let Some(node) = self.nodes.get_mut(tree_id)
            && let SceneNodeData::UiTreeList(tree) = &mut node.data
        {
            tree.internal_rows = rows;
            tree.internal_toggles = toggles;
            tree.internal_icons = icons;
            tree.internal_labels = labels;
            tree.internal_lines = lines;
        }
    }

    fn hide_tree_list_internal_node(&mut self, id: NodeID) {
        if let Some(node) = self.nodes.get_mut(id)
            && let Some(ui) = ui_root_mut_from_data(&mut node.data)
        {
            ui.visible = false;
        }
    }

    fn tree_list_internal_valid(&self, id: NodeID, parent: NodeID, kind: &str) -> bool {
        if id.is_nil() {
            return false;
        }
        self.nodes.get(id).is_some_and(|node| {
            node.parent == parent
                && match kind {
                    "button" => matches!(node.data, SceneNodeData::UiButton(_)),
                    "shape" => matches!(node.data, SceneNodeData::UiShape(_)),
                    "image" => matches!(node.data, SceneNodeData::UiImage(_)),
                    "label" => matches!(node.data, SceneNodeData::UiLabel(_)),
                    "panel" => matches!(node.data, SceneNodeData::UiPanel(_)),
                    _ => false,
                }
        })
    }

    fn insert_tree_list_row(&mut self, tree_id: NodeID, idx: usize) -> NodeID {
        let mut button = UiButton::new();
        button.base.layout.anchor = UiAnchor::Top;
        button.base.layout.z_index = 1;
        button.base.clip_children = false;
        button.style.fill = Color::TRANSPARENT;
        button.style.stroke = Color::TRANSPARENT;
        button.hover_style.fill = Color::new(0.18, 0.22, 0.30, 1.0);
        button.pressed_style.fill = Color::new(0.12, 0.16, 0.24, 1.0);
        self.insert_color_picker_internal_node(
            tree_id,
            Box::leak(format!("__perro_tree_list_row_{idx}").into_boxed_str()),
            SceneNodeData::UiButton(Box::new(button)),
        )
    }

    fn insert_tree_list_toggle(&mut self, row_id: NodeID, idx: usize) -> NodeID {
        let mut shape = perro_ui::UiShape::new();
        shape.base.layout.anchor = UiAnchor::Left;
        shape.base.layout.z_index = 3;
        shape.base.input_enabled = false;
        shape.base.mouse_filter = perro_ui::UiMouseFilter::Pass;
        shape.kind = perro_ui::UiShapeKind::Triangle;
        self.insert_color_picker_internal_node(
            row_id,
            Box::leak(format!("__perro_tree_list_toggle_{idx}").into_boxed_str()),
            SceneNodeData::UiShape(shape),
        )
    }

    fn insert_tree_list_icon(&mut self, row_id: NodeID, idx: usize) -> NodeID {
        let mut image = perro_ui::UiImage::new();
        image.base.layout.anchor = UiAnchor::Left;
        image.base.layout.z_index = 3;
        image.base.input_enabled = false;
        image.base.mouse_filter = perro_ui::UiMouseFilter::Pass;
        self.insert_color_picker_internal_node(
            row_id,
            Box::leak(format!("__perro_tree_list_icon_{idx}").into_boxed_str()),
            SceneNodeData::UiImage(Box::new(image)),
        )
    }

    fn insert_tree_list_label(&mut self, row_id: NodeID, idx: usize) -> NodeID {
        let mut label = perro_ui::UiLabel::new();
        label.base.layout.anchor = UiAnchor::Left;
        label.base.layout.z_index = 3;
        label.base.input_enabled = false;
        label.base.mouse_filter = perro_ui::UiMouseFilter::Pass;
        label.h_align = perro_ui::UiTextAlign::Start;
        label.v_align = perro_ui::UiTextAlign::Center;
        label.text_size_ratio = 0.62;
        self.insert_color_picker_internal_node(
            row_id,
            Box::leak(format!("__perro_tree_list_label_{idx}").into_boxed_str()),
            SceneNodeData::UiLabel(Box::new(label)),
        )
    }

    fn insert_tree_list_line(&mut self, row_id: NodeID, idx: usize, line_idx: usize) -> NodeID {
        let mut panel = UiPanel::new();
        panel.base.layout.anchor = UiAnchor::Left;
        panel.base.layout.z_index = 2;
        panel.base.input_enabled = false;
        panel.base.mouse_filter = perro_ui::UiMouseFilter::Pass;
        panel.style.stroke_width = 0.0;
        self.insert_color_picker_internal_node(
            row_id,
            Box::leak(format!("__perro_tree_list_line_{idx}_{line_idx}").into_boxed_str()),
            SceneNodeData::UiPanel(Box::new(panel)),
        )
    }

    fn sync_tree_list_internal_nodes(&mut self, tree_id: NodeID) {
        self.ensure_tree_list_internal_nodes_for(tree_id);
        let Some(snapshot) = self.nodes.get(tree_id).and_then(|node| match &node.data {
            SceneNodeData::UiTreeList(tree) => Some((
                tree.visible,
                tree.visible_items(),
                tree.items.clone(),
                tree.selected_index,
                tree.indent,
                tree.row_height,
                tree.v_spacing,
                tree.icon_size,
                tree.toggle_size,
                tree.line_width,
                tree.line_color,
                tree.triangle_color,
                tree.text_color,
                tree.row_style.clone(),
                tree.row_hover_style.clone(),
                tree.row_pressed_style.clone(),
                tree.selected_style.clone(),
                tree.internal_rows.clone(),
                tree.internal_toggles.clone(),
                tree.internal_icons.clone(),
                tree.internal_labels.clone(),
                tree.internal_lines.clone(),
            )),
            _ => None,
        }) else {
            return;
        };
        let (
            visible,
            rows,
            items,
            selected_index,
            indent,
            row_height,
            v_spacing,
            icon_size,
            toggle_size,
            line_width,
            line_color,
            triangle_color,
            text_color,
            row_style,
            row_hover_style,
            row_pressed_style,
            selected_style,
            internal_rows,
            internal_toggles,
            internal_icons,
            internal_labels,
            internal_lines,
        ) = snapshot;
        let spacing = ui_v_spacing_amount(v_spacing, row_height);
        for (visible_idx, row) in rows.iter().enumerate() {
            let Some(item) = items.get(row.index) else {
                continue;
            };
            let y = -((row_height + spacing) * visible_idx as f32);
            let x = indent * row.depth as f32;
            if let Some(node) = self.nodes.get_mut(internal_rows[visible_idx])
                && let SceneNodeData::UiButton(button) = &mut node.data
            {
                button.base.visible = visible;
                button.base.layout.size = UiVector2::new(
                    perro_ui::UiUnit::Percent(100.0),
                    perro_ui::UiUnit::Pixels(row_height),
                );
                button.base.transform.position = UiVector2::pixels(0.0, y);
                button.base.layout.anchor = UiAnchor::Top;
                button.style = if selected_index == Some(row.index) {
                    selected_style.clone()
                } else {
                    row_style.clone()
                };
                button.hover_style = row_hover_style.clone();
                button.pressed_style = row_pressed_style.clone();
                button.disabled = !item.selectable;
            }
            if let Some(node) = self.nodes.get_mut(internal_toggles[visible_idx])
                && let SceneNodeData::UiShape(shape) = &mut node.data
            {
                shape.base.visible = visible && row.has_children;
                shape.base.layout.size = UiVector2::pixels(toggle_size, toggle_size);
                shape.base.transform.position = UiVector2::pixels(x + toggle_size * 0.5, 0.0);
                shape.fill = triangle_color;
                shape.stroke = Color::TRANSPARENT;
                shape.base.transform.rotation = if item.open {
                    std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                };
            }
            if let Some(node) = self.nodes.get_mut(internal_icons[visible_idx])
                && let SceneNodeData::UiImage(image) = &mut node.data
            {
                image.base.visible = visible && !item.icon.is_nil();
                image.base.layout.size = UiVector2::pixels(icon_size, icon_size);
                image.base.transform.position =
                    UiVector2::pixels(x + toggle_size + icon_size * 0.5 + 3.0, 0.0);
                image.texture = item.icon;
            }
            if let Some(node) = self.nodes.get_mut(internal_labels[visible_idx])
                && let SceneNodeData::UiLabel(label) = &mut node.data
            {
                let icon_width = if item.icon.is_nil() {
                    0.0
                } else {
                    icon_size + 4.0
                };
                label.base.visible = visible;
                label.base.layout.size = UiVector2::new(
                    perro_ui::UiUnit::Percent(100.0),
                    perro_ui::UiUnit::Pixels(row_height),
                );
                label.base.transform.position =
                    UiVector2::pixels(x + toggle_size + icon_width + 8.0, 0.0);
                label.color = text_color;
                if label.text != item.label {
                    label.set_text(item.label.to_string());
                }
            }
            if let Some(pair) = internal_lines.get(visible_idx).copied() {
                self.sync_tree_list_line(
                    pair[0],
                    visible,
                    row.last_child,
                    x,
                    row_height,
                    line_width,
                    line_color,
                    true,
                );
                self.sync_tree_list_line(
                    pair[1],
                    visible,
                    row.last_child,
                    x,
                    row_height,
                    line_width,
                    line_color,
                    false,
                );
            }
        }
        for id in internal_rows.iter().copied().skip(rows.len()) {
            self.hide_tree_list_internal_node(id);
        }
        for id in internal_toggles.iter().copied().skip(rows.len()) {
            self.hide_tree_list_internal_node(id);
        }
        for id in internal_icons.iter().copied().skip(rows.len()) {
            self.hide_tree_list_internal_node(id);
        }
        for id in internal_labels.iter().copied().skip(rows.len()) {
            self.hide_tree_list_internal_node(id);
        }
        for pair in internal_lines.iter().copied().skip(rows.len()) {
            self.hide_tree_list_internal_node(pair[0]);
            self.hide_tree_list_internal_node(pair[1]);
        }
        self.mark_ui_dirty(
            tree_id,
            Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_COMMANDS,
        );
        for id in internal_rows
            .into_iter()
            .chain(internal_toggles)
            .chain(internal_icons)
            .chain(internal_labels)
            .chain(internal_lines.into_iter().flat_map(|pair| pair.into_iter()))
        {
            if !id.is_nil() {
                self.mark_ui_dirty(
                    id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_COMMANDS,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sync_tree_list_line(
        &mut self,
        id: NodeID,
        visible: bool,
        last_child: bool,
        x: f32,
        row_height: f32,
        line_width: f32,
        line_color: Color,
        vertical: bool,
    ) {
        if let Some(node) = self.nodes.get_mut(id)
            && let SceneNodeData::UiPanel(panel) = &mut node.data
        {
            panel.base.visible = visible && line_width > 0.0;
            panel.style.fill = line_color;
            if vertical {
                panel.base.visible = panel.base.visible && !last_child;
                panel.base.layout.size = UiVector2::pixels(line_width, row_height);
                panel.base.transform.position = UiVector2::pixels(x + 5.0, 0.0);
            } else {
                panel.base.layout.size = UiVector2::pixels(8.0, line_width);
                panel.base.transform.position = UiVector2::pixels(x + 9.0, 0.0);
            }
        }
    }

    fn ensure_dropdown_internal_nodes(&mut self) {
        // Same gating as tree lists: only re-sync dropdowns whose node is
        // dirty this frame. Engine-side open/select paths call
        // sync_dropdown_internal_nodes directly.
        let dropdown_ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| match &node.data {
                SceneNodeData::UiDropdown(dropdown) => Some((id, dropdown.internal_label.is_nil())),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (dropdown_id, never_synced) in dropdown_ids {
            if never_synced || self.dirty.ui_flags_at(dropdown_id.index() as usize) != 0 {
                self.ensure_dropdown_internal_nodes_for(dropdown_id);
                self.sync_dropdown_internal_nodes(dropdown_id);
            }
        }
    }

    fn ensure_dropdown_internal_nodes_for(&mut self, dropdown_id: NodeID) {
        let Some((label_id, mut option_buttons, mut option_labels, option_count)) = self
            .nodes
            .get(dropdown_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiDropdown(dropdown) => Some((
                    dropdown.internal_label,
                    dropdown.internal_option_buttons.clone(),
                    dropdown.internal_option_labels.clone(),
                    dropdown.options.len(),
                )),
                _ => None,
            })
        else {
            return;
        };

        let mut label_id = label_id;
        if !self.dropdown_internal_valid(label_id, dropdown_id, "label") {
            label_id = self.insert_dropdown_label(dropdown_id, "__perro_dropdown_label");
        }
        for id in option_buttons.iter().copied().skip(option_count) {
            if let Some(node) = self.nodes.get_mut(id)
                && let SceneNodeData::UiButton(button) = &mut node.data
            {
                button.base.visible = false;
            }
        }
        for id in option_labels.iter().copied().skip(option_count) {
            if let Some(node) = self.nodes.get_mut(id)
                && let SceneNodeData::UiLabel(label) = &mut node.data
            {
                label.base.visible = false;
            }
        }
        option_buttons.resize(option_count, NodeID::nil());
        option_labels.resize(option_count, NodeID::nil());
        for idx in 0..option_count {
            if !self.dropdown_internal_valid(option_buttons[idx], dropdown_id, "button") {
                option_buttons[idx] = self.insert_dropdown_option_button(dropdown_id, idx);
            }
            if !self.dropdown_internal_valid(option_labels[idx], option_buttons[idx], "label") {
                option_labels[idx] = self
                    .insert_dropdown_label(option_buttons[idx], "__perro_dropdown_option_label");
            }
        }

        if let Some(node) = self.nodes.get_mut(dropdown_id)
            && let SceneNodeData::UiDropdown(dropdown) = &mut node.data
        {
            dropdown.internal_label = label_id;
            dropdown.internal_option_buttons = option_buttons;
            dropdown.internal_option_labels = option_labels;
        }
    }

    fn dropdown_internal_valid(&self, id: NodeID, parent: NodeID, kind: &str) -> bool {
        if id.is_nil() {
            return false;
        }
        self.nodes.get(id).is_some_and(|node| {
            node.parent == parent
                && match kind {
                    "button" => matches!(node.data, SceneNodeData::UiButton(_)),
                    "label" => matches!(node.data, SceneNodeData::UiLabel(_)),
                    _ => false,
                }
        })
    }

    fn insert_dropdown_option_button(&mut self, dropdown_id: NodeID, idx: usize) -> NodeID {
        let mut button = UiButton::new();
        button.base.layout.z_index = 100;
        button.base.clip_children = false;
        self.insert_color_picker_internal_node(
            dropdown_id,
            Box::leak(format!("__perro_dropdown_option_{idx}").into_boxed_str()),
            SceneNodeData::UiButton(Box::new(button)),
        )
    }

    fn insert_dropdown_label(&mut self, parent_id: NodeID, name: &'static str) -> NodeID {
        let mut label = perro_ui::UiLabel::new();
        label.base.layout.z_index = 101;
        label.base.input_enabled = false;
        label.base.mouse_filter = perro_ui::UiMouseFilter::Pass;
        label.base.layout.size = UiVector2::percent(100.0, 100.0);
        label.text_size_ratio = 0.55;
        label.base.layout.padding = perro_ui::UiRect::symmetric(6.0, 2.0);
        label.h_align = perro_ui::UiTextAlign::Start;
        self.insert_color_picker_internal_node(
            parent_id,
            name,
            SceneNodeData::UiLabel(Box::new(label)),
        )
    }

    fn sync_dropdown_internal_nodes(&mut self, dropdown_id: NodeID) {
        let Some(snapshot) = self
            .nodes
            .get(dropdown_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiDropdown(dropdown) => Some((
                    dropdown.selected_label().to_string(),
                    dropdown.open,
                    dropdown.button.base.visible,
                    dropdown.option_height,
                    dropdown.option_style.clone(),
                    dropdown.option_hover_style.clone(),
                    dropdown.option_pressed_style.clone(),
                    dropdown
                        .options
                        .iter()
                        .map(|option| option.label.to_string())
                        .collect::<Vec<_>>(),
                    dropdown.internal_label,
                    dropdown.internal_option_buttons.clone(),
                    dropdown.internal_option_labels.clone(),
                )),
                _ => None,
            })
        else {
            return;
        };
        let (
            selected,
            open,
            base_visible,
            option_height,
            option_style,
            option_hover_style,
            option_pressed_style,
            labels,
            label_id,
            option_buttons,
            option_labels,
        ) = snapshot;
        if let Some(node) = self.nodes.get_mut(label_id)
            && let SceneNodeData::UiLabel(label) = &mut node.data
        {
            label.base.visible = base_visible;
            label.set_text(selected);
        }
        for (idx, button_id) in option_buttons.iter().copied().enumerate() {
            if let Some(node) = self.nodes.get_mut(button_id)
                && let SceneNodeData::UiButton(button) = &mut node.data
            {
                button.base.visible = open && base_visible;
                button.base.layout.size = UiVector2::new(
                    perro_ui::UiUnit::Percent(100.0),
                    perro_ui::UiUnit::Pixels(option_height),
                );
                button.base.transform.position =
                    UiVector2::pixels(0.0, option_height * (idx + 1) as f32);
                button.base.layout.anchor = UiAnchor::Top;
                button.base.layout.z_index = 100 + idx as i32;
                button.style = option_style.clone();
                button.hover_style = option_hover_style.clone();
                button.pressed_style = option_pressed_style.clone();
            }
            if let Some(node) = self
                .nodes
                .get_mut(option_labels.get(idx).copied().unwrap_or_default())
                && let SceneNodeData::UiLabel(label) = &mut node.data
            {
                label.base.visible = open && base_visible;
                label.set_text(labels.get(idx).cloned().unwrap_or_default());
            }
        }
        self.mark_ui_dirty(
            dropdown_id,
            Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_COMMANDS,
        );
        let mut dirty_ids = vec![label_id];
        dirty_ids.extend(option_buttons);
        dirty_ids.extend(option_labels);
        for id in dirty_ids {
            if !id.is_nil() {
                self.mark_ui_dirty(
                    id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_COMMANDS,
                );
            }
        }
    }

    fn ensure_color_picker_internal_nodes(&mut self) {
        // Same gating as tree lists: only re-sync pickers whose node is
        // dirty this frame. Engine-side popup/edit paths call
        // sync_color_picker_internal_nodes directly.
        let picker_ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| match &node.data {
                SceneNodeData::UiColorPicker(picker) => {
                    Some((id, picker.internal_swatch_button.is_nil()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for (picker_id, never_synced) in picker_ids {
            if never_synced || self.dirty.ui_flags_at(picker_id.index() as usize) != 0 {
                self.ensure_color_picker_internal_nodes_for(picker_id);
                self.sync_color_picker_internal_nodes(picker_id);
            }
        }
    }

    fn ensure_color_picker_internal_nodes_for(&mut self, picker_id: NodeID) {
        let Some(snapshot) = self.nodes.get(picker_id).and_then(|node| match &node.data {
            SceneNodeData::UiColorPicker(picker) => Some((
                picker.internal_swatch_button,
                picker.internal_popup_panel,
                picker.internal_rgba_boxes,
                picker.internal_hsv_boxes,
                picker.internal_hex_box,
            )),
            _ => None,
        }) else {
            return;
        };

        let mut ids = snapshot;
        if !self.color_picker_internal_valid(ids.0, picker_id, false) {
            ids.0 = self.insert_color_picker_swatch(picker_id);
        }
        if !self.color_picker_internal_valid(ids.1, picker_id, false) {
            ids.1 = self.insert_color_picker_popup(picker_id);
        }
        for idx in 0..ids.2.len() {
            if !self.color_picker_internal_valid(ids.2[idx], ids.1, true) {
                ids.2[idx] = self.insert_color_picker_text_box(
                    ids.1,
                    [
                        "__perro_color_rgba_r",
                        "__perro_color_rgba_g",
                        "__perro_color_rgba_b",
                        "__perro_color_rgba_a",
                    ][idx],
                );
            }
        }
        for idx in 0..ids.3.len() {
            if !self.color_picker_internal_valid(ids.3[idx], ids.1, true) {
                ids.3[idx] = self.insert_color_picker_text_box(
                    ids.1,
                    [
                        "__perro_color_hsv_h",
                        "__perro_color_hsv_s",
                        "__perro_color_hsv_v",
                    ][idx],
                );
            }
        }
        if !self.color_picker_internal_valid(ids.4, ids.1, true) {
            ids.4 = self.insert_color_picker_text_box(ids.1, "__perro_color_hex");
        }

        if let Some(node) = self.nodes.get_mut(picker_id)
            && let SceneNodeData::UiColorPicker(picker) = &mut node.data
        {
            picker.internal_swatch_button = ids.0;
            picker.internal_popup_panel = ids.1;
            picker.internal_rgba_boxes = ids.2;
            picker.internal_hsv_boxes = ids.3;
            picker.internal_hex_box = ids.4;
        }
    }

    fn color_picker_internal_valid(&self, id: NodeID, parent: NodeID, nested: bool) -> bool {
        if id.is_nil() {
            return false;
        }
        self.nodes.get(id).is_some_and(|node| {
            node.parent == parent
                && if nested {
                    matches!(node.data, SceneNodeData::UiTextBox(_))
                } else {
                    matches!(
                        node.data,
                        SceneNodeData::UiButton(_) | SceneNodeData::UiPanel(_)
                    )
                }
        })
    }

    fn insert_color_picker_swatch(&mut self, picker_id: NodeID) -> NodeID {
        let mut button = UiButton::new();
        button.base.layout.size = UiVector2::percent(100.0, 100.0);
        button.base.layout.z_index = 1;
        button.base.clip_children = false;
        button.style.set_corner_radius(0.15);
        button.hover_style.set_corner_radius(0.15);
        button.pressed_style.set_corner_radius(0.15);
        self.insert_color_picker_internal_node(
            picker_id,
            "__perro_color_picker_swatch",
            SceneNodeData::UiButton(Box::new(button)),
        )
    }

    fn insert_color_picker_popup(&mut self, picker_id: NodeID) -> NodeID {
        let mut panel = UiPanel::new();
        panel.base.layout.anchor = UiAnchor::Bottom;
        panel.base.layout.z_index = 100;
        panel.base.clip_children = false;
        self.insert_color_picker_internal_node(
            picker_id,
            "__perro_color_picker_popup",
            SceneNodeData::UiPanel(Box::new(panel)),
        )
    }

    fn insert_color_picker_text_box(&mut self, popup_id: NodeID, name: &'static str) -> NodeID {
        let mut text_box = UiTextBox::new();
        text_box.inner.base.layout.z_index = 102;
        text_box.inner.font_size = 15.0;
        text_box.inner.text_size_ratio = 0.58;
        text_box.inner.padding = perro_ui::UiRect::symmetric(6.0, 3.0);
        self.insert_color_picker_internal_node(
            popup_id,
            name,
            SceneNodeData::UiTextBox(Box::new(text_box)),
        )
    }

    fn insert_color_picker_internal_node(
        &mut self,
        parent_id: NodeID,
        name: &'static str,
        data: SceneNodeData,
    ) -> NodeID {
        let mut node = SceneNode::new(data);
        node.set_name(name);
        node.parent = parent_id;
        let id = self.nodes.insert(node);
        if let Some(inserted) = self.nodes.get_mut(id) {
            inserted.id = id;
        }
        if let Some(parent) = self.nodes.get_mut(parent_id)
            && !parent.children.contains(&id)
        {
            parent.children.push(id);
        }
        if let Some(node) = self.nodes.get(id) {
            self.register_internal_node_schedules(id, node.node_type());
        }
        self.mark_needs_rerender(id);
        self.mark_ui_dirty(
            id,
            Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_LAYOUT_PARENT | Self::UI_DIRTY_COMMANDS,
        );
        self.mark_ui_dirty(
            parent_id,
            Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_LAYOUT_PARENT | Self::UI_DIRTY_COMMANDS,
        );
        id
    }

    fn sync_color_picker_internal_nodes(&mut self, picker_id: NodeID) {
        let Some((color, popup_open, popup_style, popup_size, ids)) =
            self.nodes.get(picker_id).and_then(|node| match &node.data {
                SceneNodeData::UiColorPicker(picker) => Some((
                    picker.color,
                    picker.popup_open,
                    picker.popup_style.clone(),
                    picker.popup_size,
                    (
                        picker.internal_swatch_button,
                        picker.internal_popup_panel,
                        picker.internal_rgba_box,
                        picker.internal_hsv_box,
                        picker.internal_rgba_boxes,
                        picker.internal_hsv_boxes,
                        picker.internal_hex_box,
                    ),
                )),
                _ => None,
            })
        else {
            return;
        };

        if let Some(node) = self.nodes.get_mut(ids.0)
            && let SceneNodeData::UiButton(button) = &mut node.data
        {
            button.base.visible = true;
            button.style.fill = color;
            button.hover_style.fill = color;
            button.pressed_style.fill = Color::new(
                (color.r() * 0.8).clamp(0.0, 1.0),
                (color.g() * 0.8).clamp(0.0, 1.0),
                (color.b() * 0.8).clamp(0.0, 1.0),
                color.a(),
            );
        }
        if let Some(node) = self.nodes.get_mut(ids.1)
            && let SceneNodeData::UiPanel(panel) = &mut node.data
        {
            panel.base.visible = popup_open;
            panel.base.layout.size = UiVector2::pixels(popup_size[0], popup_size[1]);
            panel.base.transform.position = UiVector2::pixels(0.0, -popup_size[1] - 8.0);
            panel.style = popup_style;
        }
        let rgba = color_to_rgba_components(color);
        let hsv = color_to_hsv_components(color);
        let hex = color_to_hex_text(color);
        self.sync_color_picker_legacy_text_box(ids.2, false);
        self.sync_color_picker_legacy_text_box(ids.3, false);
        for (idx, text) in rgba.iter().enumerate() {
            self.sync_color_picker_component_box(
                ids.4[idx],
                popup_open,
                ColorPickerComponentLayout::new(popup_size, 198.0, idx, rgba.len()),
                text,
            );
        }
        for (idx, text) in hsv.iter().enumerate() {
            self.sync_color_picker_component_box(
                ids.5[idx],
                popup_open,
                ColorPickerComponentLayout::new(popup_size, 236.0, idx, hsv.len()),
                text,
            );
        }
        self.sync_color_picker_text_box(ids.6, popup_open, popup_size, 274.0, &hex);

        self.mark_ui_dirty(
            picker_id,
            Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_COMMANDS,
        );
        let mut dirty_ids = vec![ids.0, ids.1, ids.2, ids.3, ids.6];
        dirty_ids.extend(ids.4);
        dirty_ids.extend(ids.5);
        for id in dirty_ids {
            if !id.is_nil() {
                self.mark_ui_dirty(
                    id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_COMMANDS,
                );
            }
        }
    }

    fn sync_color_picker_legacy_text_box(&mut self, node_id: NodeID, visible: bool) {
        if let Some(node) = self.nodes.get_mut(node_id)
            && let SceneNodeData::UiTextBox(text_box) = &mut node.data
        {
            text_box.inner.base.visible = visible;
        }
    }

    fn sync_color_picker_component_box(
        &mut self,
        node_id: NodeID,
        visible: bool,
        layout: ColorPickerComponentLayout,
        text: &str,
    ) {
        if let Some(node) = self.nodes.get_mut(node_id)
            && let SceneNodeData::UiTextBox(text_box) = &mut node.data
        {
            let gap = 6.0;
            let total_gap = gap * layout.cols.saturating_sub(1) as f32;
            let width =
                ((layout.popup_size[0] - 24.0 - total_gap) / layout.cols.max(1) as f32).max(36.0);
            let left = -layout.popup_size[0] * 0.5 + 12.0 + width * 0.5;
            let x = left + layout.col as f32 * (width + gap);
            text_box.inner.base.visible = visible;
            text_box.inner.base.layout.size = UiVector2::pixels(width, 30.0);
            text_box.inner.base.transform.position =
                UiVector2::pixels(x, layout.popup_size[1] * 0.5 - layout.y_from_top);
            if self.render_ui.focused_text_edit != Some(node_id) {
                text_box.inner.set_text(text.to_string());
            }
        }
    }

    fn sync_color_picker_text_box(
        &mut self,
        node_id: NodeID,
        visible: bool,
        popup_size: [f32; 2],
        y_from_top: f32,
        text: &str,
    ) {
        if let Some(node) = self.nodes.get_mut(node_id)
            && let SceneNodeData::UiTextBox(text_box) = &mut node.data
        {
            text_box.inner.base.visible = visible;
            text_box.inner.base.layout.size =
                UiVector2::pixels((popup_size[0] - 24.0).max(48.0), 30.0);
            text_box.inner.base.transform.position =
                UiVector2::pixels(0.0, popup_size[1] * 0.5 - y_from_top);
            if self.render_ui.focused_text_edit != Some(node_id) {
                text_box.inner.set_text(text.to_string());
            }
        }
    }

    fn emit_color_picker_wheel_commands(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        viewport: Vector2,
    ) {
        let pickers = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                let SceneNodeData::UiColorPicker(picker) = &node.data else {
                    return None;
                };
                Some((
                    id,
                    picker.popup_open,
                    picker.wheel_radius,
                    picker.internal_popup_panel,
                ))
            })
            .collect::<Vec<_>>();
        for (picker_id, popup_open, wheel_radius, popup_id) in pickers {
            let wheel_node = color_picker_wheel_render_node(picker_id);
            if !popup_open {
                self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode {
                    node: wheel_node,
                }));
                continue;
            }
            let Some(popup_rect) = computed.get(&popup_id).copied().or_else(|| {
                self.render_ui
                    .retained_rects
                    .get(&popup_id)
                    .copied()
                    .map(|rect| computed_rect_from_state(&rect))
            }) else {
                continue;
            };
            let rect = color_picker_wheel_rect(popup_rect, wheel_radius);
            let rect_state = UiRectState {
                center: [rect.center.x, rect.center.y],
                size: [rect.size.x, rect.size.y],
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
                z_index: self.ui_effective_z(popup_id).saturating_add(1),
            };
            self.queue_render_command(RenderCommand::Ui(UiCommand::UpsertColorWheel {
                node: wheel_node,
                rect: rect_state,
                clip_rect: self.ui_effective_clip_rect_screen(popup_id, computed, viewport),
            }));
        }
    }

    fn color_picker_parent_for_swatch(&self, swatch_id: NodeID) -> Option<NodeID> {
        self.nodes.iter().find_map(|(id, node)| match &node.data {
            SceneNodeData::UiColorPicker(picker) if picker.internal_swatch_button == swatch_id => {
                Some(id)
            }
            _ => None,
        })
    }

    fn process_color_picker_text_edit(
        &mut self,
        text_node: NodeID,
        text: &str,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        let Some((picker_id, field, current)) = self.nodes.iter().find_map(|(id, node)| {
            let SceneNodeData::UiColorPicker(picker) = &node.data else {
                return None;
            };
            if let Some(idx) = picker
                .internal_rgba_boxes
                .iter()
                .position(|box_id| *box_id == text_node)
            {
                Some((id, ColorPickerTextField::Rgba(idx), picker.color))
            } else if let Some(idx) = picker
                .internal_hsv_boxes
                .iter()
                .position(|box_id| *box_id == text_node)
            {
                Some((id, ColorPickerTextField::Hsv(idx), picker.color))
            } else if picker.internal_hex_box == text_node {
                Some((id, ColorPickerTextField::Hex, picker.color))
            } else {
                None
            }
        }) else {
            return;
        };
        let Some(color) = parse_color_picker_text(field, text, current) else {
            return;
        };
        let Some(scene_node) = self.nodes.get_mut(picker_id) else {
            return;
        };
        let SceneNodeData::UiColorPicker(picker) = &mut scene_node.data else {
            return;
        };
        if picker.color == color {
            return;
        }
        let signals = picker.color_changed_signals.clone();
        picker.color = color;
        self.sync_color_picker_internal_nodes(picker_id);
        if command_seen.insert(picker_id) {
            command_ids.push(picker_id);
        }
        let params = [
            Variant::from(picker_id),
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

#[path = "ui/events.rs"]
mod events;
#[path = "ui/layout_core.rs"]
mod layout_core;
#[path = "ui/layout_rects.rs"]
mod layout_rects;
#[path = "ui/layout_size.rs"]
mod layout_size;

#[path = "ui/helpers.rs"]
mod helpers;

use helpers::*;

#[cfg(test)]
#[path = "ui/tests.rs"]
mod tests;

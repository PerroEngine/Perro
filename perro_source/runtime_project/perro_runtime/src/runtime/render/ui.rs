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
    CameraStreamCommand, RenderCommand, ResourceCommand, UiCommand, UiDepthEffectState,
    UiImageScaleState, UiRectState, UiTextAlignState,
};
use perro_runtime_render::{UiDirtyMask, UiExtractionOptions, ui_image_texture_request};
use perro_structs::{Color, UVector2, Vector2};
use perro_ui::{
    ComputedUiRect, UiAnchor, UiBox, UiButton, UiFontSizing, UiHorizontalAlign, UiImageScaleMode,
    UiLayoutData, UiLayoutMode, UiPanel, UiSizeMode, UiStyle, UiTextBox, UiTextEdit, UiTransform,
    UiVector2, UiVerticalAlign,
};
use perro_variant::Variant;
use std::borrow::Cow;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[path = "ui/locale.rs"]
mod locale;

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
        let has_extraction_work = self.dirty.has_any_dirty()
            || self.dirty.has_pending_transform_roots()
            || !self.render_ui.removed_nodes.is_empty()
            || bootstrap_scan
            || input_changed
            || scroll_input_changed
            || text_input_changed;
        if !has_extraction_work {
            if let Some(timing) = timing {
                timing.total = total_start.expect("ui timing total start exists").elapsed();
            }
            return;
        }
        let mut timing = timing;
        self.ensure_color_picker_internal_nodes();

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
        if let Some(timing) = timing.as_deref_mut() {
            timing.layout += layout_start
                .expect("ui layout timing start exists")
                .elapsed();
        }

        self.process_ui_focus_input(&computed, &mut command_ids, &mut command_seen);
        self.process_text_edit_input(&computed, &mut command_ids, &mut command_seen);
        self.process_ui_scroll_input(
            &mut computed,
            &mut computed_scales,
            root_rect,
            &mut command_ids,
            &mut command_seen,
        );
        self.refresh_button_visual_states(&computed, &mut command_ids, &mut command_seen);

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
        for node in self
            .render_ui
            .prev_visible
            .iter()
            .copied()
            .collect::<Vec<_>>()
        {
            if !visible_now.contains(&node)
                && self.render_ui.retained_commands.contains_key(&node)
                && self.ui_image_has_pending_texture(node)
            {
                visible_now.insert(node);
            }
        }
        self.remove_no_longer_visible_ui_nodes(&visible_now);
        if let Some(timing) = timing.as_deref_mut() {
            timing.commands += commands_start
                .expect("ui commands timing start exists")
                .elapsed();
        }

        self.render_ui.computed_rects = computed;
        self.render_ui.computed_scales = computed_scales;
        std::mem::swap(&mut self.render_ui.prev_visible, &mut visible_now);
        visible_now.clear();
        self.render_ui.visible_now = visible_now;

        self.render_ui
            .restore_extraction_plan(traversal_ids, command_ids, command_seen);

        if let Some(timing) = timing {
            timing.total = total_start.expect("ui timing total start exists").elapsed();
        }
    }
}

impl Runtime {
    fn ensure_color_picker_internal_nodes(&mut self) {
        let picker_ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                matches!(node.data, SceneNodeData::UiColorPicker(_)).then_some(id)
            })
            .collect::<Vec<_>>();
        for picker_id in picker_ids {
            self.ensure_color_picker_internal_nodes_for(picker_id);
            self.sync_color_picker_internal_nodes(picker_id);
        }
    }

    fn ensure_color_picker_internal_nodes_for(&mut self, picker_id: NodeID) {
        let Some(snapshot) = self.nodes.get(picker_id).and_then(|node| match &node.data {
            SceneNodeData::UiColorPicker(picker) => Some((
                picker.internal_swatch_button,
                picker.internal_popup_panel,
                picker.internal_rgba_box,
                picker.internal_hsv_box,
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
        if !self.color_picker_internal_valid(ids.2, ids.1, true) {
            ids.2 = self.insert_color_picker_text_box(ids.1, "__perro_color_rgba");
        }
        if !self.color_picker_internal_valid(ids.3, ids.1, true) {
            ids.3 = self.insert_color_picker_text_box(ids.1, "__perro_color_hsv");
        }
        if !self.color_picker_internal_valid(ids.4, ids.1, true) {
            ids.4 = self.insert_color_picker_text_box(ids.1, "__perro_color_hex");
        }

        if let Some(node) = self.nodes.get_mut(picker_id)
            && let SceneNodeData::UiColorPicker(picker) = &mut node.data
        {
            picker.internal_swatch_button = ids.0;
            picker.internal_popup_panel = ids.1;
            picker.internal_rgba_box = ids.2;
            picker.internal_hsv_box = ids.3;
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
        button.style.corner_radius = 0.15;
        button.hover_style.corner_radius = 0.15;
        button.pressed_style.corner_radius = 0.15;
        self.insert_color_picker_internal_node(
            picker_id,
            "__perro_color_picker_swatch",
            SceneNodeData::UiButton(button),
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
            SceneNodeData::UiPanel(panel),
        )
    }

    fn insert_color_picker_text_box(&mut self, popup_id: NodeID, name: &'static str) -> NodeID {
        let mut text_box = UiTextBox::new();
        text_box.inner.base.layout.z_index = 102;
        text_box.inner.font_size = 13.0;
        text_box.inner.text_size_ratio = 0.5;
        text_box.inner.padding = perro_ui::UiRect::symmetric(6.0, 3.0);
        self.insert_color_picker_internal_node(popup_id, name, SceneNodeData::UiTextBox(text_box))
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
        let rgba = color_to_rgba_text(color);
        let hsv = color_to_hsv_text(color);
        let hex = color_to_hex_text(color);
        self.sync_color_picker_text_box(ids.2, popup_open, popup_size, 72.0, &rgba);
        self.sync_color_picker_text_box(ids.3, popup_open, popup_size, 108.0, &hsv);
        self.sync_color_picker_text_box(ids.4, popup_open, popup_size, 144.0, &hex);

        self.mark_ui_dirty(
            picker_id,
            Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_COMMANDS,
        );
        for id in [ids.0, ids.1, ids.2, ids.3, ids.4] {
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
                UiVector2::pixels((popup_size[0] - 24.0).max(48.0), 28.0);
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
        let Some((picker_id, field, alpha)) = self.nodes.iter().find_map(|(id, node)| {
            let SceneNodeData::UiColorPicker(picker) = &node.data else {
                return None;
            };
            if picker.internal_rgba_box == text_node {
                Some((id, ColorPickerTextField::Rgba, picker.color.a()))
            } else if picker.internal_hsv_box == text_node {
                Some((id, ColorPickerTextField::Hsv, picker.color.a()))
            } else if picker.internal_hex_box == text_node {
                Some((id, ColorPickerTextField::Hex, picker.color.a()))
            } else {
                None
            }
        }) else {
            return;
        };
        let Some(color) = parse_color_picker_text(field, text, alpha) else {
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

#[derive(Clone, Copy)]
enum ColorPickerTextField {
    Rgba,
    Hsv,
    Hex,
}

fn color_to_rgba_text(color: Color) -> String {
    format!(
        "{:.3}, {:.3}, {:.3}, {:.3}",
        color.r(),
        color.g(),
        color.b(),
        color.a()
    )
}

fn color_picker_wheel_render_node(picker_id: NodeID) -> NodeID {
    NodeID::from_u64(0xC010_0000_0000_0000 ^ picker_id.as_u64())
}

fn color_picker_wheel_rect(popup_rect: ComputedUiRect, wheel_radius: f32) -> ComputedUiRect {
    let radius = wheel_radius.max(8.0);
    ComputedUiRect::new(
        Vector2::new(popup_rect.center.x, popup_rect.max().y - radius - 14.0),
        Vector2::new(radius * 2.0, radius * 2.0),
    )
}

fn color_to_hsv_text(color: Color) -> String {
    let (h, s, v) = rgb_to_hsv(color);
    format!("{:.1}, {:.3}, {:.3}", h * 360.0, s, v)
}

fn color_to_hex_text(color: Color) -> String {
    let [r, g, b, a] = color.to_rgba();
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        (r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (b.clamp(0.0, 1.0) * 255.0).round() as u8,
        (a.clamp(0.0, 1.0) * 255.0).round() as u8
    )
}

fn rgb_to_hsv(color: Color) -> (f32, f32, f32) {
    let r = color.r();
    let g = color.g();
    let b = color.b();
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let h = if delta <= f32::EPSILON {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if max == g {
        (((b - r) / delta) + 2.0) / 6.0
    } else {
        (((r - g) / delta) + 4.0) / 6.0
    };
    let s = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    (h, s, max)
}

fn parse_color_picker_text(field: ColorPickerTextField, text: &str, alpha: f32) -> Option<Color> {
    match field {
        ColorPickerTextField::Rgba => {
            let vals = parse_float_list(text);
            if vals.len() < 3 {
                return None;
            }
            Some(Color::new(
                vals[0].clamp(0.0, 1.0),
                vals[1].clamp(0.0, 1.0),
                vals[2].clamp(0.0, 1.0),
                vals.get(3).copied().unwrap_or(alpha).clamp(0.0, 1.0),
            ))
        }
        ColorPickerTextField::Hsv => {
            let vals = parse_float_list(text);
            if vals.len() < 3 {
                return None;
            }
            Some(hsv_to_color(
                (vals[0] / 360.0).rem_euclid(1.0),
                vals[1].clamp(0.0, 1.0),
                vals[2].clamp(0.0, 1.0),
                vals.get(3).copied().unwrap_or(alpha).clamp(0.0, 1.0),
            ))
        }
        ColorPickerTextField::Hex => parse_hex_color(text, alpha),
    }
}

fn parse_float_list(text: &str) -> Vec<f32> {
    text.split(|ch: char| ch == ',' || ch.is_whitespace())
        .filter_map(|part| {
            let part = part.trim();
            (!part.is_empty())
                .then(|| part.parse::<f32>().ok())
                .flatten()
        })
        .collect()
}

fn parse_hex_color(text: &str, alpha: f32) -> Option<Color> {
    let hex = text.trim().trim_start_matches('#');
    let expanded;
    let hex = match hex.len() {
        3 => {
            expanded = hex.chars().flat_map(|ch| [ch, ch]).collect::<String>();
            expanded.as_str()
        }
        4 => {
            expanded = hex.chars().flat_map(|ch| [ch, ch]).collect::<String>();
            expanded.as_str()
        }
        6 | 8 => hex,
        _ => return None,
    };
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    let a = if hex.len() >= 8 {
        u8::from_str_radix(&hex[6..8], 16).ok()? as f32 / 255.0
    } else {
        alpha
    };
    Some(Color::new(r, g, b, a))
}

fn hsv_to_color(h: f32, s: f32, v: f32, a: f32) -> Color {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::new(r, g, b, a)
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

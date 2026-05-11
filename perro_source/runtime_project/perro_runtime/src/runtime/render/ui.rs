//! Runtime UI layout, retained command extraction, and text input handling.

use super::state::{DirtyState, UiButtonVisualState};
use super::{Runtime, RuntimeUiTiming};
use ahash::AHashMap;
use perro_ids::{NodeID, SignalID, TextureID};
use perro_input::{KeyCode, MouseButton};
use perro_nodes::SceneNodeData;
use perro_render_bridge::{
    RenderCommand, ResourceCommand, UiCommand, UiDepthEffectState, UiImageScaleState, UiRectState,
    UiTextAlignState,
};
use perro_runtime_context::sub_apis::SignalAPI;
use perro_runtime_render::{UiDirtyMask, UiExtractionOptions, ui_image_texture_request};
use perro_structs::Vector2;
use perro_ui::{
    ComputedUiRect, UiBox, UiFontSizing, UiHorizontalAlign, UiImageScaleMode, UiLayoutData,
    UiLayoutMode, UiSizeMode, UiStyle, UiTextEdit, UiTransform, UiVerticalAlign,
};
use perro_variant::Variant;
use std::borrow::Cow;

#[path = "ui/locale.rs"]
mod locale;

const TEXT_EDIT_REPEAT_DELAY: f32 = 0.35;
const TEXT_EDIT_REPEAT_RATE: f32 = 0.035;

impl Runtime {
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

        Some(texture)
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
        let total_start = timing.as_ref().map(|_| std::time::Instant::now());
        let bootstrap_scan = self.render_ui.prev_visible.is_empty()
            && self.render_ui.retained_commands.is_empty()
            && self.render_ui.computed_rects.is_empty();
        let input_changed = self.ui_pointer_changed();
        let text_input_changed =
            self.render_ui.focused_text_edit.is_some() && self.ui_text_input_changed();
        let has_extraction_work = self.dirty.has_any_dirty()
            || self.dirty.has_pending_transform_roots()
            || !self.render_ui.removed_nodes.is_empty()
            || bootstrap_scan
            || input_changed
            || text_input_changed;
        if !has_extraction_work {
            if let Some(timing) = timing {
                timing.total = total_start.expect("ui timing total start exists").elapsed();
            }
            return;
        }
        let mut timing = timing;

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
                    if let SceneNodeData::UiTreeList(tree) = &node_ref.data {
                        out.extend(ui_tree_all_nodes(tree));
                    }
                }
            },
        );
        let traversal_ids = plan.traversal_ids;
        let mut command_ids = plan.command_ids;
        let mut command_seen = plan.command_seen;
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
        let layout_start = timing.as_ref().map(|_| std::time::Instant::now());
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
        if let Some(timing) = timing.as_deref_mut() {
            timing.layout += layout_start
                .expect("ui layout timing start exists")
                .elapsed();
        }

        self.process_text_edit_input(&computed, &mut command_ids, &mut command_seen);
        self.refresh_button_visual_states(&computed, &mut command_ids, &mut command_seen);

        let commands_start = timing.as_ref().map(|_| std::time::Instant::now());
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
                self.remove_retained_ui_node(node);
                if let Some(timing) = timing.as_deref_mut() {
                    timing.removed_nodes = timing.removed_nodes.saturating_add(1);
                }
                continue;
            }
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

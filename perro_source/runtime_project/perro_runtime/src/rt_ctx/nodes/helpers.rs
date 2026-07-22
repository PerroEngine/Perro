use super::*;

impl Runtime {
    pub(super) fn mark_created_ui_node_dirty(&mut self, id: perro_ids::NodeID) {
        if self
            .nodes
            .get(id)
            .and_then(|node| ui_base_from_data(&node.data))
            .is_none()
        {
            return;
        }

        self.mark_ui_dirty(
            id,
            Self::UI_DIRTY_LAYOUT_SELF
                | Self::UI_DIRTY_LAYOUT_PARENT
                | Self::UI_DIRTY_TRANSFORM
                | Self::UI_DIRTY_COMMANDS,
        );
    }

    pub(super) fn mark_ui_base_change(
        &mut self,
        id: perro_ids::NodeID,
        before: &UiNode,
        after: &UiNode,
    ) {
        let flags = classify_ui_base_change(before, after);
        if flags != 0 {
            self.mark_ui_dirty(id, flags);
        }
        if before.visible != after.visible {
            self.mark_ui_visibility_dirty_subtree(id);
        }
    }

    /// Diff a before/after UI snapshot pair and apply the resulting dirty flags.
    ///
    /// Replaces the old deep-clone `mark_ui_data_change` which diffed two
    /// `SceneNodeData` clones. Behavior is identical: base + payload flags via
    /// the snapshot diff, plus a visibility-subtree walk when `visible` flips.
    pub(super) fn mark_ui_snapshot_change(
        &mut self,
        id: perro_ids::NodeID,
        before: &UiSnapshot,
        after: &UiSnapshot,
    ) {
        let flags = classify_ui_snapshot_change(before, after);
        if flags != 0 {
            self.mark_ui_dirty(id, flags);
        }
        if before.base.visible != after.base.visible {
            self.mark_ui_visibility_dirty_subtree(id);
        }
    }

    pub(super) fn mark_ui_reparent_dirty(
        &mut self,
        child_id: perro_ids::NodeID,
        old_parent: perro_ids::NodeID,
        new_parent: perro_ids::NodeID,
    ) {
        let mut stack = vec![child_id];
        while let Some(id) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let is_ui = ui_base_from_data(&node.data).is_some();
            let children = self.nodes.children(id).map(<[NodeID]>::to_vec);
            if is_ui {
                self.mark_ui_dirty(
                    id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_TRANSFORM
                        | Self::UI_DIRTY_COMMANDS,
                );
            }
            stack.extend(children.unwrap_or_default());
        }

        let mut seen_ui_parents = std::collections::HashSet::new();
        for ui_parent_id in [
            self.closest_ui_ancestor(old_parent),
            self.closest_ui_ancestor(new_parent),
        ]
        .into_iter()
        .flatten()
        {
            if seen_ui_parents.insert(ui_parent_id) {
                self.mark_ui_dirty(
                    ui_parent_id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_COMMANDS,
                );
            }
        }
    }

    pub(super) fn closest_ui_ancestor(
        &self,
        mut node_id: perro_ids::NodeID,
    ) -> Option<perro_ids::NodeID> {
        while !node_id.is_nil() {
            let node = self.nodes.get(node_id)?;
            if ui_base_from_data(&node.data).is_some() {
                return Some(node_id);
            }
            node_id = node.parent;
        }
        None
    }

    pub(super) fn mark_ui_visibility_dirty_subtree(&mut self, root: perro_ids::NodeID) {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let is_ui = ui_base_from_data(&node.data).is_some();
            let children = self.nodes.children(id).map(<[NodeID]>::to_vec);

            if is_ui {
                self.mark_ui_dirty(
                    id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_TRANSFORM
                        | Self::UI_DIRTY_COMMANDS,
                );
            }

            stack.extend(children.unwrap_or_default());
        }
    }
}

// ---------------------------------------------------------------------------
// Snapshot-based UI change detection
// ---------------------------------------------------------------------------
//
// Historically `with_node_mut` cloned the entire `SceneNodeData` before and
// after the caller closure and diffed the clones field-by-field. That deep clone
// (strings, style vecs, option lists) dominated the cost of every UI mutation.
//
// Instead we build a compact `UiSnapshot` in a single pass over `&node.data`:
//   * The `UiNode` base is Copy-content, so it is captured by value and compared
//     exactly via `classify_ui_base_change` (no semantic change).
//   * Widget payload fields are folded into u64 "flag-group" fingerprints, one
//     per group of fields that map to the same dirty-flag set in the old
//     `classify_ui_node_payload_change`. When a group's fingerprint differs we
//     emit exactly the flags that group emitted before.
//
// Tradeoff: fingerprint comparison is hash-based. A 2^-64 hash collision would
// miss a single redraw. NaN semantics also differ slightly: the old exact
// compare marked a rewrite of a NaN field as dirty (NaN != NaN); the fingerprint
// compares bit patterns, so an identical-bits NaN rewrite is treated as
// unchanged. This is a strictly-more-correct deviation and is not covered by any
// characterization test.

use std::hash::Hasher;

#[inline]
fn new_hasher() -> ahash::AHasher {
    // Fixed keys so fingerprints are stable within a process run; we only ever
    // compare two fingerprints produced by the same build, so the exact keys are
    // irrelevant as long as they are deterministic.
    use std::hash::BuildHasher;
    ahash::RandomState::with_seeds(0x5eed_0001, 0x5eed_0002, 0x5eed_0003, 0x5eed_0004)
        .build_hasher()
}

#[inline]
fn feed_f32(h: &mut ahash::AHasher, v: f32) {
    h.write_u32(v.to_bits());
}

#[inline]
fn feed_color(h: &mut ahash::AHasher, c: perro_structs::Color) {
    h.write_u8(c.r.0);
    h.write_u8(c.g.0);
    h.write_u8(c.b.0);
    h.write_u8(c.a.0);
}

#[inline]
fn feed_vector2(h: &mut ahash::AHasher, v: perro_structs::Vector2) {
    feed_f32(h, v.x);
    feed_f32(h, v.y);
}

#[inline]
fn feed_font_sizing(h: &mut ahash::AHasher, f: &perro_ui::UiFontSizing) {
    h.write_u8(f.relative_to_virtual as u8);
    feed_f32(h, f.min_scale);
    feed_f32(h, f.max_scale);
}

#[inline]
fn feed_rect(h: &mut ahash::AHasher, r: perro_ui::UiRect) {
    feed_f32(h, r.left);
    feed_f32(h, r.top);
    feed_f32(h, r.right);
    feed_f32(h, r.bottom);
}

#[inline]
fn feed_depth_effect(h: &mut ahash::AHasher, e: &perro_ui::UiDepthEffect) {
    feed_color(h, e.color);
    feed_f32(h, e.distance);
    feed_f32(h, e.falloff);
    feed_vector2(h, e.vector);
    feed_f32(h, e.size);
}

fn feed_style(h: &mut ahash::AHasher, s: &perro_ui::UiStyle) {
    feed_color(h, s.fill);
    h.write_u8(s.fill_kind as u8);
    feed_color(h, s.gradient.start_color);
    feed_color(h, s.gradient.end_color);
    feed_vector2(h, s.gradient.vector);
    feed_color(h, s.stroke);
    feed_f32(h, s.stroke_width);
    feed_f32(h, s.corner_radii.tl);
    feed_f32(h, s.corner_radii.tr);
    feed_f32(h, s.corner_radii.br);
    feed_f32(h, s.corner_radii.bl);
    feed_depth_effect(h, &s.outer_shadow);
    feed_depth_effect(h, &s.inner_shadow);
    feed_depth_effect(h, &s.outer_highlight);
    feed_depth_effect(h, &s.inner_highlight);
}

/// Feed a Variant structurally so option/item value changes are detected without
/// cloning. Floats are hashed by bit pattern (see NaN note above).
fn feed_variant(h: &mut ahash::AHasher, v: &perro_variant::Variant) {
    use perro_variant::{EngineStruct, Number, Variant};
    match v {
        Variant::Null => h.write_u8(0),
        Variant::Bool(b) => {
            h.write_u8(1);
            h.write_u8(*b as u8);
        }
        Variant::Number(n) => {
            h.write_u8(2);
            match n {
                Number::I8(x) => h.write_i8(*x),
                Number::I16(x) => h.write_i16(*x),
                Number::I32(x) => h.write_i32(*x),
                Number::I64(x) => h.write_i64(*x),
                Number::I128(x) => h.write_i128(x.get()),
                Number::U8(x) => h.write_u8(*x),
                Number::U16(x) => h.write_u16(*x),
                Number::U32(x) => h.write_u32(*x),
                Number::U64(x) => h.write_u64(*x),
                Number::U128(x) => h.write_u128(x.get()),
                Number::F32(x) => h.write_u32(x.to_bits()),
                Number::F64(x) => h.write_u64(x.to_bits()),
            }
        }
        Variant::String(s) => {
            h.write_u8(3);
            h.write(s.as_bytes());
        }
        Variant::Bytes(b) => {
            h.write_u8(4);
            h.write(b);
        }
        Variant::ID(id) => {
            h.write_u8(5);
            h.write_u64(id.as_u64());
        }
        Variant::EngineStruct(s) => {
            h.write_u8(6);
            // Struct math types are Copy plain-data; hash their debug-stable field
            // bytes via a small dispatch. Any change flips the fingerprint.
            match s {
                EngineStruct::Vector2(v) => {
                    feed_f32(h, v.x);
                    feed_f32(h, v.y);
                }
                EngineStruct::Vector3(v) => {
                    feed_f32(h, v.x);
                    feed_f32(h, v.y);
                    feed_f32(h, v.z);
                }
                EngineStruct::Vector4(v) => {
                    feed_f32(h, v.x);
                    feed_f32(h, v.y);
                    feed_f32(h, v.z);
                    feed_f32(h, v.w);
                }
                other => {
                    // Remaining engine structs are compared via Debug bytes; this
                    // path is not exercised by dropdown/treelist option values in
                    // practice but keeps detection conservative.
                    use std::hash::Hash;
                    format!("{other:?}").hash(h);
                }
            }
        }
        Variant::Array(items) => {
            h.write_u8(7);
            h.write_usize(items.len());
            for item in items {
                feed_variant(h, item);
            }
        }
        Variant::Object(map) => {
            h.write_u8(8);
            h.write_usize(map.len());
            for (k, val) in map {
                h.write(k.as_bytes());
                feed_variant(h, val);
            }
        }
    }
}

/// One `u64` fingerprint per flag-group for a UI widget payload. Groups mirror
/// the branches of the old `classify_ui_node_payload_change`.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct UiPayloadFingerprint {
    // Which widget variant produced this fingerprint. Two snapshots with
    // different tags mean the underlying data enum arm changed (which the old
    // code treated as "no payload flags" via its `_ => 0` fallthrough), so we
    // compare group hashes only when tags match.
    tag: u8,
    group_a: u64,
    group_b: u64,
}

impl Default for UiPayloadFingerprint {
    fn default() -> Self {
        Self {
            tag: TAG_NONE,
            group_a: 0,
            group_b: 0,
        }
    }
}

// Widget tags. `TAG_NONE` = non-UI or a UI variant with no payload classification.
const TAG_NONE: u8 = 0;
const TAG_PANEL: u8 = 1;
const TAG_BUTTON: u8 = 2;
const TAG_DROPDOWN: u8 = 3;
const TAG_CHECKBOX: u8 = 4;
const TAG_COLOR_PICKER: u8 = 5;
const TAG_IMAGE_BUTTON: u8 = 6;
const TAG_LABEL: u8 = 7;
const TAG_TEXT_EDIT: u8 = 8;
const TAG_LAYOUT: u8 = 9;
const TAG_HLAYOUT: u8 = 10;
const TAG_VLAYOUT: u8 = 11;
const TAG_GRID: u8 = 12;
const TAG_TREELIST: u8 = 13;

fn text_edit_fingerprint(edit: &perro_ui::UiTextEdit) -> UiPayloadFingerprint {
    // Group A: text/font group -> TEXT|LAYOUT_SELF|LAYOUT_PARENT|COMMANDS.
    let mut a = new_hasher();
    a.write(edit.text.as_bytes());
    feed_f32(&mut a, edit.font_size);
    feed_f32(&mut a, edit.text_size_ratio);
    feed_font_sizing(&mut a, &edit.font_sizing);

    // Group B: appearance/edit-state group -> COMMANDS.
    let mut b = new_hasher();
    feed_style(&mut b, &edit.style);
    feed_style(&mut b, &edit.focused_style);
    b.write(edit.placeholder.as_bytes());
    feed_color(&mut b, edit.color);
    feed_color(&mut b, edit.placeholder_color);
    feed_color(&mut b, edit.selection_color);
    feed_color(&mut b, edit.caret_color);
    b.write_u8(edit.h_align as u8);
    b.write_u8(edit.v_align as u8);
    feed_rect(&mut b, edit.padding);
    feed_f32(&mut b, edit.h_scroll);
    feed_f32(&mut b, edit.v_scroll);
    b.write_usize(edit.caret);
    b.write_usize(edit.anchor);
    b.write_u8(edit.editable as u8);

    UiPayloadFingerprint {
        tag: TAG_TEXT_EDIT,
        group_a: a.finish(),
        group_b: b.finish(),
    }
}

fn feed_button_common(h: &mut ahash::AHasher, button: &perro_ui::UiButton) {
    feed_style(h, &button.style);
    feed_style(h, &button.pressed_style);
    feed_style(h, &button.hover_style);
    h.write_u8(button.disabled as u8);
}

/// Compute the payload fingerprint for a scene node's data in a single match.
pub(super) fn ui_payload_fingerprint(data: &SceneNodeData) -> UiPayloadFingerprint {
    match data {
        SceneNodeData::UiPanel(node) => {
            let mut a = new_hasher();
            feed_style(&mut a, &node.style);
            UiPayloadFingerprint {
                tag: TAG_PANEL,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        SceneNodeData::UiButton(node) => {
            let mut a = new_hasher();
            feed_button_common(&mut a, node);
            UiPayloadFingerprint {
                tag: TAG_BUTTON,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        SceneNodeData::UiDropdown(node) => {
            let mut a = new_hasher();
            feed_button_common(&mut a, &node.button);
            a.write_usize(node.options.len());
            for option in &node.options {
                a.write(option.label.as_bytes());
                feed_variant(&mut a, &option.value);
            }
            a.write_usize(node.selected_index);
            a.write_u8(node.open as u8);
            UiPayloadFingerprint {
                tag: TAG_DROPDOWN,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        SceneNodeData::UiCheckbox(node) => {
            let mut a = new_hasher();
            feed_button_common(&mut a, &node.button);
            feed_style(&mut a, &node.checked_style);
            feed_style(&mut a, &node.checked_hover_style);
            feed_style(&mut a, &node.checked_pressed_style);
            feed_color(&mut a, node.dot_fill);
            a.write_u8(node.checked as u8);
            UiPayloadFingerprint {
                tag: TAG_CHECKBOX,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        SceneNodeData::UiColorPicker(node) => {
            let mut a = new_hasher();
            feed_button_common(&mut a, &node.button);
            feed_color(&mut a, node.color);
            a.write_u8(node.popup_open as u8);
            UiPayloadFingerprint {
                tag: TAG_COLOR_PICKER,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        SceneNodeData::UiImageButton(node) => {
            let mut a = new_hasher();
            a.write_u64(node.texture.as_u64());
            match node.texture_region {
                Some(region) => {
                    a.write_u8(1);
                    for value in region {
                        feed_f32(&mut a, value);
                    }
                }
                None => a.write_u8(0),
            }
            feed_color(&mut a, node.tint);
            feed_color(&mut a, node.hover_tint);
            feed_color(&mut a, node.pressed_tint);
            a.write_u8(node.scale_mode as u8);
            a.write_u8(node.h_align as u8);
            a.write_u8(node.v_align as u8);
            feed_f32(&mut a, node.aspect_ratio);
            a.write_u8(node.disabled as u8);
            UiPayloadFingerprint {
                tag: TAG_IMAGE_BUTTON,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        SceneNodeData::UiNineSliceButton(node) => {
            let mut a = new_hasher();
            a.write_u64(node.texture.as_u64());
            match node.texture_region {
                Some(region) => {
                    a.write_u8(1);
                    for value in region {
                        feed_f32(&mut a, value);
                    }
                }
                None => a.write_u8(0),
            }
            for margin in node.margins {
                feed_f32(&mut a, margin);
            }
            feed_color(&mut a, node.tint);
            feed_color(&mut a, node.hover_tint);
            feed_color(&mut a, node.pressed_tint);
            a.write_u8(node.disabled as u8);
            UiPayloadFingerprint {
                tag: TAG_IMAGE_BUTTON,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        SceneNodeData::UiLabel(node) => {
            // Group A -> TEXT|LAYOUT_SELF|LAYOUT_PARENT|COMMANDS.
            let mut a = new_hasher();
            a.write(node.text.as_bytes());
            feed_f32(&mut a, node.font_size);
            feed_f32(&mut a, node.text_size_ratio);
            feed_font_sizing(&mut a, &node.font_sizing);
            // Group B -> COMMANDS.
            let mut b = new_hasher();
            feed_color(&mut b, node.color);
            b.write_u8(node.h_align as u8);
            b.write_u8(node.v_align as u8);
            UiPayloadFingerprint {
                tag: TAG_LABEL,
                group_a: a.finish(),
                group_b: b.finish(),
            }
        }
        SceneNodeData::UiTextBox(node) => text_edit_fingerprint(&node.inner),
        SceneNodeData::UiTextBlock(node) => text_edit_fingerprint(&node.inner),
        SceneNodeData::UiLayout(node) => {
            let mut a = new_hasher();
            a.write_u8(node.inner.mode as u8);
            feed_f32(&mut a, node.inner.spacing);
            feed_f32(&mut a, node.inner.h_spacing);
            feed_f32(&mut a, node.inner.v_spacing);
            a.write_u32(node.inner.columns);
            UiPayloadFingerprint {
                tag: TAG_LAYOUT,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        SceneNodeData::UiHLayout(node) => {
            let mut a = new_hasher();
            feed_f32(&mut a, node.inner.spacing);
            feed_f32(&mut a, node.inner.h_spacing);
            feed_f32(&mut a, node.inner.v_spacing);
            a.write_u32(node.inner.columns);
            UiPayloadFingerprint {
                tag: TAG_HLAYOUT,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        SceneNodeData::UiVLayout(node) => {
            let mut a = new_hasher();
            feed_f32(&mut a, node.inner.spacing);
            feed_f32(&mut a, node.inner.h_spacing);
            feed_f32(&mut a, node.inner.v_spacing);
            a.write_u32(node.inner.columns);
            UiPayloadFingerprint {
                tag: TAG_VLAYOUT,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        SceneNodeData::UiGrid(node) => {
            let mut a = new_hasher();
            a.write_u32(node.columns);
            feed_f32(&mut a, node.h_spacing);
            feed_f32(&mut a, node.v_spacing);
            UiPayloadFingerprint {
                tag: TAG_GRID,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        SceneNodeData::UiTreeList(node) => {
            let mut a = new_hasher();
            a.write_usize(node.items.len());
            for item in &node.items {
                a.write(item.id.as_bytes());
                a.write(item.label.as_bytes());
                feed_variant(&mut a, &item.value);
                a.write_u64(item.icon.as_u64());
                match item.parent {
                    Some(parent) => {
                        a.write_u8(1);
                        a.write_usize(parent);
                    }
                    None => a.write_u8(0),
                }
                a.write_u8(item.open as u8);
                a.write_u8(item.selectable as u8);
                a.write_u8(item.has_children_hint as u8);
            }
            match node.selected_index {
                Some(index) => {
                    a.write_u8(1);
                    a.write_usize(index);
                }
                None => a.write_u8(0),
            }
            feed_f32(&mut a, node.indent);
            feed_f32(&mut a, node.row_height);
            feed_f32(&mut a, node.v_spacing);
            UiPayloadFingerprint {
                tag: TAG_TREELIST,
                group_a: a.finish(),
                group_b: 0,
            }
        }
        _ => UiPayloadFingerprint::default(),
    }
}

/// Map a before/after payload fingerprint difference to dirty flags, reproducing
/// the old `classify_ui_node_payload_change` flag mapping.
pub(super) fn classify_ui_payload_fingerprint(
    before: &UiPayloadFingerprint,
    after: &UiPayloadFingerprint,
) -> u16 {
    // Mirrors the old code's `_ => 0` fallthrough: if the widget arm changed
    // (e.g. via a variant-swapping patch), no payload flags are emitted here.
    if before.tag != after.tag {
        return 0;
    }
    let group_a_changed = before.group_a != after.group_a;
    let group_b_changed = before.group_b != after.group_b;
    match before.tag {
        TAG_LABEL | TAG_TEXT_EDIT => {
            let mut flags = 0;
            if group_a_changed {
                flags |= Runtime::UI_DIRTY_TEXT
                    | Runtime::UI_DIRTY_LAYOUT_SELF
                    | Runtime::UI_DIRTY_LAYOUT_PARENT
                    | Runtime::UI_DIRTY_COMMANDS;
            }
            if group_b_changed {
                flags |= Runtime::UI_DIRTY_COMMANDS;
            }
            flags
        }
        TAG_PANEL | TAG_BUTTON | TAG_DROPDOWN | TAG_CHECKBOX | TAG_COLOR_PICKER
        | TAG_IMAGE_BUTTON
            if group_a_changed =>
        {
            Runtime::UI_DIRTY_COMMANDS
        }
        TAG_LAYOUT | TAG_HLAYOUT | TAG_VLAYOUT | TAG_GRID if group_a_changed => {
            Runtime::UI_DIRTY_LAYOUT_SELF | Runtime::UI_DIRTY_COMMANDS
        }
        TAG_TREELIST if group_a_changed => {
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS
        }
        _ => 0,
    }
}

/// Single-pass capture of a node's local visibility and base modulate.
///
/// A node exposes at most one of the `Node2D` / `Node3D` / `UiNode` bases, so a
/// single `Option<NodeModulate>` captures whichever modulate applies. This folds
/// the old separate `with_base_ref` modulate probes (2D/3D/UI) into one place:
/// visibility comes from `node_local_visible` and modulate from the same data.
pub(super) fn local_snapshot(data: &SceneNodeData) -> (bool, Option<perro_structs::NodeModulate>) {
    let visible = Runtime::node_local_visible(data);
    let modulate = Node2D::with_base_ref(data, |base| base.modulate)
        .or_else(|| Node3D::with_base_ref(data, |base| base.modulate))
        .or_else(|| UiNode::with_base_ref(data, |base| base.modulate));
    (visible, modulate)
}

/// Single-pass capture of the spatial-base fields `with_base_node_mut` diffs.
///
/// Folds the old per-field `with_base_ref` probes (2D/3D transform, 2D/3D
/// visible, 2D/3D/UI modulate) into one struct. `modulate` mirrors the old OR
/// across all three base kinds; a node exposes at most one, so a single value
/// suffices.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct BaseSpatialSnapshot {
    pub transform_2d: Option<perro_structs::Transform2D>,
    pub transform_3d: Option<perro_structs::Transform3D>,
    pub visible_2d: Option<bool>,
    pub visible_3d: Option<bool>,
    pub modulate: Option<perro_structs::NodeModulate>,
}

pub(super) fn base_spatial_snapshot(data: &SceneNodeData) -> BaseSpatialSnapshot {
    BaseSpatialSnapshot {
        transform_2d: Node2D::with_base_ref(data, |base| base.transform),
        transform_3d: Node3D::with_base_ref(data, |base| base.transform),
        visible_2d: Node2D::with_base_ref(data, |base| base.visible),
        visible_3d: Node3D::with_base_ref(data, |base| base.visible),
        modulate: Node2D::with_base_ref(data, |base| base.modulate)
            .or_else(|| Node3D::with_base_ref(data, |base| base.modulate))
            .or_else(|| UiNode::with_base_ref(data, |base| base.modulate)),
    }
}

/// Compact UI snapshot: the Copy-content base plus payload fingerprints.
#[derive(Clone)]
pub(super) struct UiSnapshot {
    pub base: UiNode,
    pub payload: UiPayloadFingerprint,
}

/// Build a UI snapshot in a single pass. Returns `None` for non-UI data.
pub(super) fn ui_snapshot(data: &SceneNodeData) -> Option<UiSnapshot> {
    let base = ui_base_from_data(data)?;
    Some(UiSnapshot {
        base: base.clone(),
        payload: ui_payload_fingerprint(data),
    })
}

/// Diff two UI snapshots into the dirty-flag set (base + payload). Mirrors the
/// combined effect of `classify_ui_base_change` + `classify_ui_node_payload_change`.
pub(super) fn classify_ui_snapshot_change(before: &UiSnapshot, after: &UiSnapshot) -> u16 {
    let mut flags = classify_ui_base_change(&before.base, &after.base);
    flags |= classify_ui_payload_fingerprint(&before.payload, &after.payload);
    flags
}

pub(super) fn classify_ui_base_change(before: &UiNode, after: &UiNode) -> u16 {
    let mut flags = 0;
    if before.transform != after.transform {
        flags |= Runtime::UI_DIRTY_TRANSFORM | Runtime::UI_DIRTY_COMMANDS;
    }
    if before.visible != after.visible {
        flags |= Runtime::UI_DIRTY_LAYOUT_SELF
            | Runtime::UI_DIRTY_LAYOUT_PARENT
            | Runtime::UI_DIRTY_COMMANDS;
    }
    if before.modulate != after.modulate {
        flags |= Runtime::UI_DIRTY_COMMANDS;
    }
    if before.layout.size != after.layout.size
        || before.layout.min_size != after.layout.min_size
        || before.layout.max_size != after.layout.max_size
        || before.layout.min_size_scale != after.layout.min_size_scale
        || before.layout.max_size_scale != after.layout.max_size_scale
        || before.layout.margin != after.layout.margin
        || before.layout.h_size != after.layout.h_size
        || before.layout.v_size != after.layout.v_size
    {
        flags |= Runtime::UI_DIRTY_LAYOUT_SELF
            | Runtime::UI_DIRTY_LAYOUT_PARENT
            | Runtime::UI_DIRTY_COMMANDS;
    }
    if before.layout.padding != after.layout.padding
        || before.layout.h_align != after.layout.h_align
        || before.layout.v_align != after.layout.v_align
        || before.layout.anchor != after.layout.anchor
    {
        flags |= Runtime::UI_DIRTY_LAYOUT_SELF | Runtime::UI_DIRTY_COMMANDS;
    }
    if before.layout.z_index != after.layout.z_index {
        flags |= Runtime::UI_DIRTY_COMMANDS;
    }
    if before.input_enabled != after.input_enabled || before.mouse_filter != after.mouse_filter {
        flags |= Runtime::UI_DIRTY_COMMANDS;
    }
    flags
}

pub(super) fn ui_base_from_data(data: &SceneNodeData) -> Option<&UiNode> {
    match data {
        SceneNodeData::UiNode(root) => Some(root),
        SceneNodeData::UiSubView(node) => Some(&node.base),
        SceneNodeData::UiPanel(node) => Some(&node.base),
        SceneNodeData::UiButton(node) => Some(&node.base),
        SceneNodeData::UiDropdown(node) => Some(&node.button.base),
        SceneNodeData::UiCheckbox(node) => Some(&node.button.base),
        SceneNodeData::UiColorPicker(node) => Some(&node.button.base),
        SceneNodeData::UiImage(node) => Some(&node.base),
        SceneNodeData::UiVideoPlayer(node) => Some(&node.base),
        SceneNodeData::UiImageButton(node) => Some(&node.base),
        SceneNodeData::UiNineSliceButton(node) => Some(&node.base),
        SceneNodeData::UiNineSlice(node) => Some(&node.base),
        SceneNodeData::UiAnimatedImage(node) => Some(&node.base),
        SceneNodeData::UiLabel(node) => Some(&node.base),
        SceneNodeData::UiTextBox(node) => Some(&node.inner.base),
        SceneNodeData::UiTextBlock(node) => Some(&node.inner.base),
        SceneNodeData::UiLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiHLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiVLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiGrid(node) => Some(&node.base),
        SceneNodeData::UiTreeList(node) => Some(&node.base),
        _ => None,
    }
}

#[cfg(test)]
mod fingerprint_tests {
    use super::*;
    use perro_nodes::SceneNodeData;
    use perro_structs::Color;
    use perro_ui::{
        UiButton, UiDropdown, UiDropdownOption, UiGrid, UiHLayout, UiLabel, UiTextBox, UiTreeList,
        UiTreeListItem, UiVector2,
    };
    use std::borrow::Cow;

    const F_TEXT: u16 = Runtime::UI_DIRTY_TEXT;
    const F_LAYOUT_SELF: u16 = Runtime::UI_DIRTY_LAYOUT_SELF;
    const F_LAYOUT_PARENT: u16 = Runtime::UI_DIRTY_LAYOUT_PARENT;
    const F_COMMANDS: u16 = Runtime::UI_DIRTY_COMMANDS;
    const F_TRANSFORM: u16 = Runtime::UI_DIRTY_TRANSFORM;

    fn payload_flags(before: &SceneNodeData, after: &SceneNodeData) -> u16 {
        classify_ui_payload_fingerprint(
            &ui_payload_fingerprint(before),
            &ui_payload_fingerprint(after),
        )
    }

    fn snapshot_flags(before: &SceneNodeData, after: &SceneNodeData) -> u16 {
        classify_ui_snapshot_change(&ui_snapshot(before).unwrap(), &ui_snapshot(after).unwrap())
    }

    #[test]
    fn label_unchanged_is_zero() {
        let data = SceneNodeData::UiLabel(Box::new(UiLabel::new()));
        assert_eq!(payload_flags(&data, &data), 0);
        assert_eq!(snapshot_flags(&data, &data), 0);
    }

    #[test]
    fn label_text_change_text_layout_commands() {
        let before = SceneNodeData::UiLabel(Box::new(UiLabel::new()));
        let mut label = UiLabel::new();
        label.text = Cow::Borrowed("hi");
        let after = SceneNodeData::UiLabel(Box::new(label));
        assert_eq!(
            payload_flags(&before, &after),
            F_TEXT | F_LAYOUT_SELF | F_LAYOUT_PARENT | F_COMMANDS
        );
    }

    #[test]
    fn label_color_change_commands_only() {
        let before = SceneNodeData::UiLabel(Box::new(UiLabel::new()));
        let mut label = UiLabel::new();
        label.color = Color::RED;
        let after = SceneNodeData::UiLabel(Box::new(label));
        assert_eq!(payload_flags(&before, &after), F_COMMANDS);
    }

    #[test]
    fn button_disabled_change_commands() {
        let before = SceneNodeData::UiButton(Box::new(UiButton::new()));
        let mut button = UiButton::new();
        button.disabled = true;
        let after = SceneNodeData::UiButton(Box::new(button));
        assert_eq!(payload_flags(&before, &after), F_COMMANDS);
    }

    #[test]
    fn dropdown_option_and_variant_change_commands() {
        let before = SceneNodeData::UiDropdown(Box::new(UiDropdown::new()));
        let mut dd = UiDropdown::new();
        dd.options.push(UiDropdownOption {
            label: Cow::Borrowed("A"),
            value: perro_variant::Variant::from(1i64),
        });
        let after = SceneNodeData::UiDropdown(Box::new(dd));
        assert_eq!(payload_flags(&before, &after), F_COMMANDS);

        // Same option label, different Variant value -> still detected.
        let mut a = UiDropdown::new();
        a.options.push(UiDropdownOption {
            label: Cow::Borrowed("A"),
            value: perro_variant::Variant::from(1i64),
        });
        let mut b = UiDropdown::new();
        b.options.push(UiDropdownOption {
            label: Cow::Borrowed("A"),
            value: perro_variant::Variant::from(2i64),
        });
        assert_eq!(
            payload_flags(
                &SceneNodeData::UiDropdown(Box::new(a)),
                &SceneNodeData::UiDropdown(Box::new(b))
            ),
            F_COMMANDS
        );
    }

    #[test]
    fn textbox_text_vs_caret_groups() {
        let before = SceneNodeData::UiTextBox(Box::new(UiTextBox::new()));
        let mut text = UiTextBox::new();
        text.inner.text = Cow::Borrowed("x");
        assert_eq!(
            payload_flags(&before, &SceneNodeData::UiTextBox(Box::new(text))),
            F_TEXT | F_LAYOUT_SELF | F_LAYOUT_PARENT | F_COMMANDS
        );

        let mut caret = UiTextBox::new();
        caret.inner.caret = 0; // default already 0; bump to force change
        caret.inner.anchor = 1;
        assert_eq!(
            payload_flags(&before, &SceneNodeData::UiTextBox(Box::new(caret))),
            F_COMMANDS
        );
    }

    #[test]
    fn grid_columns_layout_self_commands() {
        let before = SceneNodeData::from(UiGrid::new());
        let mut grid = UiGrid::new();
        grid.columns += 3;
        assert_eq!(
            payload_flags(&before, &SceneNodeData::from(grid)),
            F_LAYOUT_SELF | F_COMMANDS
        );
    }

    #[test]
    fn hlayout_spacing_layout_self_commands() {
        let before = SceneNodeData::from(UiHLayout::new());
        let mut hl = UiHLayout::new();
        hl.inner.spacing += 5.0;
        assert_eq!(
            payload_flags(&before, &SceneNodeData::from(hl)),
            F_LAYOUT_SELF | F_COMMANDS
        );
    }

    #[test]
    fn treelist_items_layout_flags() {
        let before = SceneNodeData::UiTreeList(Box::new(UiTreeList::new()));
        let mut tree = UiTreeList::new();
        tree.items.push(UiTreeListItem::new("n"));
        assert_eq!(
            payload_flags(&before, &SceneNodeData::UiTreeList(Box::new(tree))),
            F_LAYOUT_SELF | F_LAYOUT_PARENT | F_COMMANDS
        );
    }

    #[test]
    fn base_transform_via_snapshot() {
        let before = SceneNodeData::UiLabel(Box::new(UiLabel::new()));
        let mut label = UiLabel::new();
        label.base.transform.position = UiVector2::pixels(9.0, 9.0);
        let after = SceneNodeData::UiLabel(Box::new(label));
        // Base transform change -> TRANSFORM|COMMANDS (payload unchanged).
        assert_eq!(snapshot_flags(&before, &after), F_TRANSFORM | F_COMMANDS);
    }

    #[test]
    fn base_layout_size_via_snapshot() {
        let before = SceneNodeData::UiLabel(Box::new(UiLabel::new()));
        let mut label = UiLabel::new();
        label.base.layout.size = UiVector2::pixels(100.0, 40.0);
        let after = SceneNodeData::UiLabel(Box::new(label));
        assert_eq!(
            snapshot_flags(&before, &after),
            F_LAYOUT_SELF | F_LAYOUT_PARENT | F_COMMANDS
        );
    }
}

//! Characterization tests locking in the current dirty/render side effects of
//! `Runtime::with_node_mut` and `Runtime::with_base_node_mut`.
//!
//! These are intentionally tight against the *current* implementation so that a
//! later refactor (snapshots instead of deep clones) can be validated against
//! observable behavior rather than internals. They assert the exact dirty-flag
//! combinations and full-scan requests each mutation produces today.

use crate::Runtime;
use crate::runtime::state::DirtyState;
use perro_ids::NodeID;
use perro_nodes::{
    Camera2D, Camera3D, Node2D, Node3D, Spatial, Sprite2D, UiButton, UiCheckbox, UiDropdown,
    UiGrid, UiHLayout, UiLabel, UiNode, UiPanel, UiTextBox, UiTreeList,
};
use perro_runtime_api::sub_apis::NodeAPI;
use perro_structs::{Color, Transform2D, Transform3D, Vector2, Vector3};
use perro_ui::UiTreeListItem;
use std::borrow::Cow;

// -- dirty-flag bit aliases (mirror DirtyState consts) ----------------------

const F_RERENDER: u16 = DirtyState::FLAG_RERENDER;
const F_TRANSFORM_UI: u16 = DirtyState::DIRTY_TRANSFORM;
const F_LAYOUT_SELF: u16 = DirtyState::DIRTY_LAYOUT_SELF;
const F_LAYOUT_PARENT: u16 = DirtyState::DIRTY_LAYOUT_PARENT;
const F_COMMANDS: u16 = DirtyState::DIRTY_COMMANDS;
const F_TEXT: u16 = DirtyState::DIRTY_TEXT;

const UI_FULL: u16 = F_LAYOUT_SELF | F_LAYOUT_PARENT | F_TRANSFORM_UI | F_COMMANDS;

// -- helpers ----------------------------------------------------------------

fn raw_flags(runtime: &Runtime, id: NodeID) -> u16 {
    runtime.dirty.flags_at(id.index() as usize)
}

fn ui_flags(runtime: &Runtime, id: NodeID) -> u16 {
    runtime.dirty.ui_flags_at(id.index() as usize)
}

fn transform_flags(runtime: &Runtime, id: NodeID) -> u16 {
    runtime.dirty.transform_flags_at(id.index() as usize)
}

fn rerender_set(runtime: &Runtime, id: NodeID) -> bool {
    raw_flags(runtime, id) & F_RERENDER != 0
}

/// Drain any full-scan pending flags left over from node creation/setup so a
/// test can assert a clean false->true transition on the act.
fn reset_all(runtime: &mut Runtime) {
    runtime.clear_dirty_flags();
    runtime.render_2d.clear_full_scan_pending();
    runtime.render_3d.clear_full_scan_pending();
}

// A distinct non-identity 2D transform for change detection.
fn moved_2d() -> Transform2D {
    Transform2D::new(Vector2::new(5.0, 7.0), 0.25, Vector2::new(2.0, 3.0))
}

fn moved_3d() -> Transform3D {
    let mut t = Transform3D::IDENTITY;
    t.position = Vector3::new(1.0, 2.0, 3.0);
    t
}

// ===========================================================================
// with_node_mut
// ===========================================================================

// 1. Sprite2D transform change -> 2D transform dirty + rerender.
//    Leaf marks the node directly; parent (with children) marks a pending root.
#[test]
fn with_node_mut_sprite2d_transform_leaf_marks_2d_transform_and_rerender() {
    let mut runtime = Runtime::new();
    let sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    reset_all(&mut runtime);

    let out = <Runtime as NodeAPI>::with_node_mut::<Sprite2D, _, _>(&mut runtime, sprite, |s| {
        s.base.transform = moved_2d();
    });
    assert!(out.is_some());

    // Leaf node: transform dirty recorded directly on the node.
    assert!(runtime.dirty.has_transform_dirty(sprite, Spatial::TwoD));
    assert_eq!(
        transform_flags(&runtime, sprite),
        DirtyState::FLAG_DIRTY_2D_TRANSFORM
    );
    assert!(rerender_set(&runtime, sprite));
    // No pending roots because the node has no children.
    assert!(!runtime.dirty.has_pending_transform_roots());
}

#[test]
fn with_node_mut_sprite2d_transform_parent_marks_pending_root() {
    let mut runtime = Runtime::new();
    let parent = NodeAPI::create::<Sprite2D>(&mut runtime);
    let child = NodeAPI::create::<Sprite2D>(&mut runtime);
    NodeAPI::reparent(&mut runtime, parent, child);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<Sprite2D, _, _>(&mut runtime, parent, |s| {
        s.base.transform = moved_2d();
    });

    // Parent has children -> pending transform root, not a direct node flag.
    assert!(runtime.dirty.has_pending_transform_roots());
    assert_eq!(transform_flags(&runtime, parent), 0);
    assert!(rerender_set(&runtime, parent));
}

// 2. Sprite2D no-op closure -> no transform dirty, but rerender still set.
#[test]
fn with_node_mut_sprite2d_noop_sets_rerender_only() {
    let mut runtime = Runtime::new();
    let sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<Sprite2D, _, _>(&mut runtime, sprite, |_s| {});

    assert!(!runtime.dirty.has_transform_dirty(sprite, Spatial::TwoD));
    assert!(!runtime.dirty.has_pending_transform_roots());
    assert_eq!(transform_flags(&runtime, sprite), 0);
    // Sprite2D is Renderable::True -> rerender fires unconditionally.
    assert!(rerender_set(&runtime, sprite));
}

// 3a. Node3D transform change -> 3D transform dirty (leaf).
#[test]
fn with_node_mut_node3d_transform_marks_3d_transform() {
    let mut runtime = Runtime::new();
    let node = NodeAPI::create::<Node3D>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<Node3D, _, _>(&mut runtime, node, |n| {
        n.transform = moved_3d();
    });

    assert!(runtime.dirty.has_transform_dirty(node, Spatial::ThreeD));
    assert_eq!(
        transform_flags(&runtime, node),
        DirtyState::FLAG_DIRTY_3D_TRANSFORM
    );
    // Node3D is Renderable::False -> no rerender flag.
    assert!(!rerender_set(&runtime, node));
}

// 3b. Node3D no-op -> no rerender (Renderable::False), no flags at all.
#[test]
fn with_node_mut_node3d_noop_sets_nothing() {
    let mut runtime = Runtime::new();
    let node = NodeAPI::create::<Node3D>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<Node3D, _, _>(&mut runtime, node, |_n| {});

    assert_eq!(raw_flags(&runtime, node), 0);
    assert!(!runtime.dirty.has_pending_transform_roots());
}

// 3c. Node2D no-op -> no rerender (Renderable::False).
#[test]
fn with_node_mut_node2d_noop_no_rerender() {
    let mut runtime = Runtime::new();
    let node = NodeAPI::create::<Node2D>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<Node2D, _, _>(&mut runtime, node, |_n| {});

    assert_eq!(raw_flags(&runtime, node), 0);
}

// 4. Type mismatch: with_node_mut::<Sprite2D> on a Node3D id -> None + no flags.
#[test]
fn with_node_mut_type_mismatch_returns_none_no_flags() {
    let mut runtime = Runtime::new();
    let node = NodeAPI::create::<Node3D>(&mut runtime);
    reset_all(&mut runtime);

    let out = <Runtime as NodeAPI>::with_node_mut::<Sprite2D, _, _>(&mut runtime, node, |_s| 1);
    assert!(out.is_none());
    // with_typed_mut returns None before any dirty marking -> node stays clean.
    assert_eq!(raw_flags(&runtime, node), 0);
}

// 5. Stale/removed id -> None, no flags.
#[test]
fn with_node_mut_removed_id_returns_none() {
    let mut runtime = Runtime::new();
    let node = NodeAPI::create::<Sprite2D>(&mut runtime);
    NodeAPI::remove_node(&mut runtime, node);
    reset_all(&mut runtime);

    let out = <Runtime as NodeAPI>::with_node_mut::<Sprite2D, _, _>(&mut runtime, node, |_s| 1);
    assert!(out.is_none());
}

// 6. Return value passthrough.
#[test]
fn with_node_mut_returns_closure_value() {
    let mut runtime = Runtime::new();
    let node = NodeAPI::create::<Sprite2D>(&mut runtime);
    reset_all(&mut runtime);

    let out = <Runtime as NodeAPI>::with_node_mut::<Sprite2D, _, _>(&mut runtime, node, |_s| 42);
    assert_eq!(out, Some(42));
}

// 7. UiLabel text / color / no-op payload flag combos.
#[test]
fn with_node_mut_uilabel_text_change_flags() {
    let mut runtime = Runtime::new();
    let label = NodeAPI::create::<UiLabel>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiLabel, _, _>(&mut runtime, label, |l| {
        l.text = Cow::Borrowed("hello");
    });

    assert_eq!(
        ui_flags(&runtime, label),
        F_TEXT | F_LAYOUT_SELF | F_LAYOUT_PARENT | F_COMMANDS
    );
    assert!(rerender_set(&runtime, label));
}

#[test]
fn with_node_mut_uilabel_color_change_commands_only() {
    let mut runtime = Runtime::new();
    let label = NodeAPI::create::<UiLabel>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiLabel, _, _>(&mut runtime, label, |l| {
        l.color = Color::RED;
    });

    assert_eq!(ui_flags(&runtime, label), F_COMMANDS);
}

#[test]
fn with_node_mut_uilabel_noop_no_ui_flags() {
    let mut runtime = Runtime::new();
    let label = NodeAPI::create::<UiLabel>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiLabel, _, _>(&mut runtime, label, |_l| {});

    assert_eq!(ui_flags(&runtime, label), 0);
    // Note: UiLabel is Renderable::True -> rerender flag still fires on no-op.
    assert!(rerender_set(&runtime, label));
}

// 8. UiButton disabled toggle -> COMMANDS.
#[test]
fn with_node_mut_uibutton_disabled_toggle_commands() {
    let mut runtime = Runtime::new();
    let button = NodeAPI::create::<UiButton>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiButton, _, _>(&mut runtime, button, |b| {
        b.disabled = !b.disabled;
    });

    assert_eq!(ui_flags(&runtime, button), F_COMMANDS);
}

// 9. UiDropdown push option / selected_index / open toggle -> COMMANDS.
#[test]
fn with_node_mut_uidropdown_option_push_commands() {
    let mut runtime = Runtime::new();
    let dd = NodeAPI::create::<UiDropdown>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiDropdown, _, _>(&mut runtime, dd, |d| {
        d.options.push(perro_ui::UiDropdownOption {
            label: Cow::Borrowed("A"),
            value: perro_variant::Variant::from(1i64),
        });
    });

    assert_eq!(ui_flags(&runtime, dd), F_COMMANDS);
}

#[test]
fn with_node_mut_uidropdown_selected_index_commands() {
    let mut runtime = Runtime::new();
    let dd = NodeAPI::create::<UiDropdown>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiDropdown, _, _>(&mut runtime, dd, |d| {
        d.selected_index = 3;
    });

    assert_eq!(ui_flags(&runtime, dd), F_COMMANDS);
}

#[test]
fn with_node_mut_uidropdown_open_toggle_commands() {
    let mut runtime = Runtime::new();
    let dd = NodeAPI::create::<UiDropdown>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiDropdown, _, _>(&mut runtime, dd, |d| {
        d.open = !d.open;
    });

    assert_eq!(ui_flags(&runtime, dd), F_COMMANDS);
}

// 10. UiCheckbox checked toggle -> COMMANDS.
#[test]
fn with_node_mut_uicheckbox_checked_toggle_commands() {
    let mut runtime = Runtime::new();
    let cb = NodeAPI::create::<UiCheckbox>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiCheckbox, _, _>(&mut runtime, cb, |c| {
        c.checked = !c.checked;
    });

    assert_eq!(ui_flags(&runtime, cb), F_COMMANDS);
}

// 11. UiTextBox text change / caret change.
#[test]
fn with_node_mut_uitextbox_text_change_flags() {
    let mut runtime = Runtime::new();
    let tb = NodeAPI::create::<UiTextBox>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiTextBox, _, _>(&mut runtime, tb, |t| {
        t.inner.text = Cow::Borrowed("typed");
    });

    assert_eq!(
        ui_flags(&runtime, tb),
        F_TEXT | F_LAYOUT_SELF | F_LAYOUT_PARENT | F_COMMANDS
    );
}

#[test]
fn with_node_mut_uitextbox_caret_change_commands_only() {
    let mut runtime = Runtime::new();
    let tb = NodeAPI::create::<UiTextBox>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiTextBox, _, _>(&mut runtime, tb, |t| {
        t.inner.caret = t.inner.caret.wrapping_add(1);
    });

    assert_eq!(ui_flags(&runtime, tb), F_COMMANDS);
}

// 12. Layout widgets: HLayout spacing / Grid columns / TreeList items.
#[test]
fn with_node_mut_uihlayout_spacing_layout_self_commands() {
    let mut runtime = Runtime::new();
    let hl = NodeAPI::create::<UiHLayout>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiHLayout, _, _>(&mut runtime, hl, |l| {
        l.inner.spacing += 4.0;
    });

    assert_eq!(ui_flags(&runtime, hl), F_LAYOUT_SELF | F_COMMANDS);
}

#[test]
fn with_node_mut_uigrid_columns_layout_self_commands() {
    let mut runtime = Runtime::new();
    let grid = NodeAPI::create::<UiGrid>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiGrid, _, _>(&mut runtime, grid, |g| {
        g.columns += 2;
    });

    assert_eq!(ui_flags(&runtime, grid), F_LAYOUT_SELF | F_COMMANDS);
}

#[test]
fn with_node_mut_uitreelist_items_layout_flags() {
    let mut runtime = Runtime::new();
    let tree = NodeAPI::create::<UiTreeList>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiTreeList, _, _>(&mut runtime, tree, |t| {
        t.items.push(UiTreeListItem {
            id: Cow::Borrowed("n"),
            label: Cow::Borrowed("n"),
            value: perro_variant::Variant::null(),
            icon: perro_ids::TextureID::nil(),
            parent: None,
            open: false,
            selectable: true,
            has_children_hint: false,
        });
    });

    assert_eq!(
        ui_flags(&runtime, tree),
        F_LAYOUT_SELF | F_LAYOUT_PARENT | F_COMMANDS
    );
}

// 13. UI base-through-payload via with_node_mut::<UiLabel>.
#[test]
fn with_node_mut_uilabel_base_layout_size_layout_flags() {
    let mut runtime = Runtime::new();
    let label = NodeAPI::create::<UiLabel>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiLabel, _, _>(&mut runtime, label, |l| {
        l.base.layout.size = perro_ui::UiVector2::pixels(100.0, 40.0);
    });

    assert_eq!(
        ui_flags(&runtime, label),
        F_LAYOUT_SELF | F_LAYOUT_PARENT | F_COMMANDS
    );
}

#[test]
fn with_node_mut_uilabel_base_transform_transform_commands() {
    let mut runtime = Runtime::new();
    let label = NodeAPI::create::<UiLabel>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiLabel, _, _>(&mut runtime, label, |l| {
        l.base.transform.position = perro_ui::UiVector2::pixels(9.0, 9.0);
    });

    assert_eq!(ui_flags(&runtime, label), F_TRANSFORM_UI | F_COMMANDS);
}

#[test]
fn with_node_mut_uilabel_base_modulate_force_rerenders_child() {
    let mut runtime = Runtime::new();
    let label = NodeAPI::create::<UiLabel>(&mut runtime);
    let child = NodeAPI::create::<UiPanel>(&mut runtime);
    NodeAPI::reparent(&mut runtime, label, child);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiLabel, _, _>(&mut runtime, label, |l| {
        l.base.modulate.modulate = Color::RED;
    });

    // Label itself is a UI node inside its own force_rerender subtree, so it
    // receives the full UI flag set (a superset of the COMMANDS bit that
    // classify_ui_data_change contributes for the modulate change).
    assert_eq!(ui_flags(&runtime, label), UI_FULL);
    assert!(rerender_set(&runtime, label));
    // force_rerender walks the subtree: UI child gets full UI flag set + rerender.
    assert_eq!(ui_flags(&runtime, child), UI_FULL);
    assert!(rerender_set(&runtime, child));
}

// 14. UI visible toggle on UiPanel with UI child -> layout flags on panel +
//     subtree visibility dirty (child gets full UI flags).
#[test]
fn with_node_mut_uipanel_visible_toggle_marks_subtree() {
    let mut runtime = Runtime::new();
    let panel = NodeAPI::create::<UiPanel>(&mut runtime);
    let child = NodeAPI::create::<UiLabel>(&mut runtime);
    NodeAPI::reparent(&mut runtime, panel, child);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<UiPanel, _, _>(&mut runtime, panel, |p| {
        p.base.visible = !p.base.visible;
    });

    // Panel: visible change -> LAYOUT_SELF|LAYOUT_PARENT|COMMANDS (classify) and
    // also full UI flags via the subtree walk (panel is part of its own subtree).
    assert_eq!(ui_flags(&runtime, panel), UI_FULL);
    // Child gets full UI flag set from mark_ui_visibility_dirty_subtree.
    assert_eq!(ui_flags(&runtime, child), UI_FULL);
}

// 15. Non-UI visibility: Sprite2D visible toggle with UiPanel child -> subtree
//     UI dirty on the panel child. Modulate change also force_rerenders subtree.
#[test]
fn with_node_mut_sprite2d_visible_toggle_marks_ui_child_subtree() {
    let mut runtime = Runtime::new();
    let sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    let panel = NodeAPI::create::<UiPanel>(&mut runtime);
    NodeAPI::reparent(&mut runtime, sprite, panel);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<Sprite2D, _, _>(&mut runtime, sprite, |s| {
        s.base.visible = !s.base.visible;
    });

    // Sprite is not UI: visibility change triggers mark_ui_visibility_dirty_subtree.
    // The UI descendant (panel) gets the full UI flag set.
    assert_eq!(ui_flags(&runtime, panel), UI_FULL);
    assert!(rerender_set(&runtime, sprite));
}

#[test]
fn with_node_mut_sprite2d_modulate_change_force_rerenders_subtree() {
    let mut runtime = Runtime::new();
    let sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    let panel = NodeAPI::create::<UiPanel>(&mut runtime);
    NodeAPI::reparent(&mut runtime, sprite, panel);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<Sprite2D, _, _>(&mut runtime, sprite, |s| {
        s.base.modulate.modulate = Color::RED;
    });

    // modulate_changed -> force_rerender(sprite) walks subtree; UI child gets full flags.
    assert!(rerender_set(&runtime, sprite));
    assert!(rerender_set(&runtime, panel));
    assert_eq!(ui_flags(&runtime, panel), UI_FULL);
}

// 16. Camera2D: zoom / active / transform changes request a 2D full scan; no-op
//     leaves it unset.
#[test]
fn with_node_mut_camera2d_zoom_requests_2d_full_scan() {
    let mut runtime = Runtime::new();
    let cam = NodeAPI::create::<Camera2D>(&mut runtime);
    reset_all(&mut runtime);
    assert!(!runtime.render_2d.full_scan_pending());

    <Runtime as NodeAPI>::with_node_mut::<Camera2D, _, _>(&mut runtime, cam, |c| {
        c.zoom += 1.0;
    });

    assert!(runtime.render_2d.full_scan_pending());
}

#[test]
fn with_node_mut_camera2d_active_toggle_requests_2d_full_scan() {
    let mut runtime = Runtime::new();
    let cam = NodeAPI::create::<Camera2D>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<Camera2D, _, _>(&mut runtime, cam, |c| {
        c.active = !c.active;
    });

    assert!(runtime.render_2d.full_scan_pending());
}

#[test]
fn with_node_mut_camera2d_transform_requests_2d_full_scan() {
    let mut runtime = Runtime::new();
    let cam = NodeAPI::create::<Camera2D>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<Camera2D, _, _>(&mut runtime, cam, |c| {
        c.base.transform = moved_2d();
    });

    assert!(runtime.render_2d.full_scan_pending());
    // Transform change on a Camera2D also marks 2D transform dirty (leaf).
    assert!(runtime.dirty.has_transform_dirty(cam, Spatial::TwoD));
}

#[test]
fn with_node_mut_camera2d_noop_no_2d_full_scan() {
    let mut runtime = Runtime::new();
    let cam = NodeAPI::create::<Camera2D>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_node_mut::<Camera2D, _, _>(&mut runtime, cam, |_c| {});

    assert!(!runtime.render_2d.full_scan_pending());
}

// 17. Camera3D active false->true: requests 3D full scan AND records activation
//     order via note_camera_3d_activated.
#[test]
fn with_node_mut_camera3d_activation_requests_3d_scan_and_records_order() {
    let mut runtime = Runtime::new();
    let cam = NodeAPI::create::<Camera3D>(&mut runtime); // default inactive
    reset_all(&mut runtime);
    assert!(!runtime.render_3d.full_scan_pending());
    assert!(!runtime.render_3d.camera_activation_order.contains_key(&cam));

    <Runtime as NodeAPI>::with_node_mut::<Camera3D, _, _>(&mut runtime, cam, |c| {
        c.active = true;
    });

    assert!(runtime.render_3d.full_scan_pending());
    assert!(runtime.render_3d.camera_activation_order.contains_key(&cam));
}

// ===========================================================================
// with_base_node_mut
// ===========================================================================

// 18. with_base_node_mut::<Node2D> on Sprite2D transform change -> 2D transform
//     dirty. No-op still sets rerender (unconditional) but no transform dirty.
#[test]
fn with_base_node_mut_node2d_transform_marks_2d_transform() {
    let mut runtime = Runtime::new();
    let sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_base_node_mut::<Node2D, _, _>(&mut runtime, sprite, |b| {
        b.transform = moved_2d();
    });

    assert!(runtime.dirty.has_transform_dirty(sprite, Spatial::TwoD));
    assert!(rerender_set(&runtime, sprite));
}

#[test]
fn with_base_node_mut_node2d_noop_sets_rerender_unconditionally() {
    let mut runtime = Runtime::new();
    let sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_base_node_mut::<Node2D, _, _>(&mut runtime, sprite, |_b| {});

    // with_base_node_mut calls mark_needs_rerender unconditionally.
    assert!(rerender_set(&runtime, sprite));
    assert!(!runtime.dirty.has_transform_dirty(sprite, Spatial::TwoD));
    assert_eq!(transform_flags(&runtime, sprite), 0);
}

// 19. visible toggle via base -> force_rerender subtree.
#[test]
fn with_base_node_mut_node2d_visible_force_rerenders_subtree() {
    let mut runtime = Runtime::new();
    let sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    let panel = NodeAPI::create::<UiPanel>(&mut runtime);
    NodeAPI::reparent(&mut runtime, sprite, panel);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_base_node_mut::<Node2D, _, _>(&mut runtime, sprite, |b| {
        b.visible = !b.visible;
    });

    assert!(rerender_set(&runtime, sprite));
    // vis change -> force_rerender walks subtree; UI child gets full flags.
    assert_eq!(ui_flags(&runtime, panel), UI_FULL);
    assert!(rerender_set(&runtime, panel));
}

// 20. modulate via base -> force_rerender subtree.
#[test]
fn with_base_node_mut_node2d_modulate_force_rerenders_subtree() {
    let mut runtime = Runtime::new();
    let sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    let panel = NodeAPI::create::<UiPanel>(&mut runtime);
    NodeAPI::reparent(&mut runtime, sprite, panel);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_base_node_mut::<Node2D, _, _>(&mut runtime, sprite, |b| {
        b.modulate.modulate = Color::RED;
    });

    assert!(rerender_set(&runtime, sprite));
    assert_eq!(ui_flags(&runtime, panel), UI_FULL);
}

// 21. with_base_node_mut::<UiNode> on UiLabel: layout.size / visible.
#[test]
fn with_base_node_mut_uinode_layout_size_layout_flags() {
    let mut runtime = Runtime::new();
    let label = NodeAPI::create::<UiLabel>(&mut runtime);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_base_node_mut::<UiNode, _, _>(&mut runtime, label, |b| {
        b.layout.size = perro_ui::UiVector2::pixels(80.0, 30.0);
    });

    assert_eq!(
        ui_flags(&runtime, label),
        F_LAYOUT_SELF | F_LAYOUT_PARENT | F_COMMANDS
    );
}

#[test]
fn with_base_node_mut_uinode_visible_layout_flags_and_subtree() {
    let mut runtime = Runtime::new();
    let panel = NodeAPI::create::<UiPanel>(&mut runtime);
    let child = NodeAPI::create::<UiLabel>(&mut runtime);
    NodeAPI::reparent(&mut runtime, panel, child);
    reset_all(&mut runtime);

    <Runtime as NodeAPI>::with_base_node_mut::<UiNode, _, _>(&mut runtime, panel, |b| {
        b.visible = !b.visible;
    });

    // visible change on the UiNode base: classify gives layout flags, and
    // mark_ui_base_change fires mark_ui_visibility_dirty_subtree -> full flags.
    assert_eq!(ui_flags(&runtime, panel), UI_FULL);
    assert_eq!(ui_flags(&runtime, child), UI_FULL);
}

// 22. Base type mismatch: with_base_node_mut::<Node3D> on Sprite2D -> None + no
//     flags. (Current code checks the base type BEFORE marking anything.)
#[test]
fn with_base_node_mut_type_mismatch_returns_none_no_flags() {
    let mut runtime = Runtime::new();
    let sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    reset_all(&mut runtime);

    let out =
        <Runtime as NodeAPI>::with_base_node_mut::<Node3D, _, _>(&mut runtime, sprite, |_b| 7);
    assert!(out.is_none());
    // Early return before mark_needs_rerender -> node stays clean.
    assert_eq!(raw_flags(&runtime, sprite), 0);
}

// 23. Camera3D transform change via with_base_node_mut::<Node3D> -> 3D full scan
//     (active-camera transform compare).
#[test]
fn with_base_node_mut_active_camera3d_transform_requests_3d_scan() {
    let mut runtime = Runtime::new();
    let cam = NodeAPI::create::<Camera3D>(&mut runtime);
    // Activate first so the active-camera transform comparison is armed.
    <Runtime as NodeAPI>::with_node_mut::<Camera3D, _, _>(&mut runtime, cam, |c| {
        c.active = true;
    });
    reset_all(&mut runtime);
    assert!(!runtime.render_3d.full_scan_pending());

    <Runtime as NodeAPI>::with_base_node_mut::<Node3D, _, _>(&mut runtime, cam, |b| {
        b.transform = moved_3d();
    });

    assert!(runtime.render_3d.full_scan_pending());
    assert!(runtime.dirty.has_transform_dirty(cam, Spatial::ThreeD));
}

// -- physics-version split ---------------------------------------------------
// Non-physics data mutations must NOT move the physics sync version; physics
// node mutations and structural changes must.

#[test]
fn with_node_mut_non_physics_keeps_physics_version() {
    let mut runtime = Runtime::new();
    let sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
    let physics_before = runtime.nodes.physics_version();
    let data_before = runtime.nodes.mutation_version();

    <Runtime as NodeAPI>::with_node_mut::<Sprite2D, _, _>(&mut runtime, sprite, |s| {
        s.base.transform.position.x += 1.0;
    });

    assert_eq!(
        runtime.nodes.physics_version(),
        physics_before,
        "sprite data mutation must not invalidate the physics sync gate"
    );
    assert_ne!(
        runtime.nodes.mutation_version(),
        data_before,
        "data mutation version still moves for resource-ref scans"
    );
}

#[test]
fn with_node_mut_physics_node_bumps_physics_version() {
    let mut runtime = Runtime::new();
    let body = NodeAPI::create::<perro_nodes::RigidBody3D>(&mut runtime);
    let physics_before = runtime.nodes.physics_version();

    <Runtime as NodeAPI>::with_node_mut::<perro_nodes::RigidBody3D, _, _>(
        &mut runtime,
        body,
        |b| {
            b.gravity_scale *= 2.0;
        },
    );

    assert_ne!(
        runtime.nodes.physics_version(),
        physics_before,
        "rigid body mutation must invalidate the physics sync gate"
    );
}

#[test]
fn structural_changes_bump_physics_version() {
    let mut runtime = Runtime::new();
    let before_insert = runtime.nodes.physics_version();
    let node = NodeAPI::create::<Sprite2D>(&mut runtime);
    assert_ne!(runtime.nodes.physics_version(), before_insert);

    let before_remove = runtime.nodes.physics_version();
    NodeAPI::remove_node(&mut runtime, node);
    assert_ne!(runtime.nodes.physics_version(), before_remove);
}

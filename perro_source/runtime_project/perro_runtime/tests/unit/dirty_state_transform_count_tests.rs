//! Unit tests 4 `DirtyState::transform_dirty_count` -- the O(1) gate behind
//! `has_transform_dirty_any`. Locks the count's inc/dec transitions so a
//! future edit to any mark/clear path can't silently desync it frm the
//! O(n) scan it replaced.

use crate::runtime::state::DirtyState;
use perro_ids::NodeID;
use perro_nodes::Spatial;

fn id(index: u32) -> NodeID {
    NodeID::from_parts(index, 0)
}

#[test]
fn mark_transform_sets_count_and_any() {
    let mut dirty = DirtyState::new();
    assert!(!dirty.has_transform_dirty_any());

    dirty.mark_transform(id(1), Spatial::TwoD);
    assert!(dirty.has_transform_dirty_any());
    assert!(dirty.has_transform_dirty(id(1), Spatial::TwoD));
}

#[test]
fn double_mark_same_slot_does_not_double_count() {
    let mut dirty = DirtyState::new();
    dirty.mark_transform(id(1), Spatial::TwoD);
    dirty.mark_transform(id(1), Spatial::TwoD);
    // 2nd mark on an already-transform-dirty slot must not inc the count
    // again -- clearing once should already bring it back to zero.
    dirty.clear_transform_dirty(id(1), Spatial::TwoD);
    assert!(!dirty.has_transform_dirty_any());
}

#[test]
fn mark_both_dims_same_slot_counts_once() {
    let mut dirty = DirtyState::new();
    // 2D then 3D on the same node: 2nd mark flips a different bit but the
    // slot already had a transform flag set, so count must stay @ 1 (a
    // single clear of either dim, once both cleared, resets to 0).
    dirty.mark_transform(id(1), Spatial::TwoD);
    dirty.mark_transform(id(1), Spatial::ThreeD);
    assert!(dirty.has_transform_dirty_any());
    dirty.clear_transform_dirty(id(1), Spatial::TwoD);
    // 3D bit still set on the slot -> still transform-dirty overall.
    assert!(dirty.has_transform_dirty_any());
    dirty.clear_transform_dirty(id(1), Spatial::ThreeD);
    assert!(!dirty.has_transform_dirty_any());
}

#[test]
fn clear_transform_dirty_decrements() {
    let mut dirty = DirtyState::new();
    dirty.mark_transform(id(1), Spatial::TwoD);
    dirty.mark_transform(id(2), Spatial::ThreeD);
    assert!(dirty.has_transform_dirty_any());

    dirty.clear_transform_dirty(id(1), Spatial::TwoD);
    assert!(dirty.has_transform_dirty_any());

    dirty.clear_transform_dirty(id(2), Spatial::ThreeD);
    assert!(!dirty.has_transform_dirty_any());
}

#[test]
fn clear_transform_dirty_at_index_decrements() {
    let mut dirty = DirtyState::new();
    dirty.mark_transform(id(3), Spatial::TwoD);
    assert!(dirty.has_transform_dirty_any());

    dirty.clear_transform_dirty_at_index(
        3,
        DirtyState::FLAG_DIRTY_2D_TRANSFORM | DirtyState::FLAG_DIRTY_3D_TRANSFORM,
    );
    assert!(!dirty.has_transform_dirty_any());
}

#[test]
fn rerender_only_marks_do_not_count_as_transform_dirty() {
    let mut dirty = DirtyState::new();
    dirty.mark_rerender(id(1));
    dirty.mark_ui(id(2), DirtyState::DIRTY_TEXT);
    assert!(dirty.has_any_dirty());
    assert!(!dirty.has_transform_dirty_any());
}

#[test]
fn mixed_rerender_and_transform_marks_count_only_transform() {
    let mut dirty = DirtyState::new();
    dirty.mark_rerender(id(1));
    dirty.mark_transform(id(2), Spatial::TwoD);
    dirty.mark_rerender(id(3));

    assert!(dirty.has_transform_dirty_any());
    dirty.clear_transform_dirty(id(2), Spatial::TwoD);
    assert!(!dirty.has_transform_dirty_any());
    // plain rerender entries stay in dirty_indices independent of the count.
    assert!(dirty.has_any_dirty());
}

#[test]
fn full_clear_resets_transform_count() {
    let mut dirty = DirtyState::new();
    dirty.mark_transform(id(1), Spatial::TwoD);
    dirty.mark_transform(id(2), Spatial::ThreeD);
    dirty.mark_rerender(id(3));
    assert!(dirty.has_transform_dirty_any());

    dirty.clear();
    assert!(!dirty.has_transform_dirty_any());
    assert!(!dirty.has_any_dirty());

    // Re-marking aft a full clear must still count correctly (no leftover
    // stale count frm b4 the clear).
    dirty.mark_transform(id(4), Spatial::TwoD);
    assert!(dirty.has_transform_dirty_any());
}

#[test]
fn clear_keep_ui_dirty_drops_transform_count_but_keeps_ui() {
    let mut dirty = DirtyState::new();
    dirty.mark_transform(id(1), Spatial::TwoD);
    dirty.mark_ui(id(2), DirtyState::DIRTY_TEXT);
    assert!(dirty.has_transform_dirty_any());

    dirty.clear_keep_ui_dirty();

    assert!(!dirty.has_transform_dirty_any());
    assert!(dirty.has_any_dirty());
    assert_ne!(dirty.ui_flags_at(id(2).index() as usize), 0);
}

#[test]
fn pending_transform_roots_count_as_transform_dirty_without_flag() {
    let mut dirty = DirtyState::new();
    assert!(!dirty.has_transform_dirty_any());
    dirty.mark_transform_root(id(5));
    // Count is still 0 (no node_flags bit set yet), but pending roots alone
    // must still report transform-dirty.
    assert!(dirty.has_transform_dirty_any());

    let mut out = Vec::new();
    dirty.take_pending_transform_roots(&mut out);
    assert!(!dirty.has_transform_dirty_any());
}

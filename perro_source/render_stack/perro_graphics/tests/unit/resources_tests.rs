use super::ResourceStore;
use perro_render_bridge::Material3D;

#[test]
fn texture_slot_reuse_bumps_generation() {
    let mut store = ResourceStore::new();
    let first = store.create_texture("__tmp_a__", false);
    assert!(store.has_texture(first));
    assert!(store.drop_texture(first));
    assert!(!store.has_texture(first));

    let second = store.create_texture("__tmp_b__", false);
    assert_eq!(first.index(), second.index());
    assert_ne!(first.generation(), second.generation());
    assert!(!store.has_texture(first));
    assert!(store.has_texture(second));
}

#[test]
fn material_source_reuses_existing() {
    let mut store = ResourceStore::new();
    let mat = Material3D::default();
    let first = store.create_material(mat, Some("res://materials/base.pmat"), false);
    let second = store.create_material(
        Material3D {
            roughness_factor: 1.0,
            ..Material3D::default()
        },
        Some("res://materials/base.pmat"),
        false,
    );
    assert_eq!(first, second);
}

#[test]
fn loaded_texture_is_not_dropped_before_first_use() {
    let mut store = ResourceStore::new();
    let id = store.create_texture("__tmp_a__", false);
    for _ in 0..120 {
        store.reset_ref_counts();
        store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    }
    assert!(store.has_texture(id));
}

#[test]
fn used_texture_drops_after_ttl_when_unreferenced() {
    let mut store = ResourceStore::new();
    let id = store.create_texture("__tmp_a__", false);
    store.reset_ref_counts();
    store.mark_texture_used(id);
    store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    assert!(store.has_texture(id));

    for _ in 0..ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES {
        store.reset_ref_counts();
        store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    }
    assert!(!store.has_texture(id));
}

#[test]
fn reserved_texture_is_not_auto_dropped() {
    let mut store = ResourceStore::new();
    let id = store.create_texture("__tmp_a__", true);
    store.reset_ref_counts();
    store.mark_texture_used(id);
    store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    for _ in 0..(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES * 2) {
        store.reset_ref_counts();
        store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    }
    assert!(store.has_texture(id));
}

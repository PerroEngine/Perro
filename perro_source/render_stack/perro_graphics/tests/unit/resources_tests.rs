use super::ResourceStore;
use perro_render_bridge::{Material3D, RuntimeMeshData, RuntimeMeshVertex, StandardMaterial3D};
use perro_structs::Unorm8x4;

fn simple_runtime_mesh(scale: f32) -> RuntimeMeshData {
    RuntimeMeshData {
        vertices: vec![
            RuntimeMeshVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: Unorm8x4::new([1.0, 0.0, 0.0, 0.0]),
            },
            RuntimeMeshVertex {
                position: [scale, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: Unorm8x4::new([1.0, 0.0, 0.0, 0.0]),
            },
            RuntimeMeshVertex {
                position: [0.0, scale, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
                joints: [0, 0, 0, 0],
                weights: Unorm8x4::new([1.0, 0.0, 0.0, 0.0]),
            },
        ],
        indices: vec![0, 1, 2],
        surface_ranges: vec![],
    }
}

#[test]
fn mesh_slot_reuse_bumps_generation() {
    let mut store = ResourceStore::new();
    let first = store.create_mesh("res://meshes/a.glb", false);
    assert!(store.has_mesh(first));
    assert!(store.drop_mesh(first));
    assert!(!store.has_mesh(first));

    let second = store.create_mesh("res://meshes/b.glb", false);
    assert_eq!(first.index(), second.index());
    assert_ne!(first.generation(), second.generation());
    assert!(!store.has_mesh(first));
    assert!(store.has_mesh(second));
}

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
fn material_slot_reuse_bumps_generation() {
    let mut store = ResourceStore::new();
    let first = store.create_material(Material3D::default(), None, false);
    assert!(store.has_material(first));
    assert!(store.drop_material(first));
    assert!(!store.has_material(first));

    let second = store.create_material(Material3D::default(), None, false);
    assert_eq!(first.index(), second.index());
    assert_ne!(first.generation(), second.generation());
    assert!(!store.has_material(first));
    assert!(store.has_material(second));
}

#[test]
fn material_source_reuses_existing() {
    let mut store = ResourceStore::new();
    let mat = Material3D::default();
    let first = store.create_material(mat, Some("res://materials/base.pmat"), false);
    let second = store.create_material(
        Material3D::Standard(StandardMaterial3D {
            roughness_factor: 1.0,
            ..StandardMaterial3D::default()
        }),
        Some("res://materials/base.pmat"),
        false,
    );
    assert_eq!(first, second);
}

#[test]
fn reserved_mesh_is_not_auto_dropped_and_keeps_runtime_data() {
    let mut store = ResourceStore::new();
    let source = "res://meshes/tree.glb";
    let id = store.create_mesh(source, true);
    let mesh = simple_runtime_mesh(1.0);
    store.set_runtime_mesh_data(source, mesh.clone());
    assert_eq!(store.runtime_mesh_data(source), Some(&mesh));

    store.reset_ref_counts();
    store.mark_mesh_used(id);
    store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    for _ in 0..(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES * 2) {
        store.reset_ref_counts();
        store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    }

    assert!(store.has_mesh(id));
    assert_eq!(store.runtime_mesh_data(source), Some(&mesh));
}

#[test]
fn dropping_mesh_clears_runtime_mesh_source_and_recreate_loads_new_data() {
    let mut store = ResourceStore::new();
    let source = "res://meshes/rock.glb";
    let id = store.create_mesh(source, false);
    let mesh_a = simple_runtime_mesh(1.0);
    store.set_runtime_mesh_data(source, mesh_a.clone());
    assert_eq!(store.runtime_mesh_data(source), Some(&mesh_a));

    assert!(store.drop_mesh(id));
    assert!(store.runtime_mesh_data(source).is_none());
    assert!(!store.has_mesh_source(source));

    let id2 = store.create_mesh(source, false);
    let mesh_b = simple_runtime_mesh(2.0);
    store.set_runtime_mesh_data(source, mesh_b.clone());

    assert!(store.has_mesh(id2));
    assert_eq!(store.runtime_mesh_data(source), Some(&mesh_b));
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

#[test]
fn auto_gc_texture_drains_source_maps() {
    let mut store = ResourceStore::new();
    let source = "__tmp_auto_gc_texture__";
    let id = store.create_texture(source, false);
    store.mark_texture_used(id);

    for _ in 0..ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES {
        store.reset_ref_counts();
        store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    }

    assert!(!store.has_texture(id));
    assert!(!store.has_texture_source(source));
    assert!(store.texture_source(id).is_none());
    assert!(store.texture_source_by_index(id.index()).is_none());

    let next = store.create_texture(source, false);
    assert_ne!(next, id);
    assert!(store.has_texture(next));
}

#[test]
fn auto_gc_mesh_drains_source_maps() {
    let mut store = ResourceStore::new();
    let source = "res://meshes/auto_gc.glb";
    let id = store.create_mesh(source, false);
    store.set_runtime_mesh_data(source, simple_runtime_mesh(1.0));
    store.mark_mesh_used(id);

    for _ in 0..ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES {
        store.reset_ref_counts();
        store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    }

    assert!(!store.has_mesh(id));
    assert!(!store.has_mesh_source(source));
    assert!(store.mesh_source(id).is_none());
    assert!(store.runtime_mesh_data(source).is_none());

    let next = store.create_mesh(source, false);
    assert_ne!(next, id);
    assert!(store.has_mesh(next));
}

#[test]
fn auto_gc_material_drains_source_maps() {
    let mut store = ResourceStore::new();
    let source = "res://materials/auto_gc.pmat";
    let id = store.create_material(Material3D::default(), Some(source), false);
    store.mark_material_used(id);

    for _ in 0..ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES {
        store.reset_ref_counts();
        store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    }

    assert!(!store.has_material(id));

    let next = store.create_material(Material3D::default(), Some(source), false);
    assert_ne!(next, id);
    assert!(store.has_material(next));
}

#[test]
fn stale_source_entries_are_not_reused() {
    let mut store = ResourceStore::new();

    let texture_source = "__tmp_stale_texture__";
    let texture = store.create_texture(texture_source, false);
    store
        .texture_by_source
        .insert(texture_source.to_string(), texture);
    store.texture_source_by.remove(&texture);
    assert!(store.drop_texture(texture));
    let texture_next = store.create_texture(texture_source, false);
    assert_ne!(texture_next, texture);

    let mesh_source = "res://meshes/stale.glb";
    let mesh = store.create_mesh(mesh_source, false);
    store.mesh_by_source.insert(mesh_source.to_string(), mesh);
    store.mesh_source_by.remove(&mesh);
    assert!(store.drop_mesh(mesh));
    let mesh_next = store.create_mesh(mesh_source, false);
    assert_ne!(mesh_next, mesh);

    let material_source = "res://materials/stale.pmat";
    let material = store.create_material(Material3D::default(), Some(material_source), false);
    store
        .material_by_source
        .insert(material_source.to_string(), material);
    store.material_source_by.remove(&material);
    assert!(store.drop_material(material));
    let material_next = store.create_material(Material3D::default(), Some(material_source), false);
    assert_ne!(material_next, material);
}

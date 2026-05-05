use super::ResourceStore;
use perro_render_bridge::{
    Material3D, RuntimeMeshData, RuntimeMeshVertex, StandardMaterial3D,
};

fn simple_runtime_mesh(scale: f32) -> RuntimeMeshData {
    RuntimeMeshData {
        vertices: vec![
            RuntimeMeshVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            RuntimeMeshVertex {
                position: [scale, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            RuntimeMeshVertex {
                position: [0.0, scale, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
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

use super::{DecodedTextureRgba, ResourceStore};
use perro_ids::{MaterialID, MeshID, TextureID};
use perro_render_bridge::{Material3D, RuntimeMeshData, RuntimeMeshVertex, StandardMaterial3D};
use perro_structs::UnitVector4;

fn simple_runtime_mesh(scale: f32) -> RuntimeMeshData {
    RuntimeMeshData {
        vertices: vec![
            RuntimeMeshVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                paint_uv: [0.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: UnitVector4::new([1.0, 0.0, 0.0, 0.0]),
            },
            RuntimeMeshVertex {
                position: [scale, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                paint_uv: [1.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: UnitVector4::new([1.0, 0.0, 0.0, 0.0]),
            },
            RuntimeMeshVertex {
                position: [0.0, scale, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
                paint_uv: [0.0, 1.0],
                joints: [0, 0, 0, 0],
                weights: UnitVector4::new([1.0, 0.0, 0.0, 0.0]),
            },
        ],
        indices: vec![0, 1, 2],
        surface_ranges: vec![],
        blend_shapes: vec![],
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
fn reserving_existing_texture_resets_zero_ref_ttl() {
    let mut store = ResourceStore::new();
    let source = "__tmp_reserve_existing__";
    let id = store.create_texture(source, false);
    store.mark_texture_used(id);

    for _ in 0..(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES - 1) {
        store.reset_ref_counts();
        store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    }

    let reserved = store.create_texture(source, true);
    assert_eq!(reserved, id);
    assert_eq!(
        store
            .texture_meta_by
            .get(&id)
            .expect("test setup/result must succeed")
            .zero_ref_frames,
        0
    );

    for _ in 0..(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES * 2) {
        store.reset_ref_counts();
        store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
    }
    assert!(store.has_texture(id));
}

#[test]
fn duplicate_create_with_id_for_same_loading_texture_keeps_original_id() {
    let mut store = ResourceStore::new();
    let source = "__tmp_pending_texture__";
    let first = TextureID::from_parts(77, 3);
    let second = TextureID::from_parts(77, 4);

    let out_first = store.create_texture_with_id(first, source, false);
    let out_second = store.create_texture_with_id(second, source, false);

    assert_eq!(out_first, first);
    assert_eq!(out_second, first);
    assert!(store.has_texture(first));
    assert!(!store.has_texture(second));
}

#[test]
fn stale_generation_drop_keeps_reused_live_resources() {
    let mut store = ResourceStore::new();

    let old_texture = store.create_texture("__stale_texture_a__", false);
    assert!(store.drop_texture(old_texture));
    let texture = store.create_texture("__stale_texture_b__", false);
    assert_eq!(texture.index(), old_texture.index());
    assert!(store.set_decoded_texture_data(
        texture,
        DecodedTextureRgba {
            rgba: vec![1, 2, 3, 4],
            width: 1,
            height: 1,
        }
    ));
    assert!(!store.drop_texture(old_texture));
    assert!(store.has_texture(texture));
    assert_eq!(store.texture_source(texture), Some("__stale_texture_b__"));
    assert!(store.decoded_texture_data(texture).is_some());
    assert!(store.texture_meta_by.contains_key(&texture));

    let old_mesh = store.create_mesh("__stale_mesh_a__", false);
    assert!(store.drop_mesh(old_mesh));
    let mesh = store.create_mesh("__stale_mesh_b__", false);
    assert_eq!(mesh.index(), old_mesh.index());
    assert!(store.set_runtime_mesh_data_by_id(mesh, simple_runtime_mesh(1.0)));
    assert!(!store.drop_mesh(old_mesh));
    assert!(store.has_mesh(mesh));
    assert_eq!(store.mesh_source(mesh), Some("__stale_mesh_b__"));
    assert!(store.runtime_mesh_data_by_id(mesh).is_some());
    assert!(store.mesh_meta_by.contains_key(&mesh));

    let old_material =
        store.create_material(Material3D::default(), Some("__stale_material_a__"), false);
    assert!(store.drop_material(old_material));
    let material =
        store.create_material(Material3D::default(), Some("__stale_material_b__"), false);
    assert_eq!(material.index(), old_material.index());
    assert!(!store.drop_material(old_material));
    assert!(store.has_material(material));
    assert!(store.material(material).is_some());
    assert!(store.material_meta_by.contains_key(&material));
}

#[test]
fn reserve_toggle_keeps_one_gc_candidate_and_one_age_step() {
    let mut store = ResourceStore::new();
    let texture = store.create_texture("__reserve_toggle_texture__", false);
    store.mark_texture_used(texture);

    for _ in 0..100 {
        assert!(store.set_texture_reserved(texture, true));
        assert!(store.set_texture_reserved(texture, false));
    }

    assert_eq!(
        store
            .texture_gc_candidates
            .iter()
            .filter(|candidate| **candidate == texture)
            .count(),
        1
    );
    store.reset_ref_counts();
    let dropped = store.gc_unused_after_frames(60, 1, usize::MAX);
    assert!(dropped.textures.is_empty());
    assert!(store.has_texture(texture));
    assert_eq!(store.texture_meta_by[&texture].zero_ref_frames, 1);
}

#[test]
fn mark_used_count_tracks_multiple_live_users() {
    let mut store = ResourceStore::new();
    let texture = store.create_texture("__tmp_ref_count_texture__", false);
    let mesh = store.create_mesh("res://meshes/ref_count.glb", false);
    let material = store.create_material(
        Material3D::default(),
        Some("res://materials/ref_count.pmat"),
        false,
    );

    store.mark_texture_used_count(texture, 3);
    store.mark_mesh_used_count(mesh, 2);
    store.mark_material_used_count(material, 4);

    assert_eq!(
        store
            .texture_meta_by
            .get(&texture)
            .expect("test setup/result must succeed")
            .ref_count,
        3
    );
    assert_eq!(
        store
            .mesh_meta_by
            .get(&mesh)
            .expect("test setup/result must succeed")
            .ref_count,
        2
    );
    assert_eq!(
        store
            .material_meta_by
            .get(&material)
            .expect("test setup/result must succeed")
            .ref_count,
        4
    );

    store.reset_ref_counts();

    assert_eq!(
        store
            .texture_meta_by
            .get(&texture)
            .expect("test setup/result must succeed")
            .ref_count,
        0
    );
    assert_eq!(
        store
            .mesh_meta_by
            .get(&mesh)
            .expect("test setup/result must succeed")
            .ref_count,
        0
    );
    assert_eq!(
        store
            .material_meta_by
            .get(&material)
            .expect("test setup/result must succeed")
            .ref_count,
        0
    );
}

#[test]
fn gc_drop_budget_batches_expired_candidates() {
    let mut store = ResourceStore::new();
    let mut textures = Vec::new();
    for i in 0..70 {
        let texture = store.create_texture(&format!("__tmp_batch_drop_{i}__"), false);
        store.mark_texture_used(texture);
        textures.push(texture);
    }
    store.reset_ref_counts();

    let first = store.gc_unused_after_frames(1, 1, 8);
    assert_eq!(first.textures.len(), 8);
    assert_eq!(store.active_texture_count(), 62);

    let second = store.gc_unused_after_frames(1, 1, 8);
    assert_eq!(second.textures.len(), 8);
    assert_eq!(store.active_texture_count(), 54);

    for id in first.textures.into_iter().chain(second.textures) {
        assert!(!store.has_texture(id));
    }
    assert!(textures.iter().filter(|id| store.has_texture(**id)).count() >= 54);
}

#[test]
fn resource_store_handles_10k_load_mark_and_gc_budget() {
    let mut store = ResourceStore::new();
    let mut textures = Vec::with_capacity(10_000);
    let mut meshes = Vec::with_capacity(10_000);
    let mut materials = Vec::with_capacity(10_000);

    for i in 0..10_000 {
        textures.push(store.create_texture(&format!("__tmp_stress_texture_{i}__"), false));
        meshes.push(store.create_mesh(&format!("res://meshes/stress_{i}.glb"), false));
        materials.push(store.create_material(
            Material3D::default(),
            Some(&format!("res://materials/stress_{i}.pmat")),
            false,
        ));
    }

    for i in 0..10_000 {
        store.mark_texture_used(textures[i]);
        store.mark_mesh_used(meshes[i]);
        store.mark_material_used(materials[i]);
    }
    store.reset_ref_counts();

    let drops = store.gc_unused_after_frames(1, 1, 128);
    assert_eq!(drops.textures.len(), 128);
    assert_eq!(drops.meshes.len(), 128);
    assert_eq!(drops.materials.len(), 128);
    assert_eq!(store.active_texture_count(), 9_872);
    assert_eq!(store.active_mesh_count(), 9_872);
    assert_eq!(store.active_material_count(), 9_872);
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
        .insert(perro_ids::string_to_u64(texture_source), texture);
    store.texture_source_by.remove(&texture);
    assert!(store.drop_texture(texture));
    let texture_next = store.create_texture(texture_source, false);
    assert_ne!(texture_next, texture);

    let mesh_source = "res://meshes/stale.glb";
    let mesh = store.create_mesh(mesh_source, false);
    store
        .mesh_by_source
        .insert(perro_ids::string_to_u64(mesh_source), mesh);
    store.mesh_source_by.remove(&mesh);
    assert!(store.drop_mesh(mesh));
    let mesh_next = store.create_mesh(mesh_source, false);
    assert_ne!(mesh_next, mesh);

    let material_source = "res://materials/stale.pmat";
    let material = store.create_material(Material3D::default(), Some(material_source), false);
    store
        .material_by_source
        .insert(perro_ids::string_to_u64(material_source), material);
    store.material_source_by.remove(&material);
    assert!(store.drop_material(material));
    let material_next = store.create_material(Material3D::default(), Some(material_source), false);
    assert_ne!(material_next, material);
}

#[test]
fn write_stream_texture_data_reuses_buffer_and_falls_back_by_source() {
    let mut store = ResourceStore::new();
    let source = "webcam://node/1";
    let id = store.create_texture(source, true);

    // first write establishes the resident by_id buffer.
    assert!(store.write_stream_texture_data(id, &[1, 2, 3, 4, 5, 6, 7, 8], 2, 1));
    let ptr = store
        .decoded_texture_data(id)
        .expect("decoded")
        .rgba
        .as_ptr();

    // no by_source duplicate: lookup by source falls back to the by_id buffer.
    let by_source = store
        .decoded_texture_data_by_source(source)
        .expect("by-source fallback");
    assert_eq!(by_source.rgba, [1, 2, 3, 4, 5, 6, 7, 8]);

    // same-size repeat copies in place: same allocation, updated bytes.
    assert!(store.write_stream_texture_data(id, &[8, 7, 6, 5, 4, 3, 2, 1], 2, 1));
    let decoded = store.decoded_texture_data(id).expect("decoded");
    assert_eq!(decoded.rgba, [8, 7, 6, 5, 4, 3, 2, 1]);
    assert_eq!(decoded.rgba.as_ptr(), ptr, "buffer reused, no realloc");

    // resolution change reallocates to the new size.
    assert!(store.write_stream_texture_data(id, &[9, 9, 9, 9, 9, 9, 9, 9], 1, 2));
    let decoded = store.decoded_texture_data(id).expect("decoded");
    assert_eq!((decoded.width, decoded.height), (1, 2));
}

#[test]
fn decoded_texture_source_lookup_uses_canonical_id_buffer() {
    let mut store = ResourceStore::new();
    let source = "res://textures/canonical.png";
    let id = store.create_texture(source, false);
    assert!(store.set_decoded_texture_data(
        id,
        DecodedTextureRgba {
            rgba: vec![1, 2, 3, 4, 5, 6, 7, 8],
            width: 2,
            height: 1,
        }
    ));

    let by_id = store.decoded_texture_data(id).expect("by id");
    let by_source = store
        .decoded_texture_data_by_source(source)
        .expect("by source");
    assert!(std::ptr::eq(by_id, by_source));
    assert_eq!(by_id.rgba.as_ptr(), by_source.rgba.as_ptr());
}

#[test]
fn huge_explicit_resource_ids_fall_back_without_slot_growth() {
    let mut store = ResourceStore::new();
    let huge_texture = TextureID::from_parts(u32::MAX, 7);
    let huge_mesh = MeshID::from_parts(u32::MAX, 7);
    let huge_material = MaterialID::from_parts(u32::MAX, 7);

    let texture = store.create_texture_with_id(huge_texture, "huge-texture", false);
    let mesh = store.create_mesh_with_id(huge_mesh, "huge-mesh", false);
    let material = store.create_material_with_id(
        huge_material,
        Material3D::default(),
        Some("huge-material"),
        false,
    );

    assert_ne!(texture, huge_texture);
    assert_ne!(mesh, huge_mesh);
    assert_ne!(material, huge_material);
    assert_eq!(store.rejected_explicit_id_count(), 3);
    assert_eq!(store.textures.generations.len(), 1);
    assert_eq!(store.meshes.generations.len(), 1);
    assert_eq!(store.materials.generations.len(), 1);
    assert_eq!(store.texture_source_by_slot.len(), 1);
    assert_eq!(store.mesh_source_by_slot.len(), 1);
}

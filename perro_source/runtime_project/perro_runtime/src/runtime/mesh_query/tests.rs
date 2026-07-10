use super::*;
use perro_nodes::{MultiMeshInstance3D, MultiMeshInstanceTransform};
use perro_runtime_api::sub_apis::NodeAPI;
use perro_structs::{Quaternion, Transform3D, UnitVector4};

#[test]
fn runtime_mesh_data_builds_query_surfaces() {
    let vertex = |position| perro_render_bridge::RuntimeMeshVertex {
        position,
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
        paint_uv: [0.0, 0.0],
        joints: [0; 4],
        weights: UnitVector4::ZERO,
    };
    let mesh = Mesh3D {
        vertices: vec![
            vertex([0.0, 0.0, 0.0]),
            vertex([1.0, 0.0, 0.0]),
            vertex([0.0, 1.0, 0.0]),
            vertex([0.0, 0.0, 1.0]),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        surface_ranges: vec![
            perro_render_bridge::MeshSurfaceRange {
                index_start: 0,
                index_count: 3,
            },
            perro_render_bridge::MeshSurfaceRange {
                index_start: 3,
                index_count: 3,
            },
        ],
        blend_shapes: Vec::new(),
    };

    let query = build_query_mesh_from_runtime_mesh(&mesh).expect("query mesh");

    assert_eq!(query.triangles.len(), 2);
    assert_eq!(query.triangles[0].surface_index, 0);
    assert_eq!(query.triangles[1].surface_index, 1);
}

#[test]
fn runtime_mesh_ray_interpolates_uv_and_barycentric() {
    let vertex = |position, uv| perro_render_bridge::RuntimeMeshVertex {
        position,
        normal: [0.0, 0.0, 1.0],
        uv,
        paint_uv: uv,
        joints: [0; 4],
        weights: UnitVector4::ZERO,
    };
    let mesh = Mesh3D {
        vertices: vec![
            vertex([0.0, 0.0, 0.0], [0.0, 0.0]),
            vertex([1.0, 0.0, 0.0], [1.0, 0.0]),
            vertex([0.0, 1.0, 0.0], [0.0, 1.0]),
        ],
        indices: vec![0, 1, 2],
        surface_ranges: vec![],
        blend_shapes: Vec::new(),
    };
    let query = build_query_mesh_from_runtime_mesh(&mesh).expect("query mesh");
    let hit = query_ray_tri_local(&query, 0, Vec3::new(0.25, 0.5, 1.0), Vec3::NEG_Z, 2.0, None)
        .flatten()
        .expect("hit");

    assert_eq!(hit.triangle_index, 0);
    assert!(
        hit.barycentric
            .abs_diff_eq(Vec3::new(0.25, 0.25, 0.5), 1e-5)
    );
    assert!(hit.uv0.abs_diff_eq(Vec2::new(0.25, 0.5), 1e-5));
    assert_eq!(hit.paint_uv, hit.uv0);
}

#[test]
fn posed_skin_query_keeps_tri_and_uv_attrs() {
    let mesh = build_query_mesh_data_with_skin(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y],
        vec![Vec2::ZERO, Vec2::X, Vec2::Y],
        vec![Vec2::new(0.1, 0.2), Vec2::new(0.8, 0.2), Vec2::new(0.1, 0.9)],
        vec![[0, 0, 0, 0]; 3],
        vec![[1.0, 0.0, 0.0, 0.0]; 3],
        vec![QueryTri { a: 0, b: 1, c: 2, surface_index: 7 }],
    )
    .expect("bind query mesh");
    let posed = skin_query_mesh_with_palette(
        &mesh,
        &[Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0))],
    )
    .expect("posed query mesh");
    let hit = query_ray_tri_local(
        &posed,
        0,
        Vec3::new(0.25, 0.25, 4.0),
        Vec3::NEG_Z,
        4.0,
        None,
    )
    .flatten()
    .expect("posed hit");

    assert_eq!(hit.surface_index, 7);
    assert_eq!(hit.triangle_index, 0);
    assert!(hit.local_point.abs_diff_eq(Vec3::new(0.25, 0.25, 2.0), 1e-5));
    assert!(hit.barycentric.abs_diff_eq(Vec3::new(0.5, 0.25, 0.25), 1e-5));
    assert!(hit.uv0.abs_diff_eq(Vec2::new(0.25, 0.25), 1e-5));
    assert!(hit.paint_uv.abs_diff_eq(Vec2::new(0.275, 0.375), 1e-5));
}

#[test]
fn batch_global_ray_query_preserves_order_and_surface_index() {
    let vertex = |position| perro_render_bridge::RuntimeMeshVertex {
        position,
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
        paint_uv: [0.0, 0.0],
        joints: [0; 4],
        weights: UnitVector4::ZERO,
    };
    let mesh = Mesh3D {
        vertices: vec![
            vertex([-1.0, 0.0, -1.0]),
            vertex([1.0, 0.0, -1.0]),
            vertex([0.0, 0.0, 1.0]),
        ],
        indices: vec![0, 1, 2],
        surface_ranges: vec![],
        blend_shapes: Vec::new(),
    };
    let query = build_query_mesh_from_runtime_mesh(&mesh).expect("query mesh");
    let rays = [
        MeshSurfaceRay3D {
            origin: Vector3::new(0.0, 1.0, 0.0),
            direction: Vector3::new(0.0, -1.0, 0.0),
            max_distance: 4.0,
        },
        MeshSurfaceRay3D {
            origin: Vector3::new(4.0, 1.0, 4.0),
            direction: Vector3::new(0.0, -1.0, 0.0),
            max_distance: 4.0,
        },
    ];

    let hits: Vec<_> = rays
        .iter()
        .map(|ray| {
            query_global_ray_candidates_for_node_mesh(
                &query,
                &[Mat4::IDENTITY],
                Mat4::IDENTITY,
                *ray,
                false,
            )
        })
        .collect();

    assert_eq!(hits[0].as_ref().map(|hit| hit.surface_index), Some(0));
    assert!(hits[1].is_none());
}

/// Builds a `MultiMeshInstance3D` node wired to the builtin cube mesh, with
/// `instance_count` instances laid out along +X so each has a distinct hit
/// position for a straight-down ray.
fn build_multimesh_cube_node(runtime: &mut Runtime, instance_count: usize) -> NodeID {
    let node_id = runtime.create::<MultiMeshInstance3D>();
    runtime
        .render_3d
        .mesh_sources
        .insert(node_id, "__cube__".to_string());
    runtime.with_node_mut::<MultiMeshInstance3D, _, _>(node_id, |mesh| {
        mesh.instances = (0..instance_count)
            .map(|i| {
                MultiMeshInstanceTransform::new(Transform3D::new(
                    Vector3::new(i as f32 * 4.0, 0.0, 0.0),
                    Quaternion::IDENTITY,
                    Vector3::ONE,
                ))
            })
            .collect();
    });
    node_id
}

#[test]
fn repeated_multimesh_query_hits_cache_and_skips_rebuild() {
    let mut runtime = Runtime::new();
    let node_id = build_multimesh_cube_node(&mut runtime, 8);

    let ray_origin = Vector3::new(0.0, 10.0, 0.0);
    let ray_dir = Vector3::new(0.0, -1.0, 0.0);

    let first = NodeAPI::mesh_instance_surface_on_global_ray(
        &mut runtime,
        node_id,
        ray_origin,
        ray_dir,
        100.0,
    );
    assert!(first.is_some(), "ray must hit instance 0's cube");
    let rebuilds_after_first = runtime.mesh_query_node_rebuilds.get();
    assert!(
        rebuilds_after_first >= 1,
        "first query must build the cache entry"
    );

    // Repeated queries against the same unchanged node must reuse the cached
    // QueryNodeData -- no further rebuilds, and identical results.
    for _ in 0..5 {
        let hit = NodeAPI::mesh_instance_surface_on_global_ray(
            &mut runtime,
            node_id,
            ray_origin,
            ray_dir,
            100.0,
        );
        assert_eq!(
            hit.map(|h| (h.instance_index, h.surface_index)),
            first.map(|h| (h.instance_index, h.surface_index)),
            "cached query must return identical hit"
        );
    }
    assert_eq!(
        runtime.mesh_query_node_rebuilds.get(),
        rebuilds_after_first,
        "repeated query on unchanged multimesh must not rebuild QueryNodeData"
    );

    // A point query against a different instance's expected position must
    // also hit via the same cached instance_local snapshot.
    let point_hit = NodeAPI::mesh_instance_surface_at_global_point(
        &mut runtime,
        node_id,
        Vector3::new(4.0, 0.0, 0.0),
    );
    assert!(
        point_hit.is_some(),
        "point query must hit instance 1's cube"
    );
    assert_eq!(
        runtime.mesh_query_node_rebuilds.get(),
        rebuilds_after_first,
        "point query against unchanged node must also hit the cache"
    );
}

#[test]
fn mutating_multimesh_instance_transform_invalidates_cache_and_reflects_change() {
    let mut runtime = Runtime::new();
    let node_id = build_multimesh_cube_node(&mut runtime, 4);

    let ray_origin = Vector3::new(0.0, 10.0, 0.0);
    let ray_dir = Vector3::new(0.0, -1.0, 0.0);

    // Warm the cache at instance 0's original position (origin).
    let before = NodeAPI::mesh_instance_surface_on_global_ray(
        &mut runtime,
        node_id,
        ray_origin,
        ray_dir,
        100.0,
    );
    assert!(before.is_some(), "ray must hit instance 0 @ origin");
    let rebuilds_before_mutation = runtime.mesh_query_node_rebuilds.get();

    // Move instance 0 away from the ray so the straight-down ray @ origin no
    // longer hits it (only instance 0 was ever under the ray).
    runtime.with_node_mut::<MultiMeshInstance3D, _, _>(node_id, |mesh| {
        mesh.instances[0].transform.position = Vector3::new(100.0, 0.0, 0.0);
    });

    let after = NodeAPI::mesh_instance_surface_on_global_ray(
        &mut runtime,
        node_id,
        ray_origin,
        ray_dir,
        100.0,
    );
    assert!(
        after.is_none(),
        "cache must reflect the moved instance, not the stale cached transform"
    );
    assert!(
        runtime.mesh_query_node_rebuilds.get() > rebuilds_before_mutation,
        "mutating an instance transform must invalidate the cached QueryNodeData"
    );
}

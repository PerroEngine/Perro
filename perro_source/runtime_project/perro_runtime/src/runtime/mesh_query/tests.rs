use super::*;
use perro_nodes::{
    MultiMeshInstance3D, MultiMeshInstanceTransform,
    mesh_instance_3d::MeshInstance3D,
    skeleton_3d::{Bone3D, Skeleton3D},
};
use perro_resource_api::sub_apis::MeshAPI;
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
        vec![
            Vec2::new(0.1, 0.2),
            Vec2::new(0.8, 0.2),
            Vec2::new(0.1, 0.9),
        ],
        vec![[0, 0, 0, 0]; 3],
        vec![[1.0, 0.0, 0.0, 0.0]; 3],
        vec![QueryTri {
            a: 0,
            b: 1,
            c: 2,
            surface_index: 7,
        }],
    )
    .expect("bind query mesh");
    let posed =
        skin_query_mesh_with_palette(&mesh, &[Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0))])
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
    assert!(
        hit.local_point
            .abs_diff_eq(Vec3::new(0.25, 0.25, 2.0), 1e-5)
    );
    assert!(
        hit.barycentric
            .abs_diff_eq(Vec3::new(0.5, 0.25, 0.25), 1e-5)
    );
    assert!(hit.uv0.abs_diff_eq(Vec2::new(0.25, 0.25), 1e-5));
    assert!(hit.paint_uv.abs_diff_eq(Vec2::new(0.275, 0.375), 1e-5));
}

#[test]
fn posed_surface_global_point_tracks_live_bone_pose_and_query_triangle_id() {
    let mut runtime = Runtime::new();
    let vertex = |position| perro_render_bridge::RuntimeMeshVertex {
        position,
        normal: [0.0, 0.0, 1.0],
        uv: [0.0, 0.0],
        paint_uv: [0.0, 0.0],
        joints: [0; 4],
        weights: UnitVector4::new([1.0, 0.0, 0.0, 0.0]),
    };
    let mesh_id = MeshAPI::create_mesh_data(
        runtime.resource_api.as_ref(),
        Mesh3D {
            vertices: vec![
                vertex([0.0, 0.0, 0.0]),
                vertex([1.0, 0.0, 0.0]),
                vertex([0.0, 1.0, 0.0]),
                vertex([1.0, 1.0, 0.0]),
            ],
            indices: vec![0, 1, 2, 1, 3, 2],
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
        },
    );
    let mut skeleton = Skeleton3D::default();
    skeleton.bones = vec![Bone3D {
        pose: Transform3D::new(
            Vector3::new(0.0, 0.0, 2.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
        inv_bind: Transform3D::IDENTITY,
        ..Bone3D::new()
    }];
    skeleton.refresh_inv_bind_cache();
    let skeleton_id = runtime.create::<Skeleton3D>();
    runtime.with_node_mut::<Skeleton3D, _, _>(skeleton_id, |node| *node = skeleton);
    let mesh_node = runtime.create::<MeshInstance3D>();
    runtime.with_node_mut::<MeshInstance3D, _, _>(mesh_node, |mesh| {
        mesh.mesh = mesh_id;
        mesh.skeleton = skeleton_id;
    });

    let point = NodeAPI::mesh_instance_surface_global_point(
        &mut runtime,
        mesh_node,
        1,
        Vector3::new(0.5, 0.25, 0.25),
    )
    .expect("posed point");
    assert_eq!(point, Vector3::new(0.75, 0.5, 2.0));

    runtime.with_node_mut::<Skeleton3D, _, _>(skeleton_id, |skeleton| {
        skeleton.bones[0].pose.position.z = 4.0;
    });
    let moved = NodeAPI::mesh_instance_surface_global_point(
        &mut runtime,
        mesh_node,
        1,
        Vector3::new(0.5, 0.25, 0.25),
    )
    .expect("moved posed point");
    assert_eq!(moved, Vector3::new(0.75, 0.5, 4.0));
}

#[test]
fn surface_global_point_rejects_invalid_barycentric_and_non_mesh_instance() {
    let mut runtime = Runtime::new();
    let multi = build_multimesh_cube_node(&mut runtime, 1);
    assert!(
        NodeAPI::mesh_instance_surface_global_point(
            &mut runtime,
            multi,
            0,
            Vector3::new(0.5, 0.25, 0.25),
        )
        .is_none()
    );
    let mesh = runtime.create::<MeshInstance3D>();
    runtime
        .render_3d
        .mesh_sources
        .insert(mesh, "__cube__".to_string());
    for barycentric in [
        Vector3::new(0.5, 0.25, 0.1),
        Vector3::new(-0.1, 0.55, 0.55),
        Vector3::new(f32::NAN, 0.5, 0.5),
    ] {
        assert!(
            NodeAPI::mesh_instance_surface_global_point(&mut runtime, mesh, 0, barycentric)
                .is_none()
        );
    }
    assert!(
        NodeAPI::mesh_instance_surface_global_point(
            &mut runtime,
            mesh,
            u32::MAX,
            Vector3::new(0.5, 0.25, 0.25),
        )
        .is_none()
    );
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

    let node = single_instance_query_node_data();
    let hits: Vec<_> = rays
        .iter()
        .map(|ray| {
            query_global_ray_candidates_for_node_mesh(&query, &node, Mat4::IDENTITY, *ray, None)
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
fn mutating_an_unrelated_node_keeps_the_multimesh_snapshot_cached() {
    let mut runtime = Runtime::new();
    let node_id = build_multimesh_cube_node(&mut runtime, 8);
    let other_id = build_multimesh_cube_node(&mut runtime, 8);

    let ray_origin = Vector3::new(0.0, 10.0, 0.0);
    let ray_dir = Vector3::new(0.0, -1.0, 0.0);

    assert!(
        NodeAPI::mesh_instance_surface_on_global_ray(
            &mut runtime,
            node_id,
            ray_origin,
            ray_dir,
            100.0,
        )
        .is_some(),
        "ray must hit instance 0's cube"
    );
    let rebuilds = runtime.mesh_query_node_rebuilds.get();

    // Data write 2 a DIFFERENT node: bumps the global mutation revision but
    // not this node's stamp, so the cached snapshot must survive.
    runtime.with_node_mut::<MultiMeshInstance3D, _, _>(other_id, |mesh| {
        mesh.instances[0].transform.position = Vector3::new(50.0, 0.0, 0.0);
    });

    assert!(
        NodeAPI::mesh_instance_surface_on_global_ray(
            &mut runtime,
            node_id,
            ray_origin,
            ray_dir,
            100.0,
        )
        .is_some(),
        "query must still hit after an unrelated node mutation"
    );
    assert_eq!(
        runtime.mesh_query_node_rebuilds.get(),
        rebuilds,
        "another node's mutation must not retire this node's QueryNodeData"
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

fn single_instance_query_node_data() -> QueryNodeData {
    QueryNodeData {
        mesh_id: MeshID::nil(),
        source: None,
        surfaces: Vec::new(),
        instance_local: vec![Mat4::IDENTITY],
        skeleton: None,
        instance_accel: Mutex::default(),
    }
}

/// Deterministic 32-bit LCG, so the accel-vs-linear comparison covers a spread
/// of transforms and ray directions without a rand dep or flaky input.
struct TestRng(u32);

impl TestRng {
    fn next_unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 16_777_216.0
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_unit() * (hi - lo)
    }
}

/// Multimesh with enough instances to cross `INSTANCE_ACCEL_MIN_INSTANCES`,
/// scattered with rotations and non-uniform scales. Every 8th instance
/// duplicates its predecessor, so metric ties between two instances really
/// occur and the tie-break gets exercised.
fn build_scattered_multimesh(
    runtime: &mut Runtime,
    instance_count: usize,
) -> (NodeID, Vec<Transform3D>) {
    let node_id = runtime.create::<MultiMeshInstance3D>();
    runtime
        .render_3d
        .mesh_sources
        .insert(node_id, "__cube__".to_string());
    let mut rng = TestRng(0x1234_5678);
    let mut transforms: Vec<Transform3D> = Vec::with_capacity(instance_count);
    for index in 0..instance_count {
        if index % 8 == 7 {
            transforms.push(transforms[index - 1]);
            continue;
        }
        transforms.push(Transform3D::new(
            Vector3::new(
                rng.range(-30.0, 30.0),
                rng.range(-6.0, 6.0),
                rng.range(-30.0, 30.0),
            ),
            Quaternion::from_euler_xyz(
                rng.range(0.0, std::f32::consts::TAU),
                rng.range(0.0, std::f32::consts::TAU),
                rng.range(0.0, std::f32::consts::TAU),
            ),
            Vector3::new(
                rng.range(0.4, 2.5),
                rng.range(0.4, 2.5),
                rng.range(0.4, 2.5),
            ),
        ));
    }
    let instances = transforms.clone();
    runtime.with_node_mut::<MultiMeshInstance3D, _, _>(node_id, |mesh| {
        mesh.instances = instances
            .into_iter()
            .map(MultiMeshInstanceTransform::new)
            .collect();
    });
    (node_id, transforms)
}

fn assert_same_hit(
    accelerated: Option<MeshSurfaceHit3D>,
    linear: Option<MeshSurfaceHit3D>,
    context: &str,
) {
    match (accelerated, linear) {
        (None, None) => {}
        (Some(a), Some(b)) => {
            assert_eq!(a.instance_index, b.instance_index, "{context}: instance");
            assert_eq!(a.surface_index, b.surface_index, "{context}: surface");
            assert_eq!(a.triangle_index, b.triangle_index, "{context}: triangle");
            assert_eq!(a.distance, b.distance, "{context}: distance");
            assert_eq!(a.global_point, b.global_point, "{context}: global point");
            assert_eq!(a.local_point, b.local_point, "{context}: local point");
            assert_eq!(a.global_normal, b.global_normal, "{context}: global normal");
        }
        (a, b) => panic!("{context}: accel {a:?} vs linear {b:?}"),
    }
}

/// The instance acceleration structure must be a pure culling device: same
/// node + same ray => byte-identical hit to the linear scan, including which
/// instance wins a distance tie.
#[test]
fn instance_accel_ray_hits_match_the_linear_scan() {
    let mut runtime = Runtime::new();
    let (node_id, transforms) = build_scattered_multimesh(&mut runtime, 96);
    // Node transform w/ rotation + non-uniform scale: exercises the node-space
    // `t` <-> global-distance conversion the culling bound rests on.
    let node_transform = Transform3D::new(
        Vector3::new(3.0, -1.5, 2.0),
        Quaternion::from_euler_xyz(0.4, 0.9, -0.3),
        Vector3::new(1.7, 0.6, 1.1),
    );
    assert!(NodeAPI::set_global_transform_3d(
        &mut runtime,
        node_id,
        node_transform
    ));
    let node_global = node_transform.to_mat4();

    let mut rng = TestRng(0x0bad_c0de);
    let mut hits = 0usize;
    let mut distinct_instances = std::collections::BTreeSet::new();
    for case in 0..192 {
        // Two thirds of the cases aim at a real instance (so the sweep is
        // hit-rich and exercises the pruning), the rest are free rays that
        // mostly miss (so the culling is checked against "no hit" too).
        let (origin, direction) = if case % 3 == 0 {
            (
                Vector3::new(
                    rng.range(-40.0, 40.0),
                    rng.range(-20.0, 20.0),
                    rng.range(-40.0, 40.0),
                ),
                Vector3::new(
                    rng.range(-1.0, 1.0),
                    rng.range(-1.0, 1.0),
                    rng.range(-1.0, 1.0),
                ),
            )
        } else {
            let target: Vec3 =
                node_global.transform_point3(transforms[case % transforms.len()].position.into());
            let offset = Vec3::new(
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
            )
            .normalize_or(Vec3::Y)
                * rng.range(4.0, 30.0);
            let jitter = Vec3::new(
                rng.range(-0.12, 0.12),
                rng.range(-0.12, 0.12),
                rng.range(-0.12, 0.12),
            );
            let origin = target + offset;
            ((origin).into(), (target + jitter - origin).into())
        };
        if direction.length_squared() < 1.0e-6 {
            continue;
        }
        // Mix bounded and effectively unbounded rays.
        let max_distance = if case % 4 == 0 {
            rng.range(1.0, 25.0)
        } else {
            1_000.0
        };

        let accelerated = NodeAPI::mesh_instance_surface_on_global_ray(
            &mut runtime,
            node_id,
            origin,
            direction,
            max_distance,
        );
        let linear = without_instance_accel(|| {
            NodeAPI::mesh_instance_surface_on_global_ray(
                &mut runtime,
                node_id,
                origin,
                direction,
                max_distance,
            )
        });
        assert_same_hit(accelerated, linear, &format!("case {case}"));
        if let Some(hit) = accelerated {
            hits += 1;
            distinct_instances.insert(hit.instance_index);
        }
    }
    // Guard against a vacuous pass: the sweep has to actually land on a spread
    // of instances, not miss everything.
    assert!(hits >= 40, "expect a hit-rich sweep, got {hits} hits");
    assert!(
        distinct_instances.len() >= 15,
        "expect many instances hit, got {}",
        distinct_instances.len()
    );
}

/// Duplicate instance transforms produce identical metrics. The linear fold
/// keeps the accumulator on a tie (lowest index wins); the accel visits
/// instances in box-distance order, so it must reproduce that explicitly.
#[test]
fn instance_accel_breaks_metric_ties_on_lowest_instance_index() {
    let mut runtime = Runtime::new();
    let node_id = runtime.create::<MultiMeshInstance3D>();
    runtime
        .render_3d
        .mesh_sources
        .insert(node_id, "__cube__".to_string());
    // 64 instances (over the accel threshold) parked far away, except three
    // stacked @ origin: 5, 20 + 40 share one transform, so a straight-down ray
    // hits all three @ the exact same distance.
    runtime.with_node_mut::<MultiMeshInstance3D, _, _>(node_id, |mesh| {
        mesh.instances = (0..64)
            .map(|index| {
                let position = if matches!(index, 5 | 20 | 40) {
                    Vector3::ZERO
                } else {
                    Vector3::new(500.0 + index as f32, 0.0, 0.0)
                };
                MultiMeshInstanceTransform::new(Transform3D::new(
                    position,
                    Quaternion::IDENTITY,
                    Vector3::ONE,
                ))
            })
            .collect();
    });

    let origin = Vector3::new(0.0, 10.0, 0.0);
    let direction = Vector3::new(0.0, -1.0, 0.0);
    let accelerated = NodeAPI::mesh_instance_surface_on_global_ray(
        &mut runtime,
        node_id,
        origin,
        direction,
        100.0,
    );
    let linear = without_instance_accel(|| {
        NodeAPI::mesh_instance_surface_on_global_ray(
            &mut runtime,
            node_id,
            origin,
            direction,
            100.0,
        )
    });
    assert_eq!(
        accelerated.map(|hit| hit.instance_index),
        Some(5),
        "tie must resolve 2 lowest instance index"
    );
    assert_same_hit(accelerated, linear, "stacked-instance tie");
}

/// The accel lives on the `QueryNodeData` snapshot, so writing the node must
/// retire it with the snapshot -- a stale accel would keep culling against the
/// old instance boxes.
#[test]
fn instance_accel_retires_with_the_node_snapshot() {
    let mut runtime = Runtime::new();
    let node_id = build_multimesh_cube_node(&mut runtime, 64);
    let origin = Vector3::new(0.0, 10.0, 0.0);
    let direction = Vector3::new(0.0, -1.0, 0.0);

    // Instances sit @ x = 4*i, so only instance 0 is under the ray.
    assert!(
        NodeAPI::mesh_instance_surface_on_global_ray(
            &mut runtime,
            node_id,
            origin,
            direction,
            100.0
        )
        .is_some(),
        "ray must hit instance 0 b4 the move"
    );

    runtime.with_node_mut::<MultiMeshInstance3D, _, _>(node_id, |mesh| {
        mesh.instances[0].transform.position = Vector3::new(900.0, 0.0, 0.0);
    });
    assert!(
        NodeAPI::mesh_instance_surface_on_global_ray(
            &mut runtime,
            node_id,
            origin,
            direction,
            100.0
        )
        .is_none(),
        "accel must not survive the instance write that moved the box"
    );

    // Moving an instance INTO the ray must be picked up too.
    runtime.with_node_mut::<MultiMeshInstance3D, _, _>(node_id, |mesh| {
        mesh.instances[63].transform.position = Vector3::ZERO;
    });
    assert_eq!(
        NodeAPI::mesh_instance_surface_on_global_ray(
            &mut runtime,
            node_id,
            origin,
            direction,
            100.0
        )
        .map(|hit| hit.instance_index),
        Some(63),
        "accel must see an instance moved under the ray"
    );
}

/// The accel is built in NODE space precisely so a parent's movement -- which
/// never touches this node's change stamp -- cannot stale it.
#[test]
fn instance_accel_survives_parent_movement_without_stale_hits() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<perro_nodes::Node3D>();
    let node_id = build_multimesh_cube_node(&mut runtime, 64);
    assert!(NodeAPI::reparent(&mut runtime, parent_id, node_id));

    let origin = Vector3::new(0.0, 10.0, 0.0);
    let direction = Vector3::new(0.0, -1.0, 0.0);
    assert_eq!(
        NodeAPI::mesh_instance_surface_on_global_ray(
            &mut runtime,
            node_id,
            origin,
            direction,
            100.0
        )
        .map(|hit| hit.instance_index),
        Some(0),
        "ray must hit instance 0 b4 the parent moves"
    );

    // Slide the parent so instance 1 (local x = 4) lands under the ray.
    assert!(NodeAPI::set_global_transform_3d(
        &mut runtime,
        parent_id,
        Transform3D::new(
            Vector3::new(-4.0, 0.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
    ));
    assert_eq!(
        NodeAPI::mesh_instance_surface_on_global_ray(
            &mut runtime,
            node_id,
            origin,
            direction,
            100.0
        )
        .map(|hit| hit.instance_index),
        Some(1),
        "node-space accel must follow the parent transform"
    );
}

/// Batched ray queries share one resolved accel; they must still agree w/ the
/// linear path ray 4 ray.
#[test]
fn instance_accel_batched_rays_match_the_linear_scan() {
    let mut runtime = Runtime::new();
    let (node_id, transforms) = build_scattered_multimesh(&mut runtime, 96);
    let mut rng = TestRng(0x00c0_ffee);
    // Aim each ray at an instance so the batch is hit-rich.
    let rays: Vec<MeshSurfaceRay3D> = (0..48)
        .map(|index| {
            let target: Vec3 = transforms[index % transforms.len()].position.into();
            let origin = target
                + Vec3::new(
                    rng.range(-1.0, 1.0),
                    rng.range(-1.0, 1.0),
                    rng.range(-1.0, 1.0),
                )
                .normalize_or(Vec3::Y)
                    * rng.range(4.0, 30.0);
            MeshSurfaceRay3D {
                origin: origin.into(),
                direction: (target - origin).into(),
                max_distance: 200.0,
            }
        })
        .collect();

    let accelerated =
        NodeAPI::mesh_instance_surfaces_on_global_rays(&mut runtime, node_id, &rays, true);
    let linear = without_instance_accel(|| {
        NodeAPI::mesh_instance_surfaces_on_global_rays(&mut runtime, node_id, &rays, true)
    });
    assert_eq!(accelerated.len(), rays.len());
    for (index, (a, b)) in accelerated.into_iter().zip(linear).enumerate() {
        assert_same_hit(a, b, &format!("batched ray {index}"));
    }
}

/// Nodes under the instance threshold must not build an accel @ all, and the
/// structure that is built has 2 cover every instance.
#[test]
fn instance_accel_threshold_and_coverage() {
    let mut runtime = Runtime::new();
    let small_id = build_multimesh_cube_node(&mut runtime, INSTANCE_ACCEL_MIN_INSTANCES - 1);
    let big_id = build_multimesh_cube_node(&mut runtime, INSTANCE_ACCEL_MIN_INSTANCES + 5);

    let mesh = runtime
        .load_query_mesh_data("__cube__")
        .expect("builtin cube query mesh");

    let small = runtime.query_node_mesh_data(small_id).expect("small node");
    assert!(
        small.ray_instance_accel(&mesh).is_none(),
        "under-threshold node must stay on the linear path"
    );

    let big = runtime.query_node_mesh_data(big_id).expect("big node");
    let accel = big.ray_instance_accel(&mesh).expect("accel 4 big node");
    assert_eq!(
        accel.instance_count(),
        INSTANCE_ACCEL_MIN_INSTANCES + 5,
        "accel must cover every instance"
    );
    // Second lookup reuses the cached structure.
    assert!(Arc::ptr_eq(
        &accel,
        &big.ray_instance_accel(&mesh).expect("cached accel")
    ));
}

/// Regression: a ray that enters a triangle's AABB but misses the triangle
/// must not abandon the rest of the scan.
///
/// `query_ray_tri_*` used `ray_intersect_triangle(..)?`, folding that miss into
/// the outer `None` that callers read as "abort this mesh/instance". Any
/// triangle ordered before the real hit could silently kill the whole query --
/// a rotated cube missed a ray straight through its centre.
#[test]
fn ray_query_survives_a_triangle_miss_before_the_hit() {
    let mut runtime = Runtime::new();
    // Linear tri scan: two coplanar triangles sharing one AABB, ray through
    // the SECOND only. Built directly (not via `create_mesh_data`) because the
    // decoded-query cache is process-global and keyed on `MeshID` + revision,
    // which collides across `Runtime`s inside one test binary.
    let vertex = |position| perro_render_bridge::RuntimeMeshVertex {
        position,
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
        paint_uv: [0.0, 0.0],
        joints: [0; 4],
        weights: UnitVector4::ZERO,
    };
    let query = build_query_mesh_from_runtime_mesh(&Mesh3D {
        vertices: vec![
            // Triangle 0: lower-left half of the [0,1]^2 quad.
            vertex([0.0, 0.0, 0.0]),
            vertex([1.0, 0.0, 0.0]),
            vertex([0.0, 0.0, 1.0]),
            // Triangle 1: upper-right half, sharing the same AABB.
            vertex([1.0, 0.0, 1.0]),
        ],
        indices: vec![0, 1, 2, 1, 3, 2],
        surface_ranges: vec![],
        blend_shapes: Vec::new(),
    })
    .expect("query mesh");
    let mut best = None;
    for tri_idx in 0..query.triangles.len() {
        best = query_ray_tri_local(
            &query,
            tri_idx,
            Vec3::new(0.85, 1.0, 0.85),
            Vec3::NEG_Y,
            10.0,
            best,
        )
        .expect("a triangle miss must not abort the scan");
    }
    let hit = best.expect("ray thru triangle 1 must hit");
    assert_eq!(hit.triangle_index, 1);
    assert!((hit.metric - 1.0).abs() < 1.0e-4);

    // Node ray: a rotated cube instance. Every face triangle whose AABB the ray
    // crosses but whose surface it misses used to abort the instance.
    let node_id = runtime.create::<MultiMeshInstance3D>();
    runtime
        .render_3d
        .mesh_sources
        .insert(node_id, "__cube__".to_string());
    runtime.with_node_mut::<MultiMeshInstance3D, _, _>(node_id, |mesh| {
        mesh.instances = vec![MultiMeshInstanceTransform::new(Transform3D::new(
            Vector3::ZERO,
            Quaternion::from_euler_xyz(0.3, 1.1, 2.0),
            Vector3::ONE,
        ))];
    });
    let hit = NodeAPI::mesh_instance_surface_on_global_ray(
        &mut runtime,
        node_id,
        Vector3::new(0.0, 40.0, 0.0),
        Vector3::new(0.0, -1.0, 0.0),
        1_000.0,
    )
    .expect("ray straight thru a rotated cube's centre must hit");
    assert_eq!(hit.instance_index, 0);
    assert!(
        (hit.distance - 39.44).abs() < 0.05,
        "hit @ the rotated cube surface, got {}",
        hit.distance
    );
}

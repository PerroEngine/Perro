use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_render_bridge::{Mesh3D, MeshSurfaceRange, RuntimeMeshVertex};
use perro_runtime::Runtime;
use perro_runtime_api::sub_apis::NodeAPI;
use perro_structs::Vector3;

fn closest_point_on_triangle(p: Vector3, a: Vector3, b: Vector3, c: Vector3) -> Vector3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

fn mesh_query_workload(tri_count: usize, surface_count: usize) -> f32 {
    let mut out = 0.0_f32;
    let p = Vector3::new(0.13, 0.21, -0.37);
    for tri in 0..tri_count {
        let s = (tri % surface_count.max(1)) as f32 * 0.0001;
        let a = Vector3::new(s, 0.1 + s, -0.2);
        let b = Vector3::new(0.4 + s, 0.3, 0.15);
        let c = Vector3::new(-0.2, 0.5 + s, 0.35);
        out += closest_point_on_triangle(p, a, b, c).length_squared();
    }
    out
}

fn bench_mesh_query_synthetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("mesh_query/synthetic");
    for (triangles, surfaces) in [(128usize, 1usize), (2_048, 8), (16_384, 16)] {
        group.bench_with_input(
            BenchmarkId::new(
                "closest_point_scan",
                format!("{triangles}_tri_{surfaces}_surf"),
            ),
            &(triangles, surfaces),
            |b, &(triangles, surfaces)| {
                b.iter(|| {
                    black_box(mesh_query_workload(
                        black_box(triangles),
                        black_box(surfaces),
                    ))
                })
            },
        );
    }
    group.finish();
}

fn grid_mesh(side: usize) -> Mesh3D {
    let vertex = |x: usize, z: usize| RuntimeMeshVertex {
        position: [x as f32, ((x * 17 + z * 31) % 11) as f32 * 0.03, z as f32],
        normal: [0.0, 1.0, 0.0],
        uv: [x as f32 / side as f32, z as f32 / side as f32],
        paint_uv: [x as f32 / side as f32, z as f32 / side as f32],
        joints: [0; 4],
        weights: Default::default(),
    };
    let mut vertices = Vec::with_capacity((side + 1) * (side + 1));
    for z in 0..=side {
        for x in 0..=side {
            vertices.push(vertex(x, z));
        }
    }

    let mut indices = Vec::with_capacity(side * side * 6);
    for z in 0..side {
        for x in 0..side {
            let a = (z * (side + 1) + x) as u32;
            let b = a + 1;
            let c = a + (side + 1) as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    Mesh3D {
        vertices,
        surface_ranges: vec![MeshSurfaceRange {
            index_start: 0,
            index_count: indices.len() as u32,
        }],
        indices,
        blend_shapes: vec![],
    }
}

fn bench_mesh_query_bvh_runtime_api(c: &mut Criterion) {
    let mut group = c.benchmark_group("mesh_query/bvh_runtime_api");
    for side in [1usize, 2, 4, 8, 16, 64, 128] {
        let mut runtime = Runtime::new();
        let mesh = runtime.bench_create_mesh_data(grid_mesh(side));
        let tri_count = side * side * 2;
        let point = Vector3::new(side as f32 * 0.52, 1.7, side as f32 * 0.48);
        let ray_origin = Vector3::new(side as f32 * 0.5, 10.0, side as f32 * 0.5);
        let ray_dir = Vector3::new(0.0, -1.0, 0.0);

        black_box(NodeAPI::mesh_data_surface_at_local_point(
            &mut runtime,
            mesh,
            point,
        ));
        black_box(NodeAPI::mesh_data_surface_on_local_ray(
            &mut runtime,
            mesh,
            ray_origin,
            ray_dir,
            100.0,
        ));

        group.bench_with_input(
            BenchmarkId::new("point_nearest", format!("{tri_count}_tri")),
            &mesh,
            |b, &mesh| {
                b.iter(|| {
                    black_box(NodeAPI::mesh_data_surface_at_local_point(
                        &mut runtime,
                        black_box(mesh),
                        black_box(point),
                    ))
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("ray_hit", format!("{tri_count}_tri")),
            &mesh,
            |b, &mesh| {
                b.iter(|| {
                    black_box(NodeAPI::mesh_data_surface_on_local_ray(
                        &mut runtime,
                        black_box(mesh),
                        black_box(ray_origin),
                        black_box(ray_dir),
                        black_box(100.0),
                    ))
                })
            },
        );
    }
    group.finish();
}

/// Node-level point/ray/region queries against a `MultiMeshInstance3D`.
/// Rebuilds a per-instance `Vec<Mat4>` (+ a surfaces clone) from scratch on
/// every call unless the runtime caches `QueryNodeData` per node -- this is
/// the hot path a per-node cache targets.
fn build_multimesh_node(runtime: &mut Runtime, instance_count: usize) -> perro_ids::NodeID {
    use perro_nodes::{MultiMeshInstance3D, MultiMeshInstanceTransform};
    use perro_structs::{Quaternion, Transform3D};

    let node_id = NodeAPI::create::<MultiMeshInstance3D>(runtime);
    runtime.bench_set_mesh_source(node_id, "__cube__");
    NodeAPI::with_node_mut::<MultiMeshInstance3D, _, _>(runtime, node_id, |mesh| {
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

fn bench_multimesh_node_query(c: &mut Criterion) {
    use perro_nodes::MultiMeshInstance3D;

    let mut group = c.benchmark_group("mesh_query/multimesh_node");
    let ray_origin = Vector3::new(0.0, 10.0, 0.0);
    let ray_dir = Vector3::new(0.0, -1.0, 0.0);

    for instance_count in [100usize, 1_000, 10_000] {
        // Steady state: nothing in the scene changes between queries, so the
        // node snapshot AND its instance acceleration structure both stay warm.
        // This case cannot tell a cache hit from a miss -- it never mutates.
        {
            let mut runtime = Runtime::new();
            let node_id = build_multimesh_node(&mut runtime, instance_count);
            group.bench_with_input(
                BenchmarkId::new("ray_hit_repeated", instance_count),
                &instance_count,
                |b, _| {
                    b.iter(|| {
                        black_box(NodeAPI::mesh_instance_surface_on_global_ray(
                            &mut runtime,
                            node_id,
                            black_box(ray_origin),
                            black_box(ray_dir),
                            black_box(100.0),
                        ))
                    })
                },
            );
        }

        // A DIFFERENT node is written between queries -- the live-scene case.
        // The old global-mutation-revision cache key missed every iteration
        // here (and dropped every other node's entry on the way); the per-node
        // change stamp keeps this a hit, so it should track `ray_hit_repeated`
        // plus the cost of the unrelated write.
        {
            let mut runtime = Runtime::new();
            let node_id = build_multimesh_node(&mut runtime, instance_count);
            let other_id = build_multimesh_node(&mut runtime, 4);
            group.bench_with_input(
                BenchmarkId::new("ray_hit_unrelated_write", instance_count),
                &instance_count,
                |b, _| {
                    let mut tick = 0.0_f32;
                    b.iter(|| {
                        tick += 1.0;
                        NodeAPI::with_node_mut::<MultiMeshInstance3D, _, _>(
                            &mut runtime,
                            other_id,
                            |mesh| {
                                mesh.instances[0].transform.position =
                                    Vector3::new(black_box(tick), 0.0, 0.0);
                            },
                        );
                        black_box(NodeAPI::mesh_instance_surface_on_global_ray(
                            &mut runtime,
                            node_id,
                            black_box(ray_origin),
                            black_box(ray_dir),
                            black_box(100.0),
                        ))
                    })
                },
            );
        }

        // The QUERIED node is written between queries -- a moving multimesh.
        // Forced full rebuild every iteration: the `Vec<Mat4>` snapshot plus
        // the instance acceleration structure. This is the worst case for any
        // lazily built per-node structure, so it is the one that has to stay
        // cheaper than the linear scan it replaces.
        {
            let mut runtime = Runtime::new();
            let node_id = build_multimesh_node(&mut runtime, instance_count);
            group.bench_with_input(
                BenchmarkId::new("ray_hit_self_write", instance_count),
                &instance_count,
                |b, _| {
                    let mut tick = 0.0_f32;
                    b.iter(|| {
                        tick += 1.0;
                        NodeAPI::with_node_mut::<MultiMeshInstance3D, _, _>(
                            &mut runtime,
                            node_id,
                            |mesh| {
                                let last = mesh.instances.len() - 1;
                                mesh.instances[last].transform.position =
                                    Vector3::new(black_box(tick), 40.0, 0.0);
                            },
                        );
                        black_box(NodeAPI::mesh_instance_surface_on_global_ray(
                            &mut runtime,
                            node_id,
                            black_box(ray_origin),
                            black_box(ray_dir),
                            black_box(100.0),
                        ))
                    })
                },
            );
        }
    }
    group.finish();
}

/// `MeshInstance3D` with a bound `Skeleton3D`, so node queries route through
/// `load_query_node_mesh_data`'s skinning path. `bone_count == 0` exercises
/// the empty-palette early return (skeleton without bones hands the cached
/// mesh `Arc` straight back); a non-zero count pays the full per-query skin
/// rebuild (posed vertices + BVH).
fn build_skinned_mesh_node(
    runtime: &mut Runtime,
    side: usize,
    bone_count: usize,
) -> perro_ids::NodeID {
    use perro_nodes::{Bone3D, MeshInstance3D, Skeleton3D};

    let mut mesh = grid_mesh(side);
    for vertex in &mut mesh.vertices {
        vertex.weights = perro_structs::UnitVector4::new([1.0, 0.0, 0.0, 0.0]);
    }
    let mesh_id = runtime.bench_create_mesh_data(mesh);

    let node_id = NodeAPI::create::<MeshInstance3D>(runtime);
    NodeAPI::with_node_mut::<MeshInstance3D, _, _>(runtime, node_id, |mesh| {
        mesh.mesh = mesh_id;
    });

    let skeleton_id = NodeAPI::create::<Skeleton3D>(runtime);
    NodeAPI::with_node_mut::<Skeleton3D, _, _>(runtime, skeleton_id, |skeleton| {
        skeleton.bones = (0..bone_count)
            .map(|index| {
                let mut bone = Bone3D::new();
                bone.parent = index as i32 - 1;
                bone
            })
            .collect();
        skeleton.refresh_inv_bind_cache();
    });
    runtime.bench_bind_mesh_skeleton(node_id, skeleton_id);
    node_id
}

fn bench_skinned_node_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("mesh_query/skinned_node");
    let side = 16usize;
    for (name, bone_count) in [("palette_16_bones", 16usize), ("empty_palette", 0usize)] {
        let mut runtime = Runtime::new();
        let node_id = build_skinned_mesh_node(&mut runtime, side, bone_count);
        let ray_origin = Vector3::new(side as f32 * 0.5, 10.0, side as f32 * 0.5);
        let ray_dir = Vector3::new(0.0, -1.0, 0.0);

        // Warm the mesh-data and node-snapshot caches so iterations measure
        // the per-query skin (or the empty-palette pass-through), not decode.
        black_box(NodeAPI::mesh_instance_surface_on_global_ray(
            &mut runtime,
            node_id,
            ray_origin,
            ray_dir,
            100.0,
        ));

        group.bench_function(BenchmarkId::new("ray_hit", name), |b| {
            b.iter(|| {
                black_box(NodeAPI::mesh_instance_surface_on_global_ray(
                    &mut runtime,
                    node_id,
                    black_box(ray_origin),
                    black_box(ray_dir),
                    black_box(100.0),
                ))
            })
        });
    }
    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_mesh_query_synthetic(c);
    bench_mesh_query_bvh_runtime_api(c);
    bench_multimesh_node_query(c);
    bench_skinned_node_query(c);
}

criterion_group! {
    name = mesh_query_hotpaths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(mesh_query_hotpaths);

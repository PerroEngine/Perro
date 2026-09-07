//! Public-API fixtures usable unchanged on the audit baseline.
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_ids::TagID;
use perro_nodes::skeleton_3d::Bone3D;
use perro_nodes::{AnimationPlayer, AnimationTree, Node3D, PointLight3D, Skeleton3D};
use perro_nodes::{CollisionShape3D, RigidBody3D, SubView3D};
use perro_resource_api::sub_apis::{NavMesh3D, NavMeshTriangle3D};
use perro_runtime::Runtime;
use perro_runtime_api::sub_apis::{
    NavMeshAPI, NavMeshPathOptions, NavMeshPathStatus, NodeAPI, NodeQuery, QueryBounds, QueryExpr,
    QueryScope,
};
use perro_structs::{BitMask, Vector3};
use std::fmt::Write as _;

const TREE: &str = "[AnimationTree]\nname=\"AuditTree\"\n[/AnimationTree]\n[AnimationSlots]\nBase\n[/AnimationSlots]\n[Output]\ninput=@Base\n[/Output]\n";

fn bones(c: &mut Criterion) {
    let mut group = c.benchmark_group("audit_runtime/tree_bones");
    for count in [60usize, 120] {
        for skeletons in [1usize, 100] {
            let mut runtime = Runtime::new();
            let tree = NodeAPI::create::<AnimationTree>(&mut runtime);
            let mut source = String::from(
                "[Animation]\nname=\"AuditBones\"\nfps=60\n[/Animation]\n[Objects]\nRig=Skeleton3D\n[/Objects]\n",
            );
            for frame in [0, 59] {
                let _ = writeln!(source, "[Frame{frame}]\n@Rig {{");
                for bone in 0..count {
                    let _ = writeln!(source, "bone[{bone}].position=({},1,0)", frame);
                }
                let _ = writeln!(source, "}}\n[/Frame{frame}]");
            }
            let (clip, asset) = runtime.bench_with_script_context(tree, |ctx| {
                (
                    ctx.res.Animations().create_from_bytes(source.as_bytes()),
                    ctx.res.AnimationTrees().create_from_bytes(TREE.as_bytes()),
                )
            });
            assert!(!clip.is_nil() && !asset.is_nil());
            for i in 0..skeletons {
                let skeleton = NodeAPI::create::<Skeleton3D>(&mut runtime);
                let _ = runtime.with_node_mut::<Skeleton3D, _, _>(skeleton, |node| {
                    node.bones.resize(count, Bone3D::new());
                });
                let node = if i == 0 {
                    tree
                } else {
                    NodeAPI::create::<AnimationTree>(&mut runtime)
                };
                let _ = runtime.with_node_mut::<AnimationTree, _, _>(node, |node| {
                    node.set_tree(asset);
                    node.speed = 1.0;
                    node.set_clip_by_index(0, clip);
                    node.set_slot_binding(0, "Rig", skeleton);
                });
            }
            runtime.update(1.0 / 60.0);
            group.bench_function(format!("{count}_bones/{skeletons}_rigs"), |b| {
                b.iter(|| runtime.update(black_box(1.0 / 60.0)))
            });
        }
    }
    group.finish();
}

fn player_bindings(c: &mut Criterion) {
    let mut group = c.benchmark_group("audit_runtime/player_bindings");
    for count in [8usize, 100, 1000] {
        let mut runtime = Runtime::new();
        let player = NodeAPI::create::<AnimationPlayer>(&mut runtime);
        let mut source =
            String::from("[Animation]\nname=\"BindingAudit\"\nfps=60\n[/Animation]\n[Objects]\n");
        for i in 0..count {
            let _ = writeln!(source, "Obj{i}=PointLight3D");
        }
        source.push_str("[/Objects]\n");
        for frame in [0, 59] {
            let _ = writeln!(source, "[Frame{frame}]");
            for i in 0..count {
                let _ = writeln!(source, "@Obj{i} {{ intensity={} }}", frame + 1);
            }
            let _ = writeln!(source, "[/Frame{frame}]");
        }
        let clip = runtime.bench_with_script_context(player, |ctx| {
            ctx.res.Animations().create_from_bytes(source.as_bytes())
        });
        assert!(!clip.is_nil());
        let targets: Vec<_> = (0..count)
            .map(|_| NodeAPI::create::<PointLight3D>(&mut runtime))
            .collect();
        let _ = runtime.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.set_animation(clip);
            node.speed = 1.0;
            for (i, &target) in targets.iter().enumerate() {
                node.set_binding(&format!("Obj{i}"), target);
            }
        });
        runtime.update(1.0 / 60.0);
        group.bench_function(BenchmarkId::from_parameter(count), |b| {
            b.iter(|| runtime.update(black_box(1.0 / 60.0)))
        });
    }
    group.finish();
}

fn physics_worlds(c: &mut Criterion) {
    let mut group = c.benchmark_group("audit_runtime/physics_worlds");
    for worlds in [1usize, 4, 16] {
        let mut runtime = Runtime::new();
        for _ in 0..worlds {
            let view = NodeAPI::create::<SubView3D>(&mut runtime);
            for i in 0..1024 / worlds {
                let body = NodeAPI::create::<RigidBody3D>(&mut runtime);
                let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
                assert!(runtime.reparent(view, body));
                assert!(runtime.reparent(body, shape));
                runtime
                    .with_node_mut::<RigidBody3D, _, _>(body, |node| {
                        node.position.x = i as f32 * 3.0;
                        node.gravity_scale = 0.0;
                        node.can_sleep = false;
                        node.linear_velocity = Vector3::new(1.0, 0.0, 0.0);
                    })
                    .expect("live body");
            }
        }
        for _ in 0..3 {
            runtime.fixed_update(1.0 / 60.0);
        }
        group.bench_function(BenchmarkId::new("awake_1024", worlds), |b| {
            b.iter(|| runtime.fixed_update(black_box(1.0 / 60.0)))
        });
    }
    group.finish();
}

fn strip(cells: usize) -> NavMesh3D {
    let vertices = (0..=cells)
        .flat_map(|x| {
            [
                Vector3::new(x as f32, 0.0, 0.0),
                Vector3::new(x as f32, 0.0, 1.0),
            ]
        })
        .collect();
    let triangles = (0..cells as u32)
        .flat_map(|x| {
            let a = x * 2;
            [[a, a + 2, a + 1], [a + 2, a + 3, a + 1]].map(|vertices| NavMeshTriangle3D {
                vertices,
                layers: BitMask::ALL,
            })
        })
        .collect();
    NavMesh3D {
        vertices,
        triangles,
    }
}

fn navmesh(c: &mut Criterion) {
    let mut group = c.benchmark_group("audit_runtime/navmesh");
    for triangles in [10_000usize, 100_000] {
        let mut runtime = Runtime::new();
        let caller = NodeAPI::create::<Node3D>(&mut runtime);
        let id = runtime.bench_with_script_context(caller, |ctx| {
            ctx.res.NavMeshes().create(strip(triangles / 2))
        });
        assert!(!id.is_nil());
        for (name, end_x) in [("local", 3.8), ("long", triangles as f32 / 2.0 - 0.2)] {
            let start = Vector3::new(0.2, 0.0, 0.2);
            let end = Vector3::new(end_x, 0.0, 0.8);
            let path = runtime.navmesh_find_path_3d(id, start, end, NavMeshPathOptions::default());
            assert_eq!(path.status, NavMeshPathStatus::Complete);
            group.bench_function(BenchmarkId::new(name, triangles), |b| {
                b.iter(|| {
                    black_box(runtime.navmesh_find_path_3d(
                        id,
                        black_box(start),
                        black_box(end),
                        NavMeshPathOptions::default(),
                    ))
                })
            });
        }
    }
    group.finish();
}

fn spatial(c: &mut Criterion) {
    let mut group = c.benchmark_group("audit_runtime/spatial");
    for count in [10_000usize, 100_000] {
        let mut runtime = Runtime::new();
        let tag = TagID::from_string("audit_candidate");
        let mut first = None;
        for _ in 0..count {
            let id = NodeAPI::create::<Node3D>(&mut runtime);
            first.get_or_insert(id);
        }
        let first = first.expect("nonempty fixture");
        assert!(NodeAPI::add_node_tag(&mut runtime, first, tag));
        let within = QueryExpr::Within(QueryBounds::Box3D {
            origin: Vector3::ZERO,
            size: Vector3::new(2.0, 2.0, 2.0),
        });
        for (name, expr) in [
            (
                "one_candidate",
                QueryExpr::All(vec![QueryExpr::Tags(vec![tag]), within.clone()]),
            ),
            ("first_hit", within),
        ] {
            let query = NodeQuery {
                expr: Some(expr),
                scope: QueryScope::Root,
            };
            assert_eq!(runtime.query_first_node(query.as_view()), Some(first));
            group.bench_function(BenchmarkId::new(name, count), |b| {
                b.iter(|| black_box(runtime.query_first_node(black_box(query.as_view()))))
            });
        }
    }
    group.finish();
}

criterion_group!(
    audit_runtime,
    navmesh,
    spatial,
    bones,
    player_bindings,
    physics_worlds
);
criterion_main!(audit_runtime);

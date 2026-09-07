//! Public-API controls for graph sharing and bone collision workload shape.
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_nodes::skeleton_3d::Bone3D;
use perro_nodes::{
    AnimationTree, BoneCollider3D, CollisionShape3D, PhysicsBoneChain3D, Shape3D, Skeleton3D,
};
use perro_runtime::Runtime;
use perro_runtime_api::sub_apis::NodeAPI;
use perro_structs::Vector3;
use std::fmt::Write as _;

fn graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("audit_remaining/graph");
    for depth in [0usize, 4, 8] {
        for shared in [false, true] {
            let mut runtime = Runtime::new();
            let tree = NodeAPI::create::<AnimationTree>(&mut runtime);
            let skeleton = NodeAPI::create::<Skeleton3D>(&mut runtime);
            let mut clip = String::from(
                "[Animation]\nname=\"Graph\"\nfps=60\n[/Animation]\n[Objects]\nRig=Skeleton3D\n[/Objects]\n",
            );
            for frame in [0, 59] {
                writeln!(clip, "[Frame{frame}]\n@Rig {{")
                    .expect("write animation frame into fixture string");
                for bone in 0..60 {
                    writeln!(clip, "bone[{bone}].position=({frame},1,0)")
                        .expect("write bone track into fixture string");
                }
                writeln!(clip, "}}\n[/Frame{frame}]")
                    .expect("close animation frame in fixture string");
            }
            let mut source = String::from("[AnimationSlots]\nBase\n[/AnimationSlots]\n");
            let mut previous = "Base".to_string();
            for index in 0..depth {
                let input = if shared {
                    format!("@{previous}, @{previous}")
                } else {
                    format!("@{previous}")
                };
                writeln!(
                    source,
                    "[N{index}]\n[Blend]\ninputs=[{input}]\nweights=[1,1]\n[/Blend]\n[/N{index}]"
                )
                .expect("write graph node into fixture string");
                previous = format!("N{index}");
            }
            writeln!(source, "[Output]\ninput=@{previous}\n[/Output]")
                .expect("write graph output into fixture string");
            let (clip, asset) = runtime.bench_with_script_context(tree, |ctx| {
                (
                    ctx.res.Animations().create_from_bytes(clip.as_bytes()),
                    ctx.res
                        .AnimationTrees()
                        .create_from_bytes(source.as_bytes()),
                )
            });
            assert!(!clip.is_nil() && !asset.is_nil());
            runtime
                .with_node_mut::<Skeleton3D, _, _>(skeleton, |node| {
                    node.bones.resize(60, Bone3D::new())
                })
                .expect("graph fixture skeleton exists");
            runtime
                .with_node_mut::<AnimationTree, _, _>(tree, |node| {
                    node.set_tree(asset);
                    node.speed = 1.0;
                    node.set_clip_by_index(0, clip);
                    node.set_slot_binding(0, "Rig", skeleton);
                })
                .expect("graph fixture animation tree exists");
            runtime.update(1.0 / 60.0);
            let name = if shared { "diamond" } else { "linear" };
            group.bench_function(BenchmarkId::new(name, depth), |b| {
                b.iter(|| runtime.update(black_box(1.0 / 60.0)))
            });
        }
    }
    group.finish();
}

fn collisions(c: &mut Criterion) {
    let mut group = c.benchmark_group("audit_remaining/bone_collisions");
    for (rig_count, bone_count, count) in [0usize, 16, 256, 2048]
        .map(|count| (16usize, 16i32, count))
        .into_iter()
        .chain([(1usize, 2i32, 16usize)])
    {
        for dense in [false, true] {
            let mut runtime = Runtime::new();
            for rig in 0..rig_count {
                let skeleton = NodeAPI::create::<Skeleton3D>(&mut runtime);
                runtime
                    .with_node_mut::<Skeleton3D, _, _>(skeleton, |node| {
                        node.position.x = rig as f32 * 3.0;
                        node.bones = (0..bone_count)
                            .map(|bone| {
                                let mut out = Bone3D::new();
                                out.parent = bone - 1;
                                out.rest.position.y = if bone == 0 { 0.0 } else { 1.0 };
                                out.pose = out.rest;
                                out
                            })
                            .collect();
                    })
                    .expect("collision fixture skeleton exists");
                let chain = NodeAPI::create::<PhysicsBoneChain3D>(&mut runtime);
                runtime
                    .with_node_mut::<PhysicsBoneChain3D, _, _>(chain, |node| {
                        node.skeleton = skeleton;
                        node.bone_index = bone_count - 1;
                        node.chain_length = bone_count as u32;
                        node.iterations = 3;
                    })
                    .expect("collision fixture bone chain exists");
            }
            for index in 0..count {
                let collider = NodeAPI::create::<BoneCollider3D>(&mut runtime);
                let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
                assert!(runtime.reparent(collider, shape));
                runtime
                    .with_node_mut::<BoneCollider3D, _, _>(collider, |node| {
                        node.position = if dense && rig_count == 1 {
                            Vector3::new(0.1, 1.0, 0.0)
                        } else if dense {
                            Vector3::new((index % 16) as f32 * 3.0 + 0.1, (index % 15) as f32, 0.0)
                        } else {
                            Vector3::new(100.0 + index as f32 * 3.0, 0.0, 0.0)
                        };
                    })
                    .expect("collision fixture bone collider exists");
                runtime
                    .with_node_mut::<CollisionShape3D, _, _>(shape, |node| {
                        node.shape = Shape3D::Cube {
                            size: Vector3::new(1.0, 1.0, 1.0),
                        };
                    })
                    .expect("collision fixture shape exists");
            }
            for _ in 0..3 {
                runtime.fixed_update(1.0 / 60.0);
            }
            let name = if dense { "dense" } else { "sparse" };
            let name = if rig_count == 1 {
                format!("tiny_{name}")
            } else {
                name.to_string()
            };
            group.bench_function(BenchmarkId::new(name, count), |b| {
                b.iter(|| runtime.fixed_update(black_box(1.0 / 60.0)))
            });
        }
    }
    group.finish();
}

criterion_group!(audit_remaining, graph, collisions);
criterion_main!(audit_remaining);

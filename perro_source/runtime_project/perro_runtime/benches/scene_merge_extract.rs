use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use perro_runtime::bench_prepare_merge_extract_scene;
use perro_scene::{Parser, Scene};

fn scene_source(nodes: usize) -> String {
    let mut src = String::with_capacity(nodes * 180);
    src.push_str("$root = @root\n\n[root]\n[Node3D]\n[/Node3D]\n[/root]\n\n");
    src.push_str("[camera]\nparent = @root\n[Camera3D]\nactive = true\n[/Camera3D]\n[/camera]\n\n");
    for i in 0..nodes {
        let x = (i % 256) as f32;
        let z = (i / 256) as f32;
        if i % 4 == 0 {
            src.push_str(&format!(
                "[dense_{i}]\nparent = @root\n[MultiMeshInstance3D]\nmesh = \"res://mesh_{}.glb\"\nsurfaces = [{{ material = \"res://mat_{}.pmat\" }}]\ninstances_grid = {{ columns = 16 rows = 16 spacing = (0.5, 0, 0.5) }}\n[/MultiMeshInstance3D]\n[/dense_{i}]\n\n",
                i % 16,
                i % 8
            ));
        } else {
            src.push_str(&format!(
                "[mesh_{i}]\nparent = @root\n[MeshInstance3D]\nmesh = \"res://mesh_{}.glb\"\nsurfaces = [{{ material = \"res://mat_{}.pmat\" }}]\n[Node3D]\nposition = ({x}, 0, {z})\n[/Node3D]\n[/MeshInstance3D]\n[/mesh_{i}]\n\n",
                i % 16,
                i % 8
            ));
        }
    }
    src
}

fn parse(src: &str) -> Scene {
    Parser::new(src).parse_scene()
}

fn merge_extract(scene: &Scene) -> (usize, usize) {
    bench_prepare_merge_extract_scene(scene).expect("merge + extract")
}

fn bench_merge_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_merge_extract");
    for nodes in [64_usize, 512, 2048, 8192] {
        let src = scene_source(nodes);
        let scene = parse(&src);
        group.throughput(Throughput::Elements(nodes as u64));
        group.bench_with_input(BenchmarkId::from_parameter(nodes), &scene, |b, scene| {
            b.iter(|| black_box(merge_extract(black_box(scene))));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_merge_extract);
criterion_main!(benches);

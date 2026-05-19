use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use perro_runtime::bench_prepare_scene;
use perro_scene::{Parser, Scene};

fn bench_scene_source(nodes: usize) -> String {
    let mut src = String::with_capacity(nodes * 220);
    src.push_str("$root = @node_0\n\n");
    for i in 0..nodes {
        let parent = if i == 0 {
            String::new()
        } else {
            format!("parent = @node_{}\n", (i - 1) / 3)
        };
        let x = (i % 128) as f32;
        let z = (i / 128) as f32;
        if i % 5 == 0 {
            src.push_str(&format!(
                "[node_{i}]\n{parent}[UiPanel]\nposition_ratio = (0.5, 0.5)\nsize_pixels = (96, 32)\nstyle = {{ fill = \"#101820FF\" stroke = \"#FFFFFFFF\" stroke_width = 1 }}\n[/UiPanel]\n[/node_{i}]\n\n"
            ));
        } else if i % 3 == 0 {
            src.push_str(&format!(
                "[node_{i}]\n{parent}[MultiMeshInstance3D]\nmesh = \"res://mesh_{:02}.glb\"\nmaterial = \"res://mat_{:02}.pmat\"\ninstances_grid = {{ columns = 8 rows = 8 spacing = (1, 0, 1) }}\n[/MultiMeshInstance3D]\n[/node_{i}]\n\n",
                i % 16,
                i % 8
            ));
        } else {
            src.push_str(&format!(
                "[node_{i}]\n{parent}[MeshInstance3D]\nmesh = \"res://mesh_{:02}.glb\"\nsurfaces = [{{ material = \"res://mat_{:02}.pmat\" }}]\n[Node3D]\nposition = ({x}, 0, {z})\n[/Node3D]\n[/MeshInstance3D]\n[/node_{i}]\n\n",
                i % 16,
                i % 8
            ));
        }
    }
    src
}

fn parse_scene(src: &str) -> Scene {
    Parser::new(src).parse_scene()
}

fn parse_prepare(src: &str) -> (usize, usize) {
    let scene = parse_scene(src);
    bench_prepare_scene(&scene).expect("prepare scene")
}

fn bench_parse_prepare(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_parse_prepare");
    for nodes in [64_usize, 512, 2048, 8192] {
        let src = bench_scene_source(nodes);
        group.throughput(Throughput::Elements(nodes as u64));
        group.bench_with_input(BenchmarkId::from_parameter(nodes), &src, |b, src| {
            b.iter(|| black_box(parse_prepare(black_box(src))));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parse_prepare);
criterion_main!(benches);

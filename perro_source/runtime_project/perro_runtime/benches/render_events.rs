use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use perro_runtime::BenchSceneSpawner;
use perro_scene::{Parser, Scene};
use std::time::Duration;

// mirror scene_loading bench shape: mixed Node2D/Sprite2D/Camera2D tree.
fn bench_scene_source(nodes: usize) -> String {
    let mut src = String::with_capacity(nodes * 180);
    src.push_str("$root = @node_0\n\n");
    for i in 0..nodes {
        let parent = if i == 0 {
            String::new()
        } else {
            format!("parent = @node_{}\n", (i - 1) / 2)
        };
        let x = (i % 64) as f32;
        let y = (i / 64) as f32;
        if i % 3 == 0 {
            src.push_str(&format!(
                "[node_{i}]\n{parent}[Node2D]\nposition = ({x}, {y})\nscale = (1, 1)\nrotation = 0.0\nvisible = true\n[/Node2D]\n[/node_{i}]\n\n"
            ));
        } else if i % 3 == 1 {
            src.push_str(&format!(
                "[node_{i}]\n{parent}[Sprite2D]\nposition = ({x}, {y})\ntexture = \"res://sprites/sprite_{}.png\"\nz_index = {}\n[/Sprite2D]\n[/node_{i}]\n\n",
                i % 16,
                i % 8
            ));
        } else {
            src.push_str(&format!(
                "[node_{i}]\n{parent}[Camera2D]\nposition = ({x}, {y})\nzoom = (1, 1)\n[/Camera2D]\n[/node_{i}]\n\n"
            ));
        }
    }
    src
}

fn parse_scene(src: &str) -> Scene {
    Parser::new(src).parse_scene()
}

fn spawner_with_nodes(nodes: usize) -> BenchSceneSpawner {
    let scene = parse_scene(&bench_scene_source(nodes));
    let mut spawner = BenchSceneSpawner::new();
    spawner
        .spawn_uncompiled(&scene)
        .expect("bench scene spawn");
    spawner
}

// load storm: EVENTS MaterialLoaded results land in one apply batch.
// sequential = old per-event behavior (invalidation pass + scan flags flushed
// after every event); batched = one flush for the whole batch.
fn bench_material_loaded_storm(c: &mut Criterion) {
    const EVENTS: usize = 64;
    let node_counts = [512_usize, 2048];
    let material_ids: Vec<u64> = (1..=EVENTS as u64).collect();

    let mut group = c.benchmark_group("render_event_material_loaded_storm");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_secs(2));

    for nodes in node_counts {
        let mut spawner = spawner_with_nodes(nodes);
        group.throughput(Throughput::Elements(EVENTS as u64));
        group.bench_with_input(
            BenchmarkId::new("sequential", nodes),
            &material_ids,
            |b, ids| {
                b.iter(|| {
                    spawner.apply_material_loaded_events_sequential(black_box(ids));
                })
            },
        );
        let mut spawner = spawner_with_nodes(nodes);
        group.bench_with_input(
            BenchmarkId::new("batched", nodes),
            &material_ids,
            |b, ids| {
                b.iter(|| {
                    spawner.apply_material_loaded_events_batched(black_box(ids));
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_material_loaded_storm);
criterion_main!(benches);

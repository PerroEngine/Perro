use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use perro_runtime::{bench_prepare_and_merge_scene, bench_prepare_scene};
use perro_scene::{Parser, Scene};
use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct CountingAllocator;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy, Debug)]
struct AllocSample {
    allocs: usize,
    bytes: usize,
}

fn reset_allocs() {
    ALLOCS.store(0, Ordering::Relaxed);
    ALLOC_BYTES.store(0, Ordering::Relaxed);
}

fn read_allocs() -> AllocSample {
    AllocSample {
        allocs: ALLOCS.load(Ordering::Relaxed),
        bytes: ALLOC_BYTES.load(Ordering::Relaxed),
    }
}

fn sample_allocs<T>(f: impl FnOnce() -> T) -> (T, AllocSample) {
    reset_allocs();
    let value = f();
    let sample = read_allocs();
    (value, sample)
}

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

fn prepared_counts(scene: &Scene) -> (usize, usize) {
    bench_prepare_scene(scene).expect("bench scene prepare")
}

fn runtime_node_count(scene: &Scene) -> usize {
    bench_prepare_and_merge_scene(scene).expect("bench scene prepare + merge")
}

fn dynamic_parse_and_prepare(src: &str) -> (usize, usize) {
    let scene = parse_scene(src);
    prepared_counts(&scene)
}

fn dynamic_read_parse_and_prepare(path: &Path) -> (usize, usize) {
    let src = fs::read_to_string(path).expect("read bench scene");
    dynamic_parse_and_prepare(&src)
}

fn bench_dynamic_parse_prepare(c: &mut Criterion) {
    let node_counts = [1_usize, 16, 32, 64, 512, 2048];
    let mut group = c.benchmark_group("scene_dynamic_raw_scn_parse_prepare");

    for nodes in node_counts {
        let src = bench_scene_source(nodes);
        let (counts, allocs) = sample_allocs(|| dynamic_parse_and_prepare(black_box(&src)));
        assert_eq!(counts.0, nodes);
        eprintln!(
            "scene_dynamic_raw_scn_parse_prepare nodes={nodes} bytes={} allocs/op={} alloc_bytes/op={}",
            src.len(),
            allocs.allocs,
            allocs.bytes
        );

        group.throughput(Throughput::Bytes(src.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(nodes), &src, |b, src| {
            b.iter(|| {
                let counts = dynamic_parse_and_prepare(black_box(src));
                black_box(counts)
            })
        });
    }

    group.finish();
}

fn bench_dynamic_fs_read_parse_prepare(c: &mut Criterion) {
    let node_counts = [1_usize, 16, 32, 64, 512, 2048];
    let mut group = c.benchmark_group("scene_dynamic_fs_read_parse_prepare");

    for nodes in node_counts {
        let src = bench_scene_source(nodes);
        let path = std::env::temp_dir().join(format!("perro_scene_loading_bench_{nodes}.scn"));
        fs::write(&path, &src).expect("write bench scene");
        let (counts, allocs) = sample_allocs(|| dynamic_read_parse_and_prepare(black_box(&path)));
        assert_eq!(counts.0, nodes);
        eprintln!(
            "scene_dynamic_fs_read_parse_prepare nodes={nodes} bytes={} allocs/op={} alloc_bytes/op={}",
            src.len(),
            allocs.allocs,
            allocs.bytes
        );

        group.throughput(Throughput::Bytes(src.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(nodes), &path, |b, path| {
            b.iter(|| {
                let counts = dynamic_read_parse_and_prepare(black_box(path));
                black_box(counts)
            })
        });
    }

    group.finish();
}

fn bench_static_prepare(c: &mut Criterion) {
    let node_counts = [1_usize, 16, 32, 64, 512, 2048];
    let mut group = c.benchmark_group("scene_static_parsed_prepare");

    for nodes in node_counts {
        let src = bench_scene_source(nodes);
        let scene = parse_scene(&src);
        let (counts, allocs) = sample_allocs(|| prepared_counts(black_box(&scene)));
        assert_eq!(counts.0, nodes);
        eprintln!(
            "scene_static_parsed_prepare nodes={nodes} allocs/op={} alloc_bytes/op={}",
            allocs.allocs, allocs.bytes
        );

        group.throughput(Throughput::Elements(nodes as u64));
        group.bench_with_input(BenchmarkId::from_parameter(nodes), &scene, |b, scene| {
            b.iter(|| {
                let counts = prepared_counts(black_box(scene));
                black_box(counts)
            })
        });
    }

    group.finish();
}

fn bench_static_prepare_merge(c: &mut Criterion) {
    let node_counts = [1_usize, 16, 32, 64, 512, 2048];
    let mut group = c.benchmark_group("scene_static_parsed_prepare_runtime_build");
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_secs(1));
    group.sample_size(20);

    for nodes in node_counts {
        let src = bench_scene_source(nodes);
        let scene = parse_scene(&src);
        let (node_count, allocs) = sample_allocs(|| runtime_node_count(black_box(&scene)));
        assert_eq!(node_count, nodes + 1);
        eprintln!(
            "scene_static_parsed_prepare_runtime_build nodes={nodes} allocs/op={} alloc_bytes/op={}",
            allocs.allocs, allocs.bytes
        );

        group.throughput(Throughput::Elements(nodes as u64));
        group.bench_with_input(BenchmarkId::from_parameter(nodes), &scene, |b, scene| {
            b.iter(|| {
                let node_count = runtime_node_count(black_box(scene));
                black_box(node_count)
            })
        });
    }

    group.finish();
}

fn bench_scene_loading(c: &mut Criterion) {
    bench_dynamic_fs_read_parse_prepare(c);
    bench_dynamic_parse_prepare(c);
    bench_static_prepare(c);
    bench_static_prepare_merge(c);
}

criterion_group!(benches, bench_scene_loading);
criterion_main!(benches);

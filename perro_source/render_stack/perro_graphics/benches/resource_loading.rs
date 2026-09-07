use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_graphics::{GraphicsBackend, PerroGraphics};
use perro_ids::{MaterialID, MeshID, TextureID};
use perro_render_bridge::{
    Material3D, RenderBridge, RenderCommand, RenderEvent, RenderRequestID, ResourceCommand,
};

#[path = "fixtures/mesh_load.rs"]
mod mesh_load_fixture;
#[path = "fixtures/texture_load.rs"]
mod texture_load_fixture;

fn load_textures_to_completion(graphics: &mut PerroGraphics, count: usize) {
    graphics.submit_many(
        (0..count).map(|i| texture_create(&texture_load_fixture::source(i), i as u64 + 1)),
    );
    let mut events = Vec::new();
    let mut loaded = 0;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while loaded < count {
        graphics.draw_frame();
        graphics.drain_events(&mut events);
        for event in events.drain(..) {
            match event {
                RenderEvent::TextureCreated { .. } => loaded += 1,
                RenderEvent::Failed { reason, .. } => panic!("texture fixture: {reason}"),
                _ => {}
            }
        }
        assert!(std::time::Instant::now() < deadline, "texture load timeout");
        std::thread::yield_now();
    }
}

fn bench_completed_texture_loading(c: &mut Criterion) {
    use std::sync::atomic::Ordering;
    black_box(texture_load_fixture::bytes());
    let mut group = c.benchmark_group("resource_texture_completed");
    for count in [4_usize, 32] {
        let fixture = || {
            PerroGraphics::new().with_static_texture_lookup(texture_load_fixture::texture_lookup)
        };
        texture_load_fixture::LOOKUPS.store(0, Ordering::Relaxed);
        texture_load_fixture::PRIVATE_POOL_LOOKUPS.store(0, Ordering::Relaxed);
        load_textures_to_completion(&mut fixture(), count);
        let lookups = texture_load_fixture::LOOKUPS.load(Ordering::Relaxed);
        let private = texture_load_fixture::PRIVATE_POOL_LOOKUPS.load(Ordering::Relaxed);
        eprintln!(
            "texture_load_work textures={count} edge={} source_lookups={lookups} private_pool_lookups={private} shared_pool_lookups={}",
            texture_load_fixture::EDGE,
            lookups - private
        );
        group.bench_with_input(
            BenchmarkId::new("compressed_ptex", count),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    fixture,
                    |graphics| load_textures_to_completion(graphics, count),
                    // Each backend retains up to 32 MiB of decoded pixels.
                    // Keep one live so timing cannot turn into an RSS stress.
                    BatchSize::PerIteration,
                );
            },
        );
    }
    group.finish();
}

fn load_mesh_to_completion(graphics: &mut PerroGraphics, source: &str, triangles: usize) {
    graphics.submit(mesh_create(source, 1));
    let mut events = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        graphics.draw_frame();
        graphics.drain_events(&mut events);
        for event in events.drain(..) {
            match event {
                RenderEvent::MeshCreated {
                    mesh: Some(mesh), ..
                } => {
                    assert_eq!(mesh.vertices.len(), triangles * 3);
                    black_box(mesh);
                    return;
                }
                RenderEvent::Failed { reason, .. } => panic!("mesh fixture: {reason}"),
                _ => {}
            }
        }
        assert!(std::time::Instant::now() < deadline, "mesh load timeout");
        std::thread::yield_now();
    }
}

fn bench_completed_mesh_loading(c: &mut Criterion) {
    use std::sync::atomic::Ordering;
    let mut group = c.benchmark_group("resource_mesh_completed");
    for (triangles, source) in mesh_load_fixture::SOURCES {
        // Build/compress bytes outside every timed load. Each fresh backend has
        // an empty resource cache; the completion event fences the async decode.
        black_box(mesh_load_fixture::mesh_lookup(perro_ids::string_to_u64(
            source,
        )));
        let fixture =
            || PerroGraphics::new().with_static_mesh_lookup(mesh_load_fixture::mesh_lookup);
        mesh_load_fixture::LOOKUPS.store(0, Ordering::Relaxed);
        load_mesh_to_completion(&mut fixture(), source, triangles);
        eprintln!(
            "mesh_load_work triangles={triangles} source_lookups={}",
            mesh_load_fixture::LOOKUPS.load(Ordering::Relaxed)
        );
        group.bench_with_input(
            BenchmarkId::new("compressed_pmesh", triangles),
            &triangles,
            |b, &triangles| {
                b.iter_batched_ref(
                    fixture,
                    |graphics| load_mesh_to_completion(graphics, source, triangles),
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

fn texture_create(source: &str, request: u64) -> RenderCommand {
    RenderCommand::Resource(Box::new(ResourceCommand::CreateTexture {
        request: RenderRequestID::new(request),
        id: TextureID::nil(),
        source: source.to_string(),
        reserved: false,
    }))
}

fn mesh_create(source: &str, request: u64) -> RenderCommand {
    RenderCommand::Resource(Box::new(ResourceCommand::CreateMesh {
        request: RenderRequestID::new(request),
        id: MeshID::nil(),
        source: source.to_string(),
        reserved: false,
    }))
}

fn material_create(source: &str, request: u64) -> RenderCommand {
    RenderCommand::Resource(Box::new(ResourceCommand::CreateMaterial {
        request: RenderRequestID::new(request),
        id: MaterialID::nil(),
        material: Material3D::default().into(),
        source: Some(source.to_string()),
        reserved: false,
    }))
}

fn duplicate_texture_commands(count: usize) -> Vec<RenderCommand> {
    (0..count)
        .map(|i| texture_create("__default__", i as u64 + 1))
        .collect()
}

fn unique_texture_commands(count: usize) -> Vec<RenderCommand> {
    (0..count)
        .map(|i| texture_create(&format!("res://bench/missing_{i}.png"), i as u64 + 1))
        .collect()
}

fn duplicate_mesh_commands(count: usize) -> Vec<RenderCommand> {
    (0..count)
        .map(|i| mesh_create("res://bench/missing.glb:mesh[0]", i as u64 + 1))
        .collect()
}

fn duplicate_material_commands(count: usize) -> Vec<RenderCommand> {
    (0..count)
        .map(|i| material_create("res://bench/shared.pmat", i as u64 + 1))
        .collect()
}

fn process_once(commands: Vec<RenderCommand>) -> usize {
    let mut graphics = PerroGraphics::new();
    let mut events = Vec::<RenderEvent>::new();
    graphics.submit_many(commands);
    graphics.draw_frame();
    graphics.drain_events(&mut events);
    black_box(events.len())
}

fn bench_resource_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("resource_loading");
    for count in [1_usize, 64, 512, 2_048] {
        group.bench_with_input(
            BenchmarkId::new("duplicate_texture_pending", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || duplicate_texture_commands(count),
                    process_once,
                    BatchSize::SmallInput,
                );
            },
        );
        group.bench_with_input(
            BenchmarkId::new("unique_texture_missing", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || unique_texture_commands(count),
                    process_once,
                    BatchSize::SmallInput,
                );
            },
        );
        group.bench_with_input(
            BenchmarkId::new("duplicate_mesh_pending", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || duplicate_mesh_commands(count),
                    process_once,
                    BatchSize::SmallInput,
                );
            },
        );
        group.bench_with_input(
            BenchmarkId::new("duplicate_material_cached", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || duplicate_material_commands(count),
                    process_once,
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_resource_loading,
    bench_completed_mesh_loading,
    bench_completed_texture_loading
);
criterion_main!(benches);

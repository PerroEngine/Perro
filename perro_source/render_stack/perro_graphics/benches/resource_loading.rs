use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_graphics::{GraphicsBackend, PerroGraphics};
use perro_ids::{MaterialID, MeshID, TextureID};
use perro_render_bridge::{
    Material3D, RenderBridge, RenderCommand, RenderEvent, RenderRequestID, ResourceCommand,
};

fn texture_create(source: &str, request: u64) -> RenderCommand {
    RenderCommand::Resource(ResourceCommand::CreateTexture {
        request: RenderRequestID(request),
        id: TextureID::nil(),
        source: source.to_string(),
        reserved: false,
    })
}

fn mesh_create(source: &str, request: u64) -> RenderCommand {
    RenderCommand::Resource(ResourceCommand::CreateMesh {
        request: RenderRequestID(request),
        id: MeshID::nil(),
        source: source.to_string(),
        reserved: false,
    })
}

fn material_create(source: &str, request: u64) -> RenderCommand {
    RenderCommand::Resource(ResourceCommand::CreateMaterial {
        request: RenderRequestID(request),
        id: MaterialID::nil(),
        material: Material3D::default(),
        source: Some(source.to_string()),
        reserved: false,
    })
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

criterion_group!(benches, bench_resource_loading);
criterion_main!(benches);

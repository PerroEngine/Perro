use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use perro_graphics::{GraphicsBackend, PerroGraphics};
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_render_bridge::{
    Command2D, Command3D, LODOptions3D, MeshBlendOptions3D, MeshSurfaceBinding3D, Rect2DCommand,
    RenderBridge, RenderCommand,
};
use perro_structs::Color;
use std::sync::Arc;

fn rect_command(i: u32) -> RenderCommand {
    RenderCommand::TwoD(Command2D::UpsertRect {
        node: NodeID::from_parts(i + 1, 0),
        rect: Rect2DCommand {
            center: [(i % 256) as f32, (i / 256) as f32],
            size: [2.0, 2.0],
            color: Color::new(0.2, 0.7, 1.0, 1.0),
            z_index: i as i32,
        },
    })
}

fn draw_command(i: u32) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::Draw {
        node: NodeID::from_parts(1_000_000 + i, 0),
        mesh: MeshID::from_parts(1, 0),
        surfaces: Arc::from([MeshSurfaceBinding3D {
            material: Some(MaterialID::from_parts(1, 0)),
            overrides: Arc::from([]),
            modulate: Color::WHITE,
        }]),
        model: [
            [1.0, 0.0, 0.0, (i % 256) as f32],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, (i / 256) as f32],
            [0.0, 0.0, 0.0, 1.0],
        ],
        skeleton: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: MeshBlendOptions3D::default(),
    }))
}

fn mixed_commands(count: u32) -> Vec<RenderCommand> {
    (0..count)
        .map(|i| {
            if i % 2 == 0 {
                rect_command(i)
            } else {
                draw_command(i)
            }
        })
        .collect()
}

fn bench_command_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("command_throughput");
    for count in [1_024_u32, 8_192, 32_768] {
        let commands = mixed_commands(count);
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &commands, |b, commands| {
            b.iter_batched(
                || (PerroGraphics::new(), commands.clone()),
                |(mut graphics, commands)| {
                    graphics.submit_many(commands);
                    graphics.draw_frame();
                    black_box(graphics);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_command_throughput);
criterion_main!(benches);

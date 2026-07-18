use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use perro_graphics::ui::renderer::UiRenderer;
use perro_ids::NodeID;
use perro_render_bridge::{UiCommand, UiCornerRadiiState, UiRectState, UiTextAlignState};
use perro_structs::Color;
use std::sync::Arc;

const VIEWPORT: [f32; 2] = [1920.0, 1080.0];

fn label_command(index: usize, camera_step: f32, text: Arc<str>) -> UiCommand {
    let x = (index % 40) as f32 * 0.035 - 0.7 + camera_step;
    let y = (index / 40) as f32 * 0.04 - 0.5;
    UiCommand::UpsertLabel {
        node: NodeID::from_parts(index as u32 + 1, 0),
        rect: UiRectState {
            center: [0.0, 0.0],
            size: [160.0, 32.0],
            pivot: [0.5, 0.5],
            rotation_radians: 0.0,
            z_index: 0,
        },
        clip_rect: [0.0, 0.0, VIEWPORT[0], VIEWPORT[1]],
        text,
        color: Color::WHITE,
        font_size: 20.0,
        font: perro_ui::UiFont::Default,
        wrap_width: Some(160.0),
        h_align: UiTextAlignState::Center,
        v_align: UiTextAlignState::Center,
        backdrop_color: Color::TRANSPARENT,
        corner_radii: UiCornerRadiiState::default(),
        padding: [0.0; 4],
        projected_quad: Some([
            [x - 0.03, y + 0.015, 0.5, 1.0],
            [x + 0.03, y + 0.015, 0.5, 1.0],
            [x + 0.03, y - 0.015, 0.5, 1.0],
            [x - 0.03, y - 0.015, 0.5, 1.0],
        ]),
        depth_test: true,
        fit_content: true,
    }
}

fn built_renderer(count: usize) -> UiRenderer {
    let mut renderer = UiRenderer::new();
    for index in 0..count {
        renderer.submit(label_command(
            index,
            0.0,
            Arc::from(format!("Label {index} value 0")),
        ));
    }
    let paint = renderer.prepare_paint(VIEWPORT);
    black_box(paint.primitives.len());
    renderer
}

fn bench_label3d_ui(c: &mut Criterion) {
    let mut group = c.benchmark_group("label3d_ui");
    group.sample_size(20);
    for count in [1usize, 100, 1_000] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("spawn_shape_tess", count),
            &count,
            |b, _| {
                b.iter_batched(
                    UiRenderer::new,
                    |mut renderer| {
                        for index in 0..count {
                            renderer.submit(label_command(
                                index,
                                0.0,
                                Arc::from(format!("Label {index} value 0")),
                            ));
                        }
                        black_box(renderer.prepare_paint(VIEWPORT).primitives.len())
                    },
                    BatchSize::SmallInput,
                )
            },
        );

        let mut static_renderer = built_renderer(count);
        group.bench_with_input(BenchmarkId::new("static_cached", count), &count, |b, _| {
            b.iter(|| black_box(static_renderer.prepare_paint(VIEWPORT).primitives.len()))
        });

        let mut moving_renderer = built_renderer(count);
        let moving_texts: Vec<Arc<str>> = (0..count)
            .map(|index| Arc::from(format!("Label {index} value 0")))
            .collect();
        let mut camera_step = 0.0f32;
        group.bench_with_input(
            BenchmarkId::new("projection_only", count),
            &count,
            |b, _| {
                b.iter(|| {
                    camera_step = if camera_step == 0.0 { 0.0001 } else { 0.0 };
                    for (index, text) in moving_texts.iter().enumerate() {
                        moving_renderer.submit(label_command(index, camera_step, Arc::clone(text)));
                    }
                    black_box(moving_renderer.prepare_paint(VIEWPORT).primitives.len())
                })
            },
        );

        let mut text_renderer = built_renderer(count);
        let text_values: [Vec<Arc<str>>; 2] = std::array::from_fn(|step| {
            (0..count)
                .map(|index| Arc::from(format!("Label {index} value {step}")))
                .collect()
        });
        let mut text_step = 0usize;
        group.bench_with_input(BenchmarkId::new("text_change", count), &count, |b, _| {
            b.iter(|| {
                text_step ^= 1;
                for (index, text) in text_values[text_step].iter().enumerate() {
                    text_renderer.submit(label_command(index, 0.0, Arc::clone(text)));
                }
                black_box(text_renderer.prepare_paint(VIEWPORT).primitives.len())
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_label3d_ui);
criterion_main!(benches);

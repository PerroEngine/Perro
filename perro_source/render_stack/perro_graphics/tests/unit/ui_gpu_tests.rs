//! Retained UI supersample raster + UI GC decay.
//!
//! Runs the real `GpuUi::prepare` / `GpuUi::render_pass` against a headless
//! wgpu device; skipped with a note when no adapter is available.
use super::*;
use crate::gpu_shrink::SHRINK_LOW_TICKS;
use epaint::{Color32, Mesh, Rect, Vertex, pos2};

const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const VIEWPORT: [u32; 2] = [128, 96];

async fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .ok()?;
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("perro_ui_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

fn output_view(device: &wgpu::Device) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_ui_test_output"),
        size: wgpu::Extent3d {
            width: VIEWPORT[0],
            height: VIEWPORT[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUTPUT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn test_vertex(x: f32, y: f32) -> Vertex {
    Vertex {
        pos: pos2(x, y),
        uv: pos2(0.0, 0.0),
        color: Color32::WHITE,
    }
}

fn quad(x: f32) -> Arc<ClippedPrimitive> {
    let mut mesh = Mesh::with_texture(TextureId::default());
    mesh.vertices = vec![
        test_vertex(x, 0.0),
        test_vertex(x + 8.0, 0.0),
        test_vertex(x + 8.0, 8.0),
    ];
    mesh.indices = vec![0, 1, 2];
    Arc::new(ClippedPrimitive {
        clip_rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(64.0, 64.0)),
        primitive: Primitive::Mesh(mesh),
    })
}

fn cycle(
    ui: &mut GpuUi,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &wgpu::TextureView,
    primitives: &[Arc<ClippedPrimitive>],
    revision: u64,
) {
    let resources = ResourceStore::new();
    let mut shared_textures = SharedTextureStore::default();
    let textures_delta = TexturesDelta::default();
    ui.prepare(
        device,
        queue,
        UiPrepareInput {
            resources: &resources,
            shared_textures: &mut shared_textures,
            viewport: VIEWPORT,
            primitives,
            primitive_depths: &[],
            textures_delta: &textures_delta,
            texture_size: [0, 0],
            revision,
            static_texture_lookup: None,
        },
    );
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perro_ui_test_encoder"),
    });
    ui.render_pass(device, &mut encoder, view, VIEWPORT, None);
    queue.submit(Some(encoder.finish()));
}

#[test]
fn unchanged_ui_skips_the_supersample_raster() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip ui retained raster test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let view = output_view(&device);
        let primitives = [quad(0.0), quad(16.0)];

        cycle(&mut ui, &device, &queue, &view, &primitives, 1);
        assert_eq!(ui.ui_supersample_redraws(), 1);
        assert_eq!(ui.ui_supersample_composites(), 1);

        // Same primitives, new revision: the geometry rebuild is already
        // skipped by the signature; the raster must be skipped too.
        cycle(&mut ui, &device, &queue, &view, &primitives, 2);
        assert_eq!(ui.ui_supersample_redraws(), 1);
        assert_eq!(ui.ui_supersample_composites(), 2);
    });
}

#[test]
fn changed_ui_redraws_the_supersample_raster() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip ui raster invalidation test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let view = output_view(&device);

        cycle(&mut ui, &device, &queue, &view, &[quad(0.0)], 1);
        assert_eq!(ui.ui_supersample_redraws(), 1);

        // Re-tessellated content: a fresh primitive Arc moves the signature.
        cycle(&mut ui, &device, &queue, &view, &[quad(24.0)], 2);
        assert_eq!(ui.ui_supersample_redraws(), 2);

        // A texture delta mutates atlas pixels under an unchanged signature.
        let unchanged = quad(24.0);
        let unchanged = std::slice::from_ref(&unchanged);
        cycle(&mut ui, &device, &queue, &view, unchanged, 3);
        let retained = ui.ui_supersample_redraws();
        ui.invalidate_image_texture(perro_ids::TextureID::from_u64(7));
        cycle(&mut ui, &device, &queue, &view, unchanged, 4);
        assert_eq!(ui.ui_supersample_redraws(), retained + 1);
    });
}

#[test]
fn shrink_tick_decays_buffers_and_releases_the_target() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip ui shrink tick test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let view = output_view(&device);
        let heavy: Vec<Arc<ClippedPrimitive>> = (0..400)
            .map(|index| quad((index % 40) as f32))
            .collect();

        cycle(&mut ui, &device, &queue, &view, &heavy, 1);
        let [vertex_bytes, index_bytes] = ui.mesh_buffer_capacity_bytes();
        let [vertex_mirror, index_mirror] = ui.mesh_mirror_capacity();
        assert!(vertex_bytes > UI_MIN_VERTEX_BYTES as u64);
        assert!(index_bytes > UI_MIN_INDEX_BYTES as u64);
        assert!(ui.supersample_target_allocated());

        // UI goes empty: capacities must decay and the idle target release.
        ui.clear();
        for _ in 0..(SHRINK_LOW_TICKS + UI_TARGET_IDLE_RELEASE_TICKS + 1) {
            ui.shrink_tick(&device, &queue);
        }
        let [shrunk_vertex_bytes, shrunk_index_bytes] = ui.mesh_buffer_capacity_bytes();
        let [shrunk_vertex_mirror, shrunk_index_mirror] = ui.mesh_mirror_capacity();
        assert!(shrunk_vertex_bytes < vertex_bytes);
        assert!(shrunk_index_bytes < index_bytes);
        assert!(shrunk_vertex_mirror < vertex_mirror);
        assert!(shrunk_index_mirror < index_mirror);
        assert!(!ui.supersample_target_allocated());

        // Recreating the target forces a redraw of the retained raster.
        let redraws = ui.ui_supersample_redraws();
        cycle(&mut ui, &device, &queue, &view, &heavy, 2);
        assert_eq!(ui.ui_supersample_redraws(), redraws + 1);
        assert!(ui.supersample_target_allocated());
    });
}

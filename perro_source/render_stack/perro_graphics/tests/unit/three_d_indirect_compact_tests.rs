//! GPU indirect-command compaction (`multi_draw_indexed_indirect_count` path).
//!
//! Two halves:
//! * pure CPU checks that `plan_indirect_runs` segments the main pass into the
//!   same runs the render loop draws, and
//! * a headless wgpu cull+compact round trip that reads the compacted buffer
//!   and the per-run counts back and compares them against a CPU reference.
//!
//! GPU tests skip with a note when no adapter is available.
use super::*;
use crate::three_d::gpu::culling::plan_indirect_runs;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

// ---------------------------------------------------------------- CPU: runs

fn planning_batch(index_start: u32, state_key: u64, texture_slot: u32) -> DrawBatch {
    let material_texture_key = MaterialTextureKey::from_base(texture_slot);
    DrawBatch {
        state_key,
        render_state: render_state_key(
            state_key,
            material_texture_key.state_hash(),
            index_start,
            0,
            false,
            0,
            false,
        ),
        mesh: MeshRange {
            index_start,
            index_count: 12,
            base_vertex: 0,
        },
        instance_start: 0,
        instance_count: 1,
        path: RenderPath3D::Rigid,
        packed_lod: false,
        double_sided: false,
        material_kind: MaterialPipelineKind::Standard,
        alpha_mode: 0,
        draw_on_top: false,
        base_color_texture_slot: texture_slot,
        material_texture_key,
        local_center: [0.0, 0.0, 0.0],
        local_radius: 1.0,
        occlusion_query: None,
        disable_hiz_occlusion: false,
        casts_shadows: true,
        receives_shadows: true,
        mesh_blend: false,
        mesh_blend_screen: false,
        mesh_blend_params: 0,
        mesh_blend_params_ext: 0,
        mesh_blend_depth: false,
        blend_layers: 0,
        blend_mask: 0,
        order_index: index_start,
    }
}

fn plan(batches: &[DrawBatch], groups: [&[usize]; 3]) -> (Vec<(u32, u32)>, [Range<usize>; 3]) {
    let mut out = Vec::new();
    let mut ranges = [0..0, 0..0, 0..0];
    plan_indirect_runs(batches, groups, &mut out, &mut ranges);
    (
        out.into_iter().map(|run| (run.start, run.len)).collect(),
        ranges,
    )
}

#[test]
fn run_plan_coalesces_same_state_and_breaks_on_state_texture_and_gaps() {
    // 0,1 share state+texture; 2 changes pipeline state; 3 changes texture;
    // 4 matches 3 (joins it); 6 is non-contiguous with 4.
    let batches = vec![
        planning_batch(0, 10, 0),
        planning_batch(10, 10, 0),
        planning_batch(20, 11, 0),
        planning_batch(30, 11, 1),
        planning_batch(40, 11, 1),
        planning_batch(50, 11, 1),
        planning_batch(60, 11, 1),
    ];
    let opaque = [0usize, 1, 2, 3, 4, 6];
    let (runs, ranges) = plan(&batches, [&opaque, &[], &[]]);
    assert_eq!(runs, vec![(0, 2), (2, 1), (3, 2), (6, 1)]);
    assert_eq!(ranges[0], 0..4);
    assert_eq!(ranges[1], 4..4);
    assert_eq!(ranges[2], 4..4);
}

#[test]
fn run_plan_excludes_occlusion_query_batches_and_splits_around_them() {
    let mut batches = vec![
        planning_batch(0, 10, 0),
        planning_batch(10, 10, 0),
        planning_batch(20, 10, 0),
        planning_batch(30, 10, 0),
    ];
    batches[1].occlusion_query = Some(3);
    let opaque = [0usize, 1, 2, 3];
    let (runs, _) = plan(&batches, [&opaque, &[], &[]]);
    // Batch 1 draws on its own inside begin/end_occlusion_query, so it is not
    // part of any compacted run and it breaks the run around it.
    assert_eq!(runs, vec![(0, 1), (2, 2)]);
}

#[test]
fn run_plan_restarts_state_tracking_per_group() {
    // Same state across all three groups; the render loop resets its tracking
    // between groups, so a group boundary always opens a fresh run.
    let batches: Vec<DrawBatch> = (0..6).map(|i| planning_batch(i * 10, 10, 0)).collect();
    let opaque = [0usize, 1];
    let alpha = [2usize, 3];
    let overlay = [4usize, 5];
    let (runs, ranges) = plan(&batches, [&opaque, &alpha, &overlay]);
    assert_eq!(runs, vec![(0, 2), (2, 2), (4, 2)]);
    assert_eq!(ranges, [0..1, 1..2, 2..3]);
}

#[test]
fn run_plan_is_empty_without_batches() {
    let (runs, ranges) = plan(&[], [&[], &[], &[]]);
    assert!(runs.is_empty());
    assert_eq!(ranges, [0..0, 0..0, 0..0]);
}

// ---------------------------------------------------------------- GPU setup

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
            label: Some("perro_indirect_compact_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

fn new_gpu_3d(device: &wgpu::Device, queue: &wgpu::Queue, count_enabled: bool) -> Gpu3D {
    let cache = PipelineRegistryCache::new();
    let pipelines = cache.get_or_create(device, COLOR_FORMAT, 1);
    Gpu3D::new(
        device,
        queue,
        COLOR_FORMAT,
        Gpu3DConfig {
            sample_count: 1,
            width: 64,
            height: 64,
            meshlets_enabled: false,
            dev_meshlets: false,
            meshlet_debug_view: false,
            occlusion_culling: crate::OcclusionCullingMode::Off,
            ssao: crate::SsaoQuality::Off,
            indirect_first_instance_enabled: count_enabled,
            multi_draw_indirect_enabled: count_enabled,
            multi_draw_indirect_count_enabled: count_enabled,
            texture_filter: TextureFilterMode::default(),
            shader_variant_mode: crate::ShaderVariantMode::Generic,
            shadow_pcf_high: false,
        },
        pipelines,
    )
}

fn read_buffer(device: &wgpu::Device, queue: &wgpu::Queue, src: &wgpu::Buffer, len: u64) -> Vec<u8> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_indirect_compact_test_readback"),
        size: len,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perro_indirect_compact_test_copy"),
    });
    encoder.copy_buffer_to_buffer(src, 0, &staging, 0, len);
    queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    rx.recv()
        .expect("map callback")
        .expect("indirect readback map");
    let bytes = slice
        .get_mapped_range()
        .expect("mapped indirect readback range")
        .to_vec();
    staging.unmap();
    bytes
}

/// Cull-item pair placing a batch either at the origin (inside the test
/// frustum) or far outside it, so the real frustum-cull shader decides.
fn cull_rows(visible: bool) -> (FrustumCullStaticGpu, FrustumCullDynamicGpu) {
    let x = if visible { 0.0 } else { 1.0e6 };
    (
        FrustumCullStaticGpu {
            local_center_radius: [0.0, 0.0, 0.0, 1.0],
            cull_flags: [0; 4],
        },
        FrustumCullDynamicGpu {
            model_0: [1.0, 0.0, 0.0, 0.0],
            model_1: [0.0, 1.0, 0.0, 0.0],
            model_2: [0.0, 0.0, 1.0, 0.0],
            model_3: [x, 0.0, 0.0, 1.0],
        },
    )
}

/// Axis-aligned box frustum of half-extent 100 around the origin. Inward
/// normals with `dot(n, p) + w >= -radius` marking a sphere visible.
fn box_frustum_planes() -> [[f32; 4]; 6] {
    [
        [1.0, 0.0, 0.0, 100.0],
        [-1.0, 0.0, 0.0, 100.0],
        [0.0, 1.0, 0.0, 100.0],
        [0.0, -1.0, 0.0, 100.0],
        [0.0, 0.0, 1.0, 100.0],
        [0.0, 0.0, -1.0, 100.0],
    ]
}

struct CompactResult {
    commands: Vec<DrawIndexedIndirectGpu>,
    counts: Vec<u32>,
}

/// Run the real frustum-cull compute over `visibility`, then the real
/// compaction compute over `runs`, and read both outputs back.
fn cull_and_compact(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    gpu: &mut Gpu3D,
    visibility: &[bool],
    runs: &[(u32, u32)],
) -> CompactResult {
    let count = visibility.len();
    gpu.ensure_frustum_cull_capacity(device, count.max(1));

    let commands: Vec<DrawIndexedIndirectGpu> = (0..count)
        .map(|i| DrawIndexedIndirectGpu {
            index_count: 6,
            // Distinct per slot so a mis-ordered compaction is detectable.
            instance_count: 1 + i as u32,
            first_index: i as u32 * 6,
            base_vertex: i as i32,
            first_instance: i as u32,
        })
        .collect();
    queue.write_buffer(&gpu.indirect_buffer, 0, bytemuck::cast_slice(&commands));

    let (statics, dynamics): (Vec<_>, Vec<_>) = visibility.iter().map(|v| cull_rows(*v)).unzip();
    queue.write_buffer(
        &gpu.frustum_cull_static_buffer,
        0,
        bytemuck::cast_slice(&statics),
    );
    queue.write_buffer(
        &gpu.frustum_cull_dynamic_buffer,
        0,
        bytemuck::cast_slice(&dynamics),
    );
    queue.write_buffer(
        &gpu.frustum_cull_params_buffer,
        0,
        bytemuck::bytes_of(&FrustumCullParamsGpu {
            planes: box_frustum_planes(),
            draw_count: count as u32,
            _pad: [0; 3],
        }),
    );
    // The cull params cache would otherwise skip a repeat upload.
    gpu.last_frustum_params = None;

    gpu.indirect_run_plan.clear();
    gpu.indirect_run_plan.extend(
        runs.iter()
            .map(|&(start, len)| IndirectRunGpu { start, len }),
    );
    gpu.indirect_run_plan_groups = [0..gpu.indirect_run_plan.len(), 0..0, 0..0];

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perro_indirect_compact_test"),
    });
    {
        let mut cull = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("perro_indirect_compact_test_cull"),
            timestamp_writes: None,
        });
        cull.set_pipeline(&gpu.frustum_cull_pipeline);
        cull.set_bind_group(0, &gpu.frustum_cull_bind_group, &[]);
        cull.dispatch_workgroups((count as u32).div_ceil(FRUSTUM_CULL_WORKGROUP_SIZE), 1, 1);
    }
    gpu.encode_indirect_compaction(queue, &mut encoder);
    queue.submit([encoder.finish()]);

    let stride = std::mem::size_of::<DrawIndexedIndirectGpu>();
    let command_bytes = read_buffer(
        device,
        queue,
        &gpu.indirect_compact_buffer,
        (count * stride) as u64,
    );
    let count_bytes = read_buffer(
        device,
        queue,
        &gpu.indirect_count_buffer,
        (count * std::mem::size_of::<u32>()) as u64,
    );
    CompactResult {
        commands: bytemuck::cast_slice::<u8, DrawIndexedIndirectGpu>(&command_bytes).to_vec(),
        counts: bytemuck::cast_slice::<u8, u32>(&count_bytes).to_vec(),
    }
}

/// CPU reference: survivors of each run, in order, keyed by run start.
fn reference(visibility: &[bool], runs: &[(u32, u32)]) -> Vec<(u32, Vec<u32>)> {
    runs.iter()
        .map(|&(start, len)| {
            let survivors = (start..start + len)
                .filter(|&i| visibility[i as usize])
                // first_instance == source slot index, so it identifies the
                // command that survived.
                .collect();
            (start, survivors)
        })
        .collect()
}

fn assert_matches_reference(result: &CompactResult, visibility: &[bool], runs: &[(u32, u32)]) {
    for (start, expected) in reference(visibility, runs) {
        let actual_count = result.counts[start as usize];
        assert_eq!(
            actual_count as usize,
            expected.len(),
            "run at {start}: survivor count"
        );
        for (offset, source_slot) in expected.iter().enumerate() {
            let command = result.commands[start as usize + offset];
            assert_eq!(
                command.first_instance, *source_slot,
                "run at {start}: slot {offset} identity"
            );
            assert_eq!(
                command.instance_count,
                1 + *source_slot,
                "run at {start}: slot {offset} instance count"
            );
            assert_eq!(command.first_index, *source_slot * 6);
            assert_eq!(command.base_vertex, *source_slot as i32);
        }
    }
}

#[test]
fn cull_then_compact_matches_cpu_reference() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip indirect compaction test: no wgpu adapter");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue, true);

        // Case 1: nothing culled.
        let visibility = vec![true; 24];
        let runs = [(0u32, 10u32), (10, 14)];
        let result = cull_and_compact(&device, &queue, &mut gpu, &visibility, &runs);
        assert_matches_reference(&result, &visibility, &runs);
        assert_eq!(result.counts[0], 10);
        assert_eq!(result.counts[10], 14);

        // Case 2: everything culled.
        let visibility = vec![false; 24];
        let result = cull_and_compact(&device, &queue, &mut gpu, &visibility, &runs);
        assert_matches_reference(&result, &visibility, &runs);
        assert_eq!(result.counts[0], 0);
        assert_eq!(result.counts[10], 0);

        // Case 3: interleaved survivors, including a run that starts and one
        // that ends on a culled slot.
        let visibility: Vec<bool> = (0..24).map(|i| i % 3 != 0).collect();
        let result = cull_and_compact(&device, &queue, &mut gpu, &visibility, &runs);
        assert_matches_reference(&result, &visibility, &runs);

        // Case 4: single-command runs on both sides of the visibility split.
        let visibility = vec![true, false, true];
        let runs = [(0u32, 1u32), (1, 1), (2, 1)];
        let result = cull_and_compact(&device, &queue, &mut gpu, &visibility, &runs);
        assert_matches_reference(&result, &visibility, &runs);
        assert_eq!(result.counts[0], 1);
        assert_eq!(result.counts[1], 0);
        assert_eq!(result.counts[2], 1);
    });
}

#[test]
fn compaction_preserves_order_across_workgroup_chunks() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip indirect compaction chunk test: no wgpu adapter");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue, true);
        // One run far longer than the 64-wide workgroup so the running base
        // carried between chunks is exercised, with an irregular survivor
        // pattern that also lands survivors on chunk boundaries.
        let total = 300usize;
        let visibility: Vec<bool> = (0..total).map(|i| i % 7 != 0 && i % 5 != 1).collect();
        let runs = [(0u32, total as u32)];
        let result = cull_and_compact(&device, &queue, &mut gpu, &visibility, &runs);
        assert_matches_reference(&result, &visibility, &runs);

        let expected = visibility.iter().filter(|v| **v).count() as u32;
        assert_eq!(result.counts[0], expected);
        // Survivors keep source order (alpha / overlay groups depend on it).
        let ordered: Vec<u32> = result.commands[..expected as usize]
            .iter()
            .map(|c| c.first_instance)
            .collect();
        let mut sorted = ordered.clone();
        sorted.sort_unstable();
        assert_eq!(ordered, sorted);
    });
}

#[test]
fn heavily_culled_scene_cuts_submitted_commands() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip indirect compaction work-reduction test: no wgpu adapter");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue, true);
        // 512 batches, 70% culled, split into 8 state runs of 64.
        let total = 512usize;
        let visibility: Vec<bool> = (0..total).map(|i| i % 10 >= 7).collect();
        let runs: Vec<(u32, u32)> = (0..8).map(|r| (r * 64, 64)).collect();
        let result = cull_and_compact(&device, &queue, &mut gpu, &visibility, &runs);
        assert_matches_reference(&result, &visibility, &runs);

        let max_count: u32 = runs.iter().map(|(_, len)| *len).sum();
        let actual: u32 = runs.iter().map(|(start, _)| result.counts[*start as usize]).sum();
        let expected_visible = visibility.iter().filter(|v| **v).count() as u32;
        assert_eq!(actual, expected_visible);
        assert_eq!(max_count, total as u32);
        eprintln!(
            "[indirect-count] runs={} max_count={max_count} actual={actual} \
             submitted_commands_saved={} ({:.0}% culled)",
            runs.len(),
            max_count - actual,
            100.0 * f64::from(max_count - actual) / f64::from(max_count),
        );
        // The whole point: the GPU frontend walks `actual`, not `max_count`.
        assert!(actual * 3 < max_count);
    });
}

// ------------------------------------------------- GPU: real count-draw call

const COUNT_DRAW_WGSL: &str = "\
@vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4<f32>(p[vi % 3u], 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(1.0, 0.0, 0.0, 1.0); }
";

/// Device with the features the count path needs, or `None` to skip.
async fn count_capable_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let needed =
        wgpu::Features::MULTI_DRAW_INDIRECT_COUNT | wgpu::Features::INDIRECT_FIRST_INSTANCE;
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .ok()?;
    if !adapter.features().contains(needed) {
        return None;
    }
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("perro_indirect_count_draw_test_device"),
            required_features: needed,
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

/// End-to-end: compact on the GPU, then issue the actual
/// `multi_draw_indexed_indirect_count` the main pass issues, with the same
/// buffers / offsets / max_count. Guards against validation-level mistakes
/// (buffer usages, 4-byte offset alignment, indirect overrun) that the
/// compute-only tests cannot catch.
#[test]
fn count_draw_submits_only_surviving_commands() {
    pollster::block_on(async {
        let Some((device, queue)) = count_capable_device().await else {
            eprintln!("skip indirect count-draw test: adapter lacks MULTI_DRAW_INDIRECT_COUNT");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue, true);
        let visibility = [false, true, false, true];
        let runs = [(0u32, 4u32)];
        let compacted = cull_and_compact(&device, &queue, &mut gpu, &visibility, &runs);
        assert_eq!(compacted.counts[0], 2);

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_indirect_count_draw_test"),
            source: wgpu::ShaderSource::Wgsl(COUNT_DRAW_WGSL.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perro_indirect_count_draw_test_pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(COLOR_FORMAT.into())],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        // cull_and_compact writes first_index = slot * 6, index_count = 6.
        let indices: Vec<u32> = (0..4).flat_map(|_| [0u32, 1, 2, 0, 1, 2]).collect();
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_indirect_count_draw_test_indices"),
            size: std::mem::size_of_val(indices.as_slice()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&indices));

        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_indirect_count_draw_test_target"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_indirect_count_draw_test_encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_indirect_count_draw_test_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            // Exactly the call shape the main pass uses for a run.
            pass.multi_draw_indexed_indirect_count(
                &gpu.indirect_compact_buffer,
                0,
                &gpu.indirect_count_buffer,
                0,
                4,
            );
        }
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let validation_error = error_scope.pop().await;
        assert!(
            validation_error.is_none(),
            "multi_draw_indexed_indirect_count failed validation: {validation_error:?}"
        );

        // The two survivors rasterize the full target; a count of 0 would leave
        // it at the clear color.
        let bytes_per_row = 256u32;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_indirect_count_draw_test_readback"),
            size: u64::from(bytes_per_row) * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_indirect_count_draw_test_copy"),
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(4),
                },
            },
            wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().expect("map callback").expect("target map");
        let pixels = slice
            .get_mapped_range()
            .expect("mapped target range")
            .to_vec();
        staging.unmap();
        assert_eq!(&pixels[0..4], &[255u8, 0, 0, 255], "count draw produced no raster");
    });
}

#[test]
fn count_path_stays_off_without_the_feature() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip indirect compaction gate test: no wgpu adapter");
            return;
        };
        let gpu = new_gpu_3d(&device, &queue, false);
        // Feature absent: the render pass never plans runs, never dispatches the
        // compaction pass, and never issues a count draw.
        assert!(!gpu.multi_draw_indirect_count_enabled);
        assert!(!gpu.multi_draw_indirect_count_active);
        assert!(gpu.indirect_run_plan.is_empty());
    });
}

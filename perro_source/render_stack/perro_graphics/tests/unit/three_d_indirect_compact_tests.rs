//! GPU indirect-command compaction (`multi_draw_indexed_indirect_count` path).
//!
//! Two halves:
//! * pure CPU checks that `plan_indirect_runs` / `plan_depth_runs` segment the
//!   main pass, the depth prepass and the mesh-blend depth pass into the same
//!   runs their render loops draw, and
//! * a headless wgpu cull+compact round trip that reads the compacted buffer
//!   and the per-run counts back and compares them against a CPU reference,
//!   including multi-pass plans landing in separate buffer regions.
//!
//! GPU tests skip with a note when no adapter is available.
use super::*;
use crate::three_d::gpu::culling::{plan_depth_runs, plan_indirect_runs};
use crate::three_d::gpu::render_pass::draw_compacted_run;

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

const TEST_REGION_STRIDE: u32 = 1024;

fn plan(batches: &[DrawBatch], groups: [&[usize]; 3]) -> (Vec<(u32, u32)>, [Range<usize>; 3]) {
    let mut out = Vec::new();
    let mut ranges = [0..0, 0..0, 0..0];
    plan_indirect_runs(batches, groups, 0, &mut out, &mut ranges);
    (
        out.into_iter().map(|run| (run.start, run.len)).collect(),
        ranges,
    )
}

fn plan_depth(batches: &[DrawBatch], indices: &[usize]) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let range = plan_depth_runs(batches, indices, 0, &mut out);
    assert_eq!(range, 0..out.len());
    out.into_iter().map(|run| (run.start, run.len)).collect()
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

#[test]
fn depth_run_plan_ignores_material_texture_changes() {
    // The depth prepass binds no material textures, so a texture change that
    // splits the main pass must NOT split the prepass: one run, not four.
    let batches: Vec<DrawBatch> = (0..4)
        .map(|i| planning_batch(i * 10, 10 + u64::from(i), i))
        .collect();
    let indices = [0usize, 1, 2, 3];
    assert_eq!(plan_depth(&batches, &indices), vec![(0, 4)]);
    // Same batches through the main pass: every batch is its own run.
    let (main_runs, _) = plan(&batches, [&indices, &[], &[]]);
    assert_eq!(main_runs, vec![(0, 1), (1, 1), (2, 1), (3, 1)]);
}

#[test]
fn depth_run_plan_breaks_on_path_double_sided_packed_lod_and_gaps() {
    // Exactly the prepass loop's rebind condition, plus slot contiguity.
    let mut batches: Vec<DrawBatch> = (0..7).map(|i| planning_batch(i * 10, 10, 0)).collect();
    batches[2].path = RenderPath3D::Skinned;
    batches[3].double_sided = true;
    batches[4].double_sided = true;
    batches[5].packed_lod = true;
    let indices = [0usize, 1, 2, 3, 4, 6];
    assert_eq!(
        plan_depth(&batches, &indices),
        // 0,1 rigid/single/unpacked; 2 skinned; 3,4 double-sided; 6 back to the
        // base state but non-contiguous with 4.
        vec![(0, 2), (2, 1), (3, 2), (6, 1)]
    );
}

#[test]
fn depth_run_plan_covers_every_drawn_batch() {
    // Count mode issues nothing for batches outside a run, so every index the
    // prepass walks must belong to exactly one run.
    let mut batches: Vec<DrawBatch> = (0..32).map(|i| planning_batch(i * 10, 10, i % 3)).collect();
    for (i, batch) in batches.iter_mut().enumerate() {
        batch.double_sided = i % 5 == 0;
        // Occlusion queries never apply to the depth-only passes; set one
        // anyway to prove the depth planner does not split around it.
        batch.occlusion_query = (i == 7).then_some(1);
    }
    let indices: Vec<usize> = (0..32).collect();
    let runs = plan_depth(&batches, &indices);
    let covered: Vec<u32> = runs
        .iter()
        .flat_map(|&(start, len)| start..start + len)
        .collect();
    assert_eq!(covered, (0..32).collect::<Vec<u32>>());
}

#[test]
fn run_plan_regions_keep_passes_apart() {
    // The prepass and the mesh-blend depth pass share source slots (the blend
    // set is a subset of the prepass set), so only dst_base separates them.
    let batches: Vec<DrawBatch> = (0..4).map(|i| planning_batch(i * 10, 10, 0)).collect();
    let mut out = Vec::new();
    let prepass = plan_depth_runs(&batches, &[0, 1, 2, 3], TEST_REGION_STRIDE, &mut out);
    let blend = plan_depth_runs(&batches, &[1, 2], 2 * TEST_REGION_STRIDE, &mut out);
    assert_eq!((prepass, blend), (0..1, 1..2));
    assert_eq!((out[0].start, out[0].len, out[0].dst_base), (0, 4, 1024));
    assert_eq!((out[1].start, out[1].len, out[1].dst_base), (1, 2, 2048));
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

impl CompactResult {
    /// Destination slot of a run: its pass region plus its source slot.
    #[inline]
    fn slot(run: IndirectRunGpu) -> usize {
        (run.dst_base + run.start) as usize
    }
}

/// Main-pass (region 0) run, the shape most tests need.
fn run(start: u32, len: u32) -> IndirectRunGpu {
    IndirectRunGpu {
        start,
        len,
        dst_base: 0,
    }
}

fn runs_of(pairs: &[(u32, u32)]) -> Vec<IndirectRunGpu> {
    pairs.iter().map(|&(start, len)| run(start, len)).collect()
}

/// Run the real frustum-cull compute over `visibility`, then the real
/// compaction compute over `runs` (one dispatch, any mix of pass regions), and
/// read both outputs back.
fn cull_and_compact(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    gpu: &mut Gpu3D,
    visibility: &[bool],
    runs: &[IndirectRunGpu],
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
    gpu.indirect_run_plan.extend_from_slice(runs);
    gpu.indirect_run_plan_main = 0..gpu.indirect_run_plan.len();
    gpu.indirect_run_plan_groups = [0..gpu.indirect_run_plan.len(), 0..0, 0..0];
    gpu.last_uploaded_indirect_runs.clear();
    gpu.upload_indirect_run_plan(queue);

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
    let plan = 0..gpu.indirect_run_plan.len();
    gpu.encode_indirect_compaction(queue, &mut encoder, INDIRECT_COMPACT_DISPATCH_EARLY, plan);
    queue.submit([encoder.finish()]);

    // Read every region: runs from different passes land in different ones.
    let slots = gpu.indirect_compact_region_stride * INDIRECT_COMPACT_REGIONS;
    let stride = std::mem::size_of::<DrawIndexedIndirectGpu>();
    let command_bytes = read_buffer(
        device,
        queue,
        &gpu.indirect_compact_buffer,
        (slots * stride) as u64,
    );
    let count_bytes = read_buffer(
        device,
        queue,
        &gpu.indirect_count_buffer,
        (slots * std::mem::size_of::<u32>()) as u64,
    );
    CompactResult {
        commands: bytemuck::cast_slice::<u8, DrawIndexedIndirectGpu>(&command_bytes).to_vec(),
        counts: bytemuck::cast_slice::<u8, u32>(&count_bytes).to_vec(),
    }
}

/// CPU reference: survivors of each run, in order, keyed by destination slot.
fn reference(visibility: &[bool], runs: &[IndirectRunGpu]) -> Vec<(usize, Vec<u32>)> {
    runs.iter()
        .map(|&r| {
            let survivors = (r.start..r.start + r.len)
                .filter(|&i| visibility[i as usize])
                // first_instance == source slot index, so it identifies the
                // command that survived.
                .collect();
            (CompactResult::slot(r), survivors)
        })
        .collect()
}

fn assert_matches_reference(result: &CompactResult, visibility: &[bool], runs: &[IndirectRunGpu]) {
    for (slot, expected) in reference(visibility, runs) {
        let actual_count = result.counts[slot];
        assert_eq!(
            actual_count as usize,
            expected.len(),
            "run at dst {slot}: survivor count"
        );
        for (offset, source_slot) in expected.iter().enumerate() {
            let command = result.commands[slot + offset];
            assert_eq!(
                command.first_instance, *source_slot,
                "run at dst {slot}: slot {offset} identity"
            );
            assert_eq!(
                command.instance_count,
                1 + *source_slot,
                "run at dst {slot}: slot {offset} instance count"
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
        let runs = runs_of(&[(0, 10), (10, 14)]);
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
        let runs = runs_of(&[(0, 1), (1, 1), (2, 1)]);
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
        let runs = runs_of(&[(0, total as u32)]);
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
        // 512 batches, 70% culled. Three passes over the same culled buffer,
        // each with its own segmentation and its own compacted-buffer region,
        // all compacted by one dispatch:
        //   main         - 8 runs of 64 (breaks on material texture too)
        //   depth prepass - 2 runs of 256 (no texture breaks, so coarser)
        //   blend depth   - 1 run of 128 (a subset of the prepass set)
        let total = 512usize;
        // Region stride tracks the indirect capacity, so size up before reading it.
        gpu.ensure_frustum_cull_capacity(&device, total);
        let stride = gpu.indirect_compact_region_stride as u32;
        let visibility: Vec<bool> = (0..total).map(|i| i % 10 >= 7).collect();
        let region = |base: u32, pairs: Vec<(u32, u32)>| -> Vec<IndirectRunGpu> {
            pairs
                .into_iter()
                .map(|(start, len)| IndirectRunGpu {
                    start,
                    len,
                    dst_base: base,
                })
                .collect()
        };
        let main = region(
            INDIRECT_COMPACT_REGION_MAIN * stride,
            (0..8).map(|r| (r * 64, 64)).collect(),
        );
        let prepass = region(
            INDIRECT_COMPACT_REGION_DEPTH_PREPASS * stride,
            (0..2).map(|r| (r * 256, 256)).collect(),
        );
        let blend = region(
            INDIRECT_COMPACT_REGION_BLEND_DEPTH * stride,
            vec![(64, 128)],
        );
        let runs: Vec<IndirectRunGpu> = main
            .iter()
            .chain(prepass.iter())
            .chain(blend.iter())
            .copied()
            .collect();
        let result = cull_and_compact(&device, &queue, &mut gpu, &visibility, &runs);
        // Overlapping source slots must not clobber each other across regions.
        assert_matches_reference(&result, &visibility, &runs);

        let expected_visible = visibility.iter().filter(|v| **v).count() as u32;
        for (name, pass_runs, expect_all) in [
            ("main", &main, true),
            ("depth_prepass", &prepass, true),
            ("blend_depth", &blend, false),
        ] {
            let max_count: u32 = pass_runs.iter().map(|r| r.len).sum();
            let actual: u32 = pass_runs
                .iter()
                .map(|&r| result.counts[CompactResult::slot(r)])
                .sum();
            if expect_all {
                assert_eq!(actual, expected_visible, "{name}: survivor total");
                assert_eq!(max_count, total as u32, "{name}: max_count");
            }
            eprintln!(
                "[indirect-count] pass={name} runs={} max_count={max_count} actual={actual} \
                 commands_saved={} ({:.0}% of the walk)",
                pass_runs.len(),
                max_count - actual,
                100.0 * f64::from(max_count - actual) / f64::from(max_count),
            );
            // The whole point: the GPU frontend walks `actual`, not `max_count`.
            assert!(actual * 3 < max_count, "{name}: not enough saved");
        }
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
        // Depth-prepass region: proves the region offsets the real prepass draw
        // uses pass validation (non-zero indirect + count buffer offsets).
        gpu.ensure_frustum_cull_capacity(&device, visibility.len());
        let prepass_base =
            INDIRECT_COMPACT_REGION_DEPTH_PREPASS * gpu.indirect_compact_region_stride as u32;
        let runs = [IndirectRunGpu {
            start: 0,
            len: 4,
            dst_base: prepass_base,
        }];
        let compacted = cull_and_compact(&device, &queue, &mut gpu, &visibility, &runs);
        assert_eq!(compacted.counts[prepass_base as usize], 2);

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
            // Exactly the call shape a run issues, region offsets included.
            draw_compacted_run(
                &mut pass,
                &gpu.indirect_compact_buffer,
                &gpu.indirect_count_buffer,
                runs[0],
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
        let mut gpu = new_gpu_3d(&device, &queue, false);
        // Feature absent: no pass plans runs, no compaction dispatch is encoded,
        // and no count draw is issued - every loop keeps the plain multi-draw /
        // per-batch indirect path.
        assert!(!gpu.multi_draw_indirect_count_enabled);
        assert!(!gpu.multi_draw_indirect_count_active);
        assert!(!gpu.multi_draw_indirect_count_prepass_active);
        assert!(!gpu.multi_draw_indirect_count_blend_depth_active);
        assert!(gpu.indirect_run_plan.is_empty());
        // The compaction buffers stay at their 1-slot-per-region placeholders.
        assert_eq!(gpu.indirect_compact_region_stride, 1);

        // Planning is a no-op even with the passes marked active: without the
        // feature every gate short-circuits before a run is emitted.
        gpu.build_indirect_run_plans(false, true, true);
        assert!(gpu.indirect_run_plan.is_empty());
        assert_eq!(gpu.indirect_run_plan_prepass, 0..0);
        assert_eq!(gpu.indirect_run_plan_blend_depth, 0..0);
        assert_eq!(gpu.indirect_run_plan_main, 0..0);
        assert!(!gpu.multi_draw_indirect_count_prepass_active);
        assert!(!gpu.multi_draw_indirect_count_blend_depth_active);
        assert!(!gpu.multi_draw_indirect_count_active);
    });
}

#[test]
fn plans_split_by_pass_and_gate_on_min_commands() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip indirect compaction plan-split test: no wgpu adapter");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue, true);
        gpu.ensure_frustum_cull_capacity(&device, 128);
        let stride = gpu.indirect_compact_region_stride as u32;

        // 64 opaque batches; the prepass takes all of them, the mesh-blend depth
        // pass takes the first 40 (its set is always a prepass subset).
        gpu.draw_batches = (0..64).map(|i| planning_batch(i * 10, 10, 0)).collect();
        gpu.opaque_batch_indices = (0..64).collect();
        gpu.depth_prepass_batch_indices = (0..64).collect();
        gpu.mesh_blend_depth_batch_indices = (0..40).collect();
        gpu.build_indirect_run_plans(true, true, true);

        assert!(gpu.multi_draw_indirect_count_prepass_active);
        assert!(gpu.multi_draw_indirect_count_blend_depth_active);
        assert!(gpu.multi_draw_indirect_count_active);
        // Layout: [prepass][blend depth][main], no overlap, nothing orphaned.
        assert_eq!(gpu.indirect_run_plan_prepass, 0..1);
        assert_eq!(gpu.indirect_run_plan_blend_depth, 1..2);
        assert_eq!(gpu.indirect_run_plan_main, 2..3);
        assert_eq!(gpu.indirect_run_plan_groups[0], 2..3);
        assert_eq!(gpu.indirect_run_plan.len(), 3);
        assert_eq!(
            gpu.indirect_run_plan[0].dst_base,
            INDIRECT_COMPACT_REGION_DEPTH_PREPASS * stride
        );
        assert_eq!(
            gpu.indirect_run_plan[1].dst_base,
            INDIRECT_COMPACT_REGION_BLEND_DEPTH * stride
        );
        assert_eq!(gpu.indirect_run_plan[2].dst_base, 0);
        assert_eq!(gpu.indirect_planned_command_count, 64);
        assert_eq!(gpu.indirect_planned_command_count_prepass, 64);
        assert_eq!(gpu.indirect_planned_command_count_blend_depth, 40);

        // A pass under the min-command gate is dropped from the plan; the
        // others keep theirs, and the ranges stay contiguous.
        gpu.mesh_blend_depth_batch_indices = (0..8).collect();
        gpu.build_indirect_run_plans(true, true, true);
        assert!(gpu.multi_draw_indirect_count_prepass_active);
        assert!(!gpu.multi_draw_indirect_count_blend_depth_active);
        assert!(gpu.multi_draw_indirect_count_active);
        assert_eq!(gpu.indirect_run_plan_prepass, 0..1);
        assert_eq!(gpu.indirect_run_plan_blend_depth, 0..0);
        assert_eq!(gpu.indirect_run_plan_main, 1..2);
        assert_eq!(gpu.indirect_run_plan.len(), 2);

        // Inactive passes never enter the plan at all.
        gpu.build_indirect_run_plans(true, false, false);
        assert!(!gpu.multi_draw_indirect_count_prepass_active);
        assert!(!gpu.multi_draw_indirect_count_blend_depth_active);
        assert_eq!(gpu.indirect_run_plan_main, 0..1);
        assert_eq!(gpu.indirect_run_plan.len(), 1);
    });
}

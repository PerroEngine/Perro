//! Full-rebuild multimesh staging reuse (see `Gpu3D::can_reuse_multimesh_staging`).
//!
//! Runs the real `Gpu3D::prepare` against a headless wgpu device; skipped with
//! a note when no adapter is available.
use super::*;
use crate::resources::ResourceStore;
use crate::three_d::renderer::DenseMultiMeshDraw3D;
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_render_bridge::{DenseInstancePose3D, LODOptions3D, MeshSurfaceBinding3D};
use perro_structs::Color;
use std::time::{Duration, Instant};

const DENSE_DRAWS: u32 = 50;
const DENSE_INSTANCES: u32 = 2_000;
const REGULAR_DRAWS: u32 = 50;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

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
            label: Some("perro_prepare_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

fn new_gpu_3d(device: &wgpu::Device, queue: &wgpu::Queue) -> Gpu3D {
    let cache = PipelineRegistryCache::new();
    let pipelines = cache.get_or_create(device, COLOR_FORMAT, 1);
    Gpu3D::new(
        device,
        queue,
        COLOR_FORMAT,
        Gpu3DConfig {
            sample_count: 1,
            width: 256,
            height: 256,
            meshlets_enabled: false,
            dev_meshlets: false,
            meshlet_debug_view: false,
            occlusion_culling: crate::OcclusionCullingMode::Off,
            ssao: crate::SsaoQuality::Off,
            indirect_first_instance_enabled: false,
            multi_draw_indirect_enabled: false,
            multi_draw_indirect_count_enabled: false,
            texture_filter: TextureFilterMode::default(),
            shader_variant_mode: crate::ShaderVariantMode::Generic,
            shadow_pcf_high: false,
        },
        pipelines,
    )
}

fn surfaces(material: MaterialID, modulate: Color) -> Arc<[MeshSurfaceBinding3D]> {
    Arc::from([MeshSurfaceBinding3D {
        material: Some(material),
        overrides: Arc::from([]),
        modulate,
    }])
}

fn identity_at(x: f32) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [x, 0.0, 0.0, 1.0],
    ]
}

fn dense_draw(index: u32, mesh: MeshID, material: MaterialID) -> Draw3DInstance {
    let instances: Arc<[DenseInstancePose3D]> = (0..DENSE_INSTANCES)
        .map(|i| DenseInstancePose3D {
            position: [(i % 64) as f32 * 0.5, 0.0, (i / 64) as f32 * 0.5],
            scale: [1.0, 1.0, 1.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            has_blend_shape_weight_override: false,
            blend_shape_weights: Arc::from([]),
        })
        .collect::<Vec<_>>()
        .into();
    Draw3DInstance {
        node: NodeID::from_parts(900_000 + index, 0),
        kind: Draw3DKind::Mesh(mesh),
        surfaces: surfaces(material, Color::WHITE),
        instance_mats: Arc::from([]),
        blend_shape_weights: Arc::from([]),
        debug_color: None,
        skeleton: None,
        dense_multimesh: Some(DenseMultiMeshDraw3D {
            node_model: identity_at(index as f32 * 80.0),
            instance_scale: 1.0,
            instances,
        }),
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: MeshBlendOptions3D::default(),
        cast_shadows: true,
        receive_shadows: true,
    }
}

fn regular_draw(index: u32, mesh: MeshID, material: MaterialID, modulate: Color) -> Draw3DInstance {
    Draw3DInstance {
        node: NodeID::from_parts(index + 1, 0),
        kind: Draw3DKind::Mesh(mesh),
        surfaces: surfaces(material, modulate),
        instance_mats: Arc::from([identity_at(index as f32 * 2.0)]),
        blend_shape_weights: Arc::from([]),
        debug_color: None,
        skeleton: None,
        dense_multimesh: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: MeshBlendOptions3D::default(),
        cast_shadows: true,
        receive_shadows: true,
    }
}

/// Everything the multimesh staging feeds to the GPU + its downstream readers.
#[derive(PartialEq)]
struct StagingSnapshot {
    instances: Vec<[u8; std::mem::size_of::<MultiMeshInstanceGpu>()]>,
    draw_params: Vec<[u8; std::mem::size_of::<MultiMeshDrawParamGpu>()]>,
    blend_meta: Vec<[u8; std::mem::size_of::<BlendShapeInstanceMetaGpu>()]>,
    blend_weights: Vec<f32>,
    blend_meta_base: u32,
    blend_weight_base: u32,
    batch_keys: Vec<(u32, u32, u32, u32, u32, bool, bool)>,
    param_ranges: Vec<std::ops::Range<u32>>,
    rigid_meta_len: usize,
}

fn snapshot(gpu: &Gpu3D) -> StagingSnapshot {
    fn pod_rows<T: bytemuck::Pod, const N: usize>(items: &[T]) -> Vec<[u8; N]> {
        items
            .iter()
            .map(|item| {
                let mut row = [0u8; N];
                row.copy_from_slice(bytemuck::bytes_of(item));
                row
            })
            .collect()
    }
    StagingSnapshot {
        instances: pod_rows(&gpu.staged_multimesh_instances),
        draw_params: pod_rows(&gpu.staged_multimesh_draw_params),
        blend_meta: pod_rows(&gpu.staged_multimesh_blend_meta),
        blend_weights: gpu.staged_multimesh_blend_weights.clone(),
        blend_meta_base: gpu.multimesh_blend_meta_base,
        blend_weight_base: gpu.multimesh_blend_weight_base,
        batch_keys: gpu
            .multimesh_batches
            .iter()
            .map(|batch| {
                (
                    batch.instance_start,
                    batch.instance_count,
                    batch.draw_param_index,
                    batch.mesh.index_start,
                    batch.mesh.index_count,
                    batch.double_sided,
                    batch.casts_shadows,
                )
            })
            .collect(),
        param_ranges: gpu.last_draw_multimesh_param_ranges.clone(),
        rigid_meta_len: gpu.staged_blend_shape_instance_meta.len(),
    }
}

struct Harness {
    device: wgpu::Device,
    queue: wgpu::Queue,
    gpu: Gpu3D,
    resources: ResourceStore,
    shared_textures: SharedTextureStore,
    lighting: Lighting3DState,
    revision: u64,
}

impl Harness {
    fn prepare(&mut self, draws: &[Draw3DInstance], force_full_rebuild: bool) -> Duration {
        self.revision += 1;
        let start = Instant::now();
        self.gpu.prepare(
            &self.device,
            &self.queue,
            Prepare3D {
                resources: &self.resources,
                shared_textures: &mut self.shared_textures,
                camera: Camera3DState::default(),
                lighting: &self.lighting,
                draws,
                draws_revision: self.revision,
                force_full_rebuild,
                decals: &[],
                decals_revision: 0,
                width: 256,
                height: 256,
                static_texture_lookup: None,
                static_mesh_lookup: None,
                static_shader_lookup: None,
            },
        );
        start.elapsed()
    }
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

#[test]
fn multimesh_staging_reuse_matches_full_repack_and_is_cheaper() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip multimesh staging reuse test: no wgpu adapter");
            return;
        };
        let gpu = new_gpu_3d(&device, &queue);
        let mut resources = ResourceStore::new();
        let mesh = resources.create_mesh("__prepare_test_mesh__", true);
        let material =
            resources.create_material(Material3D::default(), Some("__prepare_test_mat__"), true);
        let mut harness = Harness {
            device,
            queue,
            gpu,
            resources,
            shared_textures: SharedTextureStore::default(),
            lighting: Lighting3DState::default(),
            revision: 0,
        };

        let mut draws: Vec<Draw3DInstance> = (0..DENSE_DRAWS)
            .map(|i| dense_draw(i, mesh, material))
            .collect();
        draws.extend((0..REGULAR_DRAWS).map(|i| regular_draw(i, mesh, material, Color::WHITE)));

        // Cold build stages everything.
        harness.prepare(&draws, true);
        assert!(
            !harness.gpu.staged_multimesh_instances.is_empty(),
            "multimesh staging never ran; scene setup is wrong"
        );
        assert_eq!(
            harness.gpu.staged_multimesh_instances.len(),
            (DENSE_DRAWS * DENSE_INSTANCES) as usize
        );
        // Rigid rows must own the head of the shared blend-shape buffer: the
        // rigid/skinned shaders index it positionally by instance index.
        assert_eq!(
            harness.gpu.staged_blend_shape_instance_meta.len(),
            harness.gpu.staged_instance_transforms.len()
        );
        assert_eq!(
            harness.gpu.multimesh_blend_meta_base,
            harness.gpu.staged_blend_shape_instance_meta.len() as u32
        );

        // Flip one regular draw's modulate: a semantic change, so prepare takes
        // the full-rebuild path, but nothing multimesh moved.
        let flip = |draws: &mut Vec<Draw3DInstance>, on: bool| {
            let modulate = if on {
                Color::from([0.5, 1.0, 1.0, 1.0])
            } else {
                Color::WHITE
            };
            draws[DENSE_DRAWS as usize] = regular_draw(0, mesh, material, modulate);
        };

        flip(&mut draws, true);
        let reuse_before = harness.gpu.multimesh_staging_reuse_count;
        harness.prepare(&draws, false);
        assert_eq!(
            harness.gpu.multimesh_staging_reuse_count,
            reuse_before + 1,
            "full rebuild did not reuse the multimesh staging"
        );
        let reused = snapshot(&harness.gpu);

        // Same draw list, but forced through the repack path: the freshly
        // packed staging must be byte-identical to what reuse kept.
        harness.prepare(&draws, true);
        let repacked = snapshot(&harness.gpu);
        assert!(
            reused == repacked,
            "reused multimesh staging diverges from a full repack"
        );

        // A moved multimesh must NOT reuse.
        let mut moved = draws.clone();
        if let Some(dense) = moved[0].dense_multimesh.as_mut() {
            dense.node_model[3][0] += 5.0;
        }
        // Also change a regular draw so the frame cannot take the
        // transform-only fast path.
        moved[DENSE_DRAWS as usize] =
            regular_draw(0, mesh, material, Color::from([0.25, 1.0, 1.0, 1.0]));
        let reuse_before = harness.gpu.multimesh_staging_reuse_count;
        harness.prepare(&moved, false);
        assert_eq!(
            harness.gpu.multimesh_staging_reuse_count, reuse_before,
            "moved multimesh must repack"
        );
        // ...and a dropped multimesh must not reuse either.
        let mut dropped = draws.clone();
        dropped.remove(0);
        let reuse_before = harness.gpu.multimesh_staging_reuse_count;
        harness.prepare(&dropped, false);
        assert_eq!(
            harness.gpu.multimesh_staging_reuse_count, reuse_before,
            "dropped multimesh draw must repack"
        );

        // Timing: same scene, same semantic change, repack vs reuse.
        const SAMPLES: usize = 15;
        let mut repack_samples = Vec::with_capacity(SAMPLES);
        let mut reuse_samples = Vec::with_capacity(SAMPLES);
        let mut on = false;
        for _ in 0..SAMPLES {
            on = !on;
            flip(&mut draws, on);
            repack_samples.push(harness.prepare(&draws, true));
            on = !on;
            flip(&mut draws, on);
            reuse_samples.push(harness.prepare(&draws, false));
        }
        let repack = median(repack_samples);
        let reuse = median(reuse_samples);
        println!(
            "multimesh full rebuild ({DENSE_DRAWS} dense x {DENSE_INSTANCES} inst + \
             {REGULAR_DRAWS} regular): repack={:?} reuse={:?} ({:.2}x)",
            repack,
            reuse,
            repack.as_secs_f64() / reuse.as_secs_f64().max(f64::EPSILON),
        );
        assert!(
            reuse < repack,
            "multimesh staging reuse ({reuse:?}) is not cheaper than a repack ({repack:?})"
        );
    });
}

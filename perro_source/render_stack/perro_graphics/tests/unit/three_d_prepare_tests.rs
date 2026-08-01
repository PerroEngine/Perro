//! Full-rebuild multimesh staging reuse (see `Gpu3D::can_reuse_multimesh_staging`).
//!
//! Runs the real `Gpu3D::prepare` against a headless wgpu device; skipped with
//! a note when no adapter is available.
use super::*;
use crate::resources::ResourceStore;
use crate::three_d::renderer::DenseMultiMeshDraw3D;
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_render_bridge::{
    CustomMaterial3D, CustomMaterialLighting3D, CustomMaterialParam3D, CustomMaterialParamValue3D,
    DenseInstancePose3D, LODOptions3D, MaterialParamOverride3D, MeshSurfaceBinding3D,
    SkeletonPalette, StandardMaterial3D, VertexModifier3D,
};
use perro_structs::Color;
use std::borrow::Cow;
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

fn new_gpu_3d(device: &wgpu::Device, queue: &wgpu::Queue, arena: &SharedMeshArena) -> Gpu3D {
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
        arena,
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

/// Custom material carrying both vertex modifiers and custom shader params, so
/// a staged entry uses every word shape the arena can hold: the `value_base`
/// header word (read by the modifier path) and the `(offset << 2) | kind` param
/// words. `extra` adds a param so the entry's length changes too.
fn custom_material(scale: f32, extra: bool) -> Material3D {
    let mut params = vec![
        CustomMaterialParam3D::named(
            "tint",
            CustomMaterialParamValue3D::Vec4([scale, 0.25, 0.5, 1.0]),
        ),
        CustomMaterialParam3D::named("wobble", CustomMaterialParamValue3D::F32(scale)),
    ];
    if extra {
        params.push(CustomMaterialParam3D::named(
            "extra",
            CustomMaterialParamValue3D::Vec2([scale, 2.0]),
        ));
    }
    Material3D::Custom(CustomMaterial3D {
        // Resolved through the harness' `static_shader_lookup` so the pipeline
        // compiles once and caches, instead of missing the asset store on
        // every draw of every build.
        shader_path: Cow::Borrowed("__prepare_test_shader__"),
        params: Cow::Owned(params),
        images: Cow::Borrowed(&[]),
        lighting: CustomMaterialLighting3D::Standard,
        surface: StandardMaterial3D::default(),
    })
    .with_vertex_modifiers(vec![VertexModifier3D::Inflate {
        amount: scale,
        mask: None,
    }])
}

/// Fragment-only custom material body (no `shade_vertex` hook, which would pull
/// these draws out of the depth/shadow batches). Reads param 0 so the staged
/// arena is actually load-bearing for the compiled shader.
fn test_custom_shader(_path_hash: u64) -> &'static str {
    r#"
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let tint = custom_f_param(in, 0u);
    return vec4<f32>(tint.rgb + in.normal_ws * 0.05, tint.a);
}
"#
}

fn surfaces_with_override(material: MaterialID, tint: f32) -> Arc<[MeshSurfaceBinding3D]> {
    Arc::from([MeshSurfaceBinding3D {
        material: Some(material),
        overrides: Arc::from([MaterialParamOverride3D {
            name: Cow::Borrowed("tint"),
            value: CustomMaterialParamValue3D::F32(tint),
        }]),
        modulate: Color::WHITE,
    }])
}

fn dense_draw(index: u32, mesh: MeshID, material: MaterialID) -> Draw3DInstance {
    dense_draw_with_surfaces(index, mesh, surfaces(material, Color::WHITE))
}

fn dense_draw_with_surfaces(
    index: u32,
    mesh: MeshID,
    surfaces: Arc<[MeshSurfaceBinding3D]>,
) -> Draw3DInstance {
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
        surfaces,
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
    custom_params_meta: Vec<u32>,
    custom_params_values: Vec<f32>,
    custom_params_meta_base: u32,
    custom_params_values_base: u32,
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
        custom_params_meta: gpu.staged_multimesh_custom_params_meta.clone(),
        custom_params_values: gpu.staged_multimesh_custom_params_values.clone(),
        custom_params_meta_base: gpu.multimesh_custom_params_meta_base,
        custom_params_values_base: gpu.multimesh_custom_params_values_base,
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
    mesh_arena: SharedMeshArena,
    lighting: Lighting3DState,
    revision: u64,
    /// Supplied by the custom-material test so its shader resolves (and its
    /// pipeline caches) instead of missing the asset store on every draw.
    static_shader_lookup: Option<StaticShaderLookup>,
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
                mesh_arena: &mut self.mesh_arena,
                mesh_arena_compact_allowed: true,
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
                static_shader_lookup: self.static_shader_lookup,
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
        let mesh_arena = SharedMeshArena::new(&device, false, false);
        let gpu = new_gpu_3d(&device, &queue, &mesh_arena);
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
            mesh_arena,
            lighting: Lighting3DState::default(),
            revision: 0,
            static_shader_lookup: None,
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

/// Dense draws whose material carries custom-shader params used to opt the whole
/// scene out of staging reuse: their param offsets pointed into the shared arena
/// that every rebuild refills from the regular draws. The params now stage into
/// their own tail arena and are rebased by a delta, so these scenes reuse like
/// any other -- and reproduce the same bytes a repack would.
#[test]
fn multimesh_custom_param_staging_reuse_matches_full_repack_and_is_cheaper() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip multimesh custom-param staging reuse test: no wgpu adapter");
            return;
        };
        let mesh_arena = SharedMeshArena::new(&device, false, false);
        let gpu = new_gpu_3d(&device, &queue, &mesh_arena);
        let mut resources = ResourceStore::new();
        let mesh = resources.create_mesh("__prepare_custom_test_mesh__", true);
        let dense_material = resources.create_material(
            custom_material(1.0, false),
            Some("__prepare_custom_dense_mat__"),
            true,
        );
        // Two regular materials with DIFFERENT param counts: swapping between
        // them changes the regular arena's length, which moves the multimesh
        // tail and forces the delta rebase to run.
        let regular_short = resources.create_material(
            custom_material(0.5, false),
            Some("__prepare_custom_reg_short__"),
            true,
        );
        let regular_long = resources.create_material(
            custom_material(0.75, true),
            Some("__prepare_custom_reg_long__"),
            true,
        );
        let mut harness = Harness {
            device,
            queue,
            gpu,
            resources,
            shared_textures: SharedTextureStore::default(),
            mesh_arena,
            lighting: Lighting3DState::default(),
            revision: 0,
            static_shader_lookup: Some(test_custom_shader),
        };

        let dense_surfaces = surfaces_with_override(dense_material, 0.125);
        let mut draws: Vec<Draw3DInstance> = (0..DENSE_DRAWS)
            .map(|i| dense_draw_with_surfaces(i, mesh, dense_surfaces.clone()))
            .collect();
        draws.extend(
            (0..REGULAR_DRAWS).map(|i| regular_draw(i, mesh, regular_short, Color::WHITE)),
        );

        harness.prepare(&draws, true);
        assert!(
            !harness.gpu.staged_multimesh_custom_params_meta.is_empty(),
            "dense custom-material params never staged; scene setup is wrong"
        );
        assert!(
            !harness.gpu.staged_custom_params_meta.is_empty(),
            "regular custom-material params never staged; scene setup is wrong"
        );
        // The tail must sit exactly after the regular rows in both arenas.
        assert_eq!(
            harness.gpu.multimesh_custom_params_meta_base,
            harness.gpu.staged_custom_params_meta.len() as u32
        );
        assert_eq!(
            harness.gpu.multimesh_custom_params_values_base,
            harness.gpu.staged_custom_params_values.len() as u32
        );
        // ...and every dense draw param must address into it.
        let tail_start = harness.gpu.multimesh_custom_params_meta_base;
        for param in harness.gpu.staged_multimesh_draw_params.iter() {
            assert!(
                param.custom_params[0] >= tail_start + 2,
                "dense draw param header {} points outside the multimesh tail (starts at {})",
                param.custom_params[0],
                tail_start,
            );
        }

        // Swap one regular draw between the short and long custom material: a
        // semantic change that forces the full rebuild AND resizes the regular
        // arena underneath the tail, but touches nothing multimesh.
        let flip = |draws: &mut Vec<Draw3DInstance>, long: bool| {
            let material = if long { regular_long } else { regular_short };
            draws[DENSE_DRAWS as usize] = regular_draw(0, mesh, material, Color::WHITE);
        };

        flip(&mut draws, true);
        let reuse_before = harness.gpu.multimesh_staging_reuse_count;
        harness.prepare(&draws, false);
        assert_eq!(
            harness.gpu.multimesh_staging_reuse_count,
            reuse_before + 1,
            "custom-param multimesh scene did not reuse its staging"
        );
        let reused = snapshot(&harness.gpu);
        // The rebase must have tracked the resized regular arena.
        assert_eq!(
            harness.gpu.multimesh_custom_params_meta_base,
            harness.gpu.staged_custom_params_meta.len() as u32
        );
        assert_eq!(
            harness.gpu.multimesh_custom_params_values_base,
            harness.gpu.staged_custom_params_values.len() as u32
        );

        // Same draw list through the repack path: byte-identical staging.
        harness.prepare(&draws, true);
        let repacked = snapshot(&harness.gpu);
        assert!(
            reused == repacked,
            "reused custom-param multimesh staging diverges from a full repack"
        );

        // Negative: a changed param VALUE on the dense surfaces must repack, and
        // the staged tail values must actually move.
        let before_values = harness.gpu.staged_multimesh_custom_params_values.clone();
        let mut retinted = draws.clone();
        let retinted_surfaces = surfaces_with_override(dense_material, 0.875);
        for draw in retinted.iter_mut().take(DENSE_DRAWS as usize) {
            draw.surfaces = retinted_surfaces.clone();
        }
        let reuse_before = harness.gpu.multimesh_staging_reuse_count;
        harness.prepare(&retinted, false);
        assert_eq!(
            harness.gpu.multimesh_staging_reuse_count, reuse_before,
            "changed custom-param value must repack"
        );
        assert_ne!(
            harness.gpu.staged_multimesh_custom_params_values, before_values,
            "changed custom-param value did not reach the staged tail"
        );

        // Back to the original params, then time repack vs reuse.
        harness.prepare(&draws, true);
        const SAMPLES: usize = 15;
        let mut repack_samples = Vec::with_capacity(SAMPLES);
        let mut reuse_samples = Vec::with_capacity(SAMPLES);
        let mut long = false;
        for _ in 0..SAMPLES {
            long = !long;
            flip(&mut draws, long);
            repack_samples.push(harness.prepare(&draws, true));
            long = !long;
            flip(&mut draws, long);
            reuse_samples.push(harness.prepare(&draws, false));
        }
        let repack = median(repack_samples);
        let reuse = median(reuse_samples);
        println!(
            "multimesh custom-param full rebuild ({DENSE_DRAWS} dense x {DENSE_INSTANCES} inst + \
             {REGULAR_DRAWS} regular): repack={:?} reuse={:?} ({:.2}x)",
            repack,
            reuse,
            repack.as_secs_f64() / reuse.as_secs_f64().max(f64::EPSILON),
        );
        assert!(
            reuse < repack,
            "custom-param staging reuse ({reuse:?}) is not cheaper than a repack ({repack:?})"
        );
    });
}

const SKINNED_BONES: usize = 64;
// Kept under FRUSTUM_CULL_MIN_BATCHES so the GPU-cull uploads (whose indirect
// records are GPU-mutated and therefore ungateable) stay out of the byte math.
const UPLOAD_SCENE_DRAWS: u32 = 80;

fn bone_palette(phase: f32) -> SkeletonPalette {
    let matrices: Arc<[[[f32; 4]; 3]]> = (0..SKINNED_BONES)
        .map(|bone| {
            let offset = phase + bone as f32 * 0.01;
            [
                [1.0, 0.0, 0.0, offset],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ]
        })
        .collect::<Vec<_>>()
        .into();
    SkeletonPalette { matrices }
}

fn skinned_draw(index: u32, mesh: MeshID, material: MaterialID, phase: f32) -> Draw3DInstance {
    Draw3DInstance {
        skeleton: Some(bone_palette(phase)),
        ..regular_draw(index, mesh, material, Color::WHITE)
    }
}

/// Byte accounting for the staging uploads a `prepare` would issue if none of
/// the lanes were gated, so the test can report "before" next to the gated
/// "after".
fn ungated_staging_bytes(gpu: &Gpu3D) -> usize {
    std::mem::size_of_val(gpu.staged_instance_transforms.as_slice())
        + std::mem::size_of_val(gpu.staged_rigid_instance_meta.as_slice())
        + std::mem::size_of_val(gpu.staged_skinned_instance_meta.as_slice())
        + std::mem::size_of_val(gpu.staged_blend_shape_weights.as_slice())
        + std::mem::size_of_val(gpu.staged_blend_shape_instance_meta.as_slice())
        + std::mem::size_of_val(gpu.staged_skeletons.as_slice())
}

/// `indirect_first_instance` gates the whole GPU frustum-cull path
/// (`frustum_cull_default`), so the cull-input tests need it on.
fn upload_harness(
    device: wgpu::Device,
    queue: wgpu::Queue,
    indirect_first_instance: bool,
) -> (Harness, MeshID, MaterialID) {
    let cache = PipelineRegistryCache::new();
    let pipelines = cache.get_or_create(&device, COLOR_FORMAT, 1);
    let mesh_arena = SharedMeshArena::new(&device, false, false);
    let gpu = Gpu3D::new(
        &device,
        &queue,
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
            indirect_first_instance_enabled: indirect_first_instance,
            multi_draw_indirect_enabled: false,
            multi_draw_indirect_count_enabled: false,
            texture_filter: TextureFilterMode::default(),
            shader_variant_mode: crate::ShaderVariantMode::Generic,
            shadow_pcf_high: false,
        },
        pipelines,
        &mesh_arena,
    );
    let mut resources = ResourceStore::new();
    let mesh = resources.create_mesh("__upload_gate_test_mesh__", true);
    let material =
        resources.create_material(Material3D::default(), Some("__upload_gate_test_mat__"), true);
    (
        Harness {
            device,
            queue,
            gpu,
            resources,
            shared_textures: SharedTextureStore::default(),
            mesh_arena,
            lighting: Lighting3DState::default(),
            revision: 0,
            static_shader_lookup: None,
        },
        mesh,
        material,
    )
}

/// Draw batches in a form two prepares can be compared by: the animation-only
/// fast path must leave the batch list byte-identical (same meshes, same
/// instance spans, same order), since it skips the restage that would rebuild
/// it.
fn batch_keys(gpu: &Gpu3D) -> Vec<(u64, u32, u32, u32, u32, i32, u32)> {
    gpu.draw_batches
        .iter()
        .map(|batch| {
            (
                batch.state_key,
                batch.instance_start,
                batch.instance_count,
                batch.mesh.index_start,
                batch.mesh.index_count,
                batch.mesh.base_vertex,
                batch.order_index,
            )
        })
        .collect()
}

/// A skinned draw whose palette changed used to defeat every fast path (the
/// draw was not "same except model"), forcing a whole rebuild. It now
/// classifies as an animation-only delta: only the bone palettes are patched
/// and uploaded, and every other staging lane stays untouched.
#[test]
fn skeleton_only_change_uploads_only_the_skeleton_bytes() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip skeleton-only upload gate test: no wgpu adapter");
            return;
        };
        let (mut harness, mesh, material) = upload_harness(device, queue, false);

        let mut draws: Vec<Draw3DInstance> = (0..UPLOAD_SCENE_DRAWS)
            .map(|i| regular_draw(i, mesh, material, Color::WHITE))
            .collect();
        draws.push(skinned_draw(UPLOAD_SCENE_DRAWS, mesh, material, 0.0));

        harness.prepare(&draws, true);
        let cold = harness.gpu.prepare_upload_stats();
        let ungated = ungated_staging_bytes(&harness.gpu);
        let skeleton_bytes = std::mem::size_of_val(harness.gpu.staged_skeletons.as_slice());
        let cold_batches = batch_keys(&harness.gpu);
        let cold_rebuilds = harness.gpu.prepare_full_rebuild_count;
        assert!(
            harness.gpu.staged_has_skinned,
            "scene setup produced no skinned batch"
        );
        assert_eq!(harness.gpu.staged_skeletons.len(), SKINNED_BONES);

        // Animate the palette only. Same node, same model, same materials.
        let last = draws.len() - 1;
        draws[last] = skinned_draw(UPLOAD_SCENE_DRAWS, mesh, material, 1.0);
        harness.prepare(&draws, false);
        let animated = harness.gpu.prepare_upload_stats();

        println!(
            "skeleton-only frame ({UPLOAD_SCENE_DRAWS} rigid + 1 skinned x {SKINNED_BONES} bones): \
             cold={} B, ungated restage={} B, patched={} B in {} write_buffer calls",
            cold.write_buffer_bytes,
            ungated,
            animated.write_buffer_bytes,
            animated.write_buffer_calls,
        );
        assert_eq!(
            harness.gpu.last_prepare_step_timing.full_rebuilds, 0,
            "a skeleton-only frame must not restage the scene"
        );
        assert_eq!(harness.gpu.prepare_full_rebuild_count, cold_rebuilds);
        assert_eq!(
            batch_keys(&harness.gpu),
            cold_batches,
            "the patch path must leave the batch list exactly as the build left it"
        );
        assert_eq!(
            animated.write_buffer_bytes, skeleton_bytes as u64,
            "a skeleton-only frame must upload the palettes and nothing else"
        );
        assert_eq!(animated.write_buffer_calls, 1);
        // The staged palettes really carry the new pose.
        assert_eq!(
            harness.gpu.staged_skeletons[0][0][3],
            bone_palette(1.0).matrices[0][0][3]
        );

        // Re-preparing the exact same animated pose patches nothing: the rows
        // are compared before they are overwritten, so a producer handing back
        // an equal-but-new palette Arc still sends no bytes.
        draws[last] = skinned_draw(UPLOAD_SCENE_DRAWS, mesh, material, 1.0);
        harness.prepare(&draws, false);
        assert_eq!(
            harness.gpu.prepare_upload_stats().write_buffer_bytes,
            0,
            "an identical re-pose must upload nothing"
        );

        // A forced restage re-primes the whole-vec skeleton gate the span
        // patches invalidated; every other lane still gates away.
        harness.prepare(&draws, true);
        let repeat = harness.gpu.prepare_upload_stats();
        assert_eq!(
            repeat.write_buffer_bytes, skeleton_bytes as u64,
            "a forced restage re-primes only the patched lane"
        );
        harness.prepare(&draws, true);
        assert_eq!(
            harness.gpu.prepare_upload_stats().write_buffer_bytes,
            0,
            "a second identical forced restage must upload nothing"
        );
    });
}

/// End-to-end shape of the win on a 1k-draw scene with one skinned character:
/// the animated frame must stay off the full rebuild and cost only the palette
/// span, and it must be measurably cheaper than the restage it replaces.
#[test]
fn animated_frame_beats_the_full_restage_on_a_1k_scene() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip animation fast-path benchmark: no wgpu adapter");
            return;
        };
        let (mut harness, mesh, material) = upload_harness(device, queue, false);

        const SCENE_DRAWS: u32 = 1_000;
        const SAMPLES: usize = 9;
        let mut draws: Vec<Draw3DInstance> = (0..SCENE_DRAWS)
            .map(|i| regular_draw(i, mesh, material, Color::WHITE))
            .collect();
        draws.push(skinned_draw(SCENE_DRAWS, mesh, material, 0.0));
        harness.prepare(&draws, true);
        let skeleton_bytes = std::mem::size_of_val(harness.gpu.staged_skeletons.as_slice());
        let ungated = ungated_staging_bytes(&harness.gpu);
        let cold_batches = batch_keys(&harness.gpu);
        let last = draws.len() - 1;

        let mut patched = Vec::with_capacity(SAMPLES);
        let mut restaged = Vec::with_capacity(SAMPLES);
        let mut patched_bytes = 0u64;
        let mut restaged_bytes = 0u64;
        for sample in 0..SAMPLES {
            let phase = 1.0 + sample as f32;
            draws[last] = skinned_draw(SCENE_DRAWS, mesh, material, phase);
            patched.push(harness.prepare(&draws, false));
            patched_bytes = harness.gpu.prepare_upload_stats().write_buffer_bytes;
            assert_eq!(harness.gpu.last_prepare_step_timing.full_rebuilds, 0);
            assert_eq!(batch_keys(&harness.gpu), cold_batches);

            // Same pose delta, but forced down the old path for the baseline.
            draws[last] = skinned_draw(SCENE_DRAWS, mesh, material, phase + 0.5);
            restaged.push(harness.prepare(&draws, true));
            restaged_bytes = harness.gpu.prepare_upload_stats().write_buffer_bytes;
            assert_eq!(harness.gpu.last_prepare_step_timing.full_rebuilds, 1);
        }
        let patched_median = median(patched);
        let restaged_median = median(restaged);
        println!(
            "1k-draw scene + 1 skinned x {SKINNED_BONES} bones: patched={patched_median:?} \
             ({patched_bytes} B) vs full restage={restaged_median:?} ({restaged_bytes} B), \
             {:.2}x cheaper; ungated staging would be {ungated} B",
            restaged_median.as_secs_f64() / patched_median.as_secs_f64().max(f64::EPSILON),
        );
        assert_eq!(patched_bytes, skeleton_bytes as u64);
        assert!(
            patched_median < restaged_median,
            "animation patch ({patched_median:?}) is not cheaper than a restage ({restaged_median:?})"
        );
    });
}

/// A walking character both moves and re-poses in the same frame. The patch
/// path must handle the union: the moved draw's model row and the changed
/// palette, and nothing else.
#[test]
fn moved_and_reposed_in_one_frame_patches_both_lanes() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip transform+animation union test: no wgpu adapter");
            return;
        };
        let (mut harness, mesh, material) = upload_harness(device, queue, false);

        let mut draws: Vec<Draw3DInstance> = (0..UPLOAD_SCENE_DRAWS)
            .map(|i| regular_draw(i, mesh, material, Color::WHITE))
            .collect();
        draws.push(skinned_draw(UPLOAD_SCENE_DRAWS, mesh, material, 0.0));
        harness.prepare(&draws, true);
        let skeleton_bytes = std::mem::size_of_val(harness.gpu.staged_skeletons.as_slice());
        let cold_batches = batch_keys(&harness.gpu);

        let last = draws.len() - 1;
        draws[last] = skinned_draw(UPLOAD_SCENE_DRAWS, mesh, material, 1.0);
        draws[last].instance_mats = Arc::from([identity_at(99.0)]);
        harness.prepare(&draws, false);
        let stats = harness.gpu.prepare_upload_stats();

        let transform_row = std::mem::size_of::<TransformInstanceGpu>();
        println!(
            "moved + re-posed frame ({UPLOAD_SCENE_DRAWS} rigid + 1 skinned): patched={} B in {} \
             calls (skeleton={skeleton_bytes} transform_row={transform_row})",
            stats.write_buffer_bytes, stats.write_buffer_calls,
        );
        assert_eq!(harness.gpu.last_prepare_step_timing.full_rebuilds, 0);
        assert_eq!(batch_keys(&harness.gpu), cold_batches);
        assert_eq!(
            stats.write_buffer_bytes,
            (skeleton_bytes + transform_row) as u64,
            "the union frame must send exactly one moved instance row plus the palette"
        );
        // The moved row really landed.
        let span = harness.gpu.last_draw_instance_spans[last].clone();
        assert_eq!(
            harness.gpu.staged_instance_transforms[span.start as usize].model_row_0[3],
            99.0
        );
    });
}

/// Blend-shape weights take the same path as the bone palettes: same weight
/// count, new values, patched in place.
#[test]
fn blend_weight_only_change_stays_on_the_patch_path() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip blend-weight patch test: no wgpu adapter");
            return;
        };
        let (mut harness, mesh, material) = upload_harness(device, queue, false);

        let mut draws: Vec<Draw3DInstance> = (0..UPLOAD_SCENE_DRAWS)
            .map(|i| regular_draw(i, mesh, material, Color::WHITE))
            .collect();
        let last = draws.len() - 1;
        draws[last].blend_shape_weights = Arc::from([0.25f32, 0.5]);
        harness.prepare(&draws, true);
        let cold_batches = batch_keys(&harness.gpu);

        // The test mesh carries no blend-shape targets, so the staged weight run
        // is empty and there is nothing to patch -- but the frame must still
        // stay off the full rebuild.
        draws[last].blend_shape_weights = Arc::from([0.75f32, 0.1]);
        harness.prepare(&draws, false);
        assert_eq!(
            harness.gpu.last_prepare_step_timing.full_rebuilds, 0,
            "a blend-weight-only frame must not restage the scene"
        );
        assert_eq!(batch_keys(&harness.gpu), cold_batches);
        println!(
            "blend-weight-only frame ({UPLOAD_SCENE_DRAWS} draws): patched={} B",
            harness.gpu.prepare_upload_stats().write_buffer_bytes
        );

        // A length change is structural and must fall back to the rebuild.
        draws[last].blend_shape_weights = Arc::from([0.75f32]);
        harness.prepare(&draws, false);
        assert_eq!(
            harness.gpu.last_prepare_step_timing.full_rebuilds, 1,
            "a weight-count change must restage"
        );
    });
}

/// Rigid-only scene: nothing samples `skinned_instances`, so those rows are
/// never sent -- not even on the cold build.
#[test]
fn rigid_only_scene_never_uploads_skinned_meta() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip rigid-only upload gate test: no wgpu adapter");
            return;
        };
        let (mut harness, mesh, material) = upload_harness(device, queue, false);

        let draws: Vec<Draw3DInstance> = (0..UPLOAD_SCENE_DRAWS)
            .map(|i| regular_draw(i, mesh, material, Color::WHITE))
            .collect();

        harness.prepare(&draws, true);
        let cold = harness.gpu.prepare_upload_stats();
        assert!(!harness.gpu.staged_has_skinned);
        let skinned_bytes =
            std::mem::size_of_val(harness.gpu.staged_skinned_instance_meta.as_slice());
        assert!(
            skinned_bytes > 0,
            "rigid draws still stage a parallel skinned meta row; nothing to prove otherwise"
        );
        let transforms = std::mem::size_of_val(harness.gpu.staged_instance_transforms.as_slice());
        let rigid_meta = std::mem::size_of_val(harness.gpu.staged_rigid_instance_meta.as_slice());
        let blend_meta =
            std::mem::size_of_val(harness.gpu.staged_blend_shape_instance_meta.as_slice());
        println!(
            "rigid-only cold build ({UPLOAD_SCENE_DRAWS} draws): uploaded={} B \
             (transforms={transforms} rigid_meta={rigid_meta} blend_meta={blend_meta}), \
             skinned_meta skipped={skinned_bytes} B",
            cold.write_buffer_bytes
        );
        assert_eq!(
            cold.write_buffer_bytes,
            (transforms + rigid_meta + blend_meta) as u64,
            "a rigid-only scene must not send skinned instance meta"
        );

        // Blend-shape lanes: no mesh in this scene carries targets, so the meta
        // rows come out byte-identical every rebuild and gate away entirely.
        harness.prepare(&draws, true);
        assert_eq!(harness.gpu.prepare_upload_stats().write_buffer_bytes, 0);
    });
}

/// The transform-only fast path refreshes `last_draws` by adopting the incoming
/// draw's `Arc` lanes rather than deep-cloning the list: lanes that did not
/// change keep the exact allocation they already held, and the one lane that did
/// change points at the producer's `Arc` (no new allocation anywhere).
#[test]
fn transform_only_frame_updates_last_draws_by_refcount_only() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip last_draws adoption test: no wgpu adapter");
            return;
        };
        let (mut harness, mesh, material) = upload_harness(device, queue, false);

        let draws: Vec<Draw3DInstance> = (0..UPLOAD_SCENE_DRAWS)
            .map(|i| regular_draw(i, mesh, material, Color::WHITE))
            .collect();
        harness.prepare(&draws, true);

        let surfaces_before: Vec<*const MeshSurfaceBinding3D> = harness
            .gpu
            .last_draws
            .iter()
            .map(|draw| Arc::as_ptr(&draw.surfaces) as *const MeshSurfaceBinding3D)
            .collect();

        // Move exactly one draw; every other draw hands back the same Arcs.
        let mut moved = draws.clone();
        moved[3].instance_mats = Arc::from([identity_at(42.0)]);
        harness.prepare(&moved, false);

        for (index, draw) in harness.gpu.last_draws.iter().enumerate() {
            assert!(
                Arc::ptr_eq(&draw.surfaces, &moved[index].surfaces),
                "draw {index}: surfaces lane was deep-copied instead of adopted"
            );
            assert!(
                Arc::ptr_eq(&draw.instance_mats, &moved[index].instance_mats),
                "draw {index}: instance_mats lane was deep-copied instead of adopted"
            );
            assert_eq!(
                Arc::as_ptr(&draw.surfaces) as *const MeshSurfaceBinding3D,
                surfaces_before[index],
                "draw {index}: unchanged surfaces lane must keep its previous allocation"
            );
        }
        println!(
            "transform-only frame: {UPLOAD_SCENE_DRAWS} draws, last_draws refreshed with 0 lane \
             reallocations ({} B patched)",
            harness.gpu.prepare_upload_stats().write_buffer_bytes
        );
    });
}

/// Same skeleton-only frame, but with enough batches to turn the GPU frustum
/// cull on. Nothing in the cull inputs depends on a bone palette, so the whole
/// cull side -- both read-only halves AND the indirect commands, now that the
/// shaders source the instance count from the static record -- must stay off
/// the wire.
#[test]
fn skeleton_only_change_skips_the_frustum_cull_uploads() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip frustum-cull upload gate test: no wgpu adapter");
            return;
        };
        let (mut harness, mesh, material) = upload_harness(device, queue, true);

        // Same-material draws compact into one batch, so the cull is reached
        // via FRUSTUM_CULL_MIN_INSTANCES rather than the batch-count threshold.
        const CULLED_DRAWS: u32 = 1100;
        let mut draws: Vec<Draw3DInstance> = (0..CULLED_DRAWS)
            .map(|i| regular_draw(i, mesh, material, Color::WHITE))
            .collect();
        draws.push(skinned_draw(CULLED_DRAWS, mesh, material, 0.0));
        harness.prepare(&draws, true);

        let cull_active = harness.gpu.should_run_frustum_cull();
        let cold = harness.gpu.prepare_upload_stats();
        let skeleton_bytes = std::mem::size_of_val(harness.gpu.staged_skeletons.as_slice());
        let indirect_bytes = std::mem::size_of_val(harness.gpu.indirect_staging.as_slice());
        let cull_bytes = std::mem::size_of_val(harness.gpu.frustum_cull_static_staging.as_slice())
            + std::mem::size_of_val(harness.gpu.frustum_cull_dynamic_staging.as_slice());
        let cold_batches = batch_keys(&harness.gpu);
        // The static cull rows carry the authoritative instance count the cull
        // shaders write back, so it must match the command they replace.
        for (row, command) in harness
            .gpu
            .frustum_cull_static_staging
            .iter()
            .zip(harness.gpu.indirect_staging.iter())
        {
            assert_eq!(row.cull_flags[1], command.instance_count);
        }

        let last = draws.len() - 1;
        draws[last] = skinned_draw(CULLED_DRAWS, mesh, material, 1.0);
        harness.prepare(&draws, false);
        let animated = harness.gpu.prepare_upload_stats();

        println!(
            "skeleton-only frame w/ frustum cull ({CULLED_DRAWS} rigid + 1 skinned, \
             cull_active={cull_active}): cold={} B, patched={} B \
             (skeleton={skeleton_bytes} indirect_skipped={indirect_bytes} \
             cull_inputs_skipped={cull_bytes})",
            cold.write_buffer_bytes, animated.write_buffer_bytes,
        );
        assert!(cull_active, "scene did not reach the GPU frustum-cull threshold");
        assert!(cull_bytes > 0 && indirect_bytes > 0);
        assert_eq!(
            harness.gpu.last_prepare_step_timing.full_rebuilds, 0,
            "a skeleton-only frame must not restage the scene"
        );
        assert_eq!(batch_keys(&harness.gpu), cold_batches);
        assert_eq!(
            animated.write_buffer_bytes, skeleton_bytes as u64,
            "cull inputs and indirect commands must both gate away on an animated frame"
        );
    });
}

/// The indirect commands are now skip-gated like every other staging lane. A
/// forced restage that reproduces the same batch topology with the cull active
/// must therefore send nothing: the cull dispatch rebuilds every
/// `instance_count` from the static record, so the CPU copy on the GPU stays
/// authoritative without a re-upload.
#[test]
fn identical_restage_gates_the_indirect_upload_with_cull_active() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip indirect gate test: no wgpu adapter");
            return;
        };
        let (mut harness, mesh, material) = upload_harness(device, queue, true);

        const CULLED_DRAWS: u32 = 1100;
        let draws: Vec<Draw3DInstance> = (0..CULLED_DRAWS)
            .map(|i| regular_draw(i, mesh, material, Color::WHITE))
            .collect();
        harness.prepare(&draws, true);
        assert!(
            harness.gpu.should_run_frustum_cull(),
            "scene did not reach the GPU frustum-cull threshold"
        );
        let indirect_bytes = std::mem::size_of_val(harness.gpu.indirect_staging.as_slice());
        assert!(indirect_bytes > 0);
        assert!(
            harness.gpu.indirect_counts_gpu_dirty,
            "an active cull leaves the GPU counts overwritten"
        );

        harness.prepare(&draws, true);
        let repeat = harness.gpu.prepare_upload_stats();
        println!(
            "identical forced restage w/ cull active ({CULLED_DRAWS} draws): \
             uploaded={} B, indirect gated={indirect_bytes} B",
            repeat.write_buffer_bytes
        );
        assert_eq!(
            repeat.write_buffer_bytes, 0,
            "an identical forced restage must upload nothing, indirect included"
        );
        assert_eq!(harness.gpu.last_prepare_step_timing.full_rebuilds, 1);
    });
}

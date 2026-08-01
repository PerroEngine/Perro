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
    StandardMaterial3D, VertexModifier3D,
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
        let gpu = new_gpu_3d(&device, &queue);
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

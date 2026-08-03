//! Scene-transition pipeline-warm cost.
//!
//! A scene load pushes every material it creates onto the backend's
//! `pending_pipeline_warms` queue, and the backend used to drain that whole
//! queue in the single frame before `render`. Each *new* builtin feature combo
//! costs one WGSL parse/validate plus four `create_render_pipeline` calls per
//! render path, so a scene with a few dozen distinct material shapes paid tens
//! of milliseconds in one frame - the scene-transition spike.
//!
//! These cases run against a headless wgpu device and are skipped with a note
//! when no adapter is available. They pin:
//!
//! * cold warm cost is real and per-*distinct-combo* (repeat materials are
//!   cache hits, so the spike scales with combo count, not material count);
//! * [`Gpu3D::warm_material_pipelines_budgeted`] never compiles more than the
//!   budget it is handed, and reports how much it actually did;
//! * a full drain and a budgeted drain reach the same cache state.
use super::*;
use perro_render_bridge::StandardMaterial3D;
use std::time::Instant;

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
    let info = adapter.get_info();
    eprintln!(
        "[pipeline-warm] adapter {} ({:?}, {:?})",
        info.name, info.device_type, info.backend
    );
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("perro_pipeline_warm_test_device"),
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
            // Auto is the shipping default and the one that builds per-feature
            // variant pipelines - the combo explosion this file measures.
            shader_variant_mode: crate::ShaderVariantMode::Auto,
            shadow_pcf_high: false,
            shadow_scale_to_target: false,
        },
        pipelines,
        arena,
    )
}

/// `i` distinct texture-slot / alpha-mode combos, so every material maps to its
/// own `MaterialShaderFeatures` and therefore its own variant pipeline set.
fn combo_material(i: u32) -> Arc<Material3D> {
    let bit = |n: u32| {
        if i & (1 << n) != 0 {
            0
        } else {
            perro_render_bridge::MATERIAL_TEXTURE_NONE
        }
    };
    Arc::new(Material3D::Standard(StandardMaterial3D {
        base_color_texture: bit(0),
        metallic_roughness_texture: bit(1),
        normal_texture: bit(2),
        occlusion_texture: bit(3),
        emissive_texture: bit(4),
        alpha_mode: (i >> 5) as u8 % 3,
        ..StandardMaterial3D::default()
    }))
}

#[test]
fn scene_load_pipeline_warm_cost_is_per_distinct_combo() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("[skip] no wgpu adapter for pipeline warm test");
        return;
    };
    let arena = SharedMeshArena::new(&device, false, false);
    let mut gpu = new_gpu_3d(&device, &queue, &arena);

    // Scene A: 16 distinct combos, each material repeated 4x (a real scene has
    // far more materials than distinct shader shapes).
    let scene_a: Vec<Arc<Material3D>> = (0..64).map(|i| combo_material(i % 16)).collect();
    let mut queued = scene_a.clone();
    let cold = Instant::now();
    gpu.warm_material_pipelines_budgeted(&device, &mut queued, None, usize::MAX, None);
    let cold = cold.elapsed();
    assert!(queued.is_empty(), "unbounded budget must drain the queue");

    let mut queued = scene_a;
    let warm = Instant::now();
    gpu.warm_material_pipelines_budgeted(&device, &mut queued, None, usize::MAX, None);
    let warm = warm.elapsed();

    eprintln!(
        "[pipeline-warm] 64 materials / 16 combos: cold {:.2}ms ({:.2}ms per combo), \
         re-warm {:.2}ms, variant pipelines {}",
        cold.as_secs_f64() * 1e3,
        cold.as_secs_f64() * 1e3 / 16.0,
        warm.as_secs_f64() * 1e3,
        gpu.builtin_variant_pipeline_count(),
    );
    // Repeats are pure cache hits: the second pass must be a small fraction of
    // the first, or the cache key is wrong and every scene pays full compile.
    assert!(
        warm * 4 < cold,
        "re-warm {warm:?} should be far cheaper than cold {cold:?}"
    );
}

/// Splits one cold combo into its two halves - WGSL module creation and the
/// four `create_render_pipeline` calls - so the fix targets the half that
/// actually costs. Diagnostic only; asserts nothing about absolute time.
#[test]
fn pipeline_warm_cost_splits_between_module_and_pipeline_creation() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("[skip] no wgpu adapter for pipeline warm test");
        return;
    };
    let arena = SharedMeshArena::new(&device, false, false);
    let gpu = new_gpu_3d(&device, &queue, &arena);
    let registry = gpu.pipeline_registry_for_test();
    let features = MaterialShaderFeatures::new(true, true, true, false, false, true, 0, false);

    let module_start = Instant::now();
    let shader =
        create_standard_shader_module_rigid_variant(&device, BuiltinShaderKind::Standard, features);
    let module = module_start.elapsed();

    let first_start = Instant::now();
    let _p0 = create_pipeline_rigid(
        &device,
        registry.rigid_material_layout(),
        &shader,
        COLOR_FORMAT,
        1,
        Some(wgpu::Face::Back),
    );
    let first = first_start.elapsed();

    let rest_start = Instant::now();
    let _p1 = create_pipeline_rigid(
        &device,
        registry.rigid_material_layout(),
        &shader,
        COLOR_FORMAT,
        1,
        None,
    );
    let _p2 = create_pipeline_rigid_blend(
        &device,
        registry.rigid_material_layout(),
        &shader,
        COLOR_FORMAT,
        1,
        Some(wgpu::Face::Back),
    );
    let _p3 = create_pipeline_rigid_blend(
        &device,
        registry.rigid_material_layout(),
        &shader,
        COLOR_FORMAT,
        1,
        None,
    );
    let rest = rest_start.elapsed();

    eprintln!(
        "[pipeline-warm] one rigid combo: module {:.2}ms, first pipeline {:.2}ms, \
         other 3 pipelines {:.2}ms",
        module.as_secs_f64() * 1e3,
        first.as_secs_f64() * 1e3,
        rest.as_secs_f64() * 1e3,
    );
}

#[test]
fn budgeted_warm_never_exceeds_its_budget_and_reaches_the_same_state() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("[skip] no wgpu adapter for pipeline warm test");
        return;
    };
    let arena = SharedMeshArena::new(&device, false, false);

    let scene: Vec<Arc<Material3D>> = (0..12).map(combo_material).collect();

    let mut full = new_gpu_3d(&device, &queue, &arena);
    let mut queued = scene.clone();
    full.warm_material_pipelines_budgeted(&device, &mut queued, None, usize::MAX, None);
    assert!(queued.is_empty());
    let full_count = full.builtin_variant_pipeline_count();
    assert!(full_count > 0, "warming must build variant pipelines");

    let mut budgeted = new_gpu_3d(&device, &queue, &arena);
    let mut queued = scene;
    let mut frames = 0_u32;
    let mut worst_frame = std::time::Duration::ZERO;
    while !queued.is_empty() {
        let before = budgeted.builtin_variant_pipeline_count();
        let frame = Instant::now();
        let compiled =
            budgeted.warm_material_pipelines_budgeted(&device, &mut queued, None, 2, None);
        worst_frame = worst_frame.max(frame.elapsed());
        assert!(compiled <= 2, "budget of 2 honored, got {compiled}");
        // Only *new* combos count against the budget; cache hits are free and
        // must not stall the drain.
        assert!(
            budgeted.builtin_variant_pipeline_count() >= before,
            "warming must not drop pipelines"
        );
        frames += 1;
        assert!(frames < 64, "budgeted drain must terminate");
    }
    assert!(
        frames > 1,
        "a 12-combo scene must spread over several frames"
    );
    assert_eq!(
        budgeted.builtin_variant_pipeline_count(),
        full_count,
        "budgeted drain must reach the same cache state as a full drain"
    );
    eprintln!(
        "[pipeline-warm] 12 combos at budget 2/frame: {frames} frames, \
         worst frame {:.2}ms",
        worst_frame.as_secs_f64() * 1e3,
    );
}

/// The whole point, end to end: the same scene switch measured as a per-frame
/// profile with the queue drained unbounded (the old behaviour) and with the
/// shipping budget. Reports the spike shape rather than asserting a wall time -
/// absolute numbers are GPU- and driver-dependent - but does pin that the worst
/// budgeted frame is a fraction of the unbounded one.
#[test]
fn budgeting_the_warm_queue_flattens_the_transition_spike() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("[skip] no wgpu adapter for pipeline warm test");
        return;
    };
    let arena = SharedMeshArena::new(&device, false, false);
    // Scene B arriving after scene A: 16 distinct shapes, 4 materials each.
    let scene: Vec<Arc<Material3D>> = (0..64).map(|i| combo_material(i % 16)).collect();

    let mut unbounded = new_gpu_3d(&device, &queue, &arena);
    let mut queued = scene.clone();
    let one_frame = Instant::now();
    unbounded.warm_material_pipelines_budgeted(&device, &mut queued, None, usize::MAX, None);
    let one_frame = one_frame.elapsed();

    let mut budgeted = new_gpu_3d(&device, &queue, &arena);
    let mut queued = scene;
    let mut per_frame = Vec::new();
    while !queued.is_empty() {
        let frame = Instant::now();
        budgeted.warm_material_pipelines_budgeted(
            &device,
            &mut queued,
            None,
            2,
            Some(std::time::Duration::from_millis(6)),
        );
        per_frame.push(frame.elapsed());
        assert!(per_frame.len() < 128, "budgeted drain must terminate");
    }
    let worst = per_frame.iter().copied().max().unwrap_or_default();
    let total: std::time::Duration = per_frame.iter().sum();
    eprintln!(
        "[pipeline-warm] transition profile - unbounded: 1 frame @ {:.1}ms | \
         budgeted: {} frames, worst {:.1}ms, total {:.1}ms",
        one_frame.as_secs_f64() * 1e3,
        per_frame.len(),
        worst.as_secs_f64() * 1e3,
        total.as_secs_f64() * 1e3,
    );
    assert!(
        worst * 3 < one_frame,
        "budgeted worst frame {worst:?} must be well under the unbounded {one_frame:?}"
    );
}

/// Speculative warming used to compile Rigid + Skinned + MultiMesh for every
/// material, so a scene with no skinned geometry paid twice what it needed.
/// Warming now follows the paths the instance has actually drawn (Rigid always,
/// since that is what a plain static mesh takes).
#[test]
fn warm_only_compiles_render_paths_the_instance_actually_draws() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("[skip] no wgpu adapter for pipeline warm test");
        return;
    };
    let arena = SharedMeshArena::new(&device, false, false);
    let mut gpu = new_gpu_3d(&device, &queue, &arena);

    let mut queued = vec![combo_material(1)];
    gpu.warm_material_pipelines_budgeted(&device, &mut queued, None, usize::MAX, None);
    assert_eq!(
        gpu.builtin_variant_pipeline_count(),
        1,
        "a rigid-only instance must warm the rigid variant and nothing else"
    );

    // Once a skinned draw has been resolved, later warms cover that path too.
    gpu.note_render_path_for_test(RenderPath3D::Skinned);
    let mut queued = vec![combo_material(2)];
    gpu.warm_material_pipelines_budgeted(&device, &mut queued, None, usize::MAX, None);
    assert_eq!(
        gpu.builtin_variant_pipeline_count(),
        3,
        "a skinned-aware instance warms both paths for the new combo"
    );
}

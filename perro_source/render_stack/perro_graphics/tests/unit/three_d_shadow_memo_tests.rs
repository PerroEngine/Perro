//! `update_shadow_state`'s input memo vs an animated sky.
//!
//! Shadow output depends on light directions/counts plus the caster set. Sky
//! colour and `sky_time_seconds` do not reach any shadow depth pass or cascade
//! fit -- but they advance every frame while a `Sky3D` runs unpaused. Comparing
//! them here (the general `Lighting3DState::content_eq` does) re-ran the whole
//! O(draw_batches) shadow setup every frame under a parked camera.
//!
//! The GPU half runs against a headless wgpu device and is skipped with a note
//! when no adapter is available; the compare half is pure CPU and always runs.
use super::*;
use perro_render_bridge::{
    AmbientLight3DState, RayLight3DState, Sky3DState, SkyTime3DState, SpotLight3DState,
};

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
            label: Some("perro_shadow_memo_test_device"),
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
            width: 64,
            height: 64,
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

fn animated_sky() -> Sky3DState {
    Sky3DState {
        day_colors: Arc::from([[0.4, 0.6, 0.9]]),
        evening_colors: Arc::from([[0.8, 0.4, 0.2]]),
        night_colors: Arc::from([[0.02, 0.03, 0.08]]),
        horizon_colors: Arc::from([[0.7, 0.7, 0.8]]),
        time: SkyTime3DState {
            time_of_day: 0.5,
            paused: false,
            scale: 1.0,
        },
        shaders: Arc::from([]),
        environment: None,
    }
}

/// Lighting for frame `frame` of an unpaused sky: identical lights, sky clock
/// advanced. This is exactly what the renderer hands over every frame while a
/// `Sky3D` runs.
fn sky_frame(frame: u32) -> Lighting3DState {
    let mut lighting = Lighting3DState {
        ambient_light: Some(AmbientLight3DState {
            color: [1.0, 1.0, 1.0],
            intensity: 0.2,
            cast_shadows: false,
        }),
        sky: Some(animated_sky()),
        sky_time_seconds: frame as f32 * (1.0 / 60.0),
        ..Lighting3DState::default()
    };
    lighting.ray_lights[0] = Some(RayLight3DState {
        direction: [-0.4, -1.0, -0.3],
        color: [1.0, 1.0, 1.0],
        intensity: 1.0,
        cast_shadows: true,
        shadow_strength: 0.82,
        shadow_depth_bias: 0.00018,
        shadow_normal_bias: 0.045,
    });
    // Frame globals advance too; they are outside both compares.
    lighting.frame_index = frame;
    lighting.frame_time_seconds = frame as f32 * (1.0 / 60.0);
    lighting
}

fn memo_camera() -> Camera3DState {
    Camera3DState {
        position: [0.0, 2.0, 8.0],
        rotation: Quat::IDENTITY.to_array(),
        projection: CameraProjectionState::Perspective {
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 100.0,
        },
        render_mask: BitMask::NONE,
        post_processing: Arc::from([]),
        audio_options: perro_structs::AudioListenerOptions::new(),
    }
}

/// The two compares must disagree about the sky: the shadow memo ignores it,
/// the prepare change gate must not (the scene itself re-renders as the sky
/// animates).
#[test]
fn shadow_input_compare_ignores_the_sky_clock_but_content_eq_does_not() {
    let a = sky_frame(0);
    let b = sky_frame(1);
    assert_ne!(a.sky_time_seconds, b.sky_time_seconds);
    assert!(
        a.shadow_input_eq(&b),
        "an advancing sky clock must not read as a shadow input change"
    );
    assert!(
        !a.content_eq(&b),
        "the prepare gate still needs the sky clock -- the scene must re-render"
    );

    // Ambient is shading-only for shadows as well.
    let mut dim_ambient = sky_frame(0);
    dim_ambient.ambient_light = Some(AmbientLight3DState {
        color: [1.0, 0.0, 0.0],
        intensity: 0.9,
        cast_shadows: false,
    });
    assert!(a.shadow_input_eq(&dim_ambient));
    assert!(!a.content_eq(&dim_ambient));

    // But every light the shadow setup actually reads still breaks it.
    let mut moved_sun = sky_frame(0);
    if let Some(sun) = moved_sun.ray_lights[0].as_mut() {
        sun.direction = [0.4, -1.0, 0.3];
    }
    assert!(
        !a.shadow_input_eq(&moved_sun),
        "a rotated sun is a shadow input change"
    );

    let mut no_sun = sky_frame(0);
    no_sun.ray_lights[0] = None;
    assert!(!a.shadow_input_eq(&no_sun));

    let mut lit_spot = sky_frame(0);
    lit_spot.spot_lights[0] = Some(SpotLight3DState {
        position: [0.0, 4.0, 0.0],
        direction: [0.0, -1.0, 0.0],
        color: [1.0, 1.0, 1.0],
        intensity: 5.0,
        range: 20.0,
        inner_angle_radians: 0.26,
        outer_angle_radians: 0.52,
        cast_shadows: true,
        shadow_strength: 0.8,
        shadow_depth_bias: 0.0002,
        shadow_normal_bias: 0.04,
    });
    assert!(!a.shadow_input_eq(&lit_spot));
}

/// End to end: a parked camera under an unpaused sky must stop re-running the
/// shadow setup. Before the shadow-specific compare, `sky_time_seconds` moved
/// every frame and the memo never hit.
#[test]
fn animated_sky_does_not_defeat_the_shadow_input_memo() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        eprintln!("no wgpu adapter; skipping shadow input memo test");
        return;
    };
    let arena = SharedMeshArena::new(&device, false, false);
    let mut gpu = new_gpu_3d(&device, &queue, &arena);
    let camera = memo_camera();

    const FRAMES: u32 = 120;
    let mut runs_per_frame = Vec::with_capacity(FRAMES as usize);
    for frame in 0..FRAMES {
        let before = gpu.shadow_setup_run_count;
        gpu.update_shadow_state(&device, &queue, &camera, &sky_frame(frame), true);
        runs_per_frame.push(gpu.shadow_setup_run_count - before);
    }

    let total = gpu.shadow_setup_run_count;
    // Warm-up may take several frames (the cascade round-robin budget keeps the
    // memo open until every pending cascade is served). What must not happen is
    // one run per frame forever.
    let tail: u64 = runs_per_frame[(FRAMES as usize - 40)..].iter().sum();
    assert_eq!(
        tail, 0,
        "shadow setup still re-ran in the last 40 frames of a parked camera under an \
         animated sky (per-frame runs: {runs_per_frame:?})"
    );
    assert!(
        total < FRAMES as u64 / 2,
        "shadow setup ran {total} times over {FRAMES} static frames; the sky clock is \
         defeating the memo"
    );
    eprintln!("shadow setup runs over {FRAMES} animated-sky frames: {total}");

    // The memo is closed, not broken: a real shadow input change still gets
    // through.
    let mut moved_sun = sky_frame(FRAMES);
    if let Some(sun) = moved_sun.ray_lights[0].as_mut() {
        sun.direction = [0.5, -1.0, 0.4];
    }
    gpu.update_shadow_state(&device, &queue, &camera, &moved_sun, true);
    assert!(
        gpu.shadow_setup_run_count > total,
        "a rotated sun must re-run the shadow setup"
    );
}

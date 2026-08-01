use super::*;
use perro_render_bridge::{ParticleExprOp3D, ParticleProfile3D};
use perro_structs::{AudioListenerOptions, BitMask, Color};
use std::borrow::Cow;

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
            label: Some("perro_particles_upload_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

fn compute_emitter(simulation_time: f32) -> PointParticles3DState {
    let ops: Cow<'static, [ParticleExprOp3D]> = Cow::Owned(vec![
        ParticleExprOp3D::T,
        ParticleExprOp3D::Const(2.0),
        ParticleExprOp3D::Mul,
    ]);
    let profile = ParticleProfile3D {
        path: ParticlePath3D::CustomCompiled {
            expr_x_ops: ops.clone(),
            expr_y_ops: ops.clone(),
            expr_z_ops: ops,
        },
        ..ParticleProfile3D::default()
    };
    PointParticles3DState {
        model: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        active: true,
        looping: true,
        prewarm: true,
        alive_budget: 64,
        emission_rate: 32.0,
        lifetime_min: 0.6,
        lifetime_max: 1.4,
        speed_min: 1.0,
        speed_max: 3.0,
        spread_radians: 0.5,
        size: 0.2,
        size_min: 0.1,
        size_max: 0.3,
        gravity: [0.0, -9.8, 0.0],
        color_start: Color::WHITE,
        color_end: Color::WHITE,
        emissive: [0.0, 0.0, 0.0],
        seed: 7,
        // Static custom params: identical every frame.
        params: vec![1.0, 2.0, 3.0, 4.0],
        // Only the clock advances between prepares.
        simulation_time,
        simulation_delta: 1.0 / 60.0,
        profile: std::sync::Arc::new(profile),
        sim_mode: ParticleSimulationMode3D::GpuCompute,
        render_mode: ParticleRenderMode3D::Point,
    }
}

fn test_camera() -> Camera3DState {
    Camera3DState {
        position: [0.0, 0.0, 5.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        projection: CameraProjectionState::Perspective {
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 1000.0,
        },
        render_mask: BitMask::ALL,
        post_processing: std::sync::Arc::from([]),
        audio_options: AudioListenerOptions::new(),
    }
}

#[test]
fn repeat_prepare_uploads_static_compute_config_once() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        // No adapter available in this environment.
        return;
    };
    if !gpu_compute_particles_enabled() {
        return;
    }
    let mut gpu =
        GpuPointParticles3D::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb, 1);

    let prepare = |gpu: &mut GpuPointParticles3D, time: f32| {
        let emitters = [(NodeID::from_parts(4, 0), compute_emitter(time))];
        gpu.prepare(
            &device,
            &queue,
            PreparePointParticles3D {
                camera: test_camera(),
                emitters: &emitters,
                width: 256,
                height: 256,
            },
        );
    };

    prepare(&mut gpu, 0.0);
    let after_first = gpu.compute_config_upload_count();
    // One expr-op run + one custom-param run on the first frame.
    assert_eq!(after_first, 2);

    // Emitter records still carry the advancing clock, but the expression
    // program and the custom param block are unchanged - no re-upload.
    for frame in 1..=6 {
        prepare(&mut gpu, frame as f32 / 60.0);
    }
    assert_eq!(gpu.compute_config_upload_count(), after_first);
}

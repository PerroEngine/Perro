use super::*;
use crate::resources::ResourceStore;
use perro_structs::Color;

const TEST_TEXTURE_SOURCE: &str = "__perro_two_d_upload_test__";

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
            label: Some("perro_two_d_upload_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

fn camera_at(x: f32, y: f32) -> Camera2DUniform {
    Camera2DUniform {
        view: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [-x, -y, 0.0, 1.0],
        ],
        // Wide NDC scale so a small pan keeps the whole sprite set on screen.
        ndc_scale: [1.0 / 960.0, 1.0 / 540.0],
        pad: [0.0, 0.0],
    }
}

fn sprite_at(texture: TextureID, x: f32, y: f32) -> Sprite2DCommand {
    Sprite2DCommand {
        texture,
        model: [[32.0, 0.0, 0.0], [0.0, 32.0, 0.0], [x, y, 1.0]],
        tint: Color::WHITE,
        z_index: 0,
        ..Sprite2DCommand::default()
    }
}

#[test]
fn sprite_instance_content_key_tracks_bytes() {
    let base = SpriteInstanceGpu {
        transform_0: [1.0, 0.0],
        transform_1: [0.0, 1.0],
        translation: [4.0, 5.0],
        uv_min: [0.0, 0.0],
        uv_max: [1.0, 1.0],
        size: [8.0, 8.0],
        z_index: 0,
        tint: [255, 255, 255, 255],
    };
    let run = [base, base];
    assert_eq!(
        sprite_instance_content_key(&run),
        sprite_instance_content_key(&run)
    );
    let mut moved = base;
    moved.translation[0] = 4.5;
    assert_ne!(
        sprite_instance_content_key(&run),
        sprite_instance_content_key(&[base, moved])
    );
    assert_ne!(
        sprite_instance_content_key(&run),
        sprite_instance_content_key(&[base])
    );
}

#[test]
fn camera_pan_with_unchanged_visible_set_skips_sprite_upload() {
    let Some((device, queue)) = pollster::block_on(test_device()) else {
        // No adapter available in this environment.
        return;
    };
    let mut gpu = Gpu2D::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        1,
        TextureFilterMode::Nearest,
    );
    let mut resources = ResourceStore::new();
    let texture = resources.create_texture(TEST_TEXTURE_SOURCE, true);
    let mut shared_textures = SharedTextureStore::default();
    // Seed the shared upload so `ensure_sprite_texture` resolves without a
    // decode of real image bytes.
    shared_textures.ensure_rgba(
        &device,
        &queue,
        SharedTextureKey::from_source(
            TEST_TEXTURE_SOURCE,
            SharedTextureColorSpace::Srgb,
            TextureFilterMode::Nearest,
        ),
        &[255u8; 4],
        1,
        1,
    );

    let sprites = [
        sprite_at(texture, -64.0, 0.0),
        sprite_at(texture, 64.0, 0.0),
    ];
    let upload = RectUploadPlan {
        full_reupload: true,
        dirty_ranges: Vec::new(),
        draw_count: 0,
    };
    let run_prepare = |gpu: &mut Gpu2D,
                           shared_textures: &mut SharedTextureStore,
                           camera: Camera2DUniform| {
        gpu.prepare(
            &device,
            &queue,
            Prepare2D {
                resources: &resources,
                shared_textures,
                camera,
                rects: &[],
                upload: &upload,
                sprites: &sprites,
                sprites_revision: 1,
                force_sprite_prepare: false,
                point_lights: &[],
                point_lights_revision: 1,
                shadow_casters: &[],
                shadow_casters_revision: 1,
                static_texture_lookup: None,
            },
        );
    };

    run_prepare(&mut gpu, &mut shared_textures, camera_at(0.0, 0.0));
    let after_first = gpu.sprite_instance_upload_count();
    assert_eq!(after_first, 1, "first frame must push the instance run");
    assert_eq!(gpu.sprite_batch_count(), 1);

    // Camera pans; both sprites stay on screen, so the culled instance run is
    // byte-identical and the upload is elided.
    for step in 1..=5 {
        run_prepare(
            &mut gpu,
            &mut shared_textures,
            camera_at(step as f32 * 3.0, step as f32 * -2.0),
        );
    }
    assert_eq!(
        gpu.sprite_instance_upload_count(),
        after_first,
        "camera pan with an unchanged visible set must not re-upload"
    );

    // Pan far enough to cull the right-hand sprite (world half extent 16, so
    // its bounds leave the +X edge first): the run shrinks and re-uploads.
    run_prepare(&mut gpu, &mut shared_textures, camera_at(-1000.0, 0.0));
    assert_eq!(gpu.sprite_batch_count(), 1);
    assert_eq!(gpu.sprite_instance_upload_count(), after_first + 1);
}

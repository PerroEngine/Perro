use crate::{
    backend::{OcclusionCullingMode, StaticMeshLookup, StaticTextureLookup},
    resources::ResourceStore,
    three_d::{
        gpu::{Gpu3D, Gpu3DConfig, Prepare3D},
        particles::gpu::{GpuPointParticles3D, PreparePointParticles3D},
        renderer::{Draw3DInstance, Lighting3DState},
    },
    two_d::{
        gpu::{Gpu2D, Prepare2D},
        renderer::{Camera2DUniform, RectInstanceGpu, RectUploadPlan},
    },
};
use perro_ids::NodeID;
use perro_render_bridge::{Camera3DState, PointParticles3DState, Sprite2DCommand};
use std::sync::Arc;
use winit::window::Window;

// Linear-space clear color for sRGB hex #1C1817.
const CLEAR_R: f64 = 0.011612245179743885;
const CLEAR_G: f64 = 0.009134058702220787;
const CLEAR_B: f64 = 0.008568125618069307;
const SMOOTH_SAMPLE_COUNT: u32 = 4;

struct MsaaColorTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

pub struct Gpu {
    window_handle: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_format: wgpu::TextureFormat,
    sample_count: u32,
    msaa_color: Option<MsaaColorTarget>,
    two_d: Gpu2D,
    three_d: Gpu3D,
    point_particles_3d: GpuPointParticles3D,
}

pub struct RenderFrame<'a> {
    pub resources: &'a ResourceStore,
    pub camera_3d: Camera3DState,
    pub lighting_3d: &'a Lighting3DState,
    pub draws_3d: &'a [Draw3DInstance],
    pub point_particles_3d: &'a [(NodeID, PointParticles3DState)],
    pub camera_2d: Camera2DUniform,
    pub rects_2d: &'a [RectInstanceGpu],
    pub upload_2d: &'a RectUploadPlan,
    pub sprites_2d: &'a [Sprite2DCommand],
    pub static_texture_lookup: Option<StaticTextureLookup>,
    pub static_mesh_lookup: Option<StaticMeshLookup>,
}

impl Gpu {
    pub fn new(
        window: Arc<Window>,
        smoothing_samples: u32,
        vsync_enabled: bool,
        meshlets_enabled: bool,
        dev_meshlets: bool,
        meshlet_debug_view: bool,
        occlusion_culling: OcclusionCullingMode,
    ) -> Option<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).ok()?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok()?;
        let adapter_features = adapter.features();
        let mut required_features = wgpu::Features::empty();
        if adapter_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE) {
            required_features |= wgpu::Features::INDIRECT_FIRST_INSTANCE;
        }

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("perro_device"),
            required_features,
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        }))
        .ok()?;
        let indirect_first_instance_enabled =
            required_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE);
        if !indirect_first_instance_enabled {
            eprintln!(
                "[perro][3d] INDIRECT_FIRST_INSTANCE not supported by adapter; falling back to CPU frustum path"
            );
        }
        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let render_format = surface_format;
        let present_mode = choose_present_mode(&caps.present_modes, vsync_enabled);
        let alpha_mode = caps.alpha_modes[0];
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let sample_count = normalize_sample_count(smoothing_samples);
        let two_d = Gpu2D::new(&device, render_format, sample_count);
        let three_d = Gpu3D::new(
            &device,
            render_format,
            Gpu3DConfig {
                sample_count,
                width,
                height,
                meshlets_enabled,
                dev_meshlets,
                meshlet_debug_view,
                occlusion_culling,
                indirect_first_instance_enabled,
            },
        );
        let point_particles_3d = GpuPointParticles3D::new(&device, render_format, sample_count);
        let msaa_color =
            create_msaa_color_target(&device, render_format, width, height, sample_count);

        Some(Self {
            window_handle: window,
            surface,
            device,
            queue,
            config,
            render_format,
            sample_count,
            msaa_color,
            two_d,
            three_d,
            point_particles_3d,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.three_d.resize(&self.device, width, height);
        self.msaa_color = create_msaa_color_target(
            &self.device,
            self.render_format,
            width,
            height,
            self.sample_count,
        );
    }

    pub fn set_smoothing_samples(&mut self, samples: u32) {
        let sample_count = normalize_sample_count(samples);
        if sample_count == self.sample_count {
            return;
        }
        self.sample_count = sample_count;
        self.two_d
            .set_sample_count(&self.device, self.render_format, sample_count);
        self.three_d.set_sample_count(
            &self.device,
            self.render_format,
            sample_count,
            self.config.width,
            self.config.height,
        );
        self.point_particles_3d
            .set_sample_count(&self.device, self.render_format, sample_count);
        self.msaa_color = create_msaa_color_target(
            &self.device,
            self.render_format,
            self.config.width,
            self.config.height,
            sample_count,
        );
    }

    pub fn render(&mut self, frame: RenderFrame<'_>) {
        let RenderFrame {
            resources,
            camera_3d,
            lighting_3d,
            draws_3d,
            point_particles_3d,
            camera_2d,
            rects_2d,
            upload_2d,
            sprites_2d,
            static_texture_lookup,
            static_mesh_lookup,
        } = frame;
        // Keep window alive for the full surface lifetime.
        self.window_handle.id();

        self.two_d.prepare(
            &self.device,
            &self.queue,
            Prepare2D {
                resources,
                camera: camera_2d,
                rects: rects_2d,
                upload: upload_2d,
                sprites: sprites_2d,
                static_texture_lookup,
            },
        );
        self.three_d.prepare(
            &self.device,
            &self.queue,
            Prepare3D {
                resources,
                camera: camera_3d,
                lighting: lighting_3d,
                draws: draws_3d,
                width: self.config.width,
                height: self.config.height,
                static_mesh_lookup,
            },
        );
        self.point_particles_3d.prepare(
            &self.device,
            &self.queue,
            PreparePointParticles3D {
                camera: camera_3d,
                emitters: point_particles_3d,
                width: self.config.width,
                height: self.config.height,
            },
        );

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => return,
            Err(wgpu::SurfaceError::Timeout) => return,
            Err(wgpu::SurfaceError::Other) => return,
        };

        let swap_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let color_view = self
            .msaa_color
            .as_ref()
            .map(|t| &t.view)
            .unwrap_or(&swap_view);
        let resolve_view = if self.sample_count > 1 {
            Some(&swap_view)
        } else {
            None
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_main_encoder"),
            });

        self.three_d.render_pass(
            &self.device,
            &mut encoder,
            color_view,
            wgpu::Color {
                r: CLEAR_R,
                g: CLEAR_G,
                b: CLEAR_B,
                a: 1.0,
            },
        );
        self.point_particles_3d
            .render_pass(&mut encoder, color_view, self.three_d.depth_view());
        self.two_d.render_pass(
            &mut encoder,
            color_view,
            resolve_view,
            upload_2d.draw_count as u32,
        );

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    pub fn virtual_size() -> [f32; 2] {
        Gpu2D::virtual_size()
    }
}

fn normalize_sample_count(samples: u32) -> u32 {
    match samples {
        0 | 1 => 1,
        2 => 2,
        _ => SMOOTH_SAMPLE_COUNT,
    }
}

fn create_msaa_color_target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    sample_count: u32,
) -> Option<MsaaColorTarget> {
    if sample_count <= 1 {
        return None;
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_msaa_color"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Some(MsaaColorTarget {
        _texture: texture,
        view,
    })
}

fn choose_present_mode(modes: &[wgpu::PresentMode], vsync_enabled: bool) -> wgpu::PresentMode {
    let preferred = if vsync_enabled {
        [
            wgpu::PresentMode::FifoRelaxed,
            wgpu::PresentMode::Fifo,
            wgpu::PresentMode::AutoVsync,
        ]
        .as_slice()
    } else {
        [
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::AutoNoVsync,
        ]
        .as_slice()
    };

    for mode in preferred {
        if modes.contains(mode) {
            return *mode;
        }
    }
    modes.first().copied().unwrap_or(wgpu::PresentMode::Fifo)
}

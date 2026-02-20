use crate::{
    resources::ResourceStore,
    three_d::{gpu::Gpu3D, renderer::Draw3DInstance},
    two_d::{
        gpu::Gpu2D,
        renderer::{Camera2DUniform, RectInstanceGpu, RectUploadPlan},
    },
};
use perro_render_bridge::Camera3DState;
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
    sample_count: u32,
    msaa_color: Option<MsaaColorTarget>,
    two_d: Gpu2D,
    three_d: Gpu3D,
}

impl Gpu {
    pub fn new(window: Arc<Window>, smoothing_samples: u32) -> Option<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).ok()?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok()?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("perro_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        }))
        .ok()?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            caps.present_modes[0]
        };
        let alpha_mode = caps.alpha_modes[0];
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let sample_count = normalize_sample_count(smoothing_samples);
        let two_d = Gpu2D::new(&device, format, sample_count);
        let three_d = Gpu3D::new(&device, format, sample_count, width, height);
        let msaa_color = create_msaa_color_target(&device, format, width, height, sample_count);

        Some(Self {
            window_handle: window,
            surface,
            device,
            queue,
            config,
            sample_count,
            msaa_color,
            two_d,
            three_d,
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
            self.config.format,
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
            .set_sample_count(&self.device, self.config.format, sample_count);
        self.three_d.set_sample_count(
            &self.device,
            self.config.format,
            sample_count,
            self.config.width,
            self.config.height,
        );
        self.msaa_color = create_msaa_color_target(
            &self.device,
            self.config.format,
            self.config.width,
            self.config.height,
            sample_count,
        );
    }

    pub fn render(
        &mut self,
        resources: &ResourceStore,
        camera_3d: Camera3DState,
        draws_3d: &[Draw3DInstance],
        camera_2d: Camera2DUniform,
        rects_2d: &[RectInstanceGpu],
        upload_2d: &RectUploadPlan,
    ) {
        // Keep window alive for the full surface lifetime.
        self.window_handle.id();

        self.two_d
            .prepare(&self.device, &self.queue, camera_2d, rects_2d, upload_2d);
        self.three_d.prepare(
            &self.device,
            &self.queue,
            resources,
            camera_3d,
            draws_3d,
            self.config.width,
            self.config.height,
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
            &mut encoder,
            color_view,
            wgpu::Color {
                r: CLEAR_R,
                g: CLEAR_G,
                b: CLEAR_B,
                a: 1.0,
            },
        );
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

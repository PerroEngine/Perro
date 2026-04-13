use crate::{
    backend::{OcclusionCullingMode, StaticMeshLookup, StaticShaderLookup, StaticTextureLookup},
    postprocess::{PostProcessChainData, PostProcessContext, PostProcessor},
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
    visual_accessibility::VisualAccessibilityProcessor,
};
use perro_ids::NodeID;
use perro_render_bridge::{Camera3DState, PointParticles3DState, Sprite2DCommand};
use perro_structs::VisualAccessibilitySettings;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::window::Window;

// Linear-space clear color for sRGB hex #1C1817.
const CLEAR_R: f64 = 0.011612245179743885;
const CLEAR_G: f64 = 0.009134058702220787;
const CLEAR_B: f64 = 0.008568125618069307;
const SMOOTH_SAMPLE_COUNT: u32 = 4;

pub const DIRTY_2D: u32 = 1 << 0;
pub const DIRTY_3D: u32 = 1 << 1;
pub const DIRTY_PARTICLES_3D: u32 = 1 << 2;
pub const DIRTY_CAMERA_2D: u32 = 1 << 3;
pub const DIRTY_CAMERA_3D: u32 = 1 << 4;
pub const DIRTY_LIGHTS_3D: u32 = 1 << 5;
pub const DIRTY_RESOURCES: u32 = 1 << 6;
pub const DIRTY_POSTFX: u32 = 1 << 7;
pub const DIRTY_ACCESSIBILITY: u32 = 1 << 8;

struct MsaaColorTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct PresentProcessor {
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
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
    post: PostProcessor,
    accessibility: VisualAccessibilityProcessor,
    present: PresentProcessor,
    two_d: Option<Gpu2D>,
    three_d: Option<Gpu3D>,
    point_particles_3d: Option<GpuPointParticles3D>,
    last_prepare_3d_camera: Option<Camera3DState>,
    last_prepare_3d_lighting: Option<Lighting3DState>,
    last_prepare_3d_draws_revision: u64,
    last_prepare_3d_width: u32,
    last_prepare_3d_height: u32,
    meshlets_enabled: bool,
    dev_meshlets: bool,
    meshlet_debug_view: bool,
    occlusion_culling: OcclusionCullingMode,
    indirect_first_instance_enabled: bool,
}

pub struct RenderFrame<'a> {
    pub resources: &'a ResourceStore,
    pub camera_3d: Camera3DState,
    pub lighting_3d: &'a Lighting3DState,
    pub draws_3d: &'a [Draw3DInstance],
    pub draws_3d_revision: u64,
    pub point_particles_3d: &'a [(NodeID, PointParticles3DState)],
    pub camera_2d: Camera2DUniform,
    pub post_processing_2d: Arc<[perro_structs::PostProcessEffect]>,
    pub post_processing_global: Arc<[perro_structs::PostProcessEffect]>,
    pub accessibility: VisualAccessibilitySettings,
    pub rects_2d: &'a [RectInstanceGpu],
    pub upload_2d: &'a RectUploadPlan,
    pub sprites_2d: &'a [Sprite2DCommand],
    pub frame_dirty_bits: u32,
    pub static_texture_lookup: Option<StaticTextureLookup>,
    pub static_mesh_lookup: Option<StaticMeshLookup>,
    pub static_shader_lookup: Option<StaticShaderLookup>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderGpuTiming {
    pub prepare_2d: Duration,
    pub prepare_3d: Duration,
    pub acquire: Duration,
    pub encode_main: Duration,
    pub submit_main: Duration,
    pub post_process: Duration,
    pub accessibility: Duration,
    pub present: Duration,
    pub draw_calls_2d: u32,
    pub draw_calls_3d: u32,
    pub total: Duration,
}

impl Gpu {
    pub fn render_idle_clear(&mut self) {
        // Keep window alive for the full surface lifetime.
        self.window_handle.id();

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
        };

        let swap_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_idle_clear_encoder"),
            });
        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_idle_clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &swap_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: CLEAR_R,
                            g: CLEAR_G,
                            b: CLEAR_B,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    pub fn new(
        window: Arc<Window>,
        smoothing_samples: u32,
        vsync_enabled: bool,
        meshlets_enabled: bool,
        dev_meshlets: bool,
        meshlet_debug_view: bool,
        occlusion_culling: OcclusionCullingMode,
    ) -> Option<Self> {
        let instance = wgpu::Instance::default();
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
        let render_format = linear_render_format(surface_format);
        let present_mode = choose_present_mode(&caps.present_modes, vsync_enabled);
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
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
        let post = PostProcessor::new(&device, render_format, width, height);
        let accessibility =
            VisualAccessibilityProcessor::new(&device, render_format, width, height);
        let present = PresentProcessor::new(&device, surface_format);

        Some(Self {
            window_handle: window,
            surface,
            device,
            queue,
            config,
            render_format,
            sample_count,
            msaa_color,
            post,
            accessibility,
            present,
            two_d: Some(two_d),
            three_d: Some(three_d),
            point_particles_3d: Some(point_particles_3d),
            last_prepare_3d_camera: None,
            last_prepare_3d_lighting: None,
            last_prepare_3d_draws_revision: u64::MAX,
            last_prepare_3d_width: width,
            last_prepare_3d_height: height,
            meshlets_enabled,
            dev_meshlets,
            meshlet_debug_view,
            occlusion_culling,
            indirect_first_instance_enabled,
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
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.resize(&self.device, width, height);
        }
        self.post.resize(&self.device, width, height);
        self.accessibility.resize(&self.device, width, height);
        self.msaa_color = create_msaa_color_target(
            &self.device,
            self.render_format,
            width,
            height,
            self.sample_count,
        );
        // Force next 3D prepare to refresh viewport-dependent GPU state.
        self.last_prepare_3d_width = 0;
        self.last_prepare_3d_height = 0;
    }

    pub fn set_smoothing_samples(&mut self, samples: u32) {
        let sample_count = normalize_sample_count(samples);
        if sample_count == self.sample_count {
            return;
        }
        self.sample_count = sample_count;
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.set_sample_count(&self.device, self.render_format, sample_count);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.set_sample_count(
                &self.device,
                self.render_format,
                sample_count,
                self.config.width,
                self.config.height,
            );
        }
        if let Some(point_particles_3d) = self.point_particles_3d.as_mut() {
            point_particles_3d.set_sample_count(&self.device, self.render_format, sample_count);
        }
        self.msaa_color = create_msaa_color_target(
            &self.device,
            self.render_format,
            self.config.width,
            self.config.height,
            sample_count,
        );
    }

    pub fn render(&mut self, frame: RenderFrame<'_>) -> RenderGpuTiming {
        let total_start = Instant::now();
        let mut timing = RenderGpuTiming::default();
        let RenderFrame {
            resources,
            camera_3d,
            lighting_3d,
            draws_3d,
            draws_3d_revision,
            point_particles_3d,
            camera_2d,
            post_processing_2d,
            post_processing_global,
            accessibility,
            rects_2d,
            upload_2d,
            sprites_2d,
            frame_dirty_bits,
            static_texture_lookup,
            static_mesh_lookup,
            static_shader_lookup,
        } = frame;
        let rect_draw_count = upload_2d.draw_count as u32;
        // Keep window alive for the full surface lifetime.
        self.window_handle.id();

        let post_requested = PostProcessor::has_effects(camera_3d.post_processing.as_ref())
            || PostProcessor::has_effects(post_processing_2d.as_ref())
            || PostProcessor::has_effects(post_processing_global.as_ref());

        let has = |bit: u32| (frame_dirty_bits & bit) != 0;

        let needs_2d = has(DIRTY_2D)
            || has(DIRTY_CAMERA_2D)
            || (has(DIRTY_RESOURCES) && !sprites_2d.is_empty())
            || upload_2d.draw_count > 0
            || !sprites_2d.is_empty();

        let three_d_content_changed = self.last_prepare_3d_camera.as_ref() != Some(&camera_3d)
            || self.last_prepare_3d_lighting.as_ref() != Some(lighting_3d)
            || self.last_prepare_3d_draws_revision != draws_3d_revision
            || self.last_prepare_3d_width != self.config.width
            || self.last_prepare_3d_height != self.config.height;

        let needs_3d = !draws_3d.is_empty();
        let needs_particles_3d = !point_particles_3d.is_empty();

        let needs_3d_pipeline = has(DIRTY_3D)
            || has(DIRTY_CAMERA_3D)
            || has(DIRTY_LIGHTS_3D)
            || needs_3d
            || needs_particles_3d
            || post_requested
            || three_d_content_changed;

        let needs_3d_particles_path = has(DIRTY_PARTICLES_3D) || needs_particles_3d;

        let prepare_2d_start = Instant::now();
        if needs_2d {
            if self.two_d.is_none() {
                self.two_d = Some(Gpu2D::new(
                    &self.device,
                    self.render_format,
                    self.sample_count,
                ));
            }
            if let Some(two_d) = self.two_d.as_mut() {
                two_d.prepare(
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
            }
        }
        timing.prepare_2d = prepare_2d_start.elapsed();

        let prepare_3d_start = Instant::now();
        if needs_3d_pipeline {
            if self.three_d.is_none() {
                self.three_d = Some(Gpu3D::new(
                    &self.device,
                    self.render_format,
                    Gpu3DConfig {
                        sample_count: self.sample_count,
                        width: self.config.width,
                        height: self.config.height,
                        meshlets_enabled: self.meshlets_enabled,
                        dev_meshlets: self.dev_meshlets,
                        meshlet_debug_view: self.meshlet_debug_view,
                        occlusion_culling: self.occlusion_culling,
                        indirect_first_instance_enabled: self.indirect_first_instance_enabled,
                    },
                ));
            }
            if needs_3d_particles_path && self.point_particles_3d.is_none() {
                self.point_particles_3d = Some(GpuPointParticles3D::new(
                    &self.device,
                    self.render_format,
                    self.sample_count,
                ));
            }
            if let Some(three_d) = self.three_d.as_mut()
                && (has(DIRTY_3D)
                    || has(DIRTY_CAMERA_3D)
                    || has(DIRTY_LIGHTS_3D)
                    || has(DIRTY_RESOURCES)
                    || needs_3d_particles_path
                    || post_requested
                    || three_d_content_changed)
            {
                three_d.prepare(
                    &self.device,
                    &self.queue,
                    Prepare3D {
                        resources,
                        camera: camera_3d.clone(),
                        lighting: lighting_3d,
                        draws: draws_3d,
                        draws_revision: draws_3d_revision,
                        width: self.config.width,
                        height: self.config.height,
                        static_texture_lookup,
                        static_mesh_lookup,
                        static_shader_lookup,
                    },
                );
                self.last_prepare_3d_camera = Some(camera_3d.clone());
                self.last_prepare_3d_lighting = Some(lighting_3d.clone());
                self.last_prepare_3d_draws_revision = draws_3d_revision;
                self.last_prepare_3d_width = self.config.width;
                self.last_prepare_3d_height = self.config.height;
            }
            if needs_3d_particles_path
                && let Some(point_particles_3d_gpu) = self.point_particles_3d.as_mut()
            {
                point_particles_3d_gpu.prepare(
                    &self.device,
                    &self.queue,
                    PreparePointParticles3D {
                        camera: camera_3d.clone(),
                        emitters: point_particles_3d,
                        width: self.config.width,
                        height: self.config.height,
                    },
                );
            }
        }
        if !needs_3d_particles_path {
            self.point_particles_3d = None;
        }
        timing.prepare_3d = prepare_3d_start.elapsed();

        let acquire_start = Instant::now();
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                timing.acquire = acquire_start.elapsed();
                timing.total = total_start.elapsed();
                return timing;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                timing.acquire = acquire_start.elapsed();
                timing.total = total_start.elapsed();
                return timing;
            }
        };
        timing.acquire = acquire_start.elapsed();

        let swap_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let (camera_post_chain, camera_post_enabled) =
            if PostProcessor::has_effects(camera_3d.post_processing.as_ref()) {
                (camera_3d.post_processing.as_ref(), true)
            } else if PostProcessor::has_effects(post_processing_2d.as_ref()) {
                (post_processing_2d.as_ref(), true)
            } else {
                (camera_3d.post_processing.as_ref(), false)
            };
        let global_post_chain = post_processing_global.as_ref();
        let global_post_enabled = PostProcessor::has_effects(global_post_chain);
        let accessibility_enabled = self.accessibility.has_settings(accessibility);
        let depth_prepass_needed = (camera_post_enabled
            && PostProcessor::uses_depth(camera_post_chain))
            || (global_post_enabled && PostProcessor::uses_depth(global_post_chain));
        let scene_view = self.post.scene_view().clone();
        let intermediate_view = self.accessibility.intermediate_view().clone();
        let color_view = self
            .msaa_color
            .as_ref()
            .map(|t| &t.view)
            .unwrap_or(&scene_view);
        let resolve_view = if self.sample_count > 1 {
            Some(&scene_view)
        } else {
            None
        };

        let encode_start = Instant::now();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_main_encoder"),
            });

        let clear_color = sky_clear_color(lighting_3d).unwrap_or(wgpu::Color {
            r: CLEAR_R,
            g: CLEAR_G,
            b: CLEAR_B,
            a: 1.0,
        });
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.render_pass(&mut encoder, color_view, clear_color, depth_prepass_needed);
            if let Some(point_particles_3d_gpu) = self.point_particles_3d.as_mut() {
                point_particles_3d_gpu.render_pass(&mut encoder, color_view, three_d.depth_view());
            }
        } else {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: resolve_view,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        if let Some(two_d) = self.two_d.as_ref() {
            two_d.render_pass(
                &mut encoder,
                color_view,
                resolve_view,
                rect_draw_count,
            );
        }
        timing.encode_main = encode_start.elapsed();

        let post_start = Instant::now();
        #[derive(Clone, Copy)]
        enum FrameTex {
            Scene,
            Intermediate,
        }
        let mut current_tex = FrameTex::Scene;

        let mut apply_post_chain = |effects: &[perro_structs::PostProcessEffect],
                                    current_tex: &mut FrameTex| {
            if effects.is_empty() {
                return;
            }
            let (input_view, output_view, next_tex) = match *current_tex {
                FrameTex::Scene => (&scene_view, &intermediate_view, FrameTex::Intermediate),
                FrameTex::Intermediate => (&intermediate_view, &scene_view, FrameTex::Scene),
            };
            let post_context = PostProcessContext {
                device: &self.device,
                queue: &self.queue,
                output_view,
                camera: &camera_3d,
                static_shader_lookup,
            };
            let post_chain_data = PostProcessChainData {
                input_view,
                depth_view: self
                    .three_d
                    .as_ref()
                    .expect("three_d is initialized when post-processing is active")
                    .depth_prepass_view(),
                effects,
            };
            self.post
                .apply_chain(&post_context, &post_chain_data, &mut encoder);
            *current_tex = next_tex;
        };
        apply_post_chain(
            if camera_post_enabled {
                camera_post_chain
            } else {
                &[]
            },
            &mut current_tex,
        );
        apply_post_chain(
            if global_post_enabled {
                global_post_chain
            } else {
                &[]
            },
            &mut current_tex,
        );
        timing.post_process = post_start.elapsed();

        let accessibility_start = Instant::now();
        if accessibility_enabled {
            let (accessibility_input_view, accessibility_output_view, next_tex) = match current_tex
            {
                FrameTex::Scene => (&scene_view, &intermediate_view, FrameTex::Intermediate),
                FrameTex::Intermediate => (&intermediate_view, &scene_view, FrameTex::Scene),
            };
            self.accessibility.apply(
                &self.device,
                &self.queue,
                &mut encoder,
                accessibility_input_view,
                accessibility_output_view,
                accessibility,
            );
            current_tex = next_tex;
        }
        timing.accessibility = accessibility_start.elapsed();

        let final_input_view = match current_tex {
            FrameTex::Scene => &scene_view,
            FrameTex::Intermediate => &intermediate_view,
        };
        self.present
            .apply(&self.device, &mut encoder, final_input_view, &swap_view);
        let submit_start = Instant::now();
        self.queue.submit(Some(encoder.finish()));
        timing.submit_main = submit_start.elapsed();
        timing.draw_calls_2d = self
            .two_d
            .as_ref()
            .map(|two_d| two_d.draw_call_count(rect_draw_count))
            .unwrap_or(0);
        timing.draw_calls_3d = self
            .three_d
            .as_ref()
            .map(|three_d| three_d.draw_call_count())
            .unwrap_or(0);
        let present_start = Instant::now();
        frame.present();
        timing.present = present_start.elapsed();
        timing.total = total_start.elapsed();
        timing
    }

    pub fn virtual_size() -> [f32; 2] {
        Gpu2D::virtual_size()
    }
}

fn sky_clear_color(lighting: &Lighting3DState) -> Option<wgpu::Color> {
    let sky = lighting.sky.as_ref()?;
    let day = sample_gradient_color(sky.day_colors.as_ref(), 0.32);
    let evening = sample_gradient_color(sky.evening_colors.as_ref(), 0.32);
    let night = sample_gradient_color(sky.night_colors.as_ref(), 0.32);
    let day_t = day_weight(sky.time.time_of_day);
    let evening_t = evening_weight(sky.time.time_of_day);
    let base = [
        night[0] + (day[0] - night[0]) * day_t,
        night[1] + (day[1] - night[1]) * day_t,
        night[2] + (day[2] - night[2]) * day_t,
    ];
    let c = [
        base[0] + (evening[0] - base[0]) * evening_t,
        base[1] + (evening[1] - base[1]) * evening_t,
        base[2] + (evening[2] - base[2]) * evening_t,
    ];
    Some(wgpu::Color {
        r: c[0].clamp(0.0, 1.0) as f64,
        g: c[1].clamp(0.0, 1.0) as f64,
        b: c[2].clamp(0.0, 1.0) as f64,
        a: 1.0,
    })
}

fn sample_gradient_color(colors: &[[f32; 3]], t: f32) -> [f32; 3] {
    if colors.is_empty() {
        return [CLEAR_R as f32, CLEAR_G as f32, CLEAR_B as f32];
    }
    if colors.len() == 1 {
        return colors[0];
    }
    let n = colors.len() - 1;
    let f = t.clamp(0.0, 1.0) * n as f32;
    let i = f.floor() as usize;
    let j = (i + 1).min(n);
    let u = f - i as f32;
    [
        colors[i][0] + (colors[j][0] - colors[i][0]) * u,
        colors[i][1] + (colors[j][1] - colors[i][1]) * u,
        colors[i][2] + (colors[j][2] - colors[i][2]) * u,
    ]
}

fn day_weight(time_of_day: f32) -> f32 {
    let t = time_of_day.rem_euclid(1.0);
    let a = (t * std::f32::consts::TAU) - std::f32::consts::FRAC_PI_2;
    ((a.sin() + 1.0) * 0.5).clamp(0.0, 1.0)
}

fn evening_weight(time_of_day: f32) -> f32 {
    let t = time_of_day.rem_euclid(1.0);
    let dist = ((t - 0.75 + 0.5).rem_euclid(1.0) - 0.5).abs();
    (1.0 - (dist / 0.23)).clamp(0.0, 1.0)
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

impl PresentProcessor {
    fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_present_shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vid], 0.0, 1.0);
    out.uv = (out.pos.xy * vec2<f32>(0.5, -0.5)) + vec2<f32>(0.5, 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(input_tex, input_sampler, in.uv);
    return vec4<f32>(c.rgb, 1.0);
}
"#
                .into(),
            ),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("perro_present_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_present_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_present_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perro_present_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            sampler,
            bgl,
            pipeline,
        }
    }

    fn apply(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_present_bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_present_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn linear_render_format(surface_format: wgpu::TextureFormat) -> wgpu::TextureFormat {
    match surface_format {
        wgpu::TextureFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8Unorm,
        _ => surface_format,
    }
}

fn choose_present_mode(modes: &[wgpu::PresentMode], vsync_enabled: bool) -> wgpu::PresentMode {
    let preferred = if vsync_enabled {
        [
            wgpu::PresentMode::Fifo,
            wgpu::PresentMode::AutoVsync,
            wgpu::PresentMode::FifoRelaxed,
        ]
        .as_slice()
    } else {
        [
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::Immediate,
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

use crate::{
    backend::{OcclusionCullingMode, StaticMeshLookup, StaticShaderLookup, StaticTextureLookup},
    postprocess::{PostProcessChainData, PostProcessContext, PostProcessor},
    resources::ResourceStore,
    three_d::{
        gpu::{Gpu3D, Gpu3DConfig, Prepare3D, Prepare3DStepTiming},
        particles::gpu::{GpuPointParticles3D, PreparePointParticles3D},
        renderer::{DenseMultiMeshDraw3D, Draw3DInstance, Draw3DKind, Lighting3DState},
    },
    two_d::{
        gpu::{Gpu2D, Prepare2D},
        renderer::{
            Camera2DUniform, RectInstanceGpu, RectUploadPlan, camera_2d_uniform_from_state,
        },
    },
    ui::gpu::{GpuUi, UiPrepareInput},
    visual_accessibility::VisualAccessibilityProcessor,
};
use ahash::AHashMap;
use epaint::{ClippedPrimitive, textures::TexturesDelta};
use glam::{Mat4, Quat, Vec3, Vec4};
use perro_ids::NodeID;
use perro_render_bridge::{
    Camera3DState, CameraProjectionState, CameraStreamDraw3DState, CameraStreamLighting3DState,
    CameraStreamSourceState, CameraStreamState, Decal3DState, Light2DState, PointParticles3DState,
    ShadowCaster2DState, Sprite2DCommand, Water2DState, Water3DState, WaterBodySampleState,
    WaterSampleState,
};
use perro_structs::TextureFilterMode;
use perro_structs::VisualAccessibilitySettings;
use std::sync::{Arc, mpsc};
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
use winit::window::Window;

#[path = "gpu/present.rs"]
mod present;
#[path = "water_gpu.rs"]
mod water_gpu;

pub(crate) use present::capped_render_size;
use present::*;
use water_gpu::{GpuWater, WaterPrepareContext};

// Linear-space clear color for sRGB hex #40474F — neutral dark slate so
// scenes without a Sky3D read as a lit viewport instead of black.
const CLEAR_R: f64 = 0.050876;
const CLEAR_G: f64 = 0.063763;
const CLEAR_B: f64 = 0.079339;
const SMOOTH_SAMPLE_COUNT: u32 = 4;

fn water_camera_view_proj(camera: &Camera3DState, width: u32, height: u32) -> Mat4 {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let aspect = w / h;
    let proj = match camera.projection {
        CameraProjectionState::Perspective {
            fov_y_degrees,
            near,
            far,
        } => {
            let fov = fov_y_degrees
                .to_radians()
                .clamp(10.0f32.to_radians(), 120.0f32.to_radians());
            Mat4::perspective_rh(
                fov,
                aspect.max(1.0e-6),
                near.max(1.0e-3),
                far.max(near + 1.0e-3),
            )
        }
        CameraProjectionState::Orthographic { size, near, far } => {
            let half_h = (size.abs() * 0.5).max(1.0e-3);
            let half_w = half_h * aspect.max(1.0e-6);
            Mat4::orthographic_rh(
                -half_w,
                half_w,
                -half_h,
                half_h,
                near.max(1.0e-3),
                far.max(near + 1.0e-3),
            )
        }
        CameraProjectionState::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        } => Mat4::frustum_rh(
            left,
            right,
            bottom,
            top,
            near.max(1.0e-3),
            far.max(near + 1.0e-3),
        ),
    };
    let pos = Vec3::from(camera.position);
    let rot_raw = Quat::from_xyzw(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
        camera.rotation[3],
    );
    let rot = if rot_raw.is_finite() && rot_raw.length_squared() > 1.0e-6 {
        rot_raw.normalize()
    } else {
        Quat::IDENTITY
    };
    let world = Mat4::from_rotation_translation(rot, pos);
    proj * world.inverse()
}

fn fill_camera_stream_draws_3d(draws: &[CameraStreamDraw3DState], out: &mut Vec<Draw3DInstance>) {
    out.clear();
    out.reserve(draws.len().saturating_sub(out.capacity()));
    out.extend(draws.iter().map(|draw| match draw {
        CameraStreamDraw3DState::Draw {
            mesh,
            surfaces,
            node,
            model,
            skeleton,
            meshlet_override,
            lod,
            blend,
        } => Draw3DInstance {
            node: *node,
            kind: Draw3DKind::Mesh(*mesh),
            surfaces: surfaces.clone(),
            instance_mats: Arc::from([*model]),
            blend_shape_weights: Arc::from([]),
            debug_color: None,
            skeleton: skeleton.clone(),
            dense_multimesh: None,
            meshlet_override: *meshlet_override,
            lod: *lod,
            blend: *blend,
            cast_shadows: true,
            receive_shadows: true,
        },
        CameraStreamDraw3DState::DrawMulti {
            mesh,
            surfaces,
            node,
            instance_mats,
            skeleton,
            meshlet_override,
            lod,
            blend,
        } => Draw3DInstance {
            node: *node,
            kind: Draw3DKind::Mesh(*mesh),
            surfaces: surfaces.clone(),
            instance_mats: instance_mats.clone(),
            blend_shape_weights: Arc::from([]),
            debug_color: None,
            skeleton: skeleton.clone(),
            dense_multimesh: None,
            meshlet_override: *meshlet_override,
            lod: *lod,
            blend: *blend,
            cast_shadows: true,
            receive_shadows: true,
        },
        CameraStreamDraw3DState::DrawMultiDense {
            mesh,
            surfaces,
            node,
            node_model,
            instance_scale,
            instances,
            meshlet_override,
            lod,
            blend,
        } => Draw3DInstance {
            node: *node,
            kind: Draw3DKind::Mesh(*mesh),
            surfaces: surfaces.clone(),
            instance_mats: Arc::from([]),
            blend_shape_weights: Arc::from([]),
            debug_color: None,
            skeleton: None,
            dense_multimesh: Some(DenseMultiMeshDraw3D {
                node_model: *node_model,
                instance_scale: *instance_scale,
                instances: instances.clone(),
            }),
            meshlet_override: *meshlet_override,
            lod: *lod,
            blend: *blend,
            cast_shadows: true,
            receive_shadows: true,
        },
    }))
}

fn camera_stream_lighting_3d(lighting: &CameraStreamLighting3DState) -> Lighting3DState {
    Lighting3DState {
        frame_time_seconds: 0.0,
        frame_delta_seconds: 0.0,
        frame_index: 0,
        ambient_light: lighting.ambient_light,
        sky: lighting.sky.clone(),
        sky_time_seconds: 0.0,
        ray_lights: lighting.ray_lights,
        point_lights: lighting.point_lights,
        spot_lights: lighting.spot_lights,
    }
}

fn water_extract_frustum_planes(view_proj: Mat4) -> [[f32; 4]; 6] {
    let r0 = Vec4::new(
        view_proj.x_axis.x,
        view_proj.y_axis.x,
        view_proj.z_axis.x,
        view_proj.w_axis.x,
    );
    let r1 = Vec4::new(
        view_proj.x_axis.y,
        view_proj.y_axis.y,
        view_proj.z_axis.y,
        view_proj.w_axis.y,
    );
    let r2 = Vec4::new(
        view_proj.x_axis.z,
        view_proj.y_axis.z,
        view_proj.z_axis.z,
        view_proj.w_axis.z,
    );
    let r3 = Vec4::new(
        view_proj.x_axis.w,
        view_proj.y_axis.w,
        view_proj.z_axis.w,
        view_proj.w_axis.w,
    );
    [
        water_normalize_plane(r3 + r0).to_array(),
        water_normalize_plane(r3 - r0).to_array(),
        water_normalize_plane(r3 + r1).to_array(),
        water_normalize_plane(r3 - r1).to_array(),
        water_normalize_plane(r3 + r2).to_array(),
        water_normalize_plane(r3 - r2).to_array(),
    ]
}

fn water_normalize_plane(plane: Vec4) -> Vec4 {
    let len = plane.truncate().length();
    if len > 1.0e-6 && len.is_finite() {
        plane / len
    } else {
        plane
    }
}

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

struct GpuTimestampTimer {
    query_set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    timestamp_period_ns: f32,
    pending_rx: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    last_main: Duration,
    last_water: Duration,
}

impl GpuTimestampTimer {
    const QUERY_COUNT: u32 = 4;
    const BUFFER_SIZE: u64 = Self::QUERY_COUNT as u64 * std::mem::size_of::<u64>() as u64;

    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("perro_gpu_timestamp_query"),
            ty: wgpu::QueryType::Timestamp,
            count: Self::QUERY_COUNT,
        });
        let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_gpu_timestamp_resolve"),
            size: Self::BUFFER_SIZE,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_gpu_timestamp_readback"),
            size: Self::BUFFER_SIZE,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            query_set,
            resolve_buffer,
            readback_buffer,
            timestamp_period_ns: queue.get_timestamp_period(),
            pending_rx: None,
            last_main: Duration::ZERO,
            last_water: Duration::ZERO,
        }
    }

    fn poll(&mut self, device: &wgpu::Device) {
        let Some(rx) = self.pending_rx.as_ref() else {
            return;
        };
        let _ = device.poll(wgpu::PollType::Poll);
        match rx.try_recv() {
            Ok(Ok(())) => {
                let slice = self.readback_buffer.slice(..);
                let Ok(data) = slice.get_mapped_range() else {
                    self.readback_buffer.unmap();
                    self.pending_rx = None;
                    return;
                };
                let timestamps: &[u64] = bytemuck::cast_slice(&data);
                if timestamps.len() >= 2 && timestamps[1] >= timestamps[0] {
                    let nanos = (timestamps[1] - timestamps[0]) as f64
                        * f64::from(self.timestamp_period_ns);
                    self.last_main = Duration::from_nanos(nanos.max(0.0) as u64);
                }
                if timestamps.len() >= 4 && timestamps[3] >= timestamps[2] {
                    let nanos = (timestamps[3] - timestamps[2]) as f64
                        * f64::from(self.timestamp_period_ns);
                    self.last_water = Duration::from_nanos(nanos.max(0.0) as u64);
                }
                drop(data);
                self.readback_buffer.unmap();
                self.pending_rx = None;
            }
            Ok(Err(_)) => {
                self.readback_buffer.unmap();
                self.pending_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.readback_buffer.unmap();
                self.pending_rx = None;
            }
        }
    }

    fn can_write(&self) -> bool {
        self.pending_rx.is_none()
    }

    fn write_start(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.write_timestamp(&self.query_set, 0);
    }

    fn write_water_start(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.write_timestamp(&self.query_set, 2);
    }

    fn write_water_end(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.write_timestamp(&self.query_set, 3);
    }

    fn write_end_and_resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.write_timestamp(&self.query_set, 1);
        encoder.resolve_query_set(
            &self.query_set,
            0..Self::QUERY_COUNT,
            &self.resolve_buffer,
            0,
        );
        encoder.copy_buffer_to_buffer(
            &self.resolve_buffer,
            0,
            &self.readback_buffer,
            0,
            Self::BUFFER_SIZE,
        );
    }

    fn request_readback(&mut self) {
        let slice = self.readback_buffer.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.pending_rx = Some(rx);
    }
}

pub struct Gpu {
    window_handle: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_width: u32,
    render_height: u32,
    render_format: wgpu::TextureFormat,
    sample_count: u32,
    max_supported_sample_count: u32,
    msaa_color: Option<MsaaColorTarget>,
    post: PostProcessor,
    accessibility: VisualAccessibilityProcessor,
    present: PresentProcessor,
    present_scene_bind_group: wgpu::BindGroup,
    present_intermediate_bind_group: wgpu::BindGroup,
    two_d: Option<Gpu2D>,
    late_overlay_2d: Option<Gpu2D>,
    ui: Option<GpuUi>,
    three_d: Option<Gpu3D>,
    point_particles_3d: Option<GpuPointParticles3D>,
    water: Option<GpuWater>,
    camera_stream_targets: AHashMap<NodeID, GpuCameraStreamTarget>,
    camera_stream_external_bindings: AHashMap<NodeID, [u32; 2]>,
    // Per-node resolution of the 3D material-texture slot last bound to a
    // camera-stream target, so the external upsert (view + bind group + retain
    // scan) only runs when the target is (re)created, not every frame.
    camera_stream_3d_bindings: AHashMap<NodeID, [u32; 2]>,
    camera_stream_2d: Option<Gpu2D>,
    camera_stream_3d: Option<Gpu3D>,
    camera_stream_particles_3d: Option<GpuPointParticles3D>,
    camera_stream_water: Option<GpuWater>,
    camera_stream_post: Option<PostProcessor>,
    camera_stream_draws_scratch: Vec<Draw3DInstance>,
    last_prepare_particles_revision: u64,
    last_prepare_water_2d_revision: u64,
    last_prepare_water_3d_revision: u64,
    last_prepare_3d_camera: Option<Camera3DState>,
    last_prepare_3d_lighting: Option<Lighting3DState>,
    last_prepare_3d_draws_revision: u64,
    last_prepare_3d_decals_revision: u64,
    last_prepare_3d_width: u32,
    last_prepare_3d_height: u32,
    meshlets_enabled: bool,
    dev_meshlets: bool,
    meshlet_debug_view: bool,
    occlusion_culling: OcclusionCullingMode,
    texture_filter: TextureFilterMode,
    indirect_first_instance_enabled: bool,
    multi_draw_indirect_enabled: bool,
    gpu_timer: Option<GpuTimestampTimer>,
}

#[derive(Clone, Copy)]
pub struct GpuConfig {
    pub smoothing_samples: u32,
    pub vsync_enabled: bool,
    pub meshlets_enabled: bool,
    pub dev_meshlets: bool,
    pub meshlet_debug_view: bool,
    pub occlusion_culling: OcclusionCullingMode,
    pub texture_filter: TextureFilterMode,
}

struct GpuCameraStreamTarget {
    texture: wgpu::Texture,
    post_input: wgpu::Texture,
    depth: wgpu::Texture,
    resolution: [u32; 2],
}

pub struct RenderFrame<'a> {
    pub resources: &'a ResourceStore,
    pub camera_3d: Camera3DState,
    pub lighting_3d: &'a Lighting3DState,
    pub draws_3d: &'a [Draw3DInstance],
    pub draws_3d_revision: u64,
    pub point_particles_3d: &'a [(NodeID, PointParticles3DState)],
    pub point_particles_3d_revision: u64,
    pub waters_3d: &'a [(NodeID, Water3DState)],
    pub waters_3d_revision: u64,
    pub decals_3d: &'a [(NodeID, Decal3DState)],
    pub decals_3d_revision: u64,
    pub camera_streams: &'a [(NodeID, CameraStreamState)],
    pub camera_2d: Camera2DUniform,
    pub camera_2d_position: [f32; 2],
    pub post_processing_2d: Arc<[perro_structs::PostProcessEffect]>,
    pub post_processing_global: Arc<[perro_structs::PostProcessEffect]>,
    pub accessibility: VisualAccessibilitySettings,
    pub rects_2d: &'a [RectInstanceGpu],
    pub upload_2d: &'a RectUploadPlan,
    pub sprites_2d: &'a [Sprite2DCommand],
    pub sprites_2d_revision: u64,
    pub point_lights_2d: &'a [Light2DState],
    pub point_lights_2d_revision: u64,
    pub shadow_casters_2d: &'a [ShadowCaster2DState],
    pub waters_2d: &'a [(NodeID, Water2DState)],
    pub waters_2d_revision: u64,
    pub late_overlay_camera_2d: Camera2DUniform,
    pub late_overlay_rects_2d: &'a [RectInstanceGpu],
    pub late_overlay_upload_2d: &'a RectUploadPlan,
    pub late_overlay_sprites_2d: &'a [Sprite2DCommand],
    pub late_overlay_sprites_2d_revision: u64,
    pub late_overlay_point_lights_2d: &'a [Light2DState],
    pub late_overlay_point_lights_2d_revision: u64,
    pub late_overlay_shadow_casters_2d: &'a [ShadowCaster2DState],
    pub ui_primitives: &'a [Arc<ClippedPrimitive>],
    pub ui_textures_delta: &'a TexturesDelta,
    pub ui_texture_size: [u32; 2],
    pub ui_revision: u64,
    pub redraw_requested: bool,
    pub frame_time_seconds: f32,
    pub frame_delta_seconds: f32,
    pub frame_dirty_bits: u32,
    pub static_texture_lookup: Option<StaticTextureLookup>,
    pub static_mesh_lookup: Option<StaticMeshLookup>,
    pub static_shader_lookup: Option<StaticShaderLookup>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderGpuTiming {
    pub prepare_2d: Duration,
    pub prepare_3d: Duration,
    pub prepare_particles_3d: Duration,
    pub prepare_3d_frustum: Duration,
    pub prepare_3d_hiz: Duration,
    pub prepare_3d_indirect: Duration,
    pub prepare_3d_cull_inputs: Duration,
    pub acquire: Duration,
    pub acquire_surface: Duration,
    pub acquire_view: Duration,
    pub encode_main: Duration,
    pub submit_main: Duration,
    pub submit_finish_main: Duration,
    pub submit_queue_main: Duration,
    pub post_process: Duration,
    pub accessibility: Duration,
    pub present: Duration,
    pub gpu_timestamp_main: Duration,
    pub gpu_timestamp_water: Duration,
    pub draw_calls_2d: u32,
    pub draw_calls_3d: u32,
    pub sprite_batches_2d: u32,
    pub sprite_bind_group_switches_2d: u32,
    pub draw_batches_3d: u32,
    pub pipeline_switches_3d: u32,
    pub texture_bind_group_switches_3d: u32,
    pub skip_prepare_2d: u32,
    pub skip_prepare_3d: u32,
    pub skip_prepare_particles_3d: u32,
    pub skip_prepare_3d_frustum: u32,
    pub skip_prepare_3d_hiz: u32,
    pub skip_prepare_3d_indirect: u32,
    pub skip_prepare_3d_cull_inputs: u32,
    pub total: Duration,
    pub presented: bool,
}

impl Gpu {
    pub fn invalidate_texture(&mut self, texture: perro_ids::TextureID, source: Option<&str>) {
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.invalidate_texture(texture);
        }
        if let Some(late_overlay_2d) = self.late_overlay_2d.as_mut() {
            late_overlay_2d.invalidate_texture(texture);
        }
        if let Some(camera_stream_2d) = self.camera_stream_2d.as_mut() {
            camera_stream_2d.invalidate_texture(texture);
        }
        if let Some(ui) = self.ui.as_mut() {
            ui.invalidate_image_texture(texture);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.invalidate_material_texture(texture.index());
            three_d.invalidate_material_texture_source(source);
        }
        if let Some(camera_stream_3d) = self.camera_stream_3d.as_mut() {
            camera_stream_3d.invalidate_material_texture(texture.index());
            camera_stream_3d.invalidate_material_texture_source(source);
        }
    }

    // mark/unmark a texture id as a stream (webcam/video) across every consumer
    // cache, so rebuilds use a single-level (no-mip) texture that supports the
    // per-frame in-place base upload.
    pub fn set_stream_texture(&mut self, texture: perro_ids::TextureID, is_stream: bool) {
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.set_stream_texture(texture, is_stream);
        }
        if let Some(late_overlay_2d) = self.late_overlay_2d.as_mut() {
            late_overlay_2d.set_stream_texture(texture, is_stream);
        }
        if let Some(camera_stream_2d) = self.camera_stream_2d.as_mut() {
            camera_stream_2d.set_stream_texture(texture, is_stream);
        }
        if let Some(ui) = self.ui.as_mut() {
            ui.set_stream_texture(texture, is_stream);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.set_stream_texture(texture.index(), is_stream);
        }
        if let Some(camera_stream_3d) = self.camera_stream_3d.as_mut() {
            camera_stream_3d.set_stream_texture(texture.index(), is_stream);
        }
    }

    // in-place base-level upload of a stream frame into every resident cache; no
    // texture/sampler/bind-group recreation, no mip regen. missing/resized caches
    // no-op (they rebuild from decoded data on the next prepare). `source` keeps
    // 3D custom-source material slots fresh (in-place write or invalidate).
    pub fn write_stream_texture(
        &mut self,
        texture: perro_ids::TextureID,
        source: Option<&str>,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) {
        let queue = &self.queue;
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.write_stream_texture(queue, texture, width, height, rgba);
        }
        if let Some(late_overlay_2d) = self.late_overlay_2d.as_mut() {
            late_overlay_2d.write_stream_texture(queue, texture, width, height, rgba);
        }
        if let Some(camera_stream_2d) = self.camera_stream_2d.as_mut() {
            camera_stream_2d.write_stream_texture(queue, texture, width, height, rgba);
        }
        if let Some(ui) = self.ui.as_mut() {
            ui.write_stream_texture(queue, texture, width, height, rgba);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.write_stream_material_texture(queue, texture.index(), width, height, rgba);
            three_d.write_stream_material_texture_source(queue, source, width, height, rgba);
        }
        if let Some(camera_stream_3d) = self.camera_stream_3d.as_mut() {
            camera_stream_3d.write_stream_material_texture(
                queue,
                texture.index(),
                width,
                height,
                rgba,
            );
            camera_stream_3d.write_stream_material_texture_source(
                queue, source, width, height, rgba,
            );
        }
    }

    fn ensure_camera_stream_target(
        &mut self,
        node: NodeID,
        resolution: [u32; 2],
    ) -> Option<&GpuCameraStreamTarget> {
        let resolution = [resolution[0].max(1), resolution[1].max(1)];
        let recreate = self
            .camera_stream_targets
            .get(&node)
            .is_none_or(|target| target.resolution != resolution);
        if recreate {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("perro_camera_stream_target"),
                size: wgpu::Extent3d {
                    width: resolution[0],
                    height: resolution[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.render_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let post_input = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("perro_camera_stream_post_input"),
                size: wgpu::Extent3d {
                    width: resolution[0],
                    height: resolution[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.render_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let depth = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("perro_camera_stream_post_depth"),
                size: wgpu::Extent3d {
                    width: resolution[0],
                    height: resolution[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth24Plus,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.camera_stream_targets.insert(
                node,
                GpuCameraStreamTarget {
                    texture,
                    post_input,
                    depth,
                    resolution,
                },
            );
            self.camera_stream_external_bindings.remove(&node);
            self.camera_stream_3d_bindings.remove(&node);
        }
        self.camera_stream_targets.get(&node)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn wait_idle(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    #[cfg(target_arch = "wasm32")]
    pub fn wait_idle(&mut self) {}

    pub fn render_idle_clear(&mut self) -> bool {
        // Keep window alive for the full surface lifetime.
        self.window_handle.id();

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return false;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return false,
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
        self.queue.present(frame);
        true
    }

    pub async fn new_async(window: Arc<Window>, cfg: GpuConfig) -> Option<Self> {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).ok()?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                apply_limit_buckets: false,
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok()?;
        let adapter_features = adapter.features();
        let mut required_features = wgpu::Features::empty();
        if adapter_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE) {
            required_features |= wgpu::Features::INDIRECT_FIRST_INSTANCE;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let enable_timestamp_queries = true;
        #[cfg(target_arch = "wasm32")]
        let enable_timestamp_queries = false;
        let timestamp_features =
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if enable_timestamp_queries && adapter_features.contains(timestamp_features) {
            required_features |= timestamp_features;
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("perro_device"),
                required_features,
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
            })
            .await
            .ok()?;
        let indirect_first_instance_enabled =
            required_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE);
        // multi_draw_indexed_indirect (non-count) needs only INDIRECT_EXECUTION,
        // the same downlevel capability draw_indexed_indirect already relies on,
        // so it rides the existing indirect path with no extra feature request.
        let multi_draw_indirect_enabled = indirect_first_instance_enabled;
        let timestamp_query_enabled = required_features.contains(timestamp_features);
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
        let present_mode = choose_present_mode(&caps.present_modes, cfg.vsync_enabled);
        let max_frame_latency = choose_max_frame_latency(cfg.vsync_enabled);
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        let (render_width, render_height) =
            capped_render_size(width, height, device.limits().max_texture_dimension_2d);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: max_frame_latency,
            // Auto keeps wgpu's pre-30 color-space behavior (sRGB / linear fp16).
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        eprintln!(
            "[perro][gfx] vsync=({}) present_mode=({present_mode:?}) max_frame_latency=({max_frame_latency}) present_caps=({:?})",
            cfg.vsync_enabled, caps.present_modes
        );
        surface.configure(&device, &config);

        let max_supported_sample_count = max_supported_msaa_sample_count(&adapter, render_format);
        let sample_count = clamp_supported_sample_count(
            normalize_sample_count(cfg.smoothing_samples),
            max_supported_sample_count,
        );
        let two_d = Gpu2D::new(&device, render_format, sample_count, cfg.texture_filter);
        let late_overlay_2d = Gpu2D::new(&device, surface_format, 1, cfg.texture_filter);
        let ui = Some(GpuUi::new(&device, surface_format, cfg.texture_filter));
        let three_d = Gpu3D::new(
            &device,
            &queue,
            render_format,
            Gpu3DConfig {
                sample_count,
                width: render_width,
                height: render_height,
                meshlets_enabled: cfg.meshlets_enabled,
                dev_meshlets: cfg.dev_meshlets,
                meshlet_debug_view: cfg.meshlet_debug_view,
                occlusion_culling: cfg.occlusion_culling,
                indirect_first_instance_enabled,
                multi_draw_indirect_enabled,
                texture_filter: cfg.texture_filter,
            },
        );
        let point_particles_3d = GpuPointParticles3D::new(&device, render_format, sample_count);
        let camera_stream_2d = Gpu2D::new(&device, render_format, 1, cfg.texture_filter);
        let water = Some(GpuWater::new(
            &device,
            render_format,
            sample_count,
            two_d.camera_bind_group_layout(),
            three_d.water_camera_bind_group_layout(),
            three_d.depth_prepass_view(),
        ));
        let msaa_color = create_msaa_color_target(
            &device,
            render_format,
            render_width,
            render_height,
            sample_count,
        );
        let post = PostProcessor::new(&device, &queue, render_format, render_width, render_height);
        let accessibility =
            VisualAccessibilityProcessor::new(&device, render_format, render_width, render_height);
        let present = PresentProcessor::new(&device, surface_format);
        let present_scene_bind_group = present.create_bind_group(&device, post.scene_view());
        let present_intermediate_bind_group =
            present.create_bind_group(&device, accessibility.intermediate_view());
        let gpu_timer = timestamp_query_enabled.then(|| GpuTimestampTimer::new(&device, &queue));

        Some(Self {
            window_handle: window,
            surface,
            device,
            queue,
            config,
            render_width,
            render_height,
            render_format,
            sample_count,
            max_supported_sample_count,
            msaa_color,
            post,
            accessibility,
            present,
            present_scene_bind_group,
            present_intermediate_bind_group,
            two_d: Some(two_d),
            late_overlay_2d: Some(late_overlay_2d),
            ui,
            three_d: Some(three_d),
            point_particles_3d: Some(point_particles_3d),
            water,
            camera_stream_targets: AHashMap::new(),
            camera_stream_external_bindings: AHashMap::new(),
            camera_stream_3d_bindings: AHashMap::new(),
            camera_stream_2d: Some(camera_stream_2d),
            camera_stream_3d: None,
            camera_stream_particles_3d: None,
            camera_stream_water: None,
            camera_stream_post: None,
            camera_stream_draws_scratch: Vec::new(),
            last_prepare_particles_revision: u64::MAX,
            last_prepare_water_2d_revision: u64::MAX,
            last_prepare_water_3d_revision: u64::MAX,
            last_prepare_3d_camera: None,
            last_prepare_3d_lighting: None,
            last_prepare_3d_draws_revision: u64::MAX,
            last_prepare_3d_decals_revision: u64::MAX,
            last_prepare_3d_width: render_width,
            last_prepare_3d_height: render_height,
            meshlets_enabled: cfg.meshlets_enabled,
            dev_meshlets: cfg.dev_meshlets,
            meshlet_debug_view: cfg.meshlet_debug_view,
            occlusion_culling: cfg.occlusion_culling,
            texture_filter: cfg.texture_filter,
            indirect_first_instance_enabled,
            multi_draw_indirect_enabled,
            gpu_timer,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(window: Arc<Window>, cfg: GpuConfig) -> Option<Self> {
        pollster::block_on(Self::new_async(window, cfg))
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
        let (render_width, render_height) =
            capped_render_size(width, height, self.device.limits().max_texture_dimension_2d);
        let render_size_changed =
            self.render_width != render_width || self.render_height != render_height;
        self.render_width = render_width;
        self.render_height = render_height;
        if !render_size_changed {
            return;
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.resize(&self.device, render_width, render_height);
        }
        self.post.resize(&self.device, render_width, render_height);
        self.accessibility
            .resize(&self.device, render_width, render_height);
        self.present_scene_bind_group = self
            .present
            .create_bind_group(&self.device, self.post.scene_view());
        self.present_intermediate_bind_group = self
            .present
            .create_bind_group(&self.device, self.accessibility.intermediate_view());
        self.msaa_color = create_msaa_color_target(
            &self.device,
            self.render_format,
            render_width,
            render_height,
            self.sample_count,
        );
        // Force next 3D prepare to refresh viewport-dependent GPU state.
        self.last_prepare_3d_width = 0;
        self.last_prepare_3d_height = 0;
    }

    pub fn set_smoothing_samples(&mut self, samples: u32) {
        let sample_count = clamp_supported_sample_count(
            normalize_sample_count(samples),
            self.max_supported_sample_count,
        );
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
                self.render_width,
                self.render_height,
            );
        }
        if let Some(point_particles_3d) = self.point_particles_3d.as_mut() {
            point_particles_3d.set_sample_count(&self.device, self.render_format, sample_count);
        }
        if self.water.is_some() {
            let rebuilt = if let (Some(water), Some(two_d), Some(three_d)) = (
                self.water.as_mut(),
                self.two_d.as_ref(),
                self.three_d.as_ref(),
            ) {
                water.set_sample_count(
                    &self.device,
                    self.render_format,
                    sample_count,
                    two_d.camera_bind_group_layout(),
                    three_d.water_camera_bind_group_layout(),
                );
                true
            } else {
                false
            };
            if !rebuilt {
                // Camera layouts unavailable: drop the water GPU state so it
                // is lazily recreated at the new sample count.
                self.water = None;
            }
        }
        self.msaa_color = create_msaa_color_target(
            &self.device,
            self.render_format,
            self.render_width,
            self.render_height,
            sample_count,
        );
    }

    pub fn render(&mut self, frame: RenderFrame<'_>) -> RenderGpuTiming {
        let total_start = Instant::now();
        let mut timing = RenderGpuTiming::default();
        if let Some(timer) = self.gpu_timer.as_mut() {
            timer.poll(&self.device);
            timing.gpu_timestamp_main = timer.last_main;
            timing.gpu_timestamp_water = timer.last_water;
        }
        let RenderFrame {
            resources,
            camera_3d,
            lighting_3d,
            draws_3d,
            draws_3d_revision,
            point_particles_3d,
            point_particles_3d_revision,
            waters_3d,
            waters_3d_revision,
            decals_3d,
            decals_3d_revision,
            camera_streams,
            camera_2d,
            camera_2d_position,
            post_processing_2d,
            post_processing_global,
            accessibility,
            rects_2d,
            upload_2d,
            sprites_2d,
            sprites_2d_revision,
            point_lights_2d,
            point_lights_2d_revision,
            shadow_casters_2d,
            waters_2d,
            waters_2d_revision,
            late_overlay_camera_2d,
            late_overlay_rects_2d,
            late_overlay_upload_2d,
            late_overlay_sprites_2d,
            late_overlay_sprites_2d_revision,
            late_overlay_point_lights_2d,
            late_overlay_point_lights_2d_revision,
            late_overlay_shadow_casters_2d,
            redraw_requested,
            frame_time_seconds,
            frame_delta_seconds,
            frame_dirty_bits,
            static_texture_lookup,
            static_mesh_lookup,
            static_shader_lookup,
            ui_primitives,
            ui_textures_delta,
            ui_texture_size,
            ui_revision,
        } = frame;
        let rect_draw_count = upload_2d.draw_count as u32;
        // Keep window alive for the full surface lifetime.
        self.window_handle.id();

        let post_requested = PostProcessor::has_effects(camera_3d.post_processing.as_ref())
            || PostProcessor::has_effects(post_processing_2d.as_ref())
            || PostProcessor::has_effects(post_processing_global.as_ref());

        let has = |bit: u32| (frame_dirty_bits & bit) != 0;

        let has_2d_content = upload_2d.draw_count > 0
            || !sprites_2d.is_empty()
            || !point_lights_2d.is_empty()
            || !waters_2d.is_empty();
        let rect_upload_dirty = upload_2d.full_reupload || !upload_2d.dirty_ranges.is_empty();
        let needs_2d_prepare = has(DIRTY_2D)
            || has(DIRTY_CAMERA_2D)
            || rect_upload_dirty
            || (has(DIRTY_RESOURCES) && has_2d_content)
            || (redraw_requested && has_2d_content);

        // A decal whose texture is still decoding must be retried each frame
        // until it resolves; otherwise it stays hidden until the next dirty
        // frame forces a re-prepare (looked like "white until reload").
        let decals_texture_pending = self
            .three_d
            .as_ref()
            .is_some_and(|three_d| three_d.decals_pending());

        let three_d_content_changed = self.last_prepare_3d_camera.as_ref() != Some(&camera_3d)
            || self.last_prepare_3d_lighting.as_ref() != Some(lighting_3d)
            || self.last_prepare_3d_draws_revision != draws_3d_revision
            || self.last_prepare_3d_decals_revision != decals_3d_revision
            || decals_texture_pending
            || self.last_prepare_3d_width != self.render_width
            || self.last_prepare_3d_height != self.render_height;

        let needs_3d = !draws_3d.is_empty();
        let needs_particles_3d = !point_particles_3d.is_empty();
        let needs_water = !waters_2d.is_empty() || !waters_3d.is_empty();

        let needs_3d_pipeline = has(DIRTY_3D)
            || has(DIRTY_CAMERA_3D)
            || has(DIRTY_LIGHTS_3D)
            || has(DIRTY_RESOURCES)
            || needs_3d
            || needs_particles_3d
            || needs_water
            || post_requested
            || three_d_content_changed;

        let needs_3d_prepare = has(DIRTY_3D)
            || has(DIRTY_CAMERA_3D)
            || has(DIRTY_LIGHTS_3D)
            || has(DIRTY_RESOURCES)
            || three_d_content_changed;

        let needs_3d_particles_path = has(DIRTY_PARTICLES_3D) || needs_particles_3d;
        let needs_3d_particles_prepare = needs_3d_particles_path
            && (has(DIRTY_PARTICLES_3D)
                || self.last_prepare_particles_revision != point_particles_3d_revision
                || three_d_content_changed);
        let needs_water_prepare = needs_water;

        if !camera_streams.is_empty() && self.two_d.is_none() {
            self.two_d = Some(Gpu2D::new(
                &self.device,
                self.render_format,
                self.sample_count,
                self.texture_filter,
            ));
        }
        for (node, stream) in camera_streams {
            if matches!(stream.source, CameraStreamSourceState::Webcam { .. }) {
                continue;
            }
            let resolution = [stream.resolution[0].max(1), stream.resolution[1].max(1)];
            let needs_external_binding =
                self.camera_stream_external_bindings.get(node).copied() != Some(resolution);
            let Some(target) = self.ensure_camera_stream_target(*node, resolution) else {
                continue;
            };
            if needs_external_binding {
                let texture_id = stream.output_texture;
                let view_2d = target
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let view_ui = target
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                if let Some(two_d) = self.two_d.as_mut() {
                    two_d.upsert_external_texture(
                        &self.device,
                        texture_id,
                        view_2d,
                        resolution[0],
                        resolution[1],
                    );
                }
                if self.ui.is_none() {
                    self.ui = Some(GpuUi::new(
                        &self.device,
                        self.config.format,
                        self.texture_filter,
                    ));
                }
                if let Some(ui) = self.ui.as_mut() {
                    ui.upsert_external_image_texture(&self.device, texture_id, view_ui, resolution);
                }
                self.camera_stream_external_bindings
                    .insert(*node, resolution);
            }
        }

        let prepare_2d_start = Instant::now();
        let mut did_prepare_2d = false;
        if needs_2d_prepare {
            if self.two_d.is_none() {
                self.two_d = Some(Gpu2D::new(
                    &self.device,
                    self.render_format,
                    self.sample_count,
                    self.texture_filter,
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
                        sprites_revision: sprites_2d_revision,
                        force_sprite_prepare: has(DIRTY_RESOURCES),
                        point_lights: point_lights_2d,
                        point_lights_revision: point_lights_2d_revision,
                        shadow_casters: shadow_casters_2d,
                        static_texture_lookup,
                    },
                );
                did_prepare_2d = true;
            }
        }
        if !did_prepare_2d {
            timing.skip_prepare_2d = 1;
        }
        if let Some(two_d) = self.two_d.as_ref() {
            timing.sprite_batches_2d = two_d.sprite_batch_count();
            timing.sprite_bind_group_switches_2d = two_d.sprite_bind_group_switch_count();
        }
        timing.prepare_2d = prepare_2d_start.elapsed();

        if needs_water_prepare {
            if self.three_d.is_none() {
                self.three_d = Some(Gpu3D::new(
                    &self.device,
                    &self.queue,
                    self.render_format,
                    Gpu3DConfig {
                        sample_count: self.sample_count,
                        width: self.render_width,
                        height: self.render_height,
                        meshlets_enabled: self.meshlets_enabled,
                        dev_meshlets: self.dev_meshlets,
                        meshlet_debug_view: self.meshlet_debug_view,
                        occlusion_culling: self.occlusion_culling,
                        indirect_first_instance_enabled: self.indirect_first_instance_enabled,
                        multi_draw_indirect_enabled: self.multi_draw_indirect_enabled,
                        texture_filter: self.texture_filter,
                    },
                ));
            }
            if self.water.is_none() {
                let Some(two_d) = self.two_d.as_ref() else {
                    return timing;
                };
                let Some(three_d) = self.three_d.as_ref() else {
                    return timing;
                };
                self.water = Some(GpuWater::new(
                    &self.device,
                    self.render_format,
                    self.sample_count,
                    two_d.camera_bind_group_layout(),
                    three_d.water_camera_bind_group_layout(),
                    three_d.depth_prepass_view(),
                ));
            }
            if let (Some(water), Some(three_d)) = (self.water.as_mut(), self.three_d.as_ref()) {
                water.set_scene_depth_view(&self.device, three_d.depth_prepass_view());
                let sky_color = sky_clear_color(lighting_3d)
                    .map(|color| [color.r as f32, color.g as f32, color.b as f32])
                    .unwrap_or([0.0, 0.0, 0.0]);
                let water_view_proj =
                    water_camera_view_proj(&camera_3d, self.render_width, self.render_height);
                water.prepare(
                    &self.device,
                    &self.queue,
                    waters_2d,
                    waters_3d,
                    WaterPrepareContext {
                        camera_2d_position,
                        camera_3d_position: camera_3d.position,
                        camera_3d_frustum_planes: water_extract_frustum_planes(water_view_proj),
                        sky_color,
                        time_seconds: frame_time_seconds,
                        delta_seconds: frame_delta_seconds,
                    },
                );
                self.last_prepare_water_2d_revision = waters_2d_revision;
                self.last_prepare_water_3d_revision = waters_3d_revision;
            }
        } else if !needs_water {
            if let Some(water) = self.water.as_mut() {
                water.clear_active();
            }
            self.last_prepare_water_2d_revision = u64::MAX;
            self.last_prepare_water_3d_revision = u64::MAX;
        }

        let prepare_3d_start = Instant::now();
        let mut did_prepare_3d = false;
        let mut prepare_3d_steps = Prepare3DStepTiming::default();
        if needs_3d_pipeline {
            if self.three_d.is_none() {
                self.three_d = Some(Gpu3D::new(
                    &self.device,
                    &self.queue,
                    self.render_format,
                    Gpu3DConfig {
                        sample_count: self.sample_count,
                        width: self.render_width,
                        height: self.render_height,
                        meshlets_enabled: self.meshlets_enabled,
                        dev_meshlets: self.dev_meshlets,
                        meshlet_debug_view: self.meshlet_debug_view,
                        occlusion_culling: self.occlusion_culling,
                        indirect_first_instance_enabled: self.indirect_first_instance_enabled,
                        multi_draw_indirect_enabled: self.multi_draw_indirect_enabled,
                        texture_filter: self.texture_filter,
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
                && needs_3d_prepare
            {
                for (node, stream) in camera_streams {
                    if matches!(stream.source, CameraStreamSourceState::Webcam { .. }) {
                        continue;
                    }
                    let resolution = [stream.resolution[0].max(1), stream.resolution[1].max(1)];
                    // Skip when the slot is already bound to the current target
                    // generation; `ensure_camera_stream_target` clears this entry
                    // whenever it recreates the target (resolution change).
                    if self.camera_stream_3d_bindings.get(node).copied() == Some(resolution) {
                        continue;
                    }
                    let Some(target) = self.camera_stream_targets.get(node) else {
                        continue;
                    };
                    let view = target
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    three_d.upsert_external_material_texture(
                        &self.device,
                        stream.output_texture.index(),
                        &view,
                        format!("__camera_stream__:{}", node.as_u64()),
                    );
                    self.camera_stream_3d_bindings.insert(*node, resolution);
                }
                three_d.prepare(
                    &self.device,
                    &self.queue,
                    Prepare3D {
                        resources,
                        camera: camera_3d.clone(),
                        lighting: lighting_3d,
                        draws: draws_3d,
                        draws_revision: draws_3d_revision,
                        force_full_rebuild: has(DIRTY_RESOURCES),
                        decals: decals_3d,
                        decals_revision: decals_3d_revision,
                        width: self.render_width,
                        height: self.render_height,
                        static_texture_lookup,
                        static_mesh_lookup,
                        static_shader_lookup,
                    },
                );
                did_prepare_3d = true;
                prepare_3d_steps = three_d.prepare_step_timing();
                self.last_prepare_3d_camera = Some(camera_3d.clone());
                self.last_prepare_3d_lighting = Some(lighting_3d.clone());
                self.last_prepare_3d_draws_revision = draws_3d_revision;
                self.last_prepare_3d_decals_revision = decals_3d_revision;
                self.last_prepare_3d_width = self.render_width;
                self.last_prepare_3d_height = self.render_height;
            }
            let prepare_particles_start = Instant::now();
            let mut did_prepare_particles_3d = false;
            if needs_3d_particles_prepare
                && let Some(point_particles_3d_gpu) = self.point_particles_3d.as_mut()
            {
                point_particles_3d_gpu.prepare(
                    &self.device,
                    &self.queue,
                    PreparePointParticles3D {
                        camera: camera_3d.clone(),
                        emitters: point_particles_3d,
                        width: self.render_width,
                        height: self.render_height,
                    },
                );
                self.last_prepare_particles_revision = point_particles_3d_revision;
                did_prepare_particles_3d = true;
            }
            timing.prepare_particles_3d = prepare_particles_start.elapsed();
            if !did_prepare_particles_3d {
                timing.skip_prepare_particles_3d = 1;
            }
        } else {
            timing.skip_prepare_particles_3d = 1;
        }
        if !did_prepare_3d {
            timing.skip_prepare_3d = 1;
            timing.skip_prepare_3d_frustum = 1;
            timing.skip_prepare_3d_hiz = 1;
            timing.skip_prepare_3d_indirect = 1;
            timing.skip_prepare_3d_cull_inputs = 1;
        } else {
            timing.prepare_3d_frustum = prepare_3d_steps.frustum_prep;
            timing.prepare_3d_hiz = prepare_3d_steps.hiz_prep;
            timing.prepare_3d_indirect = prepare_3d_steps.indirect_prep;
            timing.prepare_3d_cull_inputs = prepare_3d_steps.cull_input_prep;
            timing.skip_prepare_3d_frustum = prepare_3d_steps.frustum_skipped;
            timing.skip_prepare_3d_hiz = prepare_3d_steps.hiz_skipped;
            timing.skip_prepare_3d_indirect = prepare_3d_steps.indirect_skipped;
            timing.skip_prepare_3d_cull_inputs = prepare_3d_steps.cull_input_skipped;
        }
        if !needs_3d_particles_path {
            self.point_particles_3d = None;
            self.last_prepare_particles_revision = u64::MAX;
        }
        timing.prepare_3d = prepare_3d_start.elapsed();

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
        let surface_sized_render =
            self.render_width == self.config.width && self.render_height == self.config.height;
        // The seam pass needs a sampleable offscreen scene texture, so it
        // forces the non-direct path while active.
        let blend_screen_active = self
            .three_d
            .as_ref()
            .is_some_and(|three_d| three_d.screen_blend_active());
        let msaa_direct_present = surface_sized_render
            && self.sample_count > 1
            && !post_requested
            && !accessibility_enabled
            && !blend_screen_active
            && self.render_format == self.config.format;
        let direct_present = surface_sized_render
            && self.sample_count == 1
            && !post_requested
            && !accessibility_enabled
            && !blend_screen_active
            && self.render_format == self.config.format;
        let depth_prepass_needed = !waters_3d.is_empty()
            || (camera_post_enabled && PostProcessor::uses_depth(camera_post_chain))
            || (global_post_enabled && PostProcessor::uses_depth(global_post_chain));
        let mut frame = None;
        let mut swap_view = None;
        if direct_present || msaa_direct_present {
            let acquire_start = Instant::now();
            let acquire_surface_start = Instant::now();
            let acquired = match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    timing.acquire_surface = acquire_surface_start.elapsed();
                    self.surface.configure(&self.device, &self.config);
                    timing.acquire = acquire_start.elapsed();
                    timing.total = total_start.elapsed();
                    return timing;
                }
                wgpu::CurrentSurfaceTexture::Timeout
                | wgpu::CurrentSurfaceTexture::Occluded
                | wgpu::CurrentSurfaceTexture::Validation => {
                    timing.acquire_surface = acquire_surface_start.elapsed();
                    timing.acquire = acquire_start.elapsed();
                    timing.total = total_start.elapsed();
                    return timing;
                }
            };
            timing.acquire_surface = acquire_surface_start.elapsed();
            let acquire_view_start = Instant::now();
            let view = acquired
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            timing.acquire_view = acquire_view_start.elapsed();
            timing.acquire = acquire_start.elapsed();
            frame = Some(acquired);
            swap_view = Some(view);
        }
        let scene_view = self.post.scene_view().clone();
        let intermediate_view = self.accessibility.intermediate_view().clone();
        let color_view = if direct_present {
            let Some(view) = swap_view.as_ref() else {
                timing.total = total_start.elapsed();
                return timing;
            };
            view
        } else {
            self.msaa_color
                .as_ref()
                .map(|t| &t.view)
                .unwrap_or(&scene_view)
        };
        let resolve_view = if direct_present {
            None
        } else if msaa_direct_present {
            let Some(view) = swap_view.as_ref() else {
                timing.total = total_start.elapsed();
                return timing;
            };
            Some(view)
        } else if self.sample_count > 1 {
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
        let gpu_timer_active = self
            .gpu_timer
            .as_ref()
            .is_some_and(GpuTimestampTimer::can_write);
        if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
            timer.write_start(&mut encoder);
        }
        let clear_color = sky_clear_color(lighting_3d).unwrap_or(wgpu::Color {
            r: CLEAR_R,
            g: CLEAR_G,
            b: CLEAR_B,
            a: 1.0,
        });
        for (node, stream) in camera_streams {
            let has_stream_post = PostProcessor::has_effects(stream.post_processing.as_ref());
            let (target_view, post_input_view, post_depth_view) = {
                let Some(target) = self.camera_stream_targets.get(node) else {
                    continue;
                };
                (
                    target
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default()),
                    has_stream_post.then(|| {
                        target
                            .post_input
                            .create_view(&wgpu::TextureViewDescriptor::default())
                    }),
                    has_stream_post.then(|| {
                        target
                            .depth
                            .create_view(&wgpu::TextureViewDescriptor::default())
                    }),
                )
            };
            let Some(render_view) = (if has_stream_post {
                post_input_view.as_ref()
            } else {
                Some(&target_view)
            }) else {
                continue;
            };
            let mut stream_post_camera = None;
            let mut stream_post_depth_view = post_depth_view;
            if let CameraStreamSourceState::TwoD(camera) = &stream.source {
                let _clear_stream = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_camera_stream_clear_2d"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: render_view,
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
                drop(_clear_stream);
                if has_stream_post {
                    let Some(depth_view) = stream_post_depth_view.as_ref() else {
                        continue;
                    };
                    let _clear_depth = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("perro_camera_stream_depth_clear_2d"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    drop(_clear_depth);
                }
                if self.camera_stream_2d.is_none() {
                    self.camera_stream_2d = Some(Gpu2D::new(
                        &self.device,
                        self.render_format,
                        1,
                        self.texture_filter,
                    ));
                }
                if let Some(stream_2d) = self.camera_stream_2d.as_mut() {
                    let camera_position = camera.position;
                    let camera = camera_2d_uniform_from_state(
                        camera,
                        stream.resolution[0],
                        stream.resolution[1],
                    );
                    let empty_upload = RectUploadPlan {
                        full_reupload: true,
                        dirty_ranges: Vec::new(),
                        draw_count: 0,
                    };
                    stream_2d.prepare(
                        &self.device,
                        &self.queue,
                        Prepare2D {
                            resources,
                            camera,
                            rects: &[],
                            upload: &empty_upload,
                            sprites: stream.sprites_2d.as_ref(),
                            sprites_revision: sprites_2d_revision ^ node.as_u64(),
                            force_sprite_prepare: has(DIRTY_RESOURCES),
                            point_lights: stream.lights_2d.as_ref(),
                            point_lights_revision: u64::MAX,
                            shadow_casters: &[],
                            static_texture_lookup,
                        },
                    );
                    let particle_rect_count = stream_2d.prepare_stream_point_particles(
                        &self.device,
                        &self.queue,
                        stream.point_particles_2d.as_ref(),
                    );
                    if !stream.waters_2d.is_empty() {
                        if self.camera_stream_3d.is_none() {
                            let mut stream_3d = Gpu3D::new(
                                &self.device,
                                &self.queue,
                                self.render_format,
                                Gpu3DConfig {
                                    sample_count: 1,
                                    width: stream.resolution[0].max(1),
                                    height: stream.resolution[1].max(1),
                                    meshlets_enabled: self.meshlets_enabled,
                                    dev_meshlets: self.dev_meshlets,
                                    meshlet_debug_view: self.meshlet_debug_view,
                                    occlusion_culling: self.occlusion_culling,
                                    indirect_first_instance_enabled: self
                                        .indirect_first_instance_enabled,
                                    multi_draw_indirect_enabled: self.multi_draw_indirect_enabled,
                                    texture_filter: self.texture_filter,
                                },
                            );
                            // Camera streams render into their own targets;
                            // the seam pass only wires up the main scene.
                            stream_3d.set_screen_blend_supported(false);
                            self.camera_stream_3d = Some(stream_3d);
                        }
                        if self.camera_stream_water.is_none()
                            && let Some(stream_3d_ref) = self.camera_stream_3d.as_ref()
                        {
                            self.camera_stream_water = Some(GpuWater::new(
                                &self.device,
                                self.render_format,
                                1,
                                stream_2d.camera_bind_group_layout(),
                                stream_3d_ref.water_camera_bind_group_layout(),
                                stream_3d_ref.depth_prepass_view(),
                            ));
                        }
                        if let (Some(water), Some(stream_3d_ref)) = (
                            self.camera_stream_water.as_mut(),
                            self.camera_stream_3d.as_ref(),
                        ) {
                            water.set_scene_depth_view(
                                &self.device,
                                stream_3d_ref.depth_prepass_view(),
                            );
                            water.prepare(
                                &self.device,
                                &self.queue,
                                stream.waters_2d.as_ref(),
                                &[],
                                WaterPrepareContext {
                                    camera_2d_position: camera_position,
                                    camera_3d_position: [0.0, 0.0, 0.0],
                                    camera_3d_frustum_planes: [[0.0; 4]; 6],
                                    sky_color: [0.0, 0.0, 0.0],
                                    time_seconds: frame_time_seconds,
                                    delta_seconds: frame_delta_seconds,
                                },
                            );
                            water.encode(&mut encoder);
                            water.render_2d(
                                &mut encoder,
                                render_view,
                                None,
                                stream_2d.camera_bind_group(),
                                None,
                            );
                        }
                    }
                    stream_2d.render_pass(&mut encoder, render_view, None, particle_rect_count);
                }
            } else if let CameraStreamSourceState::ThreeD(camera) = &stream.source {
                stream_post_camera = Some(camera.clone());
                if self.camera_stream_3d.is_none() {
                    let mut stream_3d = Gpu3D::new(
                        &self.device,
                        &self.queue,
                        self.render_format,
                        Gpu3DConfig {
                            sample_count: 1,
                            width: stream.resolution[0].max(1),
                            height: stream.resolution[1].max(1),
                            meshlets_enabled: self.meshlets_enabled,
                            dev_meshlets: self.dev_meshlets,
                            meshlet_debug_view: self.meshlet_debug_view,
                            occlusion_culling: self.occlusion_culling,
                            indirect_first_instance_enabled: self.indirect_first_instance_enabled,
                            multi_draw_indirect_enabled: self.multi_draw_indirect_enabled,
                            texture_filter: self.texture_filter,
                        },
                    );
                    // Camera streams render into their own targets; the seam
                    // pass only wires up the main scene.
                    stream_3d.set_screen_blend_supported(false);
                    self.camera_stream_3d = Some(stream_3d);
                }
                if let Some(stream_3d) = self.camera_stream_3d.as_mut() {
                    let width = stream.resolution[0].max(1);
                    let height = stream.resolution[1].max(1);
                    fill_camera_stream_draws_3d(
                        stream.draws_3d.as_ref(),
                        &mut self.camera_stream_draws_scratch,
                    );
                    let stream_lighting = camera_stream_lighting_3d(&stream.lighting_3d);
                    let stream_clear_color =
                        sky_clear_color(&stream_lighting).unwrap_or(wgpu::Color {
                            r: CLEAR_R,
                            g: CLEAR_G,
                            b: CLEAR_B,
                            a: 1.0,
                        });
                    stream_3d.resize(&self.device, width, height);
                    stream_3d.prepare(
                        &self.device,
                        &self.queue,
                        Prepare3D {
                            resources,
                            camera: camera.clone(),
                            lighting: &stream_lighting,
                            draws: &self.camera_stream_draws_scratch,
                            draws_revision: draws_3d_revision ^ node.as_u64(),
                            force_full_rebuild: has(DIRTY_RESOURCES),
                            decals: &[],
                            decals_revision: 0,
                            width,
                            height,
                            static_texture_lookup,
                            static_mesh_lookup,
                            static_shader_lookup,
                        },
                    );
                    stream_3d.render_pass(&mut encoder, render_view, stream_clear_color, false);
                    if !stream.point_particles_3d.is_empty() {
                        if self.camera_stream_particles_3d.is_none() {
                            self.camera_stream_particles_3d = Some(GpuPointParticles3D::new(
                                &self.device,
                                self.render_format,
                                1,
                            ));
                        }
                        if let Some(particles) = self.camera_stream_particles_3d.as_mut() {
                            particles.prepare(
                                &self.device,
                                &self.queue,
                                PreparePointParticles3D {
                                    camera: camera.clone(),
                                    emitters: stream.point_particles_3d.as_ref(),
                                    width,
                                    height,
                                },
                            );
                            particles.render_pass(
                                &mut encoder,
                                render_view,
                                stream_3d.depth_view(),
                            );
                        }
                    }
                    if !stream.waters_3d.is_empty() {
                        if self.camera_stream_water.is_none()
                            && let Some(stream_2d_ref) = self.camera_stream_2d.as_ref()
                        {
                            self.camera_stream_water = Some(GpuWater::new(
                                &self.device,
                                self.render_format,
                                1,
                                stream_2d_ref.camera_bind_group_layout(),
                                stream_3d.water_camera_bind_group_layout(),
                                stream_3d.depth_prepass_view(),
                            ));
                        }
                        if let Some(water) = self.camera_stream_water.as_mut() {
                            water
                                .set_scene_depth_view(&self.device, stream_3d.depth_prepass_view());
                            let water_view_proj = water_camera_view_proj(camera, width, height);
                            water.prepare(
                                &self.device,
                                &self.queue,
                                &[],
                                stream.waters_3d.as_ref(),
                                WaterPrepareContext {
                                    camera_2d_position: [0.0, 0.0],
                                    camera_3d_position: camera.position,
                                    camera_3d_frustum_planes: water_extract_frustum_planes(
                                        water_view_proj,
                                    ),
                                    sky_color: sky_clear_color(&stream_lighting)
                                        .map(|color| {
                                            [color.r as f32, color.g as f32, color.b as f32]
                                        })
                                        .unwrap_or([0.0, 0.0, 0.0]),
                                    time_seconds: frame_time_seconds,
                                    delta_seconds: frame_delta_seconds,
                                },
                            );
                            water.encode(&mut encoder);
                            water.render_3d(
                                &mut encoder,
                                render_view,
                                stream_3d.depth_view(),
                                stream_3d.water_camera_bind_group(),
                                false,
                            );
                        }
                    }
                    if has_stream_post {
                        stream_post_depth_view = Some(stream_3d.depth_prepass_view().clone());
                    }
                }
            }
            if has_stream_post {
                if self.camera_stream_post.is_none() {
                    self.camera_stream_post = Some(PostProcessor::new(
                        &self.device,
                        &self.queue,
                        self.render_format,
                        stream.resolution[0].max(1),
                        stream.resolution[1].max(1),
                    ));
                }
                let camera = stream_post_camera.unwrap_or_default();
                if let Some(post) = self.camera_stream_post.as_mut() {
                    let (Some(depth_view), Some(input_view)) =
                        (stream_post_depth_view.as_ref(), post_input_view.as_ref())
                    else {
                        continue;
                    };
                    post.resize(
                        &self.device,
                        stream.resolution[0].max(1),
                        stream.resolution[1].max(1),
                    );
                    let post_context = PostProcessContext {
                        device: &self.device,
                        queue: &self.queue,
                        output_view: &target_view,
                        camera: &camera,
                        static_shader_lookup,
                        static_texture_lookup,
                    };
                    let post_chain_data = PostProcessChainData {
                        input_view,
                        depth_view,
                        effects: stream.post_processing.as_ref(),
                    };
                    post.apply_chain(&post_context, &post_chain_data, &mut encoder);
                }
            }
        }
        if let Some(water) = self.water.as_ref() {
            if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
                timer.write_water_start(&mut encoder);
            }
            water.encode(&mut encoder);
            if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
                timer.write_water_end(&mut encoder);
            }
        } else if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
            timer.write_water_start(&mut encoder);
            timer.write_water_end(&mut encoder);
        }
        let clear_in_water_pass =
            self.three_d.is_none() && self.two_d.is_some() && !waters_2d.is_empty();
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.render_pass(&mut encoder, color_view, clear_color, depth_prepass_needed);
            // Seam pass runs on the resolved offscreen scene texture, before
            // particles/water/2D draw on top.
            if blend_screen_active && !direct_present && self.sample_count == 1 {
                three_d.mesh_blend_screen_pass(
                    &self.device,
                    &mut encoder,
                    self.post.scene_texture(),
                    &scene_view,
                );
            }
            if let Some(point_particles_3d_gpu) = self.point_particles_3d.as_mut() {
                point_particles_3d_gpu.render_pass(&mut encoder, color_view, three_d.depth_view());
            }
            if let Some(water) = self.water.as_ref() {
                let clear_water_depth = draws_3d.is_empty()
                    && point_particles_3d.is_empty()
                    && lighting_3d.sky.is_none();
                water.render_3d(
                    &mut encoder,
                    color_view,
                    three_d.depth_view(),
                    three_d.water_camera_bind_group(),
                    clear_water_depth,
                );
            }
        } else if !clear_in_water_pass {
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
            let two_d_draws = two_d.draw_call_count(rect_draw_count) > 0;
            if let Some(water) = self.water.as_ref() {
                water.render_2d(
                    &mut encoder,
                    color_view,
                    (!two_d_draws).then_some(resolve_view).flatten(),
                    two_d.camera_bind_group(),
                    clear_in_water_pass.then_some(clear_color),
                );
            }
            if two_d_draws {
                two_d.render_pass(&mut encoder, color_view, resolve_view, rect_draw_count);
            } else if waters_2d.is_empty()
                && let Some(resolve_target) = resolve_view
            {
                let _resolve_only_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_msaa_resolve_only_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color_view,
                        resolve_target: Some(resolve_target),
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
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
        } else if let Some(resolve_target) = resolve_view {
            // No 2D pass still needs one resolve pass on MSAA paths.
            let _resolve_only_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_msaa_resolve_only_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: Some(resolve_target),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
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
        if blend_screen_active
            && !direct_present
            && !msaa_direct_present
            && self.sample_count > 1
            && let Some(three_d) = self.three_d.as_mut()
        {
            three_d.mesh_blend_screen_pass(
                &self.device,
                &mut encoder,
                self.post.scene_texture(),
                &scene_view,
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
                static_texture_lookup,
            };
            let Some(three_d) = self.three_d.as_ref() else {
                return;
            };
            let post_chain_data = PostProcessChainData {
                input_view,
                depth_view: three_d.depth_prepass_view(),
                effects,
            };
            self.post
                .apply_chain(&post_context, &post_chain_data, &mut encoder);
            *current_tex = next_tex;
        };
        if camera_post_enabled {
            apply_post_chain(camera_post_chain, &mut current_tex);
        }
        if global_post_enabled {
            apply_post_chain(global_post_chain, &mut current_tex);
        }
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

        if !direct_present && !msaa_direct_present {
            let final_bind_group = match current_tex {
                FrameTex::Scene => &self.present_scene_bind_group,
                FrameTex::Intermediate => &self.present_intermediate_bind_group,
            };
            let acquire_start = Instant::now();
            let acquire_surface_start = Instant::now();
            let acquired = match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    timing.acquire_surface = acquire_surface_start.elapsed();
                    self.surface.configure(&self.device, &self.config);
                    timing.acquire = acquire_start.elapsed();
                    timing.total = total_start.elapsed();
                    return timing;
                }
                wgpu::CurrentSurfaceTexture::Timeout
                | wgpu::CurrentSurfaceTexture::Occluded
                | wgpu::CurrentSurfaceTexture::Validation => {
                    timing.acquire_surface = acquire_surface_start.elapsed();
                    timing.acquire = acquire_start.elapsed();
                    timing.total = total_start.elapsed();
                    return timing;
                }
            };
            timing.acquire_surface = acquire_surface_start.elapsed();
            let acquire_view_start = Instant::now();
            let view = acquired
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            timing.acquire_view = acquire_view_start.elapsed();
            timing.acquire = acquire_start.elapsed();
            self.present.apply(&mut encoder, final_bind_group, &view);
            swap_view = Some(view);
            frame = Some(acquired);
        }
        if ui_primitives.is_empty() {
            if let Some(ui) = self.ui.as_mut() {
                ui.clear();
            }
        } else {
            if self.ui.is_none() {
                self.ui = Some(GpuUi::new(
                    &self.device,
                    self.config.format,
                    self.texture_filter,
                ));
            }
            if let (Some(ui), Some(output_view)) = (self.ui.as_mut(), swap_view.as_ref()) {
                let viewport = [self.config.width.max(1), self.config.height.max(1)];
                ui.prepare(
                    &self.device,
                    &self.queue,
                    UiPrepareInput {
                        resources,
                        viewport,
                        primitives: ui_primitives,
                        textures_delta: ui_textures_delta,
                        texture_size: ui_texture_size,
                        revision: ui_revision,
                        static_texture_lookup,
                    },
                );
                ui.render_pass(&self.device, &mut encoder, output_view, viewport);
            }
        }
        if late_overlay_upload_2d.draw_count > 0
            || !late_overlay_sprites_2d.is_empty()
            || !late_overlay_point_lights_2d.is_empty()
        {
            if self.late_overlay_2d.is_none() {
                self.late_overlay_2d = Some(Gpu2D::new(
                    &self.device,
                    self.config.format,
                    1,
                    self.texture_filter,
                ));
            }
            if let (Some(late_overlay_2d), Some(output_view)) =
                (self.late_overlay_2d.as_mut(), swap_view.as_ref())
            {
                late_overlay_2d.prepare(
                    &self.device,
                    &self.queue,
                    Prepare2D {
                        resources,
                        camera: late_overlay_camera_2d,
                        rects: late_overlay_rects_2d,
                        upload: late_overlay_upload_2d,
                        sprites: late_overlay_sprites_2d,
                        sprites_revision: late_overlay_sprites_2d_revision,
                        force_sprite_prepare: has(DIRTY_RESOURCES),
                        point_lights: late_overlay_point_lights_2d,
                        point_lights_revision: late_overlay_point_lights_2d_revision,
                        shadow_casters: late_overlay_shadow_casters_2d,
                        static_texture_lookup,
                    },
                );
                late_overlay_2d.render_pass(
                    &mut encoder,
                    output_view,
                    None,
                    late_overlay_upload_2d.draw_count as u32,
                );
            }
        }
        if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
            timer.write_end_and_resolve(&mut encoder);
        }
        if let Some(water) = self.water.as_mut() {
            water.encode_readback(&mut encoder);
        }
        let submit_start = Instant::now();
        let submit_finish_start = Instant::now();
        let command_buffer = encoder.finish();
        timing.submit_finish_main = submit_finish_start.elapsed();
        let submit_queue_start = Instant::now();
        self.queue.submit(Some(command_buffer));
        if gpu_timer_active && let Some(timer) = self.gpu_timer.as_mut() {
            timer.request_readback();
        }
        if let Some(water) = self.water.as_mut() {
            water.finish_frame();
            water.request_readback();
        }
        timing.submit_queue_main = submit_queue_start.elapsed();
        timing.submit_main = submit_start.elapsed();
        timing.draw_calls_2d = self
            .two_d
            .as_ref()
            .map(|two_d| two_d.draw_call_count(rect_draw_count))
            .unwrap_or(0)
            + self.ui.as_ref().map(GpuUi::draw_call_count).unwrap_or(0);
        timing.draw_calls_3d = self
            .three_d
            .as_ref()
            .map(|three_d| three_d.draw_call_count())
            .unwrap_or(0);
        if let Some(three_d) = self.three_d.as_ref() {
            timing.draw_batches_3d = three_d.draw_batch_count();
            timing.pipeline_switches_3d = three_d.pipeline_switch_count();
            timing.texture_bind_group_switches_3d = three_d.texture_bind_group_switch_count();
        }
        let present_start = Instant::now();
        if let Some(frame) = frame {
            self.queue.present(frame);
            timing.present = present_start.elapsed();
            timing.presented = true;
        }
        timing.total = total_start.elapsed();
        timing
    }

    pub fn drain_water_samples(&mut self, out: &mut Vec<WaterSampleState>) {
        if let Some(water) = self.water.as_mut() {
            water.drain_samples(out);
        }
    }

    pub fn drain_water_body_samples(&mut self, out: &mut Vec<WaterBodySampleState>) {
        if let Some(water) = self.water.as_mut() {
            water.drain_body_samples(out);
        }
    }

    pub fn virtual_size() -> [f32; 2] {
        Gpu2D::virtual_size()
    }
}

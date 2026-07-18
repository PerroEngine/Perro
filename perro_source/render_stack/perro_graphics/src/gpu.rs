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
    WaterSampleState, WaterShapeState,
};
use perro_structs::VisualAccessibilitySettings;
use perro_structs::{PostProcessEffect, TextureFilterMode};
use std::sync::{Arc, mpsc};
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
use winit::window::Window;

#[path = "gpu/present.rs"]
mod present;
#[path = "water_flip_gpu.rs"]
mod water_flip_gpu;
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

fn camera_underwater<'a>(
    camera: &Camera3DState,
    waters: &'a [(NodeID, Water3DState)],
) -> Option<&'a Water3DState> {
    let camera_world = Vec3::from_array(camera.position);
    waters.iter().find_map(|(_, water)| {
        let model = Mat4::from_cols_array_2d(&water.model);
        let local = model.inverse().transform_point3(camera_world);
        // Keep the exact surface in the above-water path to avoid post-effect
        // flicker as wave/camera jitter crosses y=0.
        if !local.is_finite() || local.y >= -0.02 || local.y < -water.depth.max(0.0) {
            return None;
        }
        let inside = match water.shape {
            WaterShapeState::Rect => {
                local.x.abs() <= water.size[0].abs() * 0.5
                    && local.z.abs() <= water.size[1].abs() * 0.5
            }
            WaterShapeState::Circle { radius } => {
                local.x * local.x + local.z * local.z <= radius * radius
            }
            WaterShapeState::Cylinder {
                radius,
                half_height,
            } => {
                local.x * local.x + local.z * local.z <= radius * radius
                    && local.y.abs() <= half_height.abs()
            }
        };
        inside.then_some(water)
    })
}

fn underwater_effects(water: &Water3DState) -> [PostProcessEffect; 3] {
    let color = water.deep_color;
    let tint = [color.r.to_f32(), color.g.to_f32(), color.b.to_f32()];
    let scatter = water.scattering_strength.clamp(0.0, 2.0);
    let fog = water.distance_fog_strength.clamp(0.0, 2.0);
    [
        PostProcessEffect::Warp {
            waves: 34.0,
            strength: (water.refraction_strength * 0.003).clamp(0.0005, 0.012),
        },
        PostProcessEffect::ColorFilter {
            color: tint,
            strength: (0.22 + scatter * 0.18 + fog * 0.12).clamp(0.2, 0.72),
        },
        PostProcessEffect::Vignette {
            strength: (0.08 + fog * 0.12).clamp(0.08, 0.35),
            radius: 0.82,
            softness: 0.5,
        },
    ]
}

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
    exposure_bgl: Option<wgpu::BindGroupLayout>,
    exposure_pipeline: Option<wgpu::ComputePipeline>,
    exposure_config_buffer: wgpu::Buffer,
    exposure_state_buffer: wgpu::Buffer,
    exposure_uniform_buffer: wgpu::Buffer,
}

struct PresentBindGroups {
    tonemap: wgpu::BindGroup,
    exposure: Option<wgpu::BindGroup>,
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
    surface_view_format: wgpu::TextureFormat,
    render_width: u32,
    render_height: u32,
    render_format: wgpu::TextureFormat,
    sample_count: u32,
    max_supported_sample_count: u32,
    msaa_color: Option<MsaaColorTarget>,
    post: PostProcessor,
    post_view_generation: u64,
    accessibility: VisualAccessibilityProcessor,
    present: PresentProcessor,
    present_scene_bind_group: PresentBindGroups,
    present_intermediate_bind_group: PresentBindGroups,
    two_d: Option<Gpu2D>,
    late_overlay_2d: Option<Gpu2D>,
    ui: Option<GpuUi>,
    three_d: Option<Gpu3D>,
    point_particles_3d: Option<GpuPointParticles3D>,
    water: Option<GpuWater>,
    camera_stream_targets: AHashMap<NodeID, GpuCameraStreamTarget>,
    next_camera_stream_post_view_key: u64,
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
    ssao: crate::SsaoQuality,
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
    pub ssao: crate::SsaoQuality,
    pub texture_filter: TextureFilterMode,
}

struct GpuCameraStreamTarget {
    texture: wgpu::Texture,
    post_input: wgpu::Texture,
    depth: wgpu::Texture,
    resolution: [u32; 2],
    post_view_key: u64,
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
    pub ui_primitive_depths: &'a [Option<Arc<[f32]>>],
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

#[inline]
fn next_nonzero_generation(current: u64) -> u64 {
    let next = current.wrapping_add(1);
    if next == 0 { 1 } else { next }
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

mod frame;
mod lifecycle;
mod textures;

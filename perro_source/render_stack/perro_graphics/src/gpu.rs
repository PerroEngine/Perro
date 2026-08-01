use crate::{
    backend::{OcclusionCullingMode, StaticMeshLookup, StaticShaderLookup, StaticTextureLookup},
    postprocess::{PostProcessChainData, PostProcessContext, PostProcessor},
    resources::ResourceStore,
    shared_textures::SharedTextureStore,
    three_d::{
        gpu::{
            Gpu3D, Gpu3DConfig, PipelineRegistryCache, Prepare3D, Prepare3DStepTiming,
            SharedMeshArena,
        },
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
    CameraStreamSourceState, CameraStreamState, Decal3DState, HdrColorSpace, HdrFallback, HdrMode,
    HdrStatus, LODOptions3D, Light2DState, MeshBlendOptions3D, PointParticles3DState,
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

#[path = "gpu/camera_stream_tonemap.rs"]
mod camera_stream_tonemap;
#[path = "gpu/image_save.rs"]
mod image_save;
use image_save::{CameraImageSaveRequest, PendingCameraImageSave};
#[path = "gpu/present.rs"]
mod present;
#[path = "gpu/smaa_lut.rs"]
mod smaa_lut;
#[path = "water_flip_gpu.rs"]
mod water_flip_gpu;
#[path = "water_gpu.rs"]
mod water_gpu;

use camera_stream_tonemap::{CameraStreamTonemap, CameraStreamTonemapSettings};
pub(crate) use present::capped_render_size;
use present::*;
use water_gpu::{GpuWater, WaterPrepareContext};

// Linear-space clear color for sRGB hex #40474F — neutral dark slate so
// scenes without a Sky3D read as a lit viewport instead of black.
const CLEAR_R: f64 = 0.050876;
const CLEAR_G: f64 = 0.063763;
const CLEAR_B: f64 = 0.079339;
const SMOOTH_SAMPLE_COUNT: u32 = 4;
// Frames between shared-texture refcount sweeps (~5s at 60fps; an entry is
// evicted after staying unreferenced for 2 consecutive sweeps).
const SHARED_TEXTURE_SWEEP_INTERVAL_FRAMES: u32 = 300;

fn premultiplied_clear_color(color: perro_structs::Color) -> wgpu::Color {
    let alpha = color.a().clamp(0.0, 1.0);
    wgpu::Color {
        r: (color.r() * alpha) as f64,
        g: (color.g() * alpha) as f64,
        b: (color.b() * alpha) as f64,
        a: alpha as f64,
    }
}

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
        CameraStreamDraw3DState::CameraStreamQuad {
            texture,
            tint,
            node,
            model,
            size,
        } => {
            let model = Mat4::from_cols_array_2d(model)
                * Mat4::from_scale(Vec3::new(size[0].max(0.001), size[1].max(0.001), 1.0));
            Draw3DInstance {
                node: *node,
                kind: Draw3DKind::CameraStreamQuad {
                    texture: *texture,
                    tint: tint.to_float_slice(),
                },
                surfaces: Arc::from([]),
                instance_mats: Arc::from([model.to_cols_array_2d()]),
                blend_shape_weights: Arc::from([]),
                debug_color: None,
                skeleton: None,
                dense_multimesh: None,
                meshlet_override: Some(false),
                lod: LODOptions3D::default(),
                blend: MeshBlendOptions3D::default(),
                cast_shadows: false,
                receive_shadows: false,
            }
        }
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

#[inline]
fn camera_stream_uses_render_target(stream: &CameraStreamState) -> bool {
    match &stream.source {
        CameraStreamSourceState::Webcam { texture, .. } => stream.output_texture != *texture,
        _ => true,
    }
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
/// camera-stream upsert/remove traffic. split from DIRTY_RESOURCES so stream
/// state changes stop forcing the main-3D full rebuild + sprite re-prepare;
/// per-stream change tracking decides which stream targets re-render.
pub const DIRTY_STREAMS: u32 = 1 << 9;

struct MsaaColorTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct PresentProcessor {
    // Cheap Arc handle; kept so the lazy FXAA stage can allocate its target
    // and pipeline inside apply() without widening the call signature.
    device: wgpu::Device,
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    exposure_bgl: Option<wgpu::BindGroupLayout>,
    // Auto-exposure luminance reduction, two passes: `exposure_pipeline`
    // reduces N tiles into per-workgroup partials, `exposure_resolve_pipeline`
    // folds the partials and runs the temporal adaptation. Both None when the
    // device cannot run the compute path (manual exposure fallback).
    exposure_pipeline: Option<wgpu::ComputePipeline>,
    exposure_resolve_pipeline: Option<wgpu::ComputePipeline>,
    // Pass-1 workgroup count of the last encoded frame (0 = auto-exposure did
    // not run). Diagnostics/tests only.
    last_exposure_groups: u32,
    exposure_config_buffer: wgpu::Buffer,
    exposure_state_buffer: wgpu::Buffer,
    exposure_uniform_buffer: wgpu::Buffer,
    output_uniform_buffer: wgpu::Buffer,
    // Bit pattern of the last `PresentOutputConfig` pushed. HDR status only
    // moves on a display/settings change, so the per-frame write is skipped.
    last_output_uniform: Option<[u32; 4]>,
    output_format: wgpu::TextureFormat,
    // FXAA gate: config requested it AND the live sample count is 1 (no FXAA
    // on top of MSAA). Resources below stay None until the first frame that
    // actually runs the pass (pay-for-use) and drop when it turns off.
    fxaa_active: bool,
    fxaa: Option<FxaaResources>,
    // SMAA gate; same lifecycle as the FXAA gate (config + sample count 1),
    // mutually exclusive with FXAA by construction (one anti_alias mode).
    smaa_active: bool,
    smaa: Option<SmaaResources>,
    // TAA gate; same lifecycle (config + sample count 1), mutually exclusive
    // with FXAA/SMAA by construction (one anti_alias mode).
    taa_active: bool,
    taa: Option<TaaResources>,
    // Swapchain size, pushed by the surface lifecycle. Equal to the render
    // size unless the render size is capped; the TAA resolve merges its
    // swapchain write into the resolve pass only in the equal case. [0, 0]
    // until the lifecycle pushes it (conservative two-pass path).
    output_size: [u32; 2],
    // Render passes the last TAA encode emitted (1 = merged MRT resolve,
    // 2 = resolve + upscale blit). Diagnostics/tests only.
    last_taa_passes: u32,
}

/// Lazily allocated FXAA stage: the present pass tonemaps into `target`
/// (render-resolution LDR copy in the output format) and the FXAA pass
/// reads it back out to the swapchain. Only exists while FXAA runs.
struct FxaaResources {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    _target: wgpu::Texture,
    target_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    size: [u32; 2],
}

/// Size-independent SMAA pieces: the three pipelines + layouts, the nearest
/// sampler (SearchTex must never blend adjacent step counts) and the two
/// procedurally generated lookup textures (see smaa_lut.rs). Survives
/// resizes; only rebuilt after a full SMAA off/on cycle.
struct SmaaCore {
    edge_pipeline: wgpu::RenderPipeline,
    edge_bgl: wgpu::BindGroupLayout,
    weights_pipeline: wgpu::RenderPipeline,
    weights_bgl: wgpu::BindGroupLayout,
    blend_pipeline: wgpu::RenderPipeline,
    blend_bgl: wgpu::BindGroupLayout,
    nearest_sampler: wgpu::Sampler,
    _area_tex: wgpu::Texture,
    area_view: wgpu::TextureView,
    _search_tex: wgpu::Texture,
    search_view: wgpu::TextureView,
}

/// Lazily allocated SMAA 1x stage: the present pass tonemaps into the LDR
/// target, then three passes run — edge detection (RG8 edges target),
/// blending-weight calculation (RGBA8 weights target, fed by the lookup
/// textures in `core`), and neighborhood blending back out to the swapchain
/// (doing the upscale when the render size is capped, like FXAA). The
/// render-size targets and bind groups rebuild when dimensions change; only
/// exists while SMAA runs.
struct SmaaResources {
    core: SmaaCore,
    _ldr_target: wgpu::Texture,
    ldr_view: wgpu::TextureView,
    _edges_target: wgpu::Texture,
    edges_view: wgpu::TextureView,
    _weights_target: wgpu::Texture,
    weights_view: wgpu::TextureView,
    edge_bind_group: wgpu::BindGroup,
    weights_bind_group: wgpu::BindGroup,
    blend_bind_group: wgpu::BindGroup,
    size: [u32; 2],
}

/// Size-independent TAA pieces: the resolve + blit pipelines/layouts and the
/// per-frame reprojection uniform buffer. Survives resizes; only rebuilt
/// after a full TAA off/on cycle.
struct TaaCore {
    resolve_pipeline: wgpu::RenderPipeline,
    // MRT resolve: history + swapchain in one pass, used when render size ==
    // output size (the blit would be a pure copy). Shares `resolve_bgl`.
    resolve_direct_pipeline: wgpu::RenderPipeline,
    resolve_bgl: wgpu::BindGroupLayout,
    blit_pipeline: wgpu::RenderPipeline,
    blit_bgl: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
}

/// Lazily allocated TAA v1 stage (camera-reprojection only, no per-object
/// motion vectors — moving objects ghost within the neighborhood clamp; see
/// taa.wgsl). The present pass tonemaps into the render-resolution LDR
/// target; the resolve pass reads it + scene depth + the previous history
/// texture and writes the blended result into the other history texture
/// (rgb = resolved color, a = device depth for next frame's disocclusion
/// test); the blit pass then copies/upscales that history to the swapchain.
/// The ping-pong pair means no extra copy: next frame samples what this
/// frame rendered. Only exists while TAA runs; render-size targets and the
/// blit bind groups rebuild when dimensions change (history restarts from
/// the current frame via `history_valid`).
struct TaaResources {
    core: TaaCore,
    _ldr_target: wgpu::Texture,
    ldr_view: wgpu::TextureView,
    _history: [wgpu::Texture; 2],
    history_views: [wgpu::TextureView; 2],
    // Bind group for blitting history[i] to the swapchain.
    blit_bind_groups: [wgpu::BindGroup; 2],
    // Index of the history texture holding LAST frame's resolve (the read
    // side); this frame resolves into the other one, then the index flips.
    history_index: usize,
    // False on (re)creation and whenever a frame runs without scene depth;
    // the next resolve then blends 0% history (clean restart, no stale
    // ghosting after resizes or 3D-scene switches).
    history_valid: bool,
    size: [u32; 2],
}

/// Per-frame inputs for the TAA resolve, built in `Gpu::render` while the 3D
/// pipeline is live: the scene depth view (at 1x this aliases the depth
/// prepass texture, so it holds final opaque scene depth by present time)
/// and the UNJITTERED current/previous camera matrices for reprojection.
struct PresentTaaFrame {
    depth_view: wgpu::TextureView,
    inv_view_proj: [[f32; 4]; 4],
    prev_view_proj: [[f32; 4]; 4],
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
                    // Range failure after a "successful" map means the buffer
                    // is gone (device loss destroys it); unmap would trip
                    // wgpu's destroyed-buffer validation and panic the app.
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
            // Failed or abandoned map: the buffer never reached the mapped
            // state (device loss / TDR marks it destroyed), so there is
            // nothing to unmap - calling unmap here is the destroyed-buffer
            // panic seen as `Buffer::buffer_unmap ... has been destroyed`.
            Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                self.pending_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
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
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    // Live window size recorded when an acquire reported the swapchain extent
    // drifted (Vulkan raises SUBOPTIMAL, not OUTDATED, for a size mismatch).
    // Drained by the runner, which drives a normal resize so surface, render
    // targets, and the 2D/UI viewport all move together.
    surface_resync_request: Option<(u32, u32)>,
    surface_view_format: wgpu::TextureFormat,
    hdr_status: HdrStatus,
    render_width: u32,
    render_height: u32,
    max_render_pixels: u64,
    render_format: wgpu::TextureFormat,
    sample_count: u32,
    max_supported_sample_count: u32,
    // 3D sample count latched on the first frame that needs the 3D pipeline
    // (graphics.msaa); until then the cheaper 2D setting (graphics.msaa_2d)
    // applies. Explicit set_smoothing_samples calls latch immediately.
    sample_count_3d_target: u32,
    sample_count_3d_applied: bool,
    // FXAA requested by config; the present processor's active flag follows
    // `fxaa_requested && sample_count == 1` across sample-count changes.
    fxaa_requested: bool,
    // Same gate for SMAA (anti_alias = "smaa"); never both true with FXAA.
    smaa_requested: bool,
    // Same gate for TAA (anti_alias = "taa"); exclusive with FXAA/SMAA.
    taa_requested: bool,
    // Halton(2,3) jitter sequence position; advances every rendered frame
    // while TAA runs so the sub-pixel offsets cycle through all 8 samples.
    taa_frame_index: u32,
    // Previous frame's UNJITTERED view-proj for the resolve reprojection.
    // None while TAA is off or no 3D pipeline ran last frame.
    taa_prev_view_proj: Option<Mat4>,
    shadow_pcf_high: bool,
    msaa_color: Option<MsaaColorTarget>,
    post: PostProcessor,
    post_view_generation: u64,
    accessibility: Option<VisualAccessibilityProcessor>,
    present: PresentProcessor,
    present_scene_bind_group: PresentBindGroups,
    present_intermediate_bind_group: Option<PresentBindGroups>,
    // One texel upload per (source, color space, mips) shared by every
    // consumer cache below; consumers keep handles + their own bind groups.
    shared_textures: SharedTextureStore,
    // One mesh vertex/index arena set shared by the main Gpu3D and every
    // camera-stream Gpu3D: a custom mesh drawn in N views is decoded and
    // uploaded once, not N times. Views mirror its buffer handles and compare
    // its generations at prepare (see three_d/gpu/buffers/mesh_arena.rs).
    mesh_arena: SharedMeshArena,
    // One lazily-built pipeline registry per (format, sample count); the main
    // Gpu3D and camera-stream Gpu3Ds share pipelines through it (see
    // three_d/gpu/pipeline_registry.rs).
    pipeline_registries: PipelineRegistryCache,
    shared_texture_frame_counter: u32,
    two_d: Option<Gpu2D>,
    late_overlay_2d: Option<Gpu2D>,
    ui: Option<GpuUi>,
    three_d: Option<Gpu3D>,
    point_particles_3d: Option<GpuPointParticles3D>,
    water: Option<GpuWater>,
    camera_stream_targets: AHashMap<NodeID, GpuCameraStreamTarget>,
    camera_stream_content_revisions: AHashMap<NodeID, CameraStreamContentRevision>,
    // Stream revisions stay globally unique so any shared consumer cache
    // cannot mistake equal per-node generations for equal content.
    next_camera_stream_content_revision: u64,
    next_camera_stream_post_view_key: u64,
    camera_stream_external_bindings: AHashMap<NodeID, [u32; 2]>,
    // Per-node resolution of the 3D material-texture slot last bound to a
    // camera-stream target, so the external upsert (view + bind group + retain
    // scan) only runs when the target is (re)created, not every frame.
    camera_stream_3d_bindings: AHashMap<NodeID, [u32; 2]>,
    // Sparse SoA columns keyed by the subview node. Each renderer/world only
    // exists after that view first submits content for the matching path.
    camera_stream_2d: AHashMap<NodeID, Box<Gpu2D>>,
    camera_stream_3d: AHashMap<NodeID, Box<Gpu3D>>,
    camera_stream_particles_3d: AHashMap<NodeID, Box<GpuPointParticles3D>>,
    camera_stream_water: AHashMap<NodeID, Box<GpuWater>>,
    camera_stream_post: AHashMap<NodeID, Box<PostProcessor>>,
    camera_stream_tonemap: CameraStreamTonemap,
    camera_stream_draws_scratch: Vec<Draw3DInstance>,
    camera_image_save_requests: Vec<CameraImageSaveRequest>,
    camera_image_save_pending: Vec<PendingCameraImageSave>,
    last_prepare_particles_revision: u64,
    last_prepare_water_2d_revision: u64,
    last_prepare_water_3d_revision: u64,
    last_prepare_3d_camera: Option<Camera3DState>,
    last_prepare_3d_lighting: Option<Lighting3DState>,
    last_prepare_3d_draws_revision: u64,
    last_prepare_3d_decals_revision: u64,
    last_prepare_3d_width: u32,
    last_prepare_3d_height: u32,
    // Retained-scene fast path: the scene color target (and depth) persist
    // across frames, so a frame that provably reproduces the same image can
    // skip encoding the scene chain and present the retained texture.
    // `retained_scene_valid` is cleared the moment a frame decides to render
    // the scene (so an aborted frame cannot leave a stale "valid") and set
    // again only after that frame's command buffer is submitted.
    retained_scene_valid: bool,
    retained_scene_key: RetainedSceneKey,
    meshlets_enabled: bool,
    dev_meshlets: bool,
    meshlet_debug_view: bool,
    occlusion_culling: OcclusionCullingMode,
    ssao: crate::SsaoQuality,
    texture_filter: TextureFilterMode,
    shader_variant_mode: crate::ShaderVariantMode,
    indirect_first_instance_enabled: bool,
    multi_draw_indirect_enabled: bool,
    /// `Features::MULTI_DRAW_INDIRECT_COUNT` was available and requested.
    multi_draw_indirect_count_enabled: bool,
    gpu_timer: Option<GpuTimestampTimer>,
    // Project virtual canvas for 2D aspect-fit scaling; camera-stream / sub
    // view 2D passes use the same world-to-pixel rule as the main view.
    virtual_size_2d: [f32; 2],
}

#[derive(Clone, Copy)]
pub struct GpuConfig {
    /// Initial sample count. Sessions start on the 2D setting; the first
    /// frame that needs the 3D pipeline latches `smoothing_samples_3d`.
    pub smoothing_samples: u32,
    pub smoothing_samples_3d: u32,
    /// FXAA present pass requested (anti_alias = "fxaa"). Only runs while the
    /// live sample count is 1; resources allocate lazily on first use.
    pub fxaa: bool,
    /// SMAA 1x present passes requested (anti_alias = "smaa"). Same gating
    /// and lazy-allocation rules as `fxaa`; the two are mutually exclusive.
    pub smaa: bool,
    /// TAA v1 present passes requested (anti_alias = "taa"). Same gating and
    /// lazy-allocation rules; exclusive with `fxaa`/`smaa`.
    pub taa: bool,
    pub vsync_enabled: bool,
    pub meshlets_enabled: bool,
    pub dev_meshlets: bool,
    pub meshlet_debug_view: bool,
    pub occlusion_culling: OcclusionCullingMode,
    pub ssao: crate::SsaoQuality,
    pub texture_filter: TextureFilterMode,
    pub hdr_mode: HdrMode,
    pub shader_variant_mode: crate::ShaderVariantMode,
    pub shadow_quality: crate::ShadowQuality,
}

// Views built once w/ the textures: `ensure_camera_stream_target` rebuilds the
// whole struct on any resolution/shape change, so a stored view can never go
// stale. Cloning a view is a refcount bump; create_view is not.
struct GpuCameraStreamTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    post_input_view: Option<wgpu::TextureView>,
    tonemap_input_view: Option<wgpu::TextureView>,
    depth_view: Option<wgpu::TextureView>,
    resolution: [u32; 2],
    post_view_key: u64,
}

struct CameraStreamContentRevision {
    draws: Arc<[CameraStreamDraw3DState]>,
    draw_revision: u64,
    sprites: Arc<[Sprite2DCommand]>,
    sprite_revision: u64,
}

fn update_camera_stream_content_revisions(
    revisions: &mut AHashMap<NodeID, CameraStreamContentRevision>,
    next_revision: &mut u64,
    node: NodeID,
    draws: &Arc<[CameraStreamDraw3DState]>,
    sprites: &Arc<[Sprite2DCommand]>,
) -> (u64, u64) {
    match revisions.entry(node) {
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            let state = entry.get_mut();
            // ptr_eq fast path: no upsert since last render leaves the same
            // Arc in the retained state, so skip the elementwise compare. On
            // equal content in a fresh allocation (no-op upsert), converge to
            // the new Arc so later frames hit the pointer check again.
            if !Arc::ptr_eq(&state.draws, draws) {
                if state.draws.as_ref() != draws.as_ref() {
                    *next_revision = next_nonzero_generation(*next_revision);
                    state.draw_revision = *next_revision;
                }
                state.draws = draws.clone();
            }
            if !Arc::ptr_eq(&state.sprites, sprites) {
                if state.sprites.as_ref() != sprites.as_ref() {
                    *next_revision = next_nonzero_generation(*next_revision);
                    state.sprite_revision = *next_revision;
                }
                state.sprites = sprites.clone();
            }
            (state.draw_revision, state.sprite_revision)
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            *next_revision = next_nonzero_generation(*next_revision);
            let draw_revision = *next_revision;
            *next_revision = next_nonzero_generation(*next_revision);
            let sprite_revision = *next_revision;
            entry.insert(CameraStreamContentRevision {
                draws: draws.clone(),
                draw_revision,
                sprites: sprites.clone(),
                sprite_revision,
            });
            (draw_revision, sprite_revision)
        }
    }
}

fn camera_stream_cache_entry<T>(
    cache: &mut AHashMap<NodeID, Box<T>>,
    node: NodeID,
    create: impl FnOnce() -> T,
) -> &mut T {
    cache
        .entry(node)
        .or_insert_with(|| Box::new(create()))
        .as_mut()
}

fn camera_stream_needs_3d_world(stream: &CameraStreamState) -> bool {
    !stream.draws_3d.is_empty()
        || !stream.point_particles_3d.is_empty()
        || !stream.waters_3d.is_empty()
        || stream.lighting_3d.ambient_light.is_some()
        || stream.lighting_3d.sky.is_some()
        || stream.lighting_3d.ray_lights.iter().any(Option::is_some)
        || stream.lighting_3d.point_lights.iter().any(Option::is_some)
        || stream.lighting_3d.spot_lights.iter().any(Option::is_some)
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
    pub camera_streams: &'a [(NodeID, Arc<CameraStreamState>)],
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
    pub shadow_casters_2d_revision: u64,
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
    pub late_overlay_shadow_casters_2d_revision: u64,
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
    /// streams whose 3D draws use time-reading custom shaders; they must
    /// re-render every frame, everything else can idle-skip when unchanged.
    pub animated_stream_nodes: &'a ahash::AHashSet<NodeID>,
    /// streams whose retained state changed since the last rendered frame
    /// (tracked at Upsert apply). replaces the per-frame deep state compare
    /// in the idle-skip gate; anything absent here is unchanged by
    /// construction.
    pub changed_stream_nodes: &'a ahash::AHashSet<NodeID>,
    /// scene content that animates without any command traffic: an unpaused
    /// sky, or a retained draw whose custom shader reads the frame globals.
    /// The backend already computes this to decide whether an idle frame may
    /// be dropped entirely; the retained-scene fast path needs the same answer
    /// (a frame with no commands still has to re-render an animating scene).
    pub scene_continuous_updates: bool,
}

/// Identity of the retained scene texture: everything outside per-frame
/// content that decides whether the pixels still describe the current frame.
/// Any mismatch forces a full scene encode.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RetainedSceneKey {
    pub render_width: u32,
    pub render_height: u32,
    pub sample_count: u32,
    /// bumped by `Gpu::resize` (recreates the scene texture) and by
    /// `set_smoothing_samples` (MSAA target + resolve shape change).
    pub post_view_generation: u64,
    /// sky/2D clear color: the only scene input that is read straight out of
    /// the frame state and never routed through a prepare.
    pub clear_color: [f64; 4],
    /// The depth prepass is requested per frame (3D water, depth-reading post,
    /// depth-tested UI primitives) and is NOT covered by the 3D dirty bits: a
    /// UI Label3D appearing over a static scene would otherwise depth-test
    /// against a prepass that never ran.
    pub depth_prepass_needed: bool,
    /// Screen-space mesh-blend seam pass state.
    pub blend_screen_active: bool,
    /// Post / accessibility stages; a chain edit changes what the present pass
    /// reads, so the scene has to be re-established once.
    pub post_stage_count: u32,
}

/// Inputs of the retained-scene fast path. Split out as a plain struct so the
/// gate is a pure function that can be unit-tested without a GPU: every field
/// is "true = a reason this frame might differ from the retained one", except
/// the two retained-validity fields.
///
/// Conservative by construction: an uncertain signal must be reported as
/// changed. A missed fast-path frame costs one redundant encode; a wrong skip
/// freezes the image.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SceneFastPathSignals {
    /// the last submitted frame encoded the full scene chain and nothing has
    /// invalidated the retained texture since.
    pub retained_scene_valid: bool,
    /// retained key == this frame's key (size / samples / post generation /
    /// clear color).
    pub retained_key_matches: bool,
    /// the 3D prepare ran this frame (uniforms, culling inputs, buffers moved).
    pub did_prepare_3d: bool,
    /// tracked 3D content delta vs the last prepare (camera, lighting, draws,
    /// decals, render size).
    pub three_d_content_changed: bool,
    /// DIRTY_3D / DIRTY_CAMERA_3D / DIRTY_LIGHTS_3D / DIRTY_RESOURCES.
    pub three_d_dirty: bool,
    pub did_prepare_2d: bool,
    /// The 2D scene pass inputs moved: DIRTY_2D / DIRTY_CAMERA_2D / rect
    /// upload / resources / redraw, AND there is 2D content to draw. The
    /// "and content" half matters: every UI command raises DIRTY_2D, and a UI
    /// change alone cannot alter the 2D scene pass (UI composites onto the
    /// swapchain, after the scene texture).
    pub two_d_scene_changed: bool,
    /// TAA is converging over jittered frames; a "static" frame still changes
    /// the image, so the fast path never runs with TAA on.
    pub taa_active: bool,
    /// any 2D or 3D water this frame (the sim advances every frame).
    pub needs_water: bool,
    /// any 3D point-particle emitter path this frame.
    pub needs_particles: bool,
    /// unpaused sky or frame-global-reading custom material.
    pub scene_continuous_updates: bool,
    /// at least one camera stream re-rendered its target this frame; the main
    /// scene may sample that texture.
    pub streams_rendered: bool,
    /// a decal texture is still decoding and must be retried next frame.
    pub decals_texture_pending: bool,
    /// post / accessibility stages that ping-pong scene <-> intermediate.
    /// Two or more stages write the retained scene texture, which destroys it.
    pub post_stage_count: u32,
    /// a camera image save copies out of a stream target this frame.
    pub camera_image_saves_pending: bool,
}

/// Pure gate for the retained-scene fast path (see `SceneFastPathSignals`).
/// True => the scene-building passes (depth prepass, cull, mesh, seam, 3D
/// particles, water, 2D) may be left out of this frame's encoder; the retained
/// scene texture already holds a pixel-identical image. Present/tonemap, post,
/// UI and the late overlay always still run: the swapchain image is new every
/// frame.
pub(crate) fn scene_fast_path_allowed(signals: &SceneFastPathSignals) -> bool {
    signals.retained_scene_valid
        && signals.retained_key_matches
        && !signals.did_prepare_3d
        && !signals.three_d_content_changed
        && !signals.three_d_dirty
        && !signals.did_prepare_2d
        && !signals.two_d_scene_changed
        && !signals.taa_active
        && !signals.needs_water
        && !signals.needs_particles
        && !signals.scene_continuous_updates
        && !signals.streams_rendered
        && !signals.decals_texture_pending
        && !signals.camera_image_saves_pending
        && signals.post_stage_count <= 1
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
    /// 1 when the retained-scene fast path skipped the whole 3D scene chain
    /// (depth prepass, culling, mesh pass, seam pass, 3D particles, water).
    pub skip_render_3d: u32,
    /// 1 when the same fast path skipped the 2D scene pass.
    pub skip_render_2d: u32,
    /// Scene-building passes handed to the encoder this frame. 0 on a fast
    /// path frame; present/post/UI/late-overlay passes are not counted.
    pub scene_passes_encoded: u32,
    pub total: Duration,
    pub presented: bool,
}

mod frame;
mod lifecycle;
mod textures;

#[cfg(test)]
#[path = "../tests/unit/retained_scene_tests.rs"]
mod retained_scene_tests;

#[cfg(test)]
mod camera_stream_revision_tests {
    use super::*;

    #[test]
    fn draw_revision_changes_when_async_mesh_joins_stream() {
        let node = NodeID::from_parts(7, 0);
        let mut revisions = AHashMap::new();
        let mut next_revision = 0;
        let empty: Arc<[CameraStreamDraw3DState]> = Arc::from([]);
        let empty_sprites: Arc<[Sprite2DCommand]> = Arc::from([]);
        let first = update_camera_stream_content_revisions(
            &mut revisions,
            &mut next_revision,
            node,
            &empty,
            &empty_sprites,
        );
        let ready: Arc<[CameraStreamDraw3DState]> = Arc::from([CameraStreamDraw3DState::Draw {
            mesh: perro_ids::MeshID::from_parts(11, 0),
            surfaces: Arc::from([]),
            node: NodeID::from_parts(12, 0),
            model: glam::Mat4::IDENTITY.to_cols_array_2d(),
            skeleton: None,
            meshlet_override: None,
            lod: perro_render_bridge::LODOptions3D::default(),
            blend: perro_render_bridge::MeshBlendOptions3D::default(),
        }]);
        let second = update_camera_stream_content_revisions(
            &mut revisions,
            &mut next_revision,
            node,
            &ready,
            &empty_sprites,
        );

        assert_ne!(first.0, second.0);
        assert_eq!(first.1, second.1);
        assert_eq!(
            second,
            update_camera_stream_content_revisions(
                &mut revisions,
                &mut next_revision,
                node,
                &ready,
                &empty_sprites,
            )
        );
    }

    #[test]
    fn sprite_revision_changes_when_async_texture_joins_stream() {
        let node = NodeID::from_parts(17, 0);
        let mut revisions = AHashMap::new();
        let mut next_revision = 0;
        let empty_draws: Arc<[CameraStreamDraw3DState]> = Arc::from([]);
        let empty: Arc<[Sprite2DCommand]> = Arc::from([]);
        let first = update_camera_stream_content_revisions(
            &mut revisions,
            &mut next_revision,
            node,
            &empty_draws,
            &empty,
        );
        let ready: Arc<[Sprite2DCommand]> = Arc::from([Sprite2DCommand {
            texture: perro_ids::TextureID::from_parts(21, 0),
            model: glam::Mat3::IDENTITY.to_cols_array_2d(),
            tint: perro_structs::Color::WHITE,
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            uv_normalized: true,
            size: [1.0, 1.0],
            z_index: 0,
        }]);
        let second = update_camera_stream_content_revisions(
            &mut revisions,
            &mut next_revision,
            node,
            &empty_draws,
            &ready,
        );

        assert_eq!(first.0, second.0);
        assert_ne!(first.1, second.1);
    }

    #[test]
    fn sibling_streams_get_distinct_cache_revisions() {
        let mut revisions = AHashMap::new();
        let mut next_revision = 0;
        let empty_draws: Arc<[CameraStreamDraw3DState]> = Arc::from([]);
        let empty_sprites: Arc<[Sprite2DCommand]> = Arc::from([]);
        let first = update_camera_stream_content_revisions(
            &mut revisions,
            &mut next_revision,
            NodeID::from_parts(31, 0),
            &empty_draws,
            &empty_sprites,
        );
        let second = update_camera_stream_content_revisions(
            &mut revisions,
            &mut next_revision,
            NodeID::from_parts(32, 0),
            &empty_draws,
            &empty_sprites,
        );

        assert_ne!(first.0, second.0);
        assert_ne!(first.1, second.1);
    }

    #[test]
    fn stream_reentry_gets_fresh_cache_revisions() {
        let node = NodeID::from_parts(41, 0);
        let mut revisions = AHashMap::new();
        let mut next_revision = 0;
        let draws: Arc<[CameraStreamDraw3DState]> = Arc::from([]);
        let sprites: Arc<[Sprite2DCommand]> = Arc::from([]);
        let first = update_camera_stream_content_revisions(
            &mut revisions,
            &mut next_revision,
            node,
            &draws,
            &sprites,
        );

        revisions.remove(&node);
        let second = update_camera_stream_content_revisions(
            &mut revisions,
            &mut next_revision,
            node,
            &draws,
            &sprites,
        );

        assert_ne!(first, second);
    }

    #[test]
    fn sibling_3d_streams_keep_separate_retained_caches() {
        #[derive(Clone, Debug, PartialEq)]
        struct CacheProbe {
            resolution: [u32; 2],
            mesh_count: usize,
            transforms: Vec<[[f32; 4]; 4]>,
            pipe: &'static str,
            vertex_hook: bool,
            alpha_mode: u8,
            pixels: [u8; 4],
        }

        let first = NodeID::from_parts(51, 0);
        let second = NodeID::from_parts(52, 0);
        let mut caches = AHashMap::new();
        let bg = CacheProbe {
            resolution: [3840, 2160],
            mesh_count: 34,
            transforms: vec![glam::Mat4::IDENTITY.to_cols_array_2d(); 34],
            pipe: "raw-bg",
            vertex_hook: false,
            alpha_mode: 0,
            pixels: [20, 40, 80, 255],
        };
        let logo = CacheProbe {
            resolution: [2464, 464],
            mesh_count: 1,
            transforms: vec![glam::Mat4::from_scale(glam::Vec3::splat(0.88)).to_cols_array_2d()],
            pipe: "std-logo",
            vertex_hook: true,
            alpha_mode: 0,
            pixels: [240, 80, 120, 255],
        };
        camera_stream_cache_entry(&mut caches, first, || bg.clone());
        camera_stream_cache_entry(&mut caches, second, || logo.clone());

        // Hide/show A, reload its material + shader, then switch its alpha
        // mode. B must retain its exact cache throughout.
        caches.remove(&first);
        assert_eq!(caches.get(&second).map(Box::as_ref), Some(&logo));
        let reentered = camera_stream_cache_entry(&mut caches, first, || bg.clone());
        reentered.pipe = "raw-bg-reloaded";
        reentered.vertex_hook = true;
        reentered.alpha_mode = 2;
        reentered.pixels = [30, 60, 100, 255];

        assert_eq!(caches.get(&second).map(Box::as_ref), Some(&logo));
        let reentered = caches.get(&first).expect("bg cache should reenter");
        assert_eq!(reentered.mesh_count, bg.mesh_count);
        assert_eq!(reentered.transforms, bg.transforms);
        assert_eq!(caches.len(), 2);
    }

    #[test]
    fn thousand_stream_slots_allocate_only_used_spatial_columns() {
        let nodes: Vec<_> = (0..1_000)
            .map(|index| NodeID::from_parts(index + 1, 0))
            .collect();
        let mut two_d = AHashMap::new();
        let mut three_d = AHashMap::new();

        for node in nodes.iter().step_by(100) {
            camera_stream_cache_entry(&mut two_d, *node, || "2d");
        }
        for node in nodes.iter().skip(50).step_by(200) {
            camera_stream_cache_entry(&mut three_d, *node, || "3d");
        }

        assert_eq!(nodes.len(), 1_000);
        assert_eq!(two_d.len(), 10);
        assert_eq!(three_d.len(), 5);
    }
}

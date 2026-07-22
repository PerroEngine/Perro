use crate::{
    gpu::{
        DIRTY_2D, DIRTY_3D, DIRTY_ACCESSIBILITY, DIRTY_CAMERA_2D, DIRTY_CAMERA_3D, DIRTY_LIGHTS_3D,
        DIRTY_PARTICLES_3D, DIRTY_POSTFX, DIRTY_RESOURCES, Gpu, GpuConfig, RenderFrame,
        RenderGpuTiming,
    },
    resources::{DecodedTextureRgba, ResourceStore},
    three_d::particles::renderer::Particles3DRenderer,
    three_d::renderer::Renderer3D,
    three_d::{
        gpu::{load_mesh3d_from_source, validate_mesh_source},
        renderer::Draw3DInstance,
        renderer::Draw3DKind,
    },
    two_d::renderer::{RectInstanceGpu, Renderer2D},
    ui::renderer::UiRenderer,
};
use ahash::AHashMap;
use perro_graphics_assets::{
    SVG_RASTER_SCALE, decode_image_rgba, decode_ptex, load_mesh3d_from_bytes, load_texture_rgba,
};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    CameraStreamCommand, CameraStreamSourceState, CameraStreamState, Command2D, Command3D,
    Decal3DState, DisplayCommand, HdrMode, Light2DState, Material3D, PointParticles3DState,
    PostProcessingCommand, RenderBridge, RenderCommand, RenderEvent, ResourceCommand,
    ShadowCaster2DState, Sprite2DCommand, VisualAccessibilityCommand, Water2DState, Water3DState,
};
use perro_structs::TextureFilterMode;
use perro_structs::{PostProcessSet, VisualAccessibilitySettings};
use rayon::prelude::*;
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use std::sync::Mutex;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
use winit::window::Window;

pub type StaticTextureLookup = fn(path_hash: u64) -> &'static [u8];
pub type StaticFontLookup = fn(path_hash: u64) -> &'static [u8];
pub type StaticMeshLookup = fn(path_hash: u64) -> &'static [u8];
pub type StaticShaderLookup = fn(path_hash: u64) -> &'static str;
const GC_INTERVAL_FRAMES: u32 = 60;
const GC_MAX_DROPS_PER_KIND: usize = 64;
const PARALLEL_COMMAND_SUMMARY_MIN: usize = 10_000;
const PARALLEL_RENDER_PREPARE_MIN: usize = 4_096;
const MAX_RUNTIME_TEXTURE_DIMENSION: u32 = 8_192;
const MAX_RUNTIME_TEXTURE_RGBA_BYTES: usize = 64 * 1024 * 1024;

fn checked_runtime_texture_rgba_len(width: u32, height: u32) -> Option<usize> {
    if width == 0
        || height == 0
        || width > MAX_RUNTIME_TEXTURE_DIMENSION
        || height > MAX_RUNTIME_TEXTURE_DIMENSION
    {
        return None;
    }
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .filter(|len| *len <= MAX_RUNTIME_TEXTURE_RGBA_BYTES)
}

#[cfg(not(target_arch = "wasm32"))]
fn asset_ready_log_enabled() -> bool {
    std::env::var("PERRO_ASSET_READY_LOG")
        .ok()
        .is_some_and(|raw| matches!(raw.as_str(), "1" | "true" | "yes" | "on"))
}

#[cfg(target_arch = "wasm32")]
fn asset_ready_log_enabled() -> bool {
    false
}

#[inline]
fn normalize_aa_sample_count(samples: u32) -> u32 {
    match samples {
        0 | 1 => 1,
        2 => 2,
        4 => 4,
        _ => 8,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OcclusionCullingMode {
    Cpu,
    #[default]
    Gpu,
    Off,
}

pub trait GraphicsBackend: RenderBridge {
    fn attach_window(&mut self, window: Arc<Window>);
    fn resize(&mut self, width: u32, height: u32);
    fn set_smoothing(&mut self, enabled: bool);
    fn set_smoothing_samples(&mut self, samples: u32);

    fn draw_frame(&mut self);
    fn draw_frame_timed(&mut self) -> Option<DrawFrameTiming> {
        self.draw_frame();
        None
    }
    fn draw_frame_with_late_overlay<I>(&mut self, overlay_commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.submit_many(overlay_commands);
        self.draw_frame();
    }
    fn draw_frame_with_late_overlay_timed<I>(
        &mut self,
        overlay_commands: I,
    ) -> Option<DrawFrameTiming>
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.submit_many(overlay_commands);
        self.draw_frame_timed()
    }
    fn submit_late_overlay_many<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.submit_many(commands);
    }

    fn profile_snapshot(&self) -> GraphicsProfileSnapshot {
        GraphicsProfileSnapshot::default()
    }

    fn wait_idle(&mut self) {}
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GraphicsProfileSnapshot {
    pub active_meshes: u32,
    pub active_materials: u32,
    pub active_textures: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DrawFrameTiming {
    pub process_commands: Duration,
    pub prepare_cpu: Duration,
    pub gpu_prepare_2d: Duration,
    pub gpu_prepare_3d: Duration,
    pub gpu_prepare_particles_3d: Duration,
    pub gpu_prepare_3d_frustum: Duration,
    pub gpu_prepare_3d_hiz: Duration,
    pub gpu_prepare_3d_indirect: Duration,
    pub gpu_prepare_3d_cull_inputs: Duration,
    pub gpu_acquire: Duration,
    pub gpu_acquire_surface: Duration,
    pub gpu_acquire_view: Duration,
    pub gpu_encode_main: Duration,
    pub gpu_submit_main: Duration,
    pub gpu_submit_finish_main: Duration,
    pub gpu_submit_queue_main: Duration,
    pub gpu_post_process: Duration,
    pub gpu_accessibility: Duration,
    pub gpu_present: Duration,
    pub gpu_timestamp_main: Duration,
    pub gpu_timestamp_water: Duration,
    pub draw_calls_2d: u32,
    pub draw_calls_3d: u32,
    pub sprite_batches_2d: u32,
    pub sprite_bind_group_switches_2d: u32,
    pub draw_batches_3d: u32,
    pub pipeline_switches_3d: u32,
    pub texture_bind_group_switches_3d: u32,
    pub draw_instances_3d: u32,
    pub draw_material_refs_3d: u32,
    pub skip_prepare_2d: u32,
    pub skip_prepare_3d: u32,
    pub skip_prepare_particles_3d: u32,
    pub skip_prepare_3d_frustum: u32,
    pub skip_prepare_3d_hiz: u32,
    pub skip_prepare_3d_indirect: u32,
    pub skip_prepare_3d_cull_inputs: u32,
    pub gpu_total: Duration,
    pub total: Duration,
    pub idle_clear: bool,
}

#[derive(Default)]
struct FrameState {
    pending_commands: Vec<RenderCommand>,
    scratch_commands: Vec<RenderCommand>,
    scratch_camera_commands: Vec<RenderCommand>,
    scratch_late_overlay_commands: Vec<RenderCommand>,
}

#[derive(Default)]
struct CommandSummary {
    dirty_bits: u32,
    rects_2d: usize,
    sprites_2d: usize,
    draws_3d: usize,
}

#[cfg(all(not(target_arch = "wasm32"), not(test)))]
struct AsyncMeshLoadResult {
    request: perro_render_bridge::RenderRequestID,
    id: MeshID,
    source: String,
    mesh: Option<perro_render_bridge::Mesh3D>,
    error: Option<String>,
}

#[cfg(all(not(target_arch = "wasm32"), not(test)))]
struct AsyncMeshLoadJob {
    request: perro_render_bridge::RenderRequestID,
    id: MeshID,
    source: String,
}

#[cfg(not(target_arch = "wasm32"))]
struct AsyncTextureLoadResult {
    id: TextureID,
    texture: Result<DecodedTextureRgba, String>,
}

#[cfg(not(target_arch = "wasm32"))]
struct AsyncTextureLoadJob {
    id: TextureID,
    source: String,
}

impl FrameState {
    fn queue(&mut self, command: RenderCommand) {
        self.pending_commands.push(command);
    }
}

#[inline]
fn draw_instance_count(draw: &Draw3DInstance) -> u32 {
    if let Some(dense) = &draw.dense_multimesh {
        return dense.instances.len().min(u32::MAX as usize) as u32;
    }
    let count = draw.instance_mats.len();
    if count == 0 {
        1
    } else {
        count.min(u32::MAX as usize) as u32
    }
}

#[inline]
fn command_dirty_bits(command: &RenderCommand) -> u32 {
    match command {
        RenderCommand::TwoD(cmd_2d) => {
            let mut bits = DIRTY_2D;
            if matches!(cmd_2d, Command2D::SetCamera { .. }) {
                bits |= DIRTY_CAMERA_2D;
            }
            bits
        }
        RenderCommand::ThreeD(cmd_3d) => match &**cmd_3d {
            Command3D::UpsertCameraStream { .. }
            | Command3D::Draw { .. }
            | Command3D::DrawMulti { .. }
            | Command3D::DrawMultiDense { .. }
            | Command3D::DrawDebugPoint3D { .. }
            | Command3D::DrawDebugLine3D { .. }
            | Command3D::RemoveNode { .. }
            | Command3D::UpsertWater { .. } => DIRTY_3D,
            Command3D::SetCamera { .. } => DIRTY_CAMERA_3D,
            Command3D::SetAmbientLight { .. }
            | Command3D::SetSky { .. }
            | Command3D::SetRayLight { .. }
            | Command3D::SetPointLight { .. }
            | Command3D::SetSpotLight { .. }
            | Command3D::SetDecal { .. } => DIRTY_LIGHTS_3D,
            Command3D::UpsertPointParticles { .. } => DIRTY_PARTICLES_3D,
        },
        RenderCommand::Resource(_) | RenderCommand::CameraStream(_) => DIRTY_RESOURCES,
        RenderCommand::Ui(_) => DIRTY_2D,
        RenderCommand::PostProcessing(_) => DIRTY_POSTFX,
        RenderCommand::VisualAccessibility(_) => DIRTY_ACCESSIBILITY,
        RenderCommand::Display(_) => 0,
    }
}

fn summarize_command_chunk(commands: &[RenderCommand]) -> CommandSummary {
    let mut summary = CommandSummary::default();
    for command in commands {
        summary.dirty_bits |= command_dirty_bits(command);
        match command {
            RenderCommand::TwoD(Command2D::UpsertRect { .. }) => summary.rects_2d += 1,
            RenderCommand::TwoD(
                Command2D::UpsertSprite { .. } | Command2D::UpsertCameraStream { .. },
            ) => summary.sprites_2d += 1,
            RenderCommand::ThreeD(cmd) => match &**cmd {
                Command3D::Draw { .. }
                | Command3D::DrawMulti { .. }
                | Command3D::DrawMultiDense { .. } => summary.draws_3d += 1,
                _ => {}
            },
            _ => {}
        }
    }
    summary
}

fn merge_command_summary(mut a: CommandSummary, b: CommandSummary) -> CommandSummary {
    a.dirty_bits |= b.dirty_bits;
    a.rects_2d += b.rects_2d;
    a.sprites_2d += b.sprites_2d;
    a.draws_3d += b.draws_3d;
    a
}

fn summarize_commands(commands: &[RenderCommand]) -> CommandSummary {
    if commands.len() < PARALLEL_COMMAND_SUMMARY_MIN {
        return summarize_command_chunk(commands);
    }

    commands
        .par_chunks(1024)
        .map(summarize_command_chunk)
        .reduce(CommandSummary::default, merge_command_summary)
}

#[inline]
fn camera_stream_texture_id(node: NodeID) -> TextureID {
    TextureID::from_parts(node.index(), node.generation())
}

#[inline]
fn camera_stream_uses_render_target(stream: &CameraStreamState) -> bool {
    match &stream.source {
        CameraStreamSourceState::Webcam { texture, .. } => stream.output_texture != *texture,
        _ => true,
    }
}

fn upsert_camera_stream_state(
    streams: &mut Vec<(NodeID, CameraStreamState)>,
    node: NodeID,
    state: CameraStreamState,
) {
    if let Some((_, existing)) = streams.iter_mut().find(|(id, _)| *id == node) {
        *existing = state;
    } else {
        streams.push((node, state));
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SsaoQuality {
    Off,
    Low,
    #[default]
    Medium,
    High,
    Ultra,
}

pub struct PerroGraphics {
    frame: FrameState,
    resources: ResourceStore,
    renderer_2d: Renderer2D,
    late_overlay_2d: Renderer2D,
    renderer_3d: Renderer3D,
    particles_3d: Particles3DRenderer,
    renderer_ui: UiRenderer,
    gpu: Option<Gpu>,
    events: Vec<RenderEvent>,
    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    async_mesh_load_tx: mpsc::Sender<AsyncMeshLoadResult>,
    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    async_mesh_load_rx: mpsc::Receiver<AsyncMeshLoadResult>,
    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    pending_async_mesh_loads: AHashMap<MeshID, Vec<perro_render_bridge::RenderRequestID>>,
    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    queued_async_mesh_loads: Vec<AsyncMeshLoadJob>,
    #[cfg(not(target_arch = "wasm32"))]
    async_texture_load_tx: mpsc::Sender<AsyncTextureLoadResult>,
    #[cfg(not(target_arch = "wasm32"))]
    async_texture_load_rx: mpsc::Receiver<AsyncTextureLoadResult>,
    #[cfg(not(target_arch = "wasm32"))]
    pending_async_texture_loads: AHashMap<TextureID, Vec<perro_render_bridge::RenderRequestID>>,
    #[cfg(not(target_arch = "wasm32"))]
    queued_async_texture_loads: Vec<AsyncTextureLoadJob>,
    viewport: (u32, u32),
    vsync_enabled: bool,
    smoothing_enabled: bool,
    smoothing_samples: u32,
    smoothing_quality_samples: u32,
    static_texture_lookup: Option<StaticTextureLookup>,
    static_font_lookup: Option<StaticFontLookup>,
    static_mesh_lookup: Option<StaticMeshLookup>,
    static_shader_lookup: Option<StaticShaderLookup>,
    meshlets_enabled: bool,
    dev_meshlets: bool,
    meshlet_debug_view: bool,
    occlusion_culling: OcclusionCullingMode,
    ssao: SsaoQuality,
    texture_filter: TextureFilterMode,
    hdr_mode: HdrMode,
    retained_draws_cache_revision: u64,
    retained_draw_instances_cache: u32,
    retained_point_particles_cache: Vec<(NodeID, PointParticles3DState)>,
    retained_point_particles_cache_revision: u64,
    retained_waters_2d_cache: Vec<(NodeID, Water2DState)>,
    retained_waters_2d_cache_revision: u64,
    retained_waters_3d_cache: Vec<(NodeID, Water3DState)>,
    retained_waters_3d_cache_revision: u64,
    retained_decals_3d_cache: Vec<(NodeID, Decal3DState)>,
    retained_decals_3d_cache_revision: u64,
    retained_sprites_cache: Vec<Sprite2DCommand>,
    retained_sprites_cache_revision: u64,
    retained_point_lights_cache: Vec<Light2DState>,
    retained_point_lights_cache_revision: u64,
    retained_shadow_casters_cache: Vec<ShadowCaster2DState>,
    retained_shadow_casters_cache_revision: u64,
    camera_stream_targets: AHashMap<NodeID, [u32; 2]>,
    // resolution of each resident stream (webcam/video) texture. a repeat write
    // at the same resolution updates texels in place (no rescan, no GPU rebuild);
    // a first write or resolution change falls back to the full reload path.
    stream_texture_dims: AHashMap<TextureID, [u32; 2]>,
    retained_camera_streams: Vec<(NodeID, CameraStreamState)>,
    frame_rects_cache: Vec<RectInstanceGpu>,
    late_overlay_sprites_cache: Vec<Sprite2DCommand>,
    late_overlay_sprites_cache_revision: u64,
    late_overlay_point_lights_cache: Vec<Light2DState>,
    late_overlay_point_lights_cache_revision: u64,
    late_overlay_shadow_casters_cache: Vec<ShadowCaster2DState>,
    late_overlay_rects_cache: Vec<RectInstanceGpu>,
    used_texture_refs_cache: AHashMap<TextureID, u32>,
    used_mesh_refs_cache: AHashMap<MeshID, u32>,
    used_material_refs_cache: AHashMap<MaterialID, u32>,
    scene_texture_refs_cache: AHashMap<TextureID, Vec<NodeID>>,
    scene_mesh_refs_cache: AHashMap<MeshID, Vec<NodeID>>,
    scene_material_refs_cache: AHashMap<MaterialID, Vec<NodeID>>,
    used_ref_draws_revision: u64,
    used_ref_sprites_revision: u64,
    global_post_processing: PostProcessSet,
    // Cached built effects Arc handed to the renderer each frame, rebuilt only
    // when `global_post_processing` is mutated instead of cloned+allocated every
    // frame.
    global_post_processing_cache: Arc<[perro_structs::PostProcessEffect]>,
    global_post_processing_cache_dirty: bool,
    accessibility: VisualAccessibilitySettings,
    frame_index: u32,
    redraw_requested: bool,
    frame_time_seconds: f32,
    frame_delta_seconds: f32,
    last_frame_instant: Option<Instant>,
    #[cfg(target_arch = "wasm32")]
    pending_gpu: Option<Arc<Mutex<Option<Gpu>>>>,
}

impl Default for PerroGraphics {
    fn default() -> Self {
        Self::new()
    }
}

mod asset_load;
mod command_process;
mod config;

mod assets;
mod graphics_backend;
mod queries;
mod render_bridge;

#[cfg(test)]
#[path = "../tests/unit/backend_tests.rs"]
mod tests;

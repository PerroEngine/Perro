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
#[cfg(not(target_arch = "wasm32"))]
use ahash::AHashSet;
use perro_graphics_assets::{
    decode_image_rgba, decode_ptex, load_mesh3d_from_bytes, load_texture_rgba,
};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    CameraStreamCommand, CameraStreamSourceState, CameraStreamState, Command2D, Command3D,
    Decal3DState, Light2DState, Material3D, PointParticles3DState, PostProcessingCommand,
    RenderBridge, RenderCommand, RenderEvent, ResourceCommand, ShadowCaster2DState,
    Sprite2DCommand, VisualAccessibilityCommand, Water2DState, Water3DState,
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
    texture: Option<DecodedTextureRgba>,
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
    pending_async_texture_loads: AHashSet<TextureID>,
    #[cfg(not(target_arch = "wasm32"))]
    queued_async_texture_loads: Vec<AsyncTextureLoadJob>,
    viewport: (u32, u32),
    vsync_enabled: bool,
    smoothing_enabled: bool,
    smoothing_samples: u32,
    smoothing_quality_samples: u32,
    static_texture_lookup: Option<StaticTextureLookup>,
    static_mesh_lookup: Option<StaticMeshLookup>,
    static_shader_lookup: Option<StaticShaderLookup>,
    meshlets_enabled: bool,
    dev_meshlets: bool,
    meshlet_debug_view: bool,
    occlusion_culling: OcclusionCullingMode,
    texture_filter: TextureFilterMode,
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

impl PerroGraphics {
    fn decode_texture_source(
        source: &str,
        static_texture_lookup: Option<StaticTextureLookup>,
    ) -> Option<DecodedTextureRgba> {
        let (rgba, width, height) = if source == "__default__" {
            (vec![255u8, 255, 255, 255], 1, 1)
        } else if source == "__perro_builtin_logo_svg__" {
            decode_image_rgba(include_bytes!(
                "../../../api_modules/perro_api/src/assets/perro.svg"
            ))?
        } else if let Some(lookup) = static_texture_lookup {
            let source_hash = perro_ids::parse_hashed_source_uri(source)
                .unwrap_or_else(|| perro_ids::string_to_u64(source));
            let bytes = lookup(source_hash);
            if !bytes.is_empty() {
                decode_ptex(bytes)?
            } else {
                Self::decode_texture_file(source)?
            }
        } else {
            Self::decode_texture_file(source)?
        };
        Some(DecodedTextureRgba {
            rgba,
            width: width.max(1),
            height: height.max(1),
        })
    }

    fn decode_texture_file(source: &str) -> Option<(Vec<u8>, u32, u32)> {
        load_texture_rgba(source)
    }

    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    fn start_async_mesh_load(
        &mut self,
        request: perro_render_bridge::RenderRequestID,
        id: MeshID,
        source: String,
    ) {
        self.queued_async_mesh_loads.push(AsyncMeshLoadJob {
            request,
            id,
            source,
        });
    }

    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    fn flush_async_mesh_loads(&mut self) {
        if self.queued_async_mesh_loads.is_empty() {
            return;
        }
        let jobs = std::mem::take(&mut self.queued_async_mesh_loads);
        let tx = self.async_mesh_load_tx.clone();
        let static_mesh_lookup = self.static_mesh_lookup;
        rayon::spawn(move || {
            for job in jobs {
                let error = validate_mesh_source(job.source.as_str(), static_mesh_lookup).err();
                let mesh = if error.is_none() {
                    load_mesh3d_from_source(job.source.as_str(), static_mesh_lookup)
                } else {
                    None
                };
                let _ = tx.send(AsyncMeshLoadResult {
                    request: job.request,
                    id: job.id,
                    source: job.source,
                    mesh,
                    error,
                });
            }
        });
    }

    #[cfg(any(target_arch = "wasm32", test))]
    fn start_async_mesh_load(
        &mut self,
        request: perro_render_bridge::RenderRequestID,
        id: MeshID,
        source: String,
    ) {
        if let Err(reason) = validate_mesh_source(source.as_str(), self.static_mesh_lookup) {
            self.resources.drop_mesh(id);
            self.events.push(RenderEvent::Failed { request, reason });
            return;
        }
        let mesh_data = load_mesh3d_from_source(source.as_str(), self.static_mesh_lookup);
        if let Some(mesh) = mesh_data.clone() {
            self.resources
                .set_runtime_mesh_data(source.as_str(), mesh.clone());
            let _ = self.resources.set_runtime_mesh_data_by_id(id, mesh);
        }
        self.events.push(RenderEvent::MeshCreated {
            request,
            id,
            mesh: mesh_data,
        });
    }

    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    fn poll_async_mesh_loads(&mut self) {
        while let Ok(result) = self.async_mesh_load_rx.try_recv() {
            let requests = self
                .pending_async_mesh_loads
                .remove(&result.id)
                .unwrap_or_else(|| vec![result.request]);
            if let Some(reason) = result.error {
                self.resources.drop_mesh(result.id);
                for request in requests {
                    self.events.push(RenderEvent::Failed {
                        request,
                        reason: reason.clone(),
                    });
                }
                continue;
            }
            if let Some(mesh) = result.mesh.clone() {
                self.resources
                    .set_runtime_mesh_data(result.source.as_str(), mesh.clone());
                let _ = self.resources.set_runtime_mesh_data_by_id(result.id, mesh);
            }
            for request in requests {
                self.events.push(RenderEvent::MeshCreated {
                    request,
                    id: result.id,
                    mesh: result.mesh.clone(),
                });
            }
            self.redraw_requested = true;
        }
    }

    #[cfg(any(target_arch = "wasm32", test))]
    fn poll_async_mesh_loads(&mut self) {}

    #[cfg(any(target_arch = "wasm32", test))]
    fn flush_async_mesh_loads(&mut self) {}

    #[cfg(not(target_arch = "wasm32"))]
    fn start_async_texture_load(&mut self, id: TextureID, source: String) {
        self.queued_async_texture_loads
            .push(AsyncTextureLoadJob { id, source });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn flush_async_texture_loads(&mut self) {
        if self.queued_async_texture_loads.is_empty() {
            return;
        }
        let jobs = std::mem::take(&mut self.queued_async_texture_loads);
        let tx = self.async_texture_load_tx.clone();
        let static_texture_lookup = self.static_texture_lookup;
        rayon::spawn(move || {
            for job in jobs {
                let texture =
                    Self::decode_texture_source(job.source.as_str(), static_texture_lookup);
                let _ = tx.send(AsyncTextureLoadResult {
                    id: job.id,
                    texture,
                });
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn start_async_texture_load(&mut self, id: TextureID, source: String) {
        if let Some(texture) =
            Self::decode_texture_source(source.as_str(), self.static_texture_lookup)
        {
            let _ = self.resources.set_decoded_texture_data(id, texture);
            self.events.push(RenderEvent::TextureLoaded { id });
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_async_texture_loads(&mut self) {
        while let Ok(result) = self.async_texture_load_rx.try_recv() {
            self.pending_async_texture_loads.remove(&result.id);
            if let Some(texture) = result.texture
                && self.resources.set_decoded_texture_data(result.id, texture)
            {
                self.events
                    .push(RenderEvent::TextureLoaded { id: result.id });
                self.redraw_requested = true;
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_async_texture_loads(&mut self) {}

    #[cfg(target_arch = "wasm32")]
    fn flush_async_texture_loads(&mut self) {}

    pub fn new() -> Self {
        #[cfg(all(not(target_arch = "wasm32"), not(test)))]
        let (async_mesh_load_tx, async_mesh_load_rx) = mpsc::channel();
        #[cfg(not(target_arch = "wasm32"))]
        let (async_texture_load_tx, async_texture_load_rx) = mpsc::channel();
        Self {
            frame: FrameState::default(),
            resources: ResourceStore::new(),
            renderer_2d: Renderer2D::new(),
            late_overlay_2d: Renderer2D::new(),
            renderer_3d: Renderer3D::new(),
            particles_3d: Particles3DRenderer::new(),
            renderer_ui: UiRenderer::new(),
            gpu: None,
            events: Vec::new(),
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            async_mesh_load_tx,
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            async_mesh_load_rx,
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            pending_async_mesh_loads: AHashMap::new(),
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            queued_async_mesh_loads: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            async_texture_load_tx,
            #[cfg(not(target_arch = "wasm32"))]
            async_texture_load_rx,
            #[cfg(not(target_arch = "wasm32"))]
            pending_async_texture_loads: AHashSet::new(),
            #[cfg(not(target_arch = "wasm32"))]
            queued_async_texture_loads: Vec::new(),
            viewport: (0, 0),
            vsync_enabled: true,
            smoothing_enabled: true,
            smoothing_samples: 4,
            smoothing_quality_samples: 4,
            static_texture_lookup: None,
            static_mesh_lookup: None,
            static_shader_lookup: None,
            meshlets_enabled: false,
            dev_meshlets: false,
            meshlet_debug_view: false,
            occlusion_culling: OcclusionCullingMode::Gpu,
            texture_filter: TextureFilterMode::LinearMipmap,
            retained_draws_cache_revision: u64::MAX,
            retained_draw_instances_cache: 0,
            retained_point_particles_cache: Vec::new(),
            retained_point_particles_cache_revision: u64::MAX,
            retained_waters_2d_cache: Vec::new(),
            retained_waters_2d_cache_revision: u64::MAX,
            retained_waters_3d_cache: Vec::new(),
            retained_waters_3d_cache_revision: u64::MAX,
            retained_decals_3d_cache: Vec::new(),
            retained_decals_3d_cache_revision: u64::MAX,
            retained_sprites_cache: Vec::new(),
            retained_sprites_cache_revision: u64::MAX,
            retained_point_lights_cache: Vec::new(),
            retained_point_lights_cache_revision: u64::MAX,
            retained_shadow_casters_cache: Vec::new(),
            retained_shadow_casters_cache_revision: u64::MAX,
            camera_stream_targets: AHashMap::new(),
            retained_camera_streams: Vec::new(),
            frame_rects_cache: Vec::new(),
            late_overlay_sprites_cache: Vec::new(),
            late_overlay_sprites_cache_revision: u64::MAX,
            late_overlay_point_lights_cache: Vec::new(),
            late_overlay_point_lights_cache_revision: u64::MAX,
            late_overlay_shadow_casters_cache: Vec::new(),
            late_overlay_rects_cache: Vec::new(),
            used_texture_refs_cache: AHashMap::new(),
            used_mesh_refs_cache: AHashMap::new(),
            used_material_refs_cache: AHashMap::new(),
            scene_texture_refs_cache: AHashMap::new(),
            scene_mesh_refs_cache: AHashMap::new(),
            scene_material_refs_cache: AHashMap::new(),
            used_ref_draws_revision: u64::MAX,
            used_ref_sprites_revision: u64::MAX,
            global_post_processing: PostProcessSet::new(),
            global_post_processing_cache: Arc::from(Vec::new()),
            global_post_processing_cache_dirty: true,
            accessibility: VisualAccessibilitySettings::default(),
            frame_index: 0,
            redraw_requested: true,
            frame_time_seconds: 0.0,
            frame_delta_seconds: 0.0,
            last_frame_instant: None,
            #[cfg(target_arch = "wasm32")]
            pending_gpu: None,
        }
    }

    pub fn with_vsync(mut self, enabled: bool) -> Self {
        self.vsync_enabled = enabled;
        self
    }

    pub fn with_msaa(mut self, enabled: bool) -> Self {
        self.set_smoothing(enabled);
        self
    }

    pub fn with_msaa_samples(mut self, samples: u32) -> Self {
        self.set_smoothing_samples(samples);
        self
    }

    pub fn with_static_texture_lookup(mut self, lookup: StaticTextureLookup) -> Self {
        self.static_texture_lookup = Some(lookup);
        self
    }

    pub fn with_static_mesh_lookup(mut self, lookup: StaticMeshLookup) -> Self {
        self.static_mesh_lookup = Some(lookup);
        self
    }

    pub fn with_static_shader_lookup(mut self, lookup: StaticShaderLookup) -> Self {
        self.static_shader_lookup = Some(lookup);
        self
    }

    pub fn with_dev_meshlets(mut self, enabled: bool) -> Self {
        self.dev_meshlets = enabled;
        self
    }

    pub fn with_meshlets_enabled(mut self, enabled: bool) -> Self {
        self.meshlets_enabled = enabled;
        self
    }

    pub fn with_meshlet_debug_view(mut self, enabled: bool) -> Self {
        self.meshlet_debug_view = enabled;
        self
    }

    pub fn with_occlusion_culling(mut self, mode: OcclusionCullingMode) -> Self {
        self.occlusion_culling = mode;
        self
    }

    pub fn with_texture_filter(mut self, mode: TextureFilterMode) -> Self {
        self.texture_filter = mode;
        self
    }

    fn process_commands<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.poll_async_mesh_loads();
        self.poll_async_texture_loads();
        for command in commands {
            match command {
                RenderCommand::CameraStream(command) => match command {
                    CameraStreamCommand::Upsert { node, state } => {
                        let state = *state;
                        upsert_camera_stream_state(
                            &mut self.retained_camera_streams,
                            node,
                            state.clone(),
                        );
                        if !matches!(state.source, CameraStreamSourceState::Webcam { .. }) {
                            self.upsert_camera_stream_texture(
                                node,
                                state.output_texture,
                                state.resolution,
                            );
                        }
                    }
                    CameraStreamCommand::RemoveNode { node } => {
                        let id = camera_stream_texture_id(node);
                        self.camera_stream_targets.remove(&node);
                        self.retained_camera_streams.retain(|(id, _)| *id != node);
                        let _ = self.resources.drop_texture(id);
                    }
                },
                RenderCommand::Resource(resource_cmd) => match resource_cmd {
                    ResourceCommand::CreateMesh {
                        request,
                        id,
                        source,
                        reserved,
                    } => {
                        let out_id = if id.is_nil() {
                            self.resources.create_mesh(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_mesh_with_id(id, source.as_str(), reserved)
                        };
                        if asset_ready_log_enabled() {
                            eprintln!(
                                "[perro][asset-ready] backend mesh request id={out_id:?} source={source}"
                            );
                        }
                        if let Some(mesh) = self.resources.runtime_mesh_data_by_id(out_id).cloned()
                        {
                            self.events.push(RenderEvent::MeshCreated {
                                request,
                                id: out_id,
                                mesh: Some(mesh),
                            });
                            continue;
                        }
                        #[cfg(all(not(target_arch = "wasm32"), not(test)))]
                        {
                            let waiters = self.pending_async_mesh_loads.entry(out_id).or_default();
                            let start_load = waiters.is_empty();
                            if !waiters.contains(&request) {
                                waiters.push(request);
                            }
                            if start_load {
                                self.start_async_mesh_load(request, out_id, source);
                            }
                        }
                        #[cfg(any(target_arch = "wasm32", test))]
                        self.start_async_mesh_load(request, out_id, source);
                    }
                    ResourceCommand::CreateRuntimeMesh {
                        request,
                        id,
                        source,
                        reserved,
                        mesh,
                    } => {
                        let out_id = if id.is_nil() {
                            self.resources.create_mesh(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_mesh_with_id(id, source.as_str(), reserved)
                        };
                        self.resources
                            .set_runtime_mesh_data(source.as_str(), mesh.clone());
                        let _ = self
                            .resources
                            .set_runtime_mesh_data_by_id(out_id, mesh.clone());
                        self.events.push(RenderEvent::MeshCreated {
                            request,
                            id: out_id,
                            mesh: Some(mesh),
                        });
                    }
                    ResourceCommand::CreateRuntimeMeshBytes {
                        request,
                        id,
                        source,
                        reserved,
                        bytes,
                    } => {
                        let Some(mesh) = load_mesh3d_from_bytes(bytes.as_ref()) else {
                            self.events.push(RenderEvent::Failed {
                                request,
                                reason: format!("invalid mesh bytes len={}", bytes.len()),
                            });
                            continue;
                        };
                        let out_id = if id.is_nil() {
                            self.resources.create_mesh(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_mesh_with_id(id, source.as_str(), reserved)
                        };
                        self.resources
                            .set_runtime_mesh_data(source.as_str(), mesh.clone());
                        let _ = self
                            .resources
                            .set_runtime_mesh_data_by_id(out_id, mesh.clone());
                        self.events.push(RenderEvent::MeshCreated {
                            request,
                            id: out_id,
                            mesh: Some(mesh),
                        });
                    }
                    ResourceCommand::WriteMeshData { id, mesh } => {
                        let _ = self.resources.set_runtime_mesh_data_by_id(id, mesh);
                    }
                    ResourceCommand::CreateTexture {
                        request,
                        id,
                        source,
                        reserved,
                    } => {
                        let id = if id.is_nil() {
                            self.resources.create_texture(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_texture_with_id(id, source.as_str(), reserved)
                        };
                        if self.resources.decoded_texture_data(id).is_none() {
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                if self.pending_async_texture_loads.insert(id) {
                                    self.start_async_texture_load(id, source);
                                }
                            }
                            #[cfg(target_arch = "wasm32")]
                            self.start_async_texture_load(id, source);
                        }
                        self.events
                            .push(RenderEvent::TextureCreated { request, id });
                    }
                    ResourceCommand::CreateRuntimeTexture {
                        request,
                        id,
                        source,
                        reserved,
                        width,
                        height,
                        rgba,
                    } => {
                        let expected_len = (width as usize)
                            .checked_mul(height as usize)
                            .and_then(|pixels| pixels.checked_mul(4));
                        if width == 0 || height == 0 || expected_len != Some(rgba.len()) {
                            self.events.push(RenderEvent::Failed {
                                request,
                                reason: format!(
                                    "invalid rgba texture {width}x{height} len={}",
                                    rgba.len()
                                ),
                            });
                            continue;
                        }
                        let id = if id.is_nil() {
                            self.resources.create_texture(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_texture_with_id(id, source.as_str(), reserved)
                        };
                        let _ = self.resources.set_decoded_texture_data(
                            id,
                            DecodedTextureRgba {
                                rgba: rgba.to_vec(),
                                width,
                                height,
                            },
                        );
                        self.events
                            .push(RenderEvent::TextureCreated { request, id });
                        self.events.push(RenderEvent::TextureLoaded { id });
                    }
                    ResourceCommand::CreateRuntimeTextureBytes {
                        request,
                        id,
                        source,
                        reserved,
                        bytes,
                    } => {
                        let decoded = decode_ptex(bytes.as_ref())
                            .or_else(|| decode_image_rgba(bytes.as_ref()))
                            .map(|(rgba, width, height)| DecodedTextureRgba {
                                rgba,
                                width,
                                height,
                            });
                        let Some(decoded) = decoded else {
                            self.events.push(RenderEvent::Failed {
                                request,
                                reason: format!("invalid texture bytes len={}", bytes.len()),
                            });
                            continue;
                        };
                        let id = if id.is_nil() {
                            self.resources.create_texture(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_texture_with_id(id, source.as_str(), reserved)
                        };
                        let _ = self.resources.set_decoded_texture_data(id, decoded);
                        self.events
                            .push(RenderEvent::TextureCreated { request, id });
                        self.events.push(RenderEvent::TextureLoaded { id });
                    }
                    ResourceCommand::CreateExternalTexture {
                        request,
                        id,
                        source,
                        reserved,
                        width,
                        height,
                    } => {
                        let Some(len) = checked_runtime_texture_rgba_len(width, height) else {
                            self.events.push(RenderEvent::Failed {
                                request,
                                reason: format!(
                                    "external texture size {width}x{height} exceeds runtime limits"
                                ),
                            });
                            continue;
                        };
                        let id = if id.is_nil() {
                            self.resources.create_texture(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_texture_with_id(id, source.as_str(), reserved)
                        };
                        let mut rgba = vec![0; len];
                        for pixel in rgba.chunks_exact_mut(4) {
                            pixel[3] = 255;
                        }
                        let _ = self.resources.set_decoded_texture_data(
                            id,
                            DecodedTextureRgba {
                                rgba,
                                width,
                                height,
                            },
                        );
                        self.events
                            .push(RenderEvent::TextureCreated { request, id });
                        self.events.push(RenderEvent::TextureLoaded { id });
                    }
                    ResourceCommand::WriteTextureRgba {
                        id,
                        width,
                        height,
                        rgba,
                    } => {
                        if checked_runtime_texture_rgba_len(width, height) != Some(rgba.len()) {
                            continue;
                        }
                        let _ = self.resources.set_decoded_texture_data(
                            id,
                            DecodedTextureRgba {
                                rgba: rgba.to_vec(),
                                width,
                                height,
                            },
                        );
                        let texture_source = self.resources.texture_source(id).map(str::to_owned);
                        if let Some(gpu) = self.gpu.as_mut() {
                            gpu.invalidate_texture(id, texture_source.as_deref());
                        }
                        self.retained_draws_cache_revision = u64::MAX;
                        self.retained_decals_3d_cache_revision = u64::MAX;
                        self.retained_sprites_cache_revision = u64::MAX;
                        self.events.push(RenderEvent::TextureLoaded { id });
                        self.redraw_requested = true;
                    }
                    ResourceCommand::WriteTextureRgbaRegion {
                        id,
                        x,
                        y,
                        width,
                        height,
                        rgba,
                    } => {
                        if self.resources.write_decoded_texture_region(
                            id,
                            x,
                            y,
                            width,
                            height,
                            rgba.as_ref(),
                        ) {
                            let texture_source =
                                self.resources.texture_source(id).map(str::to_owned);
                            if let Some(gpu) = self.gpu.as_mut() {
                                gpu.invalidate_texture(id, texture_source.as_deref());
                            }
                            self.retained_draws_cache_revision = u64::MAX;
                            self.retained_decals_3d_cache_revision = u64::MAX;
                            self.retained_sprites_cache_revision = u64::MAX;
                            self.events.push(RenderEvent::TextureLoaded { id });
                            self.redraw_requested = true;
                        }
                    }
                    ResourceCommand::CreateMaterial {
                        request,
                        id,
                        material,
                        source,
                        reserved,
                    } => {
                        let log_kind = if asset_ready_log_enabled() {
                            Some(match source.as_deref() {
                                Some(path) => format!("kind=source path={path}"),
                                None if material == Material3D::default() => {
                                    "kind=default".to_string()
                                }
                                None => "kind=inline".to_string(),
                            })
                        } else {
                            None
                        };
                        let id = if id.is_nil() {
                            self.resources
                                .create_material(material, source.as_deref(), reserved)
                        } else {
                            self.resources.create_material_with_id(
                                id,
                                material,
                                source.as_deref(),
                                reserved,
                            )
                        };
                        self.events
                            .push(RenderEvent::MaterialCreated { request, id });
                        if let Some(log_kind) = log_kind {
                            eprintln!(
                                "[perro][asset-ready] backend material created id={id:?} {log_kind}"
                            );
                        }
                        self.events.push(RenderEvent::MaterialLoaded { id });
                    }
                    ResourceCommand::SetSceneResourceRefs {
                        textures,
                        meshes,
                        materials,
                    } => {
                        self.scene_texture_refs_cache.clear();
                        self.scene_texture_refs_cache.extend(
                            textures
                                .into_iter()
                                .filter(|(id, nodes)| !id.is_nil() && !nodes.is_empty()),
                        );
                        self.scene_mesh_refs_cache.clear();
                        self.scene_mesh_refs_cache.extend(
                            meshes
                                .into_iter()
                                .filter(|(id, nodes)| !id.is_nil() && !nodes.is_empty()),
                        );
                        self.scene_material_refs_cache.clear();
                        self.scene_material_refs_cache.extend(
                            materials
                                .into_iter()
                                .filter(|(id, nodes)| !id.is_nil() && !nodes.is_empty()),
                        );
                    }
                    ResourceCommand::WriteMaterialData { id, material } => {
                        if self.resources.set_material_data(id, material) {
                            if asset_ready_log_enabled() {
                                eprintln!(
                                    "[perro][asset-ready] backend material data applied id={id:?}"
                                );
                            }
                            self.events.push(RenderEvent::MaterialLoaded { id });
                        }
                    }
                    ResourceCommand::SetMeshReserved { id, reserved } => {
                        self.resources.set_mesh_reserved(id, reserved);
                    }
                    ResourceCommand::SetTextureReserved { id, reserved } => {
                        self.resources.set_texture_reserved(id, reserved);
                    }
                    ResourceCommand::SetMaterialReserved { id, reserved } => {
                        self.resources.set_material_reserved(id, reserved);
                    }
                    ResourceCommand::DropMesh { id } => {
                        if self.resources.drop_mesh(id) {
                            self.events.push(RenderEvent::MeshDropped { id });
                        }
                    }
                    ResourceCommand::DropTexture { id } => {
                        if self.resources.drop_texture(id) {
                            self.events.push(RenderEvent::TextureDropped { id });
                        }
                    }
                    ResourceCommand::DropMaterial { id } => {
                        if self.resources.drop_material(id) {
                            self.events.push(RenderEvent::MaterialDropped { id });
                        }
                    }
                },
                RenderCommand::TwoD(cmd_2d) => match cmd_2d {
                    Command2D::UpsertCameraStream {
                        node,
                        stream,
                        sprite,
                    } => {
                        let stream = *stream;
                        if !matches!(stream.source, CameraStreamSourceState::Webcam { .. }) {
                            self.upsert_camera_stream_texture(
                                node,
                                stream.output_texture,
                                stream.resolution,
                            );
                        }
                        self.renderer_2d.queue_sprite(node, sprite);
                    }
                    Command2D::UpsertSprite { node, sprite } => {
                        self.renderer_2d.queue_sprite(node, sprite);
                    }
                    Command2D::UpsertTileMap { node, tilemap } => {
                        self.renderer_2d.upsert_tilemap(node, tilemap);
                    }
                    Command2D::UpsertRect { node, rect } => {
                        self.renderer_2d.queue_rect(node, rect);
                    }
                    Command2D::UpsertPointParticles { node, particles } => {
                        self.renderer_2d.queue_point_particles(node, *particles);
                    }
                    Command2D::UpsertWater { node, water } => {
                        self.renderer_2d.upsert_water(node, *water);
                    }
                    Command2D::UpsertShadowCaster { node, caster } => {
                        self.renderer_2d.upsert_shadow_caster(node, caster);
                    }
                    Command2D::SetAmbientLight { node, light } => {
                        self.renderer_2d.set_ambient_light(node, light);
                    }
                    Command2D::SetRayLight { node, light } => {
                        self.renderer_2d.set_ray_light(node, light);
                    }
                    Command2D::SetPointLight { node, light } => {
                        self.renderer_2d.set_point_light(node, light);
                    }
                    Command2D::SetSpotLight { node, light } => {
                        self.renderer_2d.set_spot_light(node, light);
                    }
                    Command2D::RemoveNode { node } => {
                        self.renderer_2d.remove_node(node);
                    }
                    Command2D::SetCamera { camera } => {
                        self.renderer_2d.set_camera(camera);
                    }
                    Command2D::DrawShape { draw } => {
                        self.renderer_2d.queue_shape(draw);
                    }
                },
                RenderCommand::ThreeD(cmd_3d) => match *cmd_3d {
                    Command3D::UpsertCameraStream { node, stream, quad } => {
                        let stream = *stream;
                        if !matches!(stream.source, CameraStreamSourceState::Webcam { .. }) {
                            self.upsert_camera_stream_texture(
                                node,
                                stream.output_texture,
                                stream.resolution,
                            );
                        }
                        self.renderer_3d.queue_camera_stream_quad(
                            node,
                            stream.output_texture,
                            quad.model,
                            quad.size,
                            quad.tint.to_float_slice(),
                        );
                    }
                    Command3D::Draw {
                        mesh,
                        surfaces,
                        node,
                        model,
                        skeleton,
                        blend_shape_weights,
                        meshlet_override,
                        lod,
                        blend,
                        cast_shadows,
                        receive_shadows,
                        ..
                    } => {
                        self.renderer_3d.queue_draw(
                            node,
                            mesh,
                            surfaces,
                            model,
                            skeleton,
                            blend_shape_weights,
                            meshlet_override,
                            lod,
                            blend,
                            cast_shadows,
                            receive_shadows,
                        );
                    }
                    Command3D::DrawMulti {
                        mesh,
                        surfaces,
                        node,
                        instance_mats,
                        skeleton,
                        blend_shape_weights,
                        meshlet_override,
                        lod,
                        blend,
                        cast_shadows,
                        receive_shadows,
                        ..
                    } => {
                        self.renderer_3d.queue_draw_multi(
                            node,
                            mesh,
                            surfaces,
                            instance_mats,
                            skeleton,
                            blend_shape_weights,
                            meshlet_override,
                            lod,
                            blend,
                            cast_shadows,
                            receive_shadows,
                        );
                    }
                    Command3D::DrawMultiDense {
                        mesh,
                        surfaces,
                        node,
                        node_model,
                        instance_scale,
                        instances,
                        blend_shape_weights,
                        meshlet_override,
                        lod,
                        blend,
                        cast_shadows,
                        receive_shadows,
                        ..
                    } => {
                        self.renderer_3d.queue_draw_multi_dense(
                            node,
                            mesh,
                            surfaces,
                            crate::three_d::renderer::DenseMultiMeshDraw3D {
                                node_model,
                                instance_scale,
                                instances,
                            },
                            blend_shape_weights,
                            meshlet_override,
                            lod,
                            blend,
                            cast_shadows,
                            receive_shadows,
                        );
                    }
                    Command3D::DrawDebugPoint3D {
                        node,
                        position,
                        size,
                        color,
                    } => {
                        self.renderer_3d
                            .queue_debug_point(node, position, size, color);
                    }
                    Command3D::DrawDebugLine3D {
                        node,
                        start,
                        end,
                        thickness,
                        color,
                    } => {
                        self.renderer_3d
                            .queue_debug_line(node, start, end, thickness, color);
                    }
                    Command3D::SetCamera { camera } => {
                        self.renderer_3d.set_camera(camera);
                    }
                    Command3D::SetAmbientLight { node, light } => {
                        self.renderer_3d.set_ambient_light(node, light);
                    }
                    Command3D::SetSky { node, sky } => {
                        self.renderer_3d.set_sky(node, *sky);
                    }
                    Command3D::SetRayLight { node, light } => {
                        self.renderer_3d.set_ray_light(node, light);
                    }
                    Command3D::SetPointLight { node, light } => {
                        self.renderer_3d.set_point_light(node, light);
                    }
                    Command3D::SetSpotLight { node, light } => {
                        self.renderer_3d.set_spot_light(node, light);
                    }
                    Command3D::SetDecal { node, decal } => {
                        self.renderer_3d.set_decal(node, *decal);
                    }
                    Command3D::UpsertPointParticles { node, particles } => {
                        self.particles_3d.queue_point_particles(node, *particles);
                    }
                    Command3D::UpsertWater { node, water } => {
                        self.renderer_3d.upsert_water(node, *water);
                    }
                    Command3D::RemoveNode { node } => {
                        self.renderer_3d.remove_node(node);
                        self.particles_3d.remove_node(node);
                    }
                },
                RenderCommand::Ui(cmd) => {
                    self.renderer_ui.submit(cmd);
                }
                RenderCommand::VisualAccessibility(command) => match command {
                    VisualAccessibilityCommand::EnableColorBlind { mode, strength } => {
                        self.accessibility.color_blind =
                            Some(perro_structs::ColorBlindSetting::new(mode, strength));
                    }
                    VisualAccessibilityCommand::DisableColorBlind => {
                        self.accessibility.color_blind = None;
                    }
                },
                RenderCommand::PostProcessing(command) => {
                    match command {
                        PostProcessingCommand::SetGlobal(set) => {
                            self.global_post_processing = set;
                        }
                        PostProcessingCommand::AddGlobalNamed { name, effect } => {
                            self.global_post_processing.add(name, effect);
                        }
                        PostProcessingCommand::AddGlobalUnnamed(effect) => {
                            self.global_post_processing.add_unnamed(effect);
                        }
                        PostProcessingCommand::RemoveGlobalByName(name) => {
                            self.global_post_processing.remove(name.as_ref());
                        }
                        PostProcessingCommand::RemoveGlobalByIndex(index) => {
                            self.global_post_processing.remove_index(index);
                        }
                        PostProcessingCommand::ClearGlobal => {
                            self.global_post_processing.clear();
                        }
                    }
                    // Any global post-processing edit invalidates the cached
                    // effects Arc rebuilt in `render`.
                    self.global_post_processing_cache_dirty = true;
                }
            }
        }
        self.flush_async_mesh_loads();
        self.flush_async_texture_loads();
        self.poll_async_mesh_loads();
        self.poll_async_texture_loads();
    }

    fn upsert_camera_stream_texture(
        &mut self,
        node: NodeID,
        texture: TextureID,
        resolution: [u32; 2],
    ) {
        let [width, height] = resolution;
        let Some(len) = checked_runtime_texture_rgba_len(width, height) else {
            return;
        };
        let texture = if texture.is_nil() {
            camera_stream_texture_id(node)
        } else {
            texture
        };
        let source = format!("__camera_stream__:{}", node.as_u64());
        let id = self
            .resources
            .create_texture_with_id(texture, &source, true);
        let resolution = [width, height];
        if self.camera_stream_targets.get(&node).copied() == Some(resolution) {
            return;
        }
        self.camera_stream_targets.insert(node, resolution);
        let _ = self.resources.set_decoded_texture_data(
            id,
            DecodedTextureRgba {
                rgba: vec![0; len],
                width,
                height,
            },
        );
    }

    fn process_late_overlay_commands<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        for command in commands {
            match command {
                RenderCommand::Resource(resource_cmd) => {
                    self.process_commands(std::iter::once(RenderCommand::Resource(resource_cmd)));
                }
                RenderCommand::TwoD(cmd_2d) => match cmd_2d {
                    Command2D::UpsertCameraStream {
                        node,
                        stream,
                        sprite,
                    } => {
                        let stream = *stream;
                        if !matches!(stream.source, CameraStreamSourceState::Webcam { .. }) {
                            self.upsert_camera_stream_texture(
                                node,
                                stream.output_texture,
                                stream.resolution,
                            );
                        }
                        self.late_overlay_2d.queue_sprite(node, sprite);
                    }
                    Command2D::UpsertSprite { node, sprite } => {
                        self.late_overlay_2d.queue_sprite(node, sprite);
                    }
                    Command2D::UpsertTileMap { node, tilemap } => {
                        self.late_overlay_2d.upsert_tilemap(node, tilemap);
                    }
                    Command2D::UpsertRect { node, rect } => {
                        self.late_overlay_2d.queue_rect(node, rect);
                    }
                    Command2D::UpsertPointParticles { node, particles } => {
                        self.late_overlay_2d.queue_point_particles(node, *particles);
                    }
                    Command2D::UpsertWater { node, water } => {
                        self.late_overlay_2d.upsert_water(node, *water);
                    }
                    Command2D::UpsertShadowCaster { node, caster } => {
                        self.late_overlay_2d.upsert_shadow_caster(node, caster);
                    }
                    Command2D::SetAmbientLight { node, light } => {
                        self.late_overlay_2d.set_ambient_light(node, light);
                    }
                    Command2D::SetRayLight { node, light } => {
                        self.late_overlay_2d.set_ray_light(node, light);
                    }
                    Command2D::SetPointLight { node, light } => {
                        self.late_overlay_2d.set_point_light(node, light);
                    }
                    Command2D::SetSpotLight { node, light } => {
                        self.late_overlay_2d.set_spot_light(node, light);
                    }
                    Command2D::RemoveNode { node } => {
                        self.late_overlay_2d.remove_node(node);
                    }
                    Command2D::SetCamera { camera } => {
                        self.late_overlay_2d.set_camera(camera);
                    }
                    Command2D::DrawShape { draw } => {
                        self.late_overlay_2d.queue_shape(draw);
                    }
                },
                _ => self.process_commands(std::iter::once(command)),
            }
        }
    }
}

impl RenderBridge for PerroGraphics {
    fn submit(&mut self, command: RenderCommand) {
        self.frame.queue(command);
        self.redraw_requested = true;
    }

    fn submit_many<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.frame.pending_commands.extend(commands);
        self.redraw_requested = true;
    }

    fn drain_events(&mut self, out: &mut Vec<RenderEvent>) {
        out.append(&mut self.events);
    }
}

impl GraphicsBackend for PerroGraphics {
    fn attach_window(&mut self, window: Arc<Window>) {
        if self.gpu.is_none() {
            #[cfg(target_arch = "wasm32")]
            {
                if self.pending_gpu.is_some() {
                    return;
                }
                let slot = Arc::new(Mutex::new(None));
                let slot_clone = slot.clone();
                let cfg = GpuConfig {
                    smoothing_samples: self.smoothing_samples,
                    vsync_enabled: self.vsync_enabled,
                    meshlets_enabled: self.meshlets_enabled,
                    dev_meshlets: self.dev_meshlets,
                    meshlet_debug_view: self.meshlet_debug_view,
                    occlusion_culling: self.occlusion_culling,
                    texture_filter: self.texture_filter,
                };
                wasm_bindgen_futures::spawn_local(async move {
                    let gpu = Gpu::new_async(window, cfg).await;
                    if let Ok(mut pending) = slot_clone.lock() {
                        *pending = gpu;
                    }
                });
                self.pending_gpu = Some(slot);
                self.redraw_requested = true;
                return;
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let cfg = GpuConfig {
                    smoothing_samples: self.smoothing_samples,
                    vsync_enabled: self.vsync_enabled,
                    meshlets_enabled: self.meshlets_enabled,
                    dev_meshlets: self.dev_meshlets,
                    meshlet_debug_view: self.meshlet_debug_view,
                    occlusion_culling: self.occlusion_culling,
                    texture_filter: self.texture_filter,
                };
                let mut gpu = Gpu::new(window, cfg);
                if let Some(gpu_ref) = gpu.as_mut() {
                    let [vw, vh] = Gpu::virtual_size();
                    self.renderer_2d.set_virtual_viewport(vw, vh);
                    self.late_overlay_2d.set_virtual_viewport(vw, vh);
                    gpu_ref.resize(self.viewport.0.max(1), self.viewport.1.max(1));
                }
                self.gpu = gpu;
                self.redraw_requested = true;
            }
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.viewport = (width, height);
        self.renderer_2d.set_viewport(width, height);
        self.late_overlay_2d.set_viewport(width, height);
        if let Some(gpu) = &mut self.gpu {
            gpu.resize(width.max(1), height.max(1));
        }
        self.redraw_requested = true;
    }

    fn set_smoothing(&mut self, enabled: bool) {
        self.smoothing_enabled = enabled;
        self.smoothing_samples = if enabled {
            self.smoothing_quality_samples.max(2)
        } else {
            1
        };
        if let Some(gpu) = &mut self.gpu {
            gpu.set_smoothing_samples(self.smoothing_samples);
        }
        self.redraw_requested = true;
    }

    fn set_smoothing_samples(&mut self, samples: u32) {
        let normalized = normalize_aa_sample_count(samples);
        self.smoothing_samples = normalized;
        self.smoothing_enabled = normalized > 1;
        if normalized > 1 {
            self.smoothing_quality_samples = normalized;
        }
        if let Some(gpu) = &mut self.gpu {
            gpu.set_smoothing_samples(normalized);
        }
        self.redraw_requested = true;
    }

    fn profile_snapshot(&self) -> GraphicsProfileSnapshot {
        GraphicsProfileSnapshot {
            active_meshes: self.resources.active_mesh_count() as u32,
            active_materials: self.resources.active_material_count() as u32,
            active_textures: self.resources.active_texture_count() as u32,
        }
    }

    fn wait_idle(&mut self) {
        if let Some(gpu) = &mut self.gpu {
            gpu.wait_idle();
        }
    }

    fn draw_frame(&mut self) {
        let _ = self.draw_frame_timed();
    }

    fn draw_frame_timed(&mut self) -> Option<DrawFrameTiming> {
        self.draw_frame_timed_internal(std::iter::empty::<RenderCommand>())
    }

    fn draw_frame_with_late_overlay<I>(&mut self, overlay_commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        let _ = self.draw_frame_timed_internal(overlay_commands);
    }

    fn draw_frame_with_late_overlay_timed<I>(
        &mut self,
        overlay_commands: I,
    ) -> Option<DrawFrameTiming>
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.draw_frame_timed_internal(overlay_commands)
    }

    fn submit_late_overlay_many<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.process_late_overlay_commands(commands);
        self.redraw_requested = true;
    }
}

impl PerroGraphics {
    fn reserve_command_buckets(&mut self, summary: &CommandSummary) {
        if summary.rects_2d > 0 {
            self.renderer_2d.reserve_queued_rects(summary.rects_2d);
        }
        if summary.sprites_2d > 0 {
            self.renderer_2d.reserve_queued_sprites(summary.sprites_2d);
        }
        if summary.draws_3d > 0 {
            self.renderer_3d.reserve_queued_draws(summary.draws_3d);
        }
    }

    fn draw_frame_timed_internal<I>(&mut self, late_overlay_commands: I) -> Option<DrawFrameTiming>
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        #[cfg(target_arch = "wasm32")]
        self.try_finish_gpu_init();
        let total_start = Instant::now();
        self.poll_async_mesh_loads();
        self.poll_async_texture_loads();
        let now = Instant::now();
        self.frame_delta_seconds = self
            .last_frame_instant
            .map(|prev| now.duration_since(prev).as_secs_f32())
            .unwrap_or(0.0)
            .max(0.0);
        self.last_frame_instant = Some(now);
        self.frame_time_seconds =
            (self.frame_time_seconds + self.frame_delta_seconds).rem_euclid(1.0e9);
        let mut late_overlay_pending =
            std::mem::take(&mut self.frame.scratch_late_overlay_commands);
        late_overlay_pending.clear();
        late_overlay_pending.extend(late_overlay_commands);
        let has_pending = !self.frame.pending_commands.is_empty();
        let has_late_overlay = !late_overlay_pending.is_empty()
            || self.late_overlay_2d.retained_sprite_count() > 0
            || !self.late_overlay_2d.retained_rects().is_empty();
        let has_continuous_updates = self.renderer_3d.has_active_sky_animation();
        let has_retained_scene = self.renderer_2d.retained_sprite_count() > 0
            || !self.renderer_2d.retained_rects().is_empty()
            || has_late_overlay
            || self.renderer_2d.retained_water_count() > 0
            || self.renderer_ui.retained_count() > 0
            || self.renderer_3d.retained_draw_count() > 0
            || self.renderer_3d.has_retained_non_draw_state()
            || self.particles_3d.retained_point_particle_count() > 0;
        if !has_pending && !has_retained_scene {
            if self.redraw_requested
                && let Some(gpu) = &mut self.gpu
            {
                self.redraw_requested = !gpu.render_idle_clear();
            }
            self.frame.scratch_late_overlay_commands = late_overlay_pending;
            return Some(DrawFrameTiming {
                total: total_start.elapsed(),
                idle_clear: true,
                ..DrawFrameTiming::default()
            });
        }
        if !has_pending && !has_continuous_updates && !self.redraw_requested {
            self.frame.scratch_late_overlay_commands = late_overlay_pending;
            return Some(DrawFrameTiming {
                total: total_start.elapsed(),
                idle_clear: true,
                ..DrawFrameTiming::default()
            });
        }
        let mut pending = std::mem::take(&mut self.frame.scratch_commands);
        pending.clear();
        std::mem::swap(&mut pending, &mut self.frame.pending_commands);
        let pending_command_count = pending.len();
        let command_summary = summarize_commands(&pending);
        let frame_dirty_bits = command_summary.dirty_bits;
        let process_start = Instant::now();
        self.reserve_command_buckets(&command_summary);
        let mut camera_commands = std::mem::take(&mut self.frame.scratch_camera_commands);
        camera_commands.clear();
        let mut write = 0usize;
        for read in 0..pending.len() {
            let is_camera_command = match &pending[read] {
                RenderCommand::TwoD(Command2D::SetCamera { .. }) => true,
                RenderCommand::ThreeD(cmd) => {
                    matches!(cmd.as_ref(), Command3D::SetCamera { .. })
                }
                _ => false,
            };
            if is_camera_command {
                camera_commands.push(pending[read].clone());
            } else {
                if read != write {
                    pending.swap(write, read);
                }
                write += 1;
            }
        }
        pending.truncate(write);
        self.process_commands(camera_commands.drain(..));
        self.process_commands(pending.drain(..));
        self.frame.scratch_camera_commands = camera_commands;
        self.process_late_overlay_commands(late_overlay_pending.drain(..));
        self.frame.scratch_late_overlay_commands = late_overlay_pending;
        let process_commands = process_start.elapsed();
        let prepare_start = Instant::now();
        let (
            (camera_2d, _stats, upload),
            (late_overlay_camera_2d, _late_overlay_stats, late_overlay_upload),
            (camera_3d, _stats_3d, lighting_3d),
        ) = if pending_command_count >= PARALLEL_RENDER_PREPARE_MIN {
            let resources = &self.resources;
            let renderer_2d = &mut self.renderer_2d;
            let late_overlay_2d = &mut self.late_overlay_2d;
            let renderer_3d = &mut self.renderer_3d;
            let particles_3d = &mut self.particles_3d;
            let ((main_2d, late_overlay), main_3d) = rayon::join(
                || {
                    let main_2d = renderer_2d.prepare_frame(resources);
                    let late_overlay = late_overlay_2d.prepare_frame(resources);
                    (main_2d, late_overlay)
                },
                || {
                    let main_3d = renderer_3d.prepare_frame(resources);
                    particles_3d.prepare_frame();
                    main_3d
                },
            );
            (main_2d, late_overlay, main_3d)
        } else {
            let main_2d = self.renderer_2d.prepare_frame(&self.resources);
            let late_overlay = self.late_overlay_2d.prepare_frame(&self.resources);
            let main_3d = self.renderer_3d.prepare_frame(&self.resources);
            self.particles_3d.prepare_frame();
            (main_2d, late_overlay, main_3d)
        };
        let camera_2d_state = self.renderer_2d.camera();
        let draws_revision = self.renderer_3d.draw_revision();
        let point_particles_revision = self.particles_3d.retained_point_particles_revision();
        if point_particles_revision != self.retained_point_particles_cache_revision {
            self.retained_point_particles_cache.clear();
            let point_particles_count = self.particles_3d.retained_point_particle_count();
            if self.retained_point_particles_cache.capacity() < point_particles_count {
                self.retained_point_particles_cache.reserve(
                    point_particles_count - self.retained_point_particles_cache.capacity(),
                );
            }
            self.retained_point_particles_cache
                .extend(self.particles_3d.retained_point_particles());
            self.retained_point_particles_cache
                .sort_unstable_by_key(|(node, _)| node.as_u64());
            self.retained_point_particles_cache_revision = point_particles_revision;
        }
        let waters_3d_revision = self.renderer_3d.retained_waters_revision();
        if waters_3d_revision != self.retained_waters_3d_cache_revision {
            self.retained_waters_3d_cache.clear();
            self.retained_waters_3d_cache
                .extend_from_slice(self.renderer_3d.retained_waters_sorted());
            self.retained_waters_3d_cache_revision = waters_3d_revision;
        }
        let decals_3d_revision = self.renderer_3d.retained_decals_revision();
        if decals_3d_revision != self.retained_decals_3d_cache_revision {
            self.retained_decals_3d_cache.clear();
            self.retained_decals_3d_cache
                .extend_from_slice(self.renderer_3d.retained_decals_sorted());
            self.retained_decals_3d_cache_revision = decals_3d_revision;
        }
        let retained_draws_3d = self.renderer_3d.retained_draws_sorted();
        if draws_revision != self.retained_draws_cache_revision {
            self.retained_draw_instances_cache =
                retained_draws_3d.iter().fold(0u32, |acc, draw| {
                    acc.saturating_add(draw_instance_count(draw))
                });
            self.retained_draws_cache_revision = draws_revision;
        }
        let waters_2d_revision = self.renderer_2d.retained_waters_revision();
        if waters_2d_revision != self.retained_waters_2d_cache_revision {
            self.retained_waters_2d_cache.clear();
            let water_count = self.renderer_2d.retained_water_count();
            if self.retained_waters_2d_cache.capacity() < water_count {
                self.retained_waters_2d_cache
                    .reserve(water_count - self.retained_waters_2d_cache.capacity());
            }
            self.retained_waters_2d_cache
                .extend(self.renderer_2d.retained_waters());
            self.retained_waters_2d_cache
                .sort_unstable_by_key(|(node, _)| node.as_u64());
            self.retained_waters_2d_cache_revision = waters_2d_revision;
        }
        let sprites_revision = self.renderer_2d.retained_sprites_revision();
        if sprites_revision != self.retained_sprites_cache_revision {
            self.retained_sprites_cache.clear();
            let sprite_count = self.renderer_2d.retained_sprite_count();
            if self.retained_sprites_cache.capacity() < sprite_count {
                self.retained_sprites_cache
                    .reserve(sprite_count - self.retained_sprites_cache.capacity());
            }
            self.retained_sprites_cache
                .extend(self.renderer_2d.retained_sprites());
            self.retained_sprites_cache_revision = sprites_revision;
        }
        let point_lights_revision = self.renderer_2d.retained_point_lights_revision();
        if point_lights_revision != self.retained_point_lights_cache_revision {
            self.retained_point_lights_cache.clear();
            let point_light_count = self.renderer_2d.light_count();
            if self.retained_point_lights_cache.capacity() < point_light_count {
                self.retained_point_lights_cache
                    .reserve(point_light_count - self.retained_point_lights_cache.capacity());
            }
            self.retained_point_lights_cache
                .extend(self.renderer_2d.lights());
            self.retained_point_lights_cache_revision = point_lights_revision;
        }
        let shadow_casters_revision = self.renderer_2d.retained_shadow_casters_revision();
        if shadow_casters_revision != self.retained_shadow_casters_cache_revision {
            self.retained_shadow_casters_cache.clear();
            self.retained_shadow_casters_cache
                .extend(self.renderer_2d.shadow_casters());
            self.retained_shadow_casters_cache_revision = shadow_casters_revision;
        }
        let retained_rect_count = self.renderer_2d.retained_rects().len();
        let frame_shape_count = self.renderer_2d.frame_shapes().len();
        let total_rect_count = retained_rect_count + frame_shape_count;
        if frame_shape_count == 0
            && !upload.full_reupload
            && upload.dirty_ranges.is_empty()
            && self.frame_rects_cache.len() == retained_rect_count
        {
            // Retained rect buffer already mirrors renderer state.
        } else {
            self.frame_rects_cache.clear();
            if self.frame_rects_cache.capacity() < total_rect_count {
                self.frame_rects_cache
                    .reserve(total_rect_count - self.frame_rects_cache.capacity());
            }
            self.frame_rects_cache
                .extend_from_slice(self.renderer_2d.retained_rects());
            self.frame_rects_cache
                .extend_from_slice(self.renderer_2d.frame_shapes());
        }
        self.late_overlay_rects_cache.clear();
        self.late_overlay_rects_cache
            .extend_from_slice(self.late_overlay_2d.retained_rects());
        self.late_overlay_rects_cache
            .extend_from_slice(self.late_overlay_2d.frame_shapes());
        let late_overlay_sprites_revision = self.late_overlay_2d.retained_sprites_revision();
        if late_overlay_sprites_revision != self.late_overlay_sprites_cache_revision {
            self.late_overlay_sprites_cache.clear();
            let sprite_count = self.late_overlay_2d.retained_sprite_count();
            if self.late_overlay_sprites_cache.capacity() < sprite_count {
                self.late_overlay_sprites_cache
                    .reserve(sprite_count - self.late_overlay_sprites_cache.capacity());
            }
            self.late_overlay_sprites_cache
                .extend(self.late_overlay_2d.retained_sprites());
            self.late_overlay_sprites_cache_revision = late_overlay_sprites_revision;
        }
        let late_overlay_point_lights_revision =
            self.late_overlay_2d.retained_point_lights_revision();
        if late_overlay_point_lights_revision != self.late_overlay_point_lights_cache_revision {
            self.late_overlay_point_lights_cache.clear();
            let point_light_count = self.late_overlay_2d.light_count();
            if self.late_overlay_point_lights_cache.capacity() < point_light_count {
                self.late_overlay_point_lights_cache
                    .reserve(point_light_count - self.late_overlay_point_lights_cache.capacity());
            }
            self.late_overlay_point_lights_cache
                .extend(self.late_overlay_2d.lights());
            self.late_overlay_point_lights_cache_revision = late_overlay_point_lights_revision;
        }
        self.late_overlay_shadow_casters_cache.clear();
        self.late_overlay_shadow_casters_cache
            .extend(self.late_overlay_2d.shadow_casters());
        let ui_image_textures: Vec<_> = self.renderer_ui.image_textures().collect();
        let ui_paint = self
            .renderer_ui
            .prepare_paint([self.viewport.0 as f32, self.viewport.1 as f32]);
        let sprites_refs_changed = self.used_ref_sprites_revision != sprites_revision;
        if sprites_refs_changed {
            self.used_texture_refs_cache.clear();
            self.used_texture_refs_cache
                .reserve(self.retained_sprites_cache.len());
            for sprite in &self.retained_sprites_cache {
                *self
                    .used_texture_refs_cache
                    .entry(sprite.texture)
                    .or_insert(0) += 1;
            }
            self.used_ref_sprites_revision = sprites_revision;
        }
        let draws_refs_changed = self.used_ref_draws_revision != draws_revision;
        if draws_refs_changed {
            self.used_mesh_refs_cache.clear();
            self.used_material_refs_cache.clear();
            self.used_mesh_refs_cache.reserve(retained_draws_3d.len());
            self.used_material_refs_cache
                .reserve(retained_draws_3d.len());
            for draw in retained_draws_3d {
                if let Draw3DKind::Mesh(mesh) = draw.kind {
                    *self.used_mesh_refs_cache.entry(mesh).or_insert(0) += 1;
                }
                for material in draw.surfaces.iter().filter_map(|surface| surface.material) {
                    *self.used_material_refs_cache.entry(material).or_insert(0) += 1;
                }
            }
            self.used_ref_draws_revision = draws_revision;
        }

        if sprites_refs_changed || draws_refs_changed || (frame_dirty_bits & DIRTY_RESOURCES) != 0 {
            self.resources.reset_ref_counts();
            for (texture, count) in &self.used_texture_refs_cache {
                self.resources.mark_texture_used_count(*texture, *count);
            }
            for (texture, nodes) in &self.scene_texture_refs_cache {
                self.resources
                    .mark_texture_used_count(*texture, nodes.len().min(u32::MAX as usize) as u32);
            }
            for texture in &ui_image_textures {
                self.resources.mark_texture_used(*texture);
            }
            for (mesh, count) in &self.used_mesh_refs_cache {
                self.resources.mark_mesh_used_count(*mesh, *count);
            }
            for (mesh, nodes) in &self.scene_mesh_refs_cache {
                self.resources
                    .mark_mesh_used_count(*mesh, nodes.len().min(u32::MAX as usize) as u32);
            }
            for (material, count) in &self.used_material_refs_cache {
                self.resources.mark_material_used_count(*material, *count);
            }
            for (material, nodes) in &self.scene_material_refs_cache {
                self.resources
                    .mark_material_used_count(*material, nodes.len().min(u32::MAX as usize) as u32);
            }
        }
        self.frame_index = self.frame_index.wrapping_add(1);
        if self.frame_index.is_multiple_of(GC_INTERVAL_FRAMES) {
            let drops = self.resources.gc_unused_after_frames(
                ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES,
                GC_INTERVAL_FRAMES,
                GC_MAX_DROPS_PER_KIND,
            );
            self.events.extend(
                drops
                    .textures
                    .into_iter()
                    .map(|id| RenderEvent::TextureDropped { id }),
            );
            self.events.extend(
                drops
                    .meshes
                    .into_iter()
                    .map(|id| RenderEvent::MeshDropped { id }),
            );
            self.events.extend(
                drops
                    .materials
                    .into_iter()
                    .map(|id| RenderEvent::MaterialDropped { id }),
            );
        }
        let prepare_cpu = prepare_start.elapsed();

        if self.global_post_processing_cache_dirty {
            self.global_post_processing_cache =
                Arc::from(self.global_post_processing.to_effects_vec());
            self.global_post_processing_cache_dirty = false;
        }

        let mut gpu_timing = RenderGpuTiming::default();
        if let Some(gpu) = &mut self.gpu {
            gpu_timing = gpu.render(RenderFrame {
                resources: &self.resources,
                camera_3d,
                lighting_3d: &lighting_3d,
                draws_3d: retained_draws_3d,
                draws_3d_revision: draws_revision,
                point_particles_3d: &self.retained_point_particles_cache,
                point_particles_3d_revision: self.retained_point_particles_cache_revision,
                waters_3d: &self.retained_waters_3d_cache,
                waters_3d_revision: self.retained_waters_3d_cache_revision,
                decals_3d: &self.retained_decals_3d_cache,
                decals_3d_revision: self.retained_decals_3d_cache_revision,
                camera_streams: &self.retained_camera_streams,
                camera_2d,
                camera_2d_position: camera_2d_state.position,
                post_processing_2d: camera_2d_state.post_processing.clone(),
                post_processing_global: self.global_post_processing_cache.clone(),
                accessibility: self.accessibility,
                rects_2d: &self.frame_rects_cache,
                upload_2d: &upload,
                sprites_2d: &self.retained_sprites_cache,
                sprites_2d_revision: self.retained_sprites_cache_revision,
                point_lights_2d: &self.retained_point_lights_cache,
                point_lights_2d_revision: self.retained_point_lights_cache_revision,
                shadow_casters_2d: &self.retained_shadow_casters_cache,
                waters_2d: &self.retained_waters_2d_cache,
                waters_2d_revision: self.retained_waters_2d_cache_revision,
                late_overlay_camera_2d,
                late_overlay_rects_2d: &self.late_overlay_rects_cache,
                late_overlay_upload_2d: &late_overlay_upload,
                late_overlay_sprites_2d: &self.late_overlay_sprites_cache,
                late_overlay_sprites_2d_revision: self.late_overlay_sprites_cache_revision,
                late_overlay_point_lights_2d: &self.late_overlay_point_lights_cache,
                late_overlay_point_lights_2d_revision: self
                    .late_overlay_point_lights_cache_revision,
                late_overlay_shadow_casters_2d: &self.late_overlay_shadow_casters_cache,
                ui_primitives: ui_paint.primitives,
                ui_textures_delta: ui_paint.textures_delta,
                ui_texture_size: ui_paint.texture_size,
                ui_revision: ui_paint.revision,
                redraw_requested: self.redraw_requested,
                frame_time_seconds: self.frame_time_seconds,
                frame_delta_seconds: self.frame_delta_seconds,
                frame_dirty_bits,
                static_texture_lookup: self.static_texture_lookup,
                static_mesh_lookup: self.static_mesh_lookup,
                static_shader_lookup: self.static_shader_lookup,
            });
            let mut water_samples = Vec::new();
            gpu.drain_water_samples(&mut water_samples);
            if !water_samples.is_empty() {
                self.events.push(RenderEvent::WaterSamples {
                    samples: Arc::from(water_samples.into_boxed_slice()),
                });
            }
            let mut water_body_samples = Vec::new();
            gpu.drain_water_body_samples(&mut water_body_samples);
            if !water_body_samples.is_empty() {
                self.events.push(RenderEvent::WaterBodySamples {
                    samples: Arc::from(water_body_samples.into_boxed_slice()),
                });
            }
            self.redraw_requested = !gpu_timing.presented;
        }
        let timing = DrawFrameTiming {
            process_commands,
            prepare_cpu,
            gpu_prepare_2d: gpu_timing.prepare_2d,
            gpu_prepare_3d: gpu_timing.prepare_3d,
            gpu_prepare_particles_3d: gpu_timing.prepare_particles_3d,
            gpu_prepare_3d_frustum: gpu_timing.prepare_3d_frustum,
            gpu_prepare_3d_hiz: gpu_timing.prepare_3d_hiz,
            gpu_prepare_3d_indirect: gpu_timing.prepare_3d_indirect,
            gpu_prepare_3d_cull_inputs: gpu_timing.prepare_3d_cull_inputs,
            gpu_acquire: gpu_timing.acquire,
            gpu_acquire_surface: gpu_timing.acquire_surface,
            gpu_acquire_view: gpu_timing.acquire_view,
            gpu_encode_main: gpu_timing.encode_main,
            gpu_submit_main: gpu_timing.submit_main,
            gpu_submit_finish_main: gpu_timing.submit_finish_main,
            gpu_submit_queue_main: gpu_timing.submit_queue_main,
            gpu_post_process: gpu_timing.post_process,
            gpu_accessibility: gpu_timing.accessibility,
            gpu_present: gpu_timing.present,
            gpu_timestamp_main: gpu_timing.gpu_timestamp_main,
            gpu_timestamp_water: gpu_timing.gpu_timestamp_water,
            draw_calls_2d: gpu_timing.draw_calls_2d,
            draw_calls_3d: gpu_timing.draw_calls_3d,
            sprite_batches_2d: gpu_timing.sprite_batches_2d,
            sprite_bind_group_switches_2d: gpu_timing.sprite_bind_group_switches_2d,
            draw_batches_3d: gpu_timing.draw_batches_3d,
            pipeline_switches_3d: gpu_timing.pipeline_switches_3d,
            texture_bind_group_switches_3d: gpu_timing.texture_bind_group_switches_3d,
            draw_instances_3d: self.retained_draw_instances_cache,
            draw_material_refs_3d: self.used_material_refs_cache.len().min(u32::MAX as usize)
                as u32,
            skip_prepare_2d: gpu_timing.skip_prepare_2d,
            skip_prepare_3d: gpu_timing.skip_prepare_3d,
            skip_prepare_particles_3d: gpu_timing.skip_prepare_particles_3d,
            skip_prepare_3d_frustum: gpu_timing.skip_prepare_3d_frustum,
            skip_prepare_3d_hiz: gpu_timing.skip_prepare_3d_hiz,
            skip_prepare_3d_indirect: gpu_timing.skip_prepare_3d_indirect,
            skip_prepare_3d_cull_inputs: gpu_timing.skip_prepare_3d_cull_inputs,
            gpu_total: gpu_timing.total,
            total: total_start.elapsed(),
            idle_clear: false,
        };
        pending.clear();
        self.frame.scratch_commands = pending;
        Some(timing)
    }
}

#[cfg(target_arch = "wasm32")]
impl PerroGraphics {
    fn try_finish_gpu_init(&mut self) {
        let Some(slot) = self.pending_gpu.as_ref() else {
            return;
        };
        let Some(mut gpu) = slot.lock().ok().and_then(|mut guard| guard.take()) else {
            return;
        };
        let [vw, vh] = Gpu::virtual_size();
        self.renderer_2d.set_virtual_viewport(vw, vh);
        self.late_overlay_2d.set_virtual_viewport(vw, vh);
        gpu.resize(self.viewport.0.max(1), self.viewport.1.max(1));
        self.gpu = Some(gpu);
        self.pending_gpu = None;
        self.redraw_requested = true;
    }
}

#[cfg(test)]
#[path = "../tests/unit/backend_tests.rs"]
mod tests;

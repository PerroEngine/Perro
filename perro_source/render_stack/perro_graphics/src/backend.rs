use crate::{
    gpu::{
        DIRTY_2D, DIRTY_3D, DIRTY_ACCESSIBILITY, DIRTY_CAMERA_2D, DIRTY_CAMERA_3D, DIRTY_LIGHTS_3D,
        DIRTY_PARTICLES_3D, DIRTY_POSTFX, DIRTY_RESOURCES, Gpu, RenderFrame, RenderGpuTiming,
    },
    resources::ResourceStore,
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
use ahash::AHashSet;
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Command2D, Command3D, PointParticles3DState, PostProcessingCommand, RenderBridge,
    RenderCommand, RenderEvent, ResourceCommand, Sprite2DCommand, VisualAccessibilityCommand,
};
use perro_structs::{PostProcessSet, VisualAccessibilitySettings};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::window::Window;

pub type StaticTextureLookup = fn(path_hash: u64) -> &'static [u8];
pub type StaticMeshLookup = fn(path_hash: u64) -> &'static [u8];
pub type StaticShaderLookup = fn(path_hash: u64) -> &'static str;
const GC_INTERVAL_FRAMES: u32 = 4;

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

    fn profile_snapshot(&self) -> GraphicsProfileSnapshot {
        GraphicsProfileSnapshot::default()
    }
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
    pub draw_calls_2d: u32,
    pub draw_calls_3d: u32,
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

#[derive(Default)]
pub struct PerroGraphics {
    frame: FrameState,
    resources: ResourceStore,
    renderer_2d: Renderer2D,
    renderer_3d: Renderer3D,
    particles_3d: Particles3DRenderer,
    renderer_ui: UiRenderer,
    gpu: Option<Gpu>,
    events: Vec<RenderEvent>,
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
    retained_draws_cache: Vec<Draw3DInstance>,
    retained_draws_cache_revision: u64,
    retained_point_particles_cache: Vec<(NodeID, PointParticles3DState)>,
    retained_point_particles_cache_revision: u64,
    retained_sprites_cache: Vec<Sprite2DCommand>,
    retained_sprites_cache_revision: u64,
    frame_rects_cache: Vec<RectInstanceGpu>,
    used_texture_refs_cache: AHashSet<TextureID>,
    used_mesh_refs_cache: AHashSet<MeshID>,
    used_material_refs_cache: AHashSet<MaterialID>,
    used_ref_draws_revision: u64,
    used_ref_sprites_revision: u64,
    global_post_processing: PostProcessSet,
    accessibility: VisualAccessibilitySettings,
    frame_index: u32,
    redraw_requested: bool,
}

impl PerroGraphics {
    pub fn new() -> Self {
        Self {
            frame: FrameState::default(),
            resources: ResourceStore::new(),
            renderer_2d: Renderer2D::new(),
            renderer_3d: Renderer3D::new(),
            particles_3d: Particles3DRenderer::new(),
            renderer_ui: UiRenderer::new(),
            gpu: None,
            events: Vec::new(),
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
            retained_draws_cache: Vec::new(),
            retained_draws_cache_revision: u64::MAX,
            retained_point_particles_cache: Vec::new(),
            retained_point_particles_cache_revision: u64::MAX,
            retained_sprites_cache: Vec::new(),
            retained_sprites_cache_revision: u64::MAX,
            frame_rects_cache: Vec::new(),
            used_texture_refs_cache: AHashSet::new(),
            used_mesh_refs_cache: AHashSet::new(),
            used_material_refs_cache: AHashSet::new(),
            used_ref_draws_revision: u64::MAX,
            used_ref_sprites_revision: u64::MAX,
            global_post_processing: PostProcessSet::new(),
            accessibility: VisualAccessibilitySettings::default(),
            frame_index: 0,
            redraw_requested: true,
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

    fn process_commands<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        for command in commands {
            match command {
                RenderCommand::Resource(resource_cmd) => match resource_cmd {
                    ResourceCommand::CreateMesh {
                        request,
                        id,
                        source,
                        reserved,
                    } => {
                        if let Err(reason) =
                            validate_mesh_source(source.as_str(), self.static_mesh_lookup)
                        {
                            self.events.push(RenderEvent::Failed { request, reason });
                            continue;
                        }
                        let out_id = if id.is_nil() {
                            self.resources.create_mesh(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_mesh_with_id(id, source.as_str(), reserved)
                        };
                        let mesh_data = load_mesh3d_from_source(source.as_str(), self.static_mesh_lookup);
                        if let Some(mesh) =
                            mesh_data.clone()
                        {
                            self.resources.set_runtime_mesh_data(source.as_str(), mesh.clone());
                            let _ = self.resources.set_runtime_mesh_data_by_id(out_id, mesh);
                        }
                        self.events.push(RenderEvent::MeshCreated {
                            request,
                            id: out_id,
                            mesh: mesh_data,
                        });
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
                        self.resources.set_runtime_mesh_data(source.as_str(), mesh.clone());
                        let _ = self.resources.set_runtime_mesh_data_by_id(out_id, mesh.clone());
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
                        self.events
                            .push(RenderEvent::TextureCreated { request, id });
                    }
                    ResourceCommand::CreateMaterial {
                        request,
                        id,
                        material,
                        source,
                        reserved,
                    } => {
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
                    }
                    ResourceCommand::WriteMaterialData { id, material } => {
                        let _ = self.resources.set_material_data(id, material);
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
                        self.resources.drop_mesh(id);
                    }
                    ResourceCommand::DropTexture { id } => {
                        self.resources.drop_texture(id);
                    }
                    ResourceCommand::DropMaterial { id } => {
                        self.resources.drop_material(id);
                    }
                },
                RenderCommand::TwoD(cmd_2d) => match cmd_2d {
                    Command2D::UpsertSprite { node, sprite } => {
                        self.renderer_2d.queue_sprite(node, sprite);
                    }
                    Command2D::UpsertRect { node, rect } => {
                        self.renderer_2d.queue_rect(node, rect);
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
                    Command3D::Draw {
                        mesh,
                        surfaces,
                        node,
                        model,
                        skeleton,
                        meshlet_override,
                    } => {
                        self.renderer_3d.queue_draw(
                            node,
                            mesh,
                            surfaces,
                            model,
                            skeleton,
                            meshlet_override,
                        );
                    }
                    Command3D::DrawMulti {
                        mesh,
                        surfaces,
                        node,
                        instance_mats,
                        skeleton,
                        meshlet_override,
                    } => {
                        self.renderer_3d.queue_draw_multi(
                            node,
                            mesh,
                            surfaces,
                            instance_mats,
                            skeleton,
                            meshlet_override,
                        );
                    }
                    Command3D::DrawMultiDense {
                        mesh,
                        surfaces,
                        node,
                        node_model,
                        instance_scale,
                        instances,
                        meshlet_override,
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
                            meshlet_override,
                        );
                    }
                    Command3D::DrawDebugPoint3D {
                        node: _,
                        position: _,
                        size: _,
                    } => {
                        // Debug draw path removed from renderer hot path.
                    }
                    Command3D::DrawDebugLine3D {
                        node: _,
                        start: _,
                        end: _,
                        thickness: _,
                    } => {
                        // Debug draw path removed from renderer hot path.
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
                    Command3D::UpsertPointParticles { node, particles } => {
                        self.particles_3d.queue_point_particles(node, *particles);
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
                RenderCommand::PostProcessing(command) => match command {
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
                },
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
            let mut gpu = Gpu::new(
                window,
                self.smoothing_samples,
                self.vsync_enabled,
                self.meshlets_enabled,
                self.dev_meshlets,
                self.meshlet_debug_view,
                self.occlusion_culling,
            );
            if let Some(gpu_ref) = gpu.as_mut() {
                let [vw, vh] = Gpu::virtual_size();
                self.renderer_2d.set_virtual_viewport(vw, vh);
                gpu_ref.resize(self.viewport.0.max(1), self.viewport.1.max(1));
            }
            self.gpu = gpu;
            self.redraw_requested = true;
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.viewport = (width, height);
        self.renderer_2d.set_viewport(width, height);
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

    fn draw_frame(&mut self) {
        let _ = self.draw_frame_timed();
    }

    fn draw_frame_timed(&mut self) -> Option<DrawFrameTiming> {
        let total_start = Instant::now();
        let has_pending = !self.frame.pending_commands.is_empty();
        let has_continuous_updates = self.renderer_3d.has_active_sky_animation();
        let has_retained_scene = self.renderer_2d.retained_sprite_count() > 0
            || !self.renderer_2d.retained_rects().is_empty()
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
            return Some(DrawFrameTiming {
                total: total_start.elapsed(),
                idle_clear: true,
                ..DrawFrameTiming::default()
            });
        }
        if !has_pending && !has_continuous_updates && !self.redraw_requested {
            return Some(DrawFrameTiming {
                total: total_start.elapsed(),
                idle_clear: true,
                ..DrawFrameTiming::default()
            });
        }
        let mut pending = std::mem::take(&mut self.frame.scratch_commands);
        pending.clear();
        std::mem::swap(&mut pending, &mut self.frame.pending_commands);
        let mut frame_dirty_bits = 0u32;
        for command in &pending {
            match command {
                RenderCommand::TwoD(cmd_2d) => {
                    frame_dirty_bits |= DIRTY_2D;
                    if matches!(cmd_2d, Command2D::SetCamera { .. }) {
                        frame_dirty_bits |= DIRTY_CAMERA_2D;
                    }
                }
                RenderCommand::ThreeD(cmd_3d) => match &**cmd_3d {
                    Command3D::Draw { .. }
                    | Command3D::DrawMulti { .. }
                    | Command3D::DrawMultiDense { .. }
                    | Command3D::DrawDebugPoint3D { .. }
                    | Command3D::DrawDebugLine3D { .. }
                    | Command3D::RemoveNode { .. } => frame_dirty_bits |= DIRTY_3D,
                    Command3D::SetCamera { .. } => frame_dirty_bits |= DIRTY_CAMERA_3D,
                    Command3D::SetAmbientLight { .. }
                    | Command3D::SetSky { .. }
                    | Command3D::SetRayLight { .. }
                    | Command3D::SetPointLight { .. }
                    | Command3D::SetSpotLight { .. } => frame_dirty_bits |= DIRTY_LIGHTS_3D,
                    Command3D::UpsertPointParticles { .. } => {
                        frame_dirty_bits |= DIRTY_PARTICLES_3D
                    }
                },
                RenderCommand::Resource(_) => frame_dirty_bits |= DIRTY_RESOURCES,
                RenderCommand::Ui(_) => frame_dirty_bits |= DIRTY_2D,
                RenderCommand::PostProcessing(_) => frame_dirty_bits |= DIRTY_POSTFX,
                RenderCommand::VisualAccessibility(_) => frame_dirty_bits |= DIRTY_ACCESSIBILITY,
            }
        }
        let process_start = Instant::now();
        self.process_commands(pending.drain(..));
        let process_commands = process_start.elapsed();
        let prepare_start = Instant::now();
        let (camera_2d, _stats, upload) = self.renderer_2d.prepare_frame(&self.resources);
        let camera_2d_state = self.renderer_2d.camera();
        let (camera_3d, _stats_3d, lighting_3d) = self.renderer_3d.prepare_frame(&self.resources);
        self.particles_3d.prepare_frame();
        let draws_revision = self.renderer_3d.draw_revision();
        if draws_revision != self.retained_draws_cache_revision {
            self.retained_draws_cache.clear();
            let draw_count = self.renderer_3d.retained_draw_count();
            if self.retained_draws_cache.capacity() < draw_count {
                self.retained_draws_cache
                    .reserve(draw_count - self.retained_draws_cache.capacity());
            }
            self.retained_draws_cache
                .extend_from_slice(self.renderer_3d.retained_draws_sorted());
            self.retained_draws_cache_revision = draws_revision;
        }
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
        self.frame_rects_cache.clear();
        let retained_rect_count = self.renderer_2d.retained_rects().len();
        let frame_shape_count = self.renderer_2d.frame_shapes().len();
        let total_rect_count = retained_rect_count + frame_shape_count;
        if self.frame_rects_cache.capacity() < total_rect_count {
            self.frame_rects_cache
                .reserve(total_rect_count - self.frame_rects_cache.capacity());
        }
        self.frame_rects_cache
            .extend_from_slice(self.renderer_2d.retained_rects());
        self.frame_rects_cache
            .extend_from_slice(self.renderer_2d.frame_shapes());
        let ui_paint = self
            .renderer_ui
            .prepare_paint([self.viewport.0 as f32, self.viewport.1 as f32]);
        let sprites_refs_changed = self.used_ref_sprites_revision != sprites_revision;
        if sprites_refs_changed {
            self.used_texture_refs_cache.clear();
            self.used_texture_refs_cache
                .reserve(self.retained_sprites_cache.len());
            self.used_texture_refs_cache.extend(
                self.retained_sprites_cache
                    .iter()
                    .map(|sprite| sprite.texture),
            );
            self.used_ref_sprites_revision = sprites_revision;
        }
        let draws_refs_changed = self.used_ref_draws_revision != draws_revision;
        if draws_refs_changed {
            self.used_mesh_refs_cache.clear();
            self.used_material_refs_cache.clear();
            self.used_mesh_refs_cache
                .reserve(self.retained_draws_cache.len());
            self.used_material_refs_cache
                .reserve(self.retained_draws_cache.len());
            for draw in &self.retained_draws_cache {
                if let Draw3DKind::Mesh(mesh) = draw.kind {
                    self.used_mesh_refs_cache.insert(mesh);
                }
                self.used_material_refs_cache
                    .extend(draw.surfaces.iter().filter_map(|surface| surface.material));
            }
            self.used_ref_draws_revision = draws_revision;
        }

        if sprites_refs_changed || draws_refs_changed || (frame_dirty_bits & DIRTY_RESOURCES) != 0 {
            self.resources.reset_ref_counts();
            for texture in &self.used_texture_refs_cache {
                self.resources.mark_texture_used(*texture);
            }
            for mesh in &self.used_mesh_refs_cache {
                self.resources.mark_mesh_used(*mesh);
            }
            for material in &self.used_material_refs_cache {
                self.resources.mark_material_used(*material);
            }
        }
        self.frame_index = self.frame_index.wrapping_add(1);
        if self.frame_index.is_multiple_of(GC_INTERVAL_FRAMES) {
            self.resources
                .gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
        }
        let prepare_cpu = prepare_start.elapsed();

        let mut gpu_timing = RenderGpuTiming::default();
        if let Some(gpu) = &mut self.gpu {
            gpu_timing = gpu.render(RenderFrame {
                resources: &self.resources,
                camera_3d,
                lighting_3d: &lighting_3d,
                draws_3d: &self.retained_draws_cache,
                draws_3d_revision: self.retained_draws_cache_revision,
                point_particles_3d: &self.retained_point_particles_cache,
                point_particles_3d_revision: self.retained_point_particles_cache_revision,
                camera_2d,
                post_processing_2d: camera_2d_state.post_processing.clone(),
                post_processing_global: self.global_post_processing.as_slice().into(),
                accessibility: self.accessibility,
                rects_2d: &self.frame_rects_cache,
                upload_2d: &upload,
                sprites_2d: &self.retained_sprites_cache,
                ui_primitives: ui_paint.primitives,
                ui_textures_delta: ui_paint.textures_delta,
                ui_texture_size: ui_paint.texture_size,
                ui_revision: ui_paint.revision,
                redraw_requested: self.redraw_requested,
                frame_dirty_bits,
                static_texture_lookup: self.static_texture_lookup,
                static_mesh_lookup: self.static_mesh_lookup,
                static_shader_lookup: self.static_shader_lookup,
            });
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
            draw_calls_2d: gpu_timing.draw_calls_2d,
            draw_calls_3d: gpu_timing.draw_calls_3d,
            draw_instances_3d: self.retained_draws_cache.iter().fold(0u32, |acc, draw| {
                acc.saturating_add(draw_instance_count(draw))
            }),
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

#[cfg(test)]
#[path = "../tests/unit/backend_tests.rs"]
mod tests;

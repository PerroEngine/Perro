use crate::{
    gpu::{
        DIRTY_2D, DIRTY_3D, DIRTY_ACCESSIBILITY, DIRTY_CAMERA_2D, DIRTY_CAMERA_3D, DIRTY_LIGHTS_3D,
        DIRTY_PARTICLES_3D, DIRTY_POSTFX, DIRTY_RESOURCES, Gpu, RenderFrame, RenderGpuTiming,
    },
    resources::ResourceStore,
    three_d::particles::renderer::Particles3DRenderer,
    three_d::renderer::Renderer3D,
    three_d::{gpu::validate_mesh_source, renderer::Draw3DInstance, renderer::Draw3DKind},
    two_d::renderer::Renderer2D,
};
use perro_ids::NodeID;
use perro_render_bridge::{
    Command2D, Command3D, PointParticles3DState, PostProcessingCommand, RenderBridge,
    RenderCommand, RenderEvent, ResourceCommand, Sprite2DCommand, VisualAccessibilityCommand,
};
use perro_structs::{PostProcessSet, VisualAccessibilitySettings};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::window::Window;

pub type StaticTextureLookup = fn(path: &str) -> Option<&'static [u8]>;
pub type StaticMeshLookup = fn(path: &str) -> Option<&'static [u8]>;
pub type StaticShaderLookup = fn(path: &str) -> Option<&'static str>;
const GC_INTERVAL_FRAMES: u32 = 4;

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
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DrawFrameTiming {
    pub process_commands: Duration,
    pub prepare_cpu: Duration,
    pub gpu_prepare_2d: Duration,
    pub gpu_prepare_3d: Duration,
    pub gpu_acquire: Duration,
    pub gpu_encode_main: Duration,
    pub gpu_submit_main: Duration,
    pub gpu_post_process: Duration,
    pub gpu_accessibility: Duration,
    pub gpu_present: Duration,
    pub gpu_total: Duration,
    pub total: Duration,
    pub idle_clear: bool,
}

#[derive(Default)]
struct FrameState {
    pending_commands: Vec<RenderCommand>,
}

impl FrameState {
    fn queue(&mut self, command: RenderCommand) {
        self.pending_commands.push(command);
    }
}

#[derive(Default)]
pub struct PerroGraphics {
    frame: FrameState,
    resources: ResourceStore,
    renderer_2d: Renderer2D,
    renderer_3d: Renderer3D,
    particles_3d: Particles3DRenderer,
    gpu: Option<Gpu>,
    events: Vec<RenderEvent>,
    viewport: (u32, u32),
    vsync_enabled: bool,
    smoothing_enabled: bool,
    smoothing_samples: u32,
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
    retained_sprites_cache: Vec<Sprite2DCommand>,
    global_post_processing: PostProcessSet,
    accessibility: VisualAccessibilitySettings,
    frame_index: u32,
}

impl PerroGraphics {
    pub fn new() -> Self {
        Self {
            frame: FrameState::default(),
            resources: ResourceStore::new(),
            renderer_2d: Renderer2D::new(),
            renderer_3d: Renderer3D::new(),
            particles_3d: Particles3DRenderer::new(),
            gpu: None,
            events: Vec::new(),
            viewport: (0, 0),
            vsync_enabled: true,
            smoothing_enabled: true,
            smoothing_samples: 4,
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
            retained_sprites_cache: Vec::new(),
            global_post_processing: PostProcessSet::new(),
            accessibility: VisualAccessibilitySettings::default(),
            frame_index: 0,
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
                        self.events.push(RenderEvent::MeshCreated {
                            request,
                            id: out_id,
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
                        self.resources.set_runtime_mesh_data(source.as_str(), mesh);
                        self.events.push(RenderEvent::MeshCreated {
                            request,
                            id: out_id,
                        });
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
                },
                RenderCommand::ThreeD(cmd_3d) => match *cmd_3d {
                    Command3D::Draw {
                        mesh,
                        material,
                        node,
                        model,
                        skeleton,
                    } => {
                        self.renderer_3d
                            .queue_draw(node, mesh, material, model, skeleton);
                    }
                    Command3D::DrawTerrain { node, model } => {
                        self.renderer_3d.queue_terrain(node, model);
                    }
                    Command3D::DrawDebugPoint3D {
                        node,
                        position,
                        size,
                    } => {
                        self.renderer_3d.queue_debug_point(node, position, size);
                    }
                    Command3D::DrawDebugLine3D {
                        node,
                        start,
                        end,
                        thickness,
                    } => {
                        self.renderer_3d
                            .queue_debug_line(node, start, end, thickness);
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
    }

    fn submit_many<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.frame.pending_commands.extend(commands);
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
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.viewport = (width, height);
        self.renderer_2d.set_viewport(width, height);
        if let Some(gpu) = &mut self.gpu {
            gpu.resize(width.max(1), height.max(1));
        }
    }

    fn set_smoothing(&mut self, enabled: bool) {
        self.smoothing_enabled = enabled;
        self.smoothing_samples = if enabled { 4 } else { 1 };
        if let Some(gpu) = &mut self.gpu {
            gpu.set_smoothing_samples(self.smoothing_samples);
        }
    }

    fn set_smoothing_samples(&mut self, samples: u32) {
        self.smoothing_samples = samples;
        self.smoothing_enabled = samples > 1;
        if let Some(gpu) = &mut self.gpu {
            gpu.set_smoothing_samples(samples);
        }
    }

    fn draw_frame(&mut self) {
        let _ = self.draw_frame_timed();
    }

    fn draw_frame_timed(&mut self) -> Option<DrawFrameTiming> {
        let total_start = Instant::now();
        let has_pending = !self.frame.pending_commands.is_empty();
        let has_retained_scene = self.renderer_2d.retained_sprite_count() > 0
            || !self.renderer_2d.retained_rects().is_empty()
            || self.renderer_3d.retained_draw_count() > 0
            || self.renderer_3d.has_retained_non_draw_state()
            || self
                .particles_3d
                .retained_point_particles()
                .next()
                .is_some();
        if !has_pending && !has_retained_scene {
            if let Some(gpu) = &mut self.gpu {
                gpu.render_idle_clear();
            }
            return Some(DrawFrameTiming {
                total: total_start.elapsed(),
                idle_clear: true,
                ..DrawFrameTiming::default()
            });
        }
        let mut pending = Vec::new();
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
                RenderCommand::ThreeD(cmd_3d) => {
                    frame_dirty_bits |= DIRTY_3D;
                    match &**cmd_3d {
                        Command3D::SetCamera { .. } => frame_dirty_bits |= DIRTY_CAMERA_3D,
                        Command3D::SetAmbientLight { .. }
                        | Command3D::SetSky { .. }
                        | Command3D::SetRayLight { .. }
                        | Command3D::SetPointLight { .. }
                        | Command3D::SetSpotLight { .. } => frame_dirty_bits |= DIRTY_LIGHTS_3D,
                        Command3D::UpsertPointParticles { .. } => {
                            frame_dirty_bits |= DIRTY_PARTICLES_3D
                        }
                        _ => {}
                    }
                }
                RenderCommand::Resource(_) => frame_dirty_bits |= DIRTY_RESOURCES,
                RenderCommand::PostProcessing(_) => frame_dirty_bits |= DIRTY_POSTFX,
                RenderCommand::VisualAccessibility(_) => frame_dirty_bits |= DIRTY_ACCESSIBILITY,
            }
        }
        let process_start = Instant::now();
        self.process_commands(pending.drain(..));
        let process_commands = process_start.elapsed();
        std::mem::swap(&mut pending, &mut self.frame.pending_commands);
        let prepare_start = Instant::now();
        let (camera_2d, _stats, upload) = self.renderer_2d.prepare_frame(&self.resources);
        let camera_2d_state = self.renderer_2d.camera();
        let (camera_3d, _stats_3d, lighting_3d) = self.renderer_3d.prepare_frame(&self.resources);
        self.particles_3d.prepare_frame();
        let draws_revision = self.renderer_3d.draw_revision();
        if draws_revision != self.retained_draws_cache_revision {
            self.retained_draws_cache.clear();
            self.retained_draws_cache
                .extend(self.renderer_3d.all_draws());
            self.retained_draws_cache
                .sort_unstable_by_key(|draw| draw.node.as_u64());
            self.retained_draws_cache_revision = draws_revision;
        }
        self.retained_point_particles_cache.clear();
        self.retained_point_particles_cache
            .extend(self.particles_3d.retained_point_particles());
        self.retained_point_particles_cache
            .sort_unstable_by_key(|(node, _)| node.as_u64());
        self.retained_sprites_cache.clear();
        self.retained_sprites_cache
            .extend(self.renderer_2d.retained_sprites());
        self.resources.reset_ref_counts();
        for sprite in &self.retained_sprites_cache {
            self.resources.mark_texture_used(sprite.texture);
        }
        for draw in &self.retained_draws_cache {
            if let Draw3DKind::Mesh(mesh) = draw.kind {
                self.resources.mark_mesh_used(mesh);
            }
            if let Some(material) = draw.material {
                self.resources.mark_material_used(material);
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
                camera_2d,
                post_processing_2d: camera_2d_state.post_processing.clone(),
                post_processing_global: self.global_post_processing.as_slice().into(),
                accessibility: self.accessibility,
                rects_2d: self.renderer_2d.retained_rects(),
                upload_2d: &upload,
                sprites_2d: &self.retained_sprites_cache,
                frame_dirty_bits,
                static_texture_lookup: self.static_texture_lookup,
                static_mesh_lookup: self.static_mesh_lookup,
                static_shader_lookup: self.static_shader_lookup,
            });
        }
        Some(DrawFrameTiming {
            process_commands,
            prepare_cpu,
            gpu_prepare_2d: gpu_timing.prepare_2d,
            gpu_prepare_3d: gpu_timing.prepare_3d,
            gpu_acquire: gpu_timing.acquire,
            gpu_encode_main: gpu_timing.encode_main,
            gpu_submit_main: gpu_timing.submit_main,
            gpu_post_process: gpu_timing.post_process,
            gpu_accessibility: gpu_timing.accessibility,
            gpu_present: gpu_timing.present,
            gpu_total: gpu_timing.total,
            total: total_start.elapsed(),
            idle_clear: false,
        })
    }
}

#[cfg(test)]
#[path = "../tests/unit/backend_tests.rs"]
mod tests;

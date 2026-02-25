use crate::{
    gpu::{Gpu, RenderFrame},
    resources::ResourceStore,
    three_d::{gpu::validate_mesh_source, renderer::Draw3DInstance},
    three_d::renderer::Renderer3D,
    two_d::renderer::Renderer2D,
};
use perro_render_bridge::{
    Command2D, Command3D, RenderBridge, RenderCommand, RenderEvent, ResourceCommand,
    Sprite2DCommand,
};
use std::sync::Arc;
use winit::window::Window;

pub type StaticTextureLookup = fn(path: &str) -> Option<&'static [u8]>;
pub type StaticMeshLookup = fn(path: &str) -> Option<&'static [u8]>;
const GC_INTERVAL_FRAMES: u32 = 4;

pub trait GraphicsBackend: RenderBridge {
    fn attach_window(&mut self, window: Arc<Window>);
    fn resize(&mut self, width: u32, height: u32);
    fn set_smoothing(&mut self, enabled: bool);
    fn set_smoothing_samples(&mut self, samples: u32);

    fn draw_frame(&mut self);
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
    gpu: Option<Gpu>,
    events: Vec<RenderEvent>,
    viewport: (u32, u32),
    vsync_enabled: bool,
    smoothing_enabled: bool,
    smoothing_samples: u32,
    static_texture_lookup: Option<StaticTextureLookup>,
    static_mesh_lookup: Option<StaticMeshLookup>,
    meshlets_enabled: bool,
    dev_meshlets: bool,
    meshlet_debug_view: bool,
    retained_draws_cache: Vec<Draw3DInstance>,
    retained_sprites_cache: Vec<Sprite2DCommand>,
    frame_index: u32,
}

impl PerroGraphics {
    pub fn new() -> Self {
        Self {
            frame: FrameState::default(),
            resources: ResourceStore::new(),
            renderer_2d: Renderer2D::new(),
            renderer_3d: Renderer3D::new(),
            gpu: None,
            events: Vec::new(),
            viewport: (0, 0),
            vsync_enabled: false,
            smoothing_enabled: true,
            smoothing_samples: 4,
            static_texture_lookup: None,
            static_mesh_lookup: None,
            meshlets_enabled: false,
            dev_meshlets: false,
            meshlet_debug_view: false,
            retained_draws_cache: Vec::new(),
            retained_sprites_cache: Vec::new(),
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
                RenderCommand::ThreeD(cmd_3d) => match cmd_3d {
                    Command3D::Draw {
                        mesh,
                        material,
                        node,
                        model,
                    } => {
                        self.renderer_3d.queue_draw(node, mesh, material, model);
                    }
                    Command3D::SetCamera { camera } => {
                        self.renderer_3d.set_camera(camera);
                    }
                    Command3D::SetAmbientLight { node, light } => {
                        self.renderer_3d.set_ambient_light(node, light);
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
                    Command3D::RemoveNode { node } => {
                        self.renderer_3d.remove_node(node);
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
        let mut pending = Vec::new();
        std::mem::swap(&mut pending, &mut self.frame.pending_commands);
        self.process_commands(pending.drain(..));
        std::mem::swap(&mut pending, &mut self.frame.pending_commands);
        let (camera_2d, _stats, upload) = self.renderer_2d.prepare_frame(&self.resources);
        let (camera_3d, _stats_3d, lighting_3d) = self.renderer_3d.prepare_frame(&self.resources);
        self.retained_draws_cache.clear();
        self.retained_draws_cache
            .extend(self.renderer_3d.retained_draws());
        self.retained_draws_cache
            .sort_unstable_by_key(|draw| draw.node.as_u64());
        self.retained_sprites_cache.clear();
        self.retained_sprites_cache
            .extend(self.renderer_2d.retained_sprites());
        self.resources.reset_ref_counts();
        for sprite in &self.retained_sprites_cache {
            self.resources.mark_texture_used(sprite.texture);
        }
        for draw in &self.retained_draws_cache {
            self.resources.mark_mesh_used(draw.mesh);
            self.resources.mark_material_used(draw.material);
        }
        self.frame_index = self.frame_index.wrapping_add(1);
        if self.frame_index.is_multiple_of(GC_INTERVAL_FRAMES) {
            self.resources
                .gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
        }

        if let Some(gpu) = &mut self.gpu {
            gpu.render(RenderFrame {
                resources: &self.resources,
                camera_3d,
                lighting_3d: &lighting_3d,
                draws_3d: &self.retained_draws_cache,
                camera_2d,
                rects_2d: self.renderer_2d.retained_rects(),
                upload_2d: &upload,
                sprites_2d: &self.retained_sprites_cache,
                static_texture_lookup: self.static_texture_lookup,
                static_mesh_lookup: self.static_mesh_lookup,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PerroGraphics;
    use crate::backend::GraphicsBackend;
    use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
    use perro_render_bridge::{
        Camera3DState, Command2D, Command3D, Material3D, RenderBridge, RenderCommand,
        ResourceCommand, Sprite2DCommand,
    };

    #[test]
    fn sprite_texture_upsert_is_accepted_after_texture_creation() {
        let mut graphics = PerroGraphics::new();
        let request = perro_render_bridge::RenderRequestID::new(99);
        let node = NodeID::from_parts(1, 0);

        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateTexture {
            request,
            id: TextureID::nil(),
            source: "__default__".to_string(),
            reserved: false,
        }));
        graphics.draw_frame();

        let mut events = Vec::new();
        graphics.drain_events(&mut events);
        let created = events
            .into_iter()
            .find_map(|event| match event {
                perro_render_bridge::RenderEvent::TextureCreated { id, .. } => Some(id),
                _ => None,
            })
            .expect("texture creation event should exist");

        graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
            node,
            sprite: Sprite2DCommand {
                texture: created,
                model: [[1.0, 0.0, 10.0], [0.0, 1.0, 5.0], [0.0, 0.0, 1.0]],
                z_index: 2,
            },
        }));
        graphics.draw_frame();

        assert_eq!(
            graphics.renderer_2d.retained_sprite(node),
            Some(Sprite2DCommand {
                texture: created,
                model: [[1.0, 0.0, 10.0], [0.0, 1.0, 5.0], [0.0, 0.0, 1.0]],
                z_index: 2,
            })
        );
    }

    #[test]
    fn draw_3d_updates_retained_state_per_node() {
        let mut graphics = PerroGraphics::new();
        let node_a = NodeID::from_parts(10, 0);
        let node_b = NodeID::from_parts(11, 0);

        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
            request: perro_render_bridge::RenderRequestID::new(1001),
            id: MeshID::nil(),
            source: "__cube__".to_string(),
            reserved: false,
        }));
        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
            request: perro_render_bridge::RenderRequestID::new(1002),
            id: MaterialID::nil(),
            material: Material3D::default(),
            source: None,
            reserved: false,
        }));
        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
            request: perro_render_bridge::RenderRequestID::new(1003),
            id: MeshID::nil(),
            source: "__sphere__".to_string(),
            reserved: false,
        }));
        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
            request: perro_render_bridge::RenderRequestID::new(1004),
            id: MaterialID::nil(),
            material: Material3D::default(),
            source: None,
            reserved: false,
        }));
        graphics.draw_frame();

        let mut events = Vec::new();
        graphics.drain_events(&mut events);
        let mut created_meshes = Vec::new();
        let mut created_materials = Vec::new();
        for event in events {
            match event {
                perro_render_bridge::RenderEvent::MeshCreated { id, .. } => created_meshes.push(id),
                perro_render_bridge::RenderEvent::MaterialCreated { id, .. } => {
                    created_materials.push(id)
                }
                _ => {}
            }
        }
        assert_eq!(created_meshes.len(), 2);
        assert_eq!(created_materials.len(), 2);

        let model_a = [
            [1.0, 0.0, 0.0, 2.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let model_b = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 3.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];

        graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
            mesh: created_meshes[0],
            material: created_materials[0],
            node: node_a,
            model: model_a,
        }));
        graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
            mesh: created_meshes[1],
            material: created_materials[1],
            node: node_b,
            model: model_b,
        }));
        graphics.draw_frame();

        assert_eq!(graphics.renderer_3d.retained_draw_count(), 2);
        assert_eq!(
            graphics.renderer_3d.retained_draw(node_a),
            Some(crate::three_d::renderer::Draw3DInstance {
                node: node_a,
                mesh: created_meshes[0],
                material: created_materials[0],
                model: model_a,
            })
        );
        assert_eq!(
            graphics.renderer_3d.retained_draw(node_b),
            Some(crate::three_d::renderer::Draw3DInstance {
                node: node_b,
                mesh: created_meshes[1],
                material: created_materials[1],
                model: model_b,
            })
        );
    }

    #[test]
    fn rejected_3d_draw_keeps_previous_retained_binding() {
        let mut graphics = PerroGraphics::new();
        let node = NodeID::from_parts(20, 0);

        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
            request: perro_render_bridge::RenderRequestID::new(2001),
            id: MeshID::nil(),
            source: "__cube__".to_string(),
            reserved: false,
        }));
        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
            request: perro_render_bridge::RenderRequestID::new(2002),
            id: MaterialID::nil(),
            material: Material3D::default(),
            source: None,
            reserved: false,
        }));
        graphics.draw_frame();

        let mut events = Vec::new();
        graphics.drain_events(&mut events);
        let mut mesh_id = MeshID::nil();
        let mut material_id = MaterialID::nil();
        for event in events {
            match event {
                perro_render_bridge::RenderEvent::MeshCreated { id, .. } => mesh_id = id,
                perro_render_bridge::RenderEvent::MaterialCreated { id, .. } => material_id = id,
                _ => {}
            }
        }
        assert!(!mesh_id.is_nil());
        assert!(!material_id.is_nil());

        let first_model = [
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
            mesh: mesh_id,
            material: material_id,
            node,
            model: first_model,
        }));
        graphics.draw_frame();
        assert_eq!(
            graphics.renderer_3d.retained_draw(node),
            Some(crate::three_d::renderer::Draw3DInstance {
                node,
                mesh: mesh_id,
                material: material_id,
                model: first_model,
            })
        );

        let missing_mesh = MeshID::from_parts(999_999, 0);
        let second_model = [
            [1.0, 0.0, 0.0, 2.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
            mesh: missing_mesh,
            material: material_id,
            node,
            model: second_model,
        }));
        graphics.draw_frame();

        assert_eq!(
            graphics.renderer_3d.retained_draw(node),
            Some(crate::three_d::renderer::Draw3DInstance {
                node,
                mesh: mesh_id,
                material: material_id,
                model: second_model,
            })
        );
    }

    #[test]
    fn rejected_3d_material_swap_keeps_previous_material_binding() {
        let mut graphics = PerroGraphics::new();
        let node = NodeID::from_parts(21, 0);

        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
            request: perro_render_bridge::RenderRequestID::new(2101),
            id: MeshID::nil(),
            source: "__cube__".to_string(),
            reserved: false,
        }));
        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
            request: perro_render_bridge::RenderRequestID::new(2102),
            id: MaterialID::nil(),
            material: Material3D::default(),
            source: None,
            reserved: false,
        }));
        graphics.draw_frame();

        let mut events = Vec::new();
        graphics.drain_events(&mut events);
        let mut mesh_id = MeshID::nil();
        let mut material_id = MaterialID::nil();
        for event in events {
            match event {
                perro_render_bridge::RenderEvent::MeshCreated { id, .. } => mesh_id = id,
                perro_render_bridge::RenderEvent::MaterialCreated { id, .. } => material_id = id,
                _ => {}
            }
        }
        assert!(!mesh_id.is_nil());
        assert!(!material_id.is_nil());

        let first_model = [
            [1.0, 0.0, 0.0, 0.5],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
            mesh: mesh_id,
            material: material_id,
            node,
            model: first_model,
        }));
        graphics.draw_frame();

        let missing_material = MaterialID::from_parts(999_998, 0);
        let second_model = [
            [1.0, 0.0, 0.0, 1.5],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
            mesh: mesh_id,
            material: missing_material,
            node,
            model: second_model,
        }));
        graphics.draw_frame();

        assert_eq!(
            graphics.renderer_3d.retained_draw(node),
            Some(crate::three_d::renderer::Draw3DInstance {
                node,
                mesh: mesh_id,
                material: material_id,
                model: second_model,
            })
        );
    }

    #[test]
    fn set_camera_3d_updates_retained_camera_state() {
        let mut graphics = PerroGraphics::new();
        graphics.submit(RenderCommand::ThreeD(Command3D::SetCamera {
            camera: Camera3DState {
                position: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.5, 0.0, 0.8660254],
                zoom: 1.25,
            },
        }));
        graphics.draw_frame();

        assert_eq!(
            graphics.renderer_3d.camera(),
            Camera3DState {
                position: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.5, 0.0, 0.8660254],
                zoom: 1.25,
            }
        );
    }

    #[test]
    fn rejected_sprite_texture_does_not_update_retained_binding() {
        let mut graphics = PerroGraphics::new();
        let node = NodeID::from_parts(2, 0);
        let missing = TextureID::from_parts(999, 0);

        graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
            node,
            sprite: Sprite2DCommand {
                texture: missing,
                model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                z_index: 0,
            },
        }));
        graphics.draw_frame();

        assert_eq!(graphics.renderer_2d.retained_sprite(node), None);
    }

    #[test]
    fn rejected_sprite_texture_swap_keeps_previous_texture_binding() {
        let mut graphics = PerroGraphics::new();
        let request = perro_render_bridge::RenderRequestID::new(3001);
        let node = NodeID::from_parts(3, 0);

        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateTexture {
            request,
            id: TextureID::nil(),
            source: "__default__".to_string(),
            reserved: false,
        }));
        graphics.draw_frame();

        let mut events = Vec::new();
        graphics.drain_events(&mut events);
        let texture = events
            .into_iter()
            .find_map(|event| match event {
                perro_render_bridge::RenderEvent::TextureCreated { id, .. } => Some(id),
                _ => None,
            })
            .expect("texture creation event should exist");

        let first_model = [[1.0, 0.0, 2.0], [0.0, 1.0, 3.0], [0.0, 0.0, 1.0]];
        graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
            node,
            sprite: Sprite2DCommand {
                texture,
                model: first_model,
                z_index: 1,
            },
        }));
        graphics.draw_frame();

        let missing_texture = TextureID::from_parts(999_997, 0);
        let second_model = [[1.0, 0.0, 9.0], [0.0, 1.0, 4.0], [0.0, 0.0, 1.0]];
        graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
            node,
            sprite: Sprite2DCommand {
                texture: missing_texture,
                model: second_model,
                z_index: 7,
            },
        }));
        graphics.draw_frame();

        assert_eq!(
            graphics.renderer_2d.retained_sprite(node),
            Some(Sprite2DCommand {
                texture,
                model: second_model,
                z_index: 7,
            })
        );
    }
}

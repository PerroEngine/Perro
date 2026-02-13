use crate::{
    gpu::Gpu,
    resources::ResourceStore,
    three_d::renderer::Renderer3D,
    two_d::renderer::Renderer2D,
};
use perro_render_bridge::{
    Command2D, Command3D, RenderBridge, RenderCommand, RenderEvent, ResourceCommand,
};
use std::sync::Arc;
use winit::window::Window;

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

    fn take_pending(&mut self) -> Vec<RenderCommand> {
        std::mem::take(&mut self.pending_commands)
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
    smoothing_enabled: bool,
    smoothing_samples: u32,
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
            smoothing_enabled: true,
            smoothing_samples: 4,
        }
    }

    fn process_commands(&mut self, commands: Vec<RenderCommand>) {
        for command in commands {
            match command {
                RenderCommand::Resource(resource_cmd) => match resource_cmd {
                    ResourceCommand::CreateMesh { request, .. } => {
                        let id = self.resources.create_mesh();
                        self.events.push(RenderEvent::MeshCreated { request, id });
                    }
                    ResourceCommand::CreateTexture { request, .. } => {
                        let id = self.resources.create_texture();
                        self.events
                            .push(RenderEvent::TextureCreated { request, id });
                    }
                    ResourceCommand::CreateMaterial { request, .. } => {
                        let id = self.resources.create_material();
                        self.events
                            .push(RenderEvent::MaterialCreated { request, id });
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
            let mut gpu = Gpu::new(window, self.smoothing_samples);
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
        let commands = self.frame.take_pending();
        self.process_commands(commands);
        let (camera_2d, _stats, upload) = self
            .renderer_2d
            .prepare_frame(&self.resources);
        let (camera_3d, _stats_3d) = self.renderer_3d.prepare_frame(&self.resources);
        let draws_3d: Vec<_> = self.renderer_3d.retained_draws().collect();

        if let Some(gpu) = &mut self.gpu {
            gpu.render(
                camera_3d,
                &draws_3d,
                camera_2d,
                self.renderer_2d.retained_rects(),
                &upload,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PerroGraphics;
    use crate::backend::GraphicsBackend;
    use perro_ids::{NodeID, TextureID};
    use perro_render_bridge::{Camera3DState, Command2D, Command3D, RenderBridge, RenderCommand, ResourceCommand, Sprite2DCommand};

    #[test]
    fn sprite_texture_upsert_is_accepted_after_texture_creation() {
        let mut graphics = PerroGraphics::new();
        let request = perro_render_bridge::RenderRequestID::new(99);
        let node = NodeID::from_parts(1, 0);

        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateTexture {
            request,
            owner: node,
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
            owner: node_a,
        }));
        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
            request: perro_render_bridge::RenderRequestID::new(1002),
            owner: node_a,
        }));
        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
            request: perro_render_bridge::RenderRequestID::new(1003),
            owner: node_b,
        }));
        graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
            request: perro_render_bridge::RenderRequestID::new(1004),
            owner: node_b,
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
}

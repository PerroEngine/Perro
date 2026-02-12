use crate::{
    resources::ResourceStore,
    two_d::{
        gpu::Gpu2D,
        renderer::Renderer2D,
    },
};
use perro_render_bridge::{
    Command2D, Command3D, RenderBridge, RenderCommand, RenderEvent, ResourceCommand,
};
use std::sync::Arc;
use winit::window::Window;

pub trait GraphicsBackend: RenderBridge {
    fn attach_window(&mut self, window: Arc<Window>);
    fn resize(&mut self, width: u32, height: u32);

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
    gpu: Option<Gpu2D>,
    events: Vec<RenderEvent>,
    viewport: (u32, u32),
}

impl PerroGraphics {
    pub fn new() -> Self {
        Self {
            frame: FrameState::default(),
            resources: ResourceStore::new(),
            renderer_2d: Renderer2D::new(),
            gpu: None,
            events: Vec::new(),
            viewport: (0, 0),
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
                    Command2D::UpsertTexture { texture, node } => {
                        self.renderer_2d.queue_texture(node, texture);
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
                    Command3D::Draw { .. } => {
                        // 3D renderer is intentionally not active in the minimal backend yet.
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
            let mut gpu = Gpu2D::new(window);
            if let Some(gpu_ref) = gpu.as_mut() {
                let [vw, vh] = Gpu2D::virtual_size();
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

    fn draw_frame(&mut self) {
        let commands = self.frame.take_pending();
        self.process_commands(commands);
        let (camera, _stats, upload) = self
            .renderer_2d
            .prepare_frame(&self.resources);

        if let Some(gpu) = &mut self.gpu {
            gpu.render(camera, self.renderer_2d.retained_rects(), &upload);
        }
    }
}

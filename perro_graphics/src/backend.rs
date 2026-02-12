use crate::{renderer_2d::Renderer2D, resources::ResourceStore};
use perro_render_bridge::{RenderBridge, RenderCommand, RenderEvent};

pub trait GraphicsBackend: RenderBridge {
    #[inline]
    fn resize(&mut self, _width: u32, _height: u32) {}

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
    events: Vec<RenderEvent>,
    viewport: (u32, u32),
}

impl PerroGraphics {
    pub fn new() -> Self {
        Self {
            frame: FrameState::default(),
            resources: ResourceStore::new(),
            renderer_2d: Renderer2D::new(),
            events: Vec::new(),
            viewport: (0, 0),
        }
    }

    fn process_commands(&mut self, commands: Vec<RenderCommand>) {
        for command in commands {
            match command {
                RenderCommand::CreateMesh { request, .. } => {
                    let id = self.resources.create_mesh();
                    self.events.push(RenderEvent::MeshCreated { request, id });
                }
                RenderCommand::CreateTexture { request, .. } => {
                    let id = self.resources.create_texture();
                    self.events
                        .push(RenderEvent::TextureCreated { request, id });
                }
                RenderCommand::CreateMaterial { request, .. } => {
                    let id = self.resources.create_material();
                    self.events
                        .push(RenderEvent::MaterialCreated { request, id });
                }
                RenderCommand::Draw2DTexture {
                    texture,
                    node,
                } => {
                    self.renderer_2d.queue_texture(texture, node);
                }
                RenderCommand::Draw3D { .. } => {
                    // 3D renderer is intentionally not active in the minimal backend yet.
                }
            }
        }
    }
}

impl RenderBridge for PerroGraphics {
    fn submit(&mut self, command: RenderCommand) {
        self.frame.queue(command);
    }

    fn drain_events(&mut self, out: &mut Vec<RenderEvent>) {
        out.append(&mut self.events);
    }
}

impl GraphicsBackend for PerroGraphics {
    fn resize(&mut self, width: u32, height: u32) {
        self.viewport = (width, height);
    }

    fn draw_frame(&mut self) {
        let commands = self.frame.take_pending();
        self.process_commands(commands);
        let _stats = self.renderer_2d.flush(&self.resources);
    }
}

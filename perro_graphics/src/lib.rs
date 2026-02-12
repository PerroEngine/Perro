use perro_ids::{MaterialID, MeshID, TextureID};
use perro_render_bridge::{RenderBridge, RenderCommand, RenderEvent};

pub trait GraphicsBackend: RenderBridge {
    fn draw_frame(&mut self);
}

#[derive(Default)]
pub struct NullGraphics {
    next_mesh_index: u32,
    next_texture_index: u32,
    next_material_index: u32,
    events: Vec<RenderEvent>,
}

impl NullGraphics {
    pub fn new() -> Self {
        Self {
            next_mesh_index: 1,
            next_texture_index: 1,
            next_material_index: 1,
            events: Vec::new(),
        }
    }

    fn alloc_mesh(&mut self) -> MeshID {
        let id = MeshID::from_parts(self.next_mesh_index, 0);
        self.next_mesh_index = self.next_mesh_index.saturating_add(1);
        id
    }

    fn alloc_texture(&mut self) -> TextureID {
        let id = TextureID::from_parts(self.next_texture_index, 0);
        self.next_texture_index = self.next_texture_index.saturating_add(1);
        id
    }

    fn alloc_material(&mut self) -> MaterialID {
        let id = MaterialID::from_parts(self.next_material_index, 0);
        self.next_material_index = self.next_material_index.saturating_add(1);
        id
    }
}

impl RenderBridge for NullGraphics {
    fn submit(&mut self, command: RenderCommand) {
        match command {
            RenderCommand::CreateMesh { request, .. } => {
                let id = self.alloc_mesh();
                self.events.push(RenderEvent::MeshCreated { request, id });
            }
            RenderCommand::CreateTexture { request, .. } => {
                let id = self.alloc_texture();
                self.events.push(RenderEvent::TextureCreated { request, id });
            }
            RenderCommand::CreateMaterial { request, .. } => {
                let id = self.alloc_material();
                self.events.push(RenderEvent::MaterialCreated { request, id });
            }
            RenderCommand::Draw { .. } => {
                // Intentionally no-op in null backend.
            }
        }
    }

    fn drain_events(&mut self, out: &mut Vec<RenderEvent>) {
        out.append(&mut self.events);
    }
}

impl GraphicsBackend for NullGraphics {
    fn draw_frame(&mut self) {
        // Intentionally no-op in null backend.
    }
}

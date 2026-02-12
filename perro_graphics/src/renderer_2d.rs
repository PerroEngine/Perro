use crate::resources::ResourceStore;
use perro_ids::{NodeID, TextureID};

#[derive(Debug, Clone, Copy)]
struct DrawPacket {
    texture: TextureID,
    node: NodeID,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Renderer2DStats {
    pub accepted_draws: u32,
    pub rejected_draws: u32,
}

#[derive(Default)]
pub struct Renderer2D {
    queued_draws: Vec<DrawPacket>,
}

impl Renderer2D {
    pub fn new() -> Self {
        Self {
            queued_draws: Vec::new(),
        }
    }

    pub fn queue_texture(&mut self, texture: TextureID, node: NodeID) {
        self.queued_draws.push(DrawPacket { texture, node });
    }

    pub fn flush(&mut self, resources: &ResourceStore) -> Renderer2DStats {
        let mut stats = Renderer2DStats::default();
        for DrawPacket { texture, node } in self.queued_draws.drain(..) {
            let _owner = node;
            if resources.has_texture(texture) {
                stats.accepted_draws = stats.accepted_draws.saturating_add(1);
            } else {
                stats.rejected_draws = stats.rejected_draws.saturating_add(1);
            }
        }
        stats
    }
}

use crate::resources::ResourceStore;
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_render_bridge::Camera3DState;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Draw3DInstance {
    pub node: NodeID,
    pub mesh: MeshID,
    pub material: MaterialID,
    pub model: [[f32; 4]; 4],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Renderer3DStats {
    pub accepted_draws: u32,
    pub rejected_draws: u32,
}

pub struct Renderer3D {
    queued_draws: Vec<Draw3DInstance>,
    retained_draws: HashMap<NodeID, Draw3DInstance>,
    camera: Camera3DState,
}

impl Renderer3D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_camera(&mut self, camera: Camera3DState) {
        self.camera = camera;
    }

    pub fn queue_draw(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        material: MaterialID,
        model: [[f32; 4]; 4],
    ) {
        self.queued_draws.push(Draw3DInstance {
            node,
            mesh,
            material,
            model,
        });
    }

    pub fn remove_node(&mut self, node: NodeID) {
        self.retained_draws.remove(&node);
    }

    pub fn prepare_frame(&mut self, resources: &ResourceStore) -> (Camera3DState, Renderer3DStats) {
        let mut stats = Renderer3DStats::default();
        for draw in self.queued_draws.drain(..) {
            if resources.has_mesh(draw.mesh) && resources.has_material(draw.material) {
                self.retained_draws.insert(draw.node, draw);
                stats.accepted_draws = stats.accepted_draws.saturating_add(1);
            } else {
                self.retained_draws.remove(&draw.node);
                stats.rejected_draws = stats.rejected_draws.saturating_add(1);
            }
        }
        (self.camera, stats)
    }

    pub fn retained_draw(&self, node: NodeID) -> Option<Draw3DInstance> {
        self.retained_draws.get(&node).copied()
    }

    pub fn retained_draw_count(&self) -> usize {
        self.retained_draws.len()
    }

    pub fn retained_draws(&self) -> impl Iterator<Item = Draw3DInstance> + '_ {
        self.retained_draws.values().copied()
    }

    pub fn camera(&self) -> Camera3DState {
        self.camera
    }
}

impl Default for Renderer3D {
    fn default() -> Self {
        Self {
            queued_draws: Vec::new(),
            retained_draws: HashMap::new(),
            // Keep a usable fallback view if no Camera3D node is active.
            camera: Camera3DState {
                position: [0.0, 0.0, 6.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                zoom: 1.0,
            },
        }
    }
}

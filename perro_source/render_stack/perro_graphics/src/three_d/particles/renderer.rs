use perro_ids::NodeID;
use perro_render_bridge::PointParticles3DState;
use std::collections::HashMap;

#[derive(Default)]
pub struct Particles3DRenderer {
    queued_points: Vec<(NodeID, PointParticles3DState)>,
    retained_points: HashMap<NodeID, PointParticles3DState>,
}

impl Particles3DRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue_point_particles(&mut self, node: NodeID, particles: PointParticles3DState) {
        self.queued_points.push((node, particles));
    }

    pub fn remove_node(&mut self, node: NodeID) {
        self.retained_points.remove(&node);
    }

    pub fn prepare_frame(&mut self) {
        for (node, particles) in self.queued_points.drain(..) {
            self.retained_points.insert(node, particles);
        }
    }

    pub fn retained_point_particles(
        &self,
    ) -> impl Iterator<Item = (NodeID, PointParticles3DState)> + '_ {
        self.retained_points
            .iter()
            .map(|(node, particles)| (*node, particles.clone()))
    }
}

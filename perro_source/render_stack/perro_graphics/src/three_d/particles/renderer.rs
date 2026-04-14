use perro_ids::NodeID;
use perro_render_bridge::PointParticles3DState;
use std::collections::HashMap;

#[derive(Default)]
pub struct Particles3DRenderer {
    queued_points: Vec<(NodeID, PointParticles3DState)>,
    retained_points: HashMap<NodeID, PointParticles3DState>,
    retained_points_revision: u64,
}

impl Particles3DRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue_point_particles(&mut self, node: NodeID, particles: PointParticles3DState) {
        self.queued_points.push((node, particles));
    }

    pub fn remove_node(&mut self, node: NodeID) {
        if self.retained_points.remove(&node).is_some() {
            self.retained_points_revision = self.retained_points_revision.wrapping_add(1);
        }
    }

    pub fn prepare_frame(&mut self) {
        let mut changed = false;
        for (node, particles) in self.queued_points.drain(..) {
            if self.retained_points.get(&node) != Some(&particles) {
                self.retained_points.insert(node, particles);
                changed = true;
            }
        }
        if changed {
            self.retained_points_revision = self.retained_points_revision.wrapping_add(1);
        }
    }

    pub fn retained_point_particles(
        &self,
    ) -> impl Iterator<Item = (NodeID, PointParticles3DState)> + '_ {
        self.retained_points
            .iter()
            .map(|(node, particles)| (*node, particles.clone()))
    }

    #[inline]
    pub fn retained_point_particles_revision(&self) -> u64 {
        self.retained_points_revision
    }
}

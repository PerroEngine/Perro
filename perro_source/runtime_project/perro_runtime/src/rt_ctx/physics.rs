use perro_ids::NodeID;
use perro_nodes::SceneNodeData;
use perro_runtime_context::sub_apis::PhysicsAPI;
use perro_structs::{Vector2, Vector3};

use crate::Runtime;

impl PhysicsAPI for Runtime {
    fn apply_force_2d(&mut self, body_id: NodeID, force: Vector2) -> bool {
        let Some(node) = self.nodes.get(body_id) else {
            return false;
        };
        if !matches!(node.data, SceneNodeData::RigidBody2D(_)) {
            return false;
        }
        self.queue_force_2d(body_id, force);
        true
    }

    fn apply_force_3d(&mut self, body_id: NodeID, force: Vector3) -> bool {
        let Some(node) = self.nodes.get(body_id) else {
            return false;
        };
        if !matches!(node.data, SceneNodeData::RigidBody3D(_)) {
            return false;
        }
        self.queue_force_3d(body_id, force);
        true
    }

    fn apply_impulse_2d(&mut self, body_id: NodeID, impulse: Vector2) -> bool {
        let Some(node) = self.nodes.get(body_id) else {
            return false;
        };
        if !matches!(node.data, SceneNodeData::RigidBody2D(_)) {
            return false;
        }
        self.queue_impulse_2d(body_id, impulse);
        true
    }

    fn apply_impulse_3d(&mut self, body_id: NodeID, impulse: Vector3) -> bool {
        let Some(node) = self.nodes.get(body_id) else {
            return false;
        };
        if !matches!(node.data, SceneNodeData::RigidBody3D(_)) {
            return false;
        }
        self.queue_impulse_3d(body_id, impulse);
        true
    }
}

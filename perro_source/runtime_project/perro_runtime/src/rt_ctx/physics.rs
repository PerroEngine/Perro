use perro_ids::NodeID;
use perro_nodes::SceneNodeData;
use perro_runtime_context::sub_apis::PhysicsAPI;
use perro_structs::{Vector2, Vector3};

use crate::Runtime;

impl PhysicsAPI for Runtime {
    fn apply_force_2d(&mut self, body_id: NodeID, direction: Vector2, amount: f32) -> bool {
        let Some(node) = self.nodes.get(body_id) else {
            return false;
        };
        if !matches!(node.data, SceneNodeData::RigidBody2D(_)) {
            return false;
        }
        self.queue_force_2d(body_id, direction, amount);
        true
    }

    fn apply_force_3d(&mut self, body_id: NodeID, direction: Vector3, amount: f32) -> bool {
        let Some(node) = self.nodes.get(body_id) else {
            return false;
        };
        if !matches!(node.data, SceneNodeData::RigidBody3D(_)) {
            return false;
        }
        self.queue_force_3d(body_id, direction, amount);
        true
    }

    fn apply_impulse_2d(&mut self, body_id: NodeID, direction: Vector2, amount: f32) -> bool {
        let Some(node) = self.nodes.get(body_id) else {
            return false;
        };
        if !matches!(node.data, SceneNodeData::RigidBody2D(_)) {
            return false;
        }
        self.queue_impulse_2d(body_id, direction, amount);
        true
    }

    fn apply_impulse_3d(&mut self, body_id: NodeID, direction: Vector3, amount: f32) -> bool {
        let Some(node) = self.nodes.get(body_id) else {
            return false;
        };
        if !matches!(node.data, SceneNodeData::RigidBody3D(_)) {
            return false;
        }
        self.queue_impulse_3d(body_id, direction, amount);
        true
    }
}

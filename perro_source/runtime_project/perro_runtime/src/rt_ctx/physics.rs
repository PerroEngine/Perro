use perro_ids::NodeID;
use perro_nodes::SceneNodeData;
use perro_runtime_context::sub_apis::{
    PhysicsAPI, PhysicsContact2D, PhysicsContact3D, PhysicsQueryFilter, PhysicsRayHit2D,
    PhysicsRayHit3D, PhysicsShapeHit2D, PhysicsShapeHit3D,
};
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

    fn raycast_3d(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        include_areas: bool,
    ) -> Option<PhysicsRayHit3D> {
        self.physics_raycast_3d(origin, direction, max_distance, include_areas)
    }

    fn raycast_2d(
        &mut self,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit2D> {
        self.physics_raycast_2d(origin, direction, max_distance, &filter)
    }

    fn shape_cast_2d(
        &mut self,
        shape: perro_nodes::Shape2D,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit2D> {
        self.physics_shape_cast_2d(shape, origin, direction, max_distance, &filter)
    }

    fn shape_cast_3d(
        &mut self,
        shape: perro_nodes::Shape3D,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit3D> {
        self.physics_shape_cast_3d(shape, origin, direction, max_distance, &filter)
    }

    fn contacts_2d(&mut self, body_id: NodeID) -> Vec<PhysicsContact2D> {
        self.physics_contacts_2d(body_id)
    }

    fn contacts_3d(&mut self, body_id: NodeID) -> Vec<PhysicsContact3D> {
        self.physics_contacts_3d(body_id)
    }

    fn physics_pause(&mut self, paused: bool) {
        self.set_physics_paused(paused);
    }

    fn physics_is_paused(&mut self) -> bool {
        self.physics_paused()
    }
}

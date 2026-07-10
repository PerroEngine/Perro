use perro_ids::NodeID;
use perro_nodes::{PhysicsForceEmitter2D, PhysicsForceEmitter3D, SceneNodeData};
use perro_runtime_api::sub_apis::{
    PhysicsAPI, PhysicsBodyPrediction2D, PhysicsBodyPrediction3D, PhysicsContact2D,
    PhysicsContact3D, PhysicsMoveResult2D, PhysicsMoveResult3D, PhysicsQueryFilter,
    PhysicsRayHit2D, PhysicsRayHit3D, PhysicsShapeHit2D, PhysicsShapeHit3D, PhysicsSlideResult2D,
    PhysicsSlideResult3D,
};
use perro_structs::{Quaternion, Vector2, Vector3};

use crate::Runtime;

impl PhysicsAPI for Runtime {
    fn get_gravity(&mut self) -> f32 {
        self.get_physics_gravity()
    }

    fn set_gravity(&mut self, gravity: f32) {
        self.set_physics_gravity(gravity);
    }

    fn get_body_gravity_scale(&mut self, body_id: NodeID) -> Option<f32> {
        match &self.nodes.get(body_id)?.data {
            SceneNodeData::RigidBody2D(body) => Some(body.gravity_scale),
            SceneNodeData::RigidBody3D(body) => Some(body.gravity_scale),
            _ => None,
        }
    }

    fn set_body_gravity_scale(&mut self, body_id: NodeID, scale: f32) -> bool {
        if !scale.is_finite() {
            return false;
        }
        let Some(mut node) = self.nodes.get_mut(body_id) else {
            return false;
        };
        match &mut node.data {
            SceneNodeData::RigidBody2D(body) => {
                body.gravity_scale = scale;
                true
            }
            SceneNodeData::RigidBody3D(body) => {
                body.gravity_scale = scale;
                true
            }
            _ => false,
        }
    }

    fn get_coefficient(&mut self) -> f32 {
        self.get_physics_coefficient()
    }

    fn set_coefficient(&mut self, coefficient: f32) {
        self.set_physics_coefficient(coefficient);
    }

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

    fn emit_force_2d(&mut self, emitter: PhysicsForceEmitter2D) -> bool {
        Runtime::emit_force_2d(self, emitter)
    }

    fn emit_force_3d(&mut self, emitter: PhysicsForceEmitter3D) -> bool {
        Runtime::emit_force_3d(self, emitter)
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

    fn raycast_3d_filtered(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit3D> {
        self.physics_raycast_3d_filtered(origin, direction, max_distance, &filter)
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

    fn move_body_2d(
        &mut self,
        body_id: NodeID,
        target: Vector2,
        margin: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult2D> {
        self.physics_move_body_2d(body_id, target, margin, &filter)
    }

    fn move_body_3d(
        &mut self,
        body_id: NodeID,
        target: Vector3,
        margin: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult3D> {
        self.physics_move_body_3d(body_id, target, margin, &filter)
    }

    fn move_and_slide_2d(
        &mut self,
        body_id: NodeID,
        motion: Vector2,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsSlideResult2D> {
        self.physics_move_and_slide_2d(body_id, motion, &filter)
    }

    fn move_and_slide_3d(
        &mut self,
        body_id: NodeID,
        motion: Vector3,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsSlideResult3D> {
        self.physics_move_and_slide_3d(body_id, motion, &filter)
    }

    fn apply_gravity_2d(
        &mut self,
        body_id: NodeID,
        dt: f32,
        max_fall_speed: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult2D> {
        self.physics_apply_gravity_2d(body_id, dt, max_fall_speed, &filter)
    }

    fn apply_gravity_3d(
        &mut self,
        body_id: NodeID,
        dt: f32,
        max_fall_speed: f32,
        filter: PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult3D> {
        self.physics_apply_gravity_3d(body_id, dt, max_fall_speed, &filter)
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

    fn predict_body_2d(
        &mut self,
        body_id: NodeID,
        time: f32,
        drift: Vector2,
    ) -> Option<PhysicsBodyPrediction2D> {
        if !time.is_finite() || time < 0.0 || !drift.x.is_finite() || !drift.y.is_finite() {
            return None;
        }

        let (velocity, angular_velocity, gravity_scale) = {
            let node = self.nodes.get(body_id)?;
            let SceneNodeData::RigidBody2D(body) = &node.data else {
                return None;
            };
            if !body.enabled {
                return None;
            }
            (
                body.linear_velocity,
                body.angular_velocity,
                body.gravity_scale,
            )
        };
        let transform = self.get_global_transform_2d(body_id)?;
        let position = transform.position;
        let rotation = transform.rotation;
        let gravity = Vector2::new(
            0.0,
            self.get_physics_gravity() * self.get_physics_coefficient(),
        ) * gravity_scale;
        Some(PhysicsBodyPrediction2D {
            position: position + (velocity + drift) * time + gravity * (0.5 * time * time),
            rotation: rotation + angular_velocity * time,
            velocity: velocity + drift + gravity * time,
            angular_velocity,
        })
    }

    fn predict_body_3d(
        &mut self,
        body_id: NodeID,
        time: f32,
        drift: Vector3,
    ) -> Option<PhysicsBodyPrediction3D> {
        if !time.is_finite()
            || time < 0.0
            || !drift.x.is_finite()
            || !drift.y.is_finite()
            || !drift.z.is_finite()
        {
            return None;
        }

        let (velocity, angular_velocity, gravity_scale) = {
            let node = self.nodes.get(body_id)?;
            let SceneNodeData::RigidBody3D(body) = &node.data else {
                return None;
            };
            if !body.enabled {
                return None;
            }
            (
                body.linear_velocity,
                body.angular_velocity,
                body.gravity_scale,
            )
        };
        let transform = self.get_global_transform_3d(body_id)?;
        let position = transform.position;
        let rotation = transform.rotation;
        let gravity = Vector3::new(
            0.0,
            self.get_physics_gravity() * self.get_physics_coefficient(),
            0.0,
        ) * gravity_scale;
        Some(PhysicsBodyPrediction3D {
            position: position + (velocity + drift) * time + gravity * (0.5 * time * time),
            rotation: predict_rotation_3d(rotation, angular_velocity, time),
            velocity: velocity + drift + gravity * time,
            angular_velocity,
        })
    }
}

fn predict_rotation_3d(rotation: Quaternion, angular_velocity: Vector3, time: f32) -> Quaternion {
    let speed = angular_velocity.length();
    if speed <= 1.0e-6 || time <= 0.0 {
        return rotation;
    }
    let axis = glam::Vec3::new(
        angular_velocity.x / speed,
        angular_velocity.y / speed,
        angular_velocity.z / speed,
    );
    let delta = glam::Quat::from_axis_angle(axis, speed * time);
    Quaternion::from_quat((delta * rotation.to_quat()).normalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_nodes::{RigidBody2D, RigidBody3D};
    use perro_runtime_api::sub_apis::{NodeAPI, PhysicsAPI};

    #[test]
    fn physics_api_sets_runtime_gravity_and_coefficient() {
        let mut runtime = Runtime::new();

        assert_eq!(PhysicsAPI::get_gravity(&mut runtime), -9.81);
        assert_eq!(PhysicsAPI::get_coefficient(&mut runtime), 1.0);

        PhysicsAPI::set_gravity(&mut runtime, -4.0);
        PhysicsAPI::set_coefficient(&mut runtime, 2.0);

        assert_eq!(PhysicsAPI::get_coefficient(&mut runtime), 2.0);
        assert_eq!(PhysicsAPI::get_gravity(&mut runtime), -4.0);
    }

    #[test]
    fn physics_api_sets_body_gravity_scale() {
        let mut runtime = Runtime::new();
        let body_2d = NodeAPI::create::<RigidBody2D>(&mut runtime);
        let body_3d = NodeAPI::create::<RigidBody3D>(&mut runtime);
        let invalid = NodeID::from_u32(999);

        assert_eq!(
            PhysicsAPI::get_body_gravity_scale(&mut runtime, body_2d),
            Some(1.0)
        );
        assert!(PhysicsAPI::set_body_gravity_scale(
            &mut runtime,
            body_2d,
            0.5
        ));
        assert!(PhysicsAPI::set_body_gravity_scale(
            &mut runtime,
            body_3d,
            0.25
        ));
        assert!(!PhysicsAPI::set_body_gravity_scale(
            &mut runtime,
            body_2d,
            f32::NAN
        ));
        assert!(!PhysicsAPI::set_body_gravity_scale(
            &mut runtime,
            invalid,
            0.5
        ));

        assert_eq!(
            PhysicsAPI::get_body_gravity_scale(&mut runtime, body_2d),
            Some(0.5)
        );
        assert_eq!(
            PhysicsAPI::get_body_gravity_scale(&mut runtime, body_3d),
            Some(0.25)
        );
        assert_eq!(
            PhysicsAPI::get_body_gravity_scale(&mut runtime, invalid),
            None
        );
    }

    #[test]
    fn physics_api_predicts_2d_body_without_mutating_state() {
        let mut runtime = Runtime::new();
        let id = NodeAPI::create::<RigidBody2D>(&mut runtime);
        let _ = NodeAPI::with_node_mut::<RigidBody2D, _, _>(&mut runtime, id, |body| {
            body.transform.position = Vector2::new(2.0, 3.0);
            body.transform.rotation = 0.25;
            body.linear_velocity = Vector2::new(4.0, 5.0);
            body.angular_velocity = 2.0;
            body.gravity_scale = 0.5;
        });
        PhysicsAPI::set_gravity(&mut runtime, -10.0);
        PhysicsAPI::set_coefficient(&mut runtime, 2.0);

        let predicted =
            PhysicsAPI::predict_body_2d(&mut runtime, id, 2.0, Vector2::new(1.0, 0.0)).unwrap();
        assert_eq!(predicted.position, Vector2::new(12.0, -7.0));
        assert_eq!(predicted.rotation, 4.25);
        assert_eq!(predicted.velocity, Vector2::new(5.0, -15.0));
        assert_eq!(predicted.angular_velocity, 2.0);
        assert_eq!(
            NodeAPI::get_global_transform_2d(&mut runtime, id)
                .unwrap()
                .position,
            Vector2::new(2.0, 3.0)
        );
    }

    #[test]
    fn physics_api_predicts_3d_body_without_mutating_state() {
        let mut runtime = Runtime::new();
        let id = NodeAPI::create::<RigidBody3D>(&mut runtime);
        let _ = NodeAPI::with_node_mut::<RigidBody3D, _, _>(&mut runtime, id, |body| {
            body.transform.position = Vector3::new(1.0, 2.0, 3.0);
            body.linear_velocity = Vector3::new(2.0, 3.0, 4.0);
            body.angular_velocity = Vector3::new(0.0, 0.0, std::f32::consts::FRAC_PI_2);
            body.gravity_scale = 1.0;
        });
        PhysicsAPI::set_gravity(&mut runtime, -10.0);

        let predicted =
            PhysicsAPI::predict_body_3d(&mut runtime, id, 1.5, Vector3::new(1.0, 0.0, -1.0))
                .unwrap();
        assert_eq!(predicted.position, Vector3::new(5.5, -4.75, 7.5));
        assert_eq!(predicted.velocity, Vector3::new(3.0, -12.0, 3.0));
        assert_eq!(
            predicted.angular_velocity,
            Vector3::new(0.0, 0.0, std::f32::consts::FRAC_PI_2)
        );
        let rotated_x = predicted
            .rotation
            .rotate_vector3(Vector3::new(1.0, 0.0, 0.0));
        assert!((rotated_x.x + std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-5);
        assert!((rotated_x.y - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-5);
        assert_eq!(
            NodeAPI::get_global_transform_3d(&mut runtime, id)
                .unwrap()
                .position,
            Vector3::new(1.0, 2.0, 3.0)
        );
    }
}

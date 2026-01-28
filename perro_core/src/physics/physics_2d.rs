//! 2D Physics system using Rapier2D

use rapier2d::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::ids::NodeID;
use crate::structs2d::Shape2D;

/// Manages the Rapier2D physics world
pub struct PhysicsWorld2D {
    /// The Rapier physics pipeline
    pub pipeline: PhysicsPipeline,
    /// The physics island manager
    pub islands: IslandManager,
    /// The broad phase collision detector
    pub broad_phase: BroadPhase,
    /// The narrow phase collision detector
    pub narrow_phase: NarrowPhase,
    /// The impulse joint set
    pub impulse_joints: ImpulseJointSet,
    /// The multibody joint set
    pub multibody_joints: MultibodyJointSet,
    /// The rigid body set
    pub bodies: RigidBodySet,
    /// The collider set
    pub colliders: ColliderSet,
    /// The query pipeline for spatial queries
    pub query_pipeline: QueryPipeline,
    /// CCD solver for continuous collision detection
    pub ccd_solver: CCDSolver,
    /// Map from node ID to collider handle
    pub node_to_collider: HashMap<NodeID, ColliderHandle>,
    /// Map from collider handle to node ID
    pub collider_to_node: HashMap<ColliderHandle, NodeID>,
    /// Map from Area2D node ID to its child collider handles
    pub area_to_colliders: HashMap<NodeID, Vec<ColliderHandle>>,
}

impl PhysicsWorld2D {
    pub fn new() -> Self {
        Self {
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            query_pipeline: QueryPipeline::new(),
            ccd_solver: CCDSolver::new(),
            node_to_collider: HashMap::new(),
            collider_to_node: HashMap::new(),
            area_to_colliders: HashMap::new(),
        }
    }

    /// Step the physics simulation
    pub fn step(&mut self, _dt: f32) {
        let gravity = vector![0.0, 9.81];
        let integration_parameters = IntegrationParameters::default();

        // Empty hooks and events for now
        struct EmptyHooks;
        impl PhysicsHooks for EmptyHooks {}
        
        struct EmptyEvents;
        impl EventHandler for EmptyEvents {
            fn handle_collision_event(
                &self,
                _bodies: &RigidBodySet,
                _colliders: &ColliderSet,
                _event: CollisionEvent,
                _contact_pair: Option<&ContactPair>,
            ) {
                // Do nothing
            }
        }

        let hooks = EmptyHooks;
        let events = EmptyEvents;

        self.pipeline.step(
            &gravity,
            &integration_parameters,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd_solver,
            &hooks,
            &events,
        );
        
        // Update query pipeline after physics step so spatial queries work correctly
        self.query_pipeline.update(&self.islands, &self.bodies, &self.colliders);
    }

    /// Create a sensor collider (for Area2D collision detection)
    /// Returns the collider handle
    pub fn create_sensor_collider(
        &mut self,
        node_id: NodeID,
        shape: Shape2D,
        position: [f32; 2],
        rotation: f32,
    ) -> ColliderHandle {
        let rapier_shape = shape_to_rapier_shape(shape);
        let collider = ColliderBuilder::new(rapier_shape)
            .sensor(true) // This makes it a sensor - detects collisions but doesn't have physics
            .translation(vector![position[0], position[1]])
            .rotation(rotation)
            .build();

        let handle = self.colliders.insert(collider);
        self.node_to_collider.insert(node_id, handle);
        self.collider_to_node.insert(handle, node_id);
        handle
    }

    /// Remove a collider
    pub fn remove_collider(&mut self, node_id: NodeID) {
        if let Some(handle) = self.node_to_collider.remove(&node_id) {
            self.collider_to_node.remove(&handle);
            let _ = self.colliders.remove(
                handle,
                &mut self.islands,
                &mut self.bodies,
                false,
            );
        }
    }

    /// Register a collider as a child of an Area2D
    pub fn register_area_collider(&mut self, area_id: NodeID, collider_handle: ColliderHandle) {
        self.area_to_colliders
            .entry(area_id)
            .or_insert_with(Vec::new)
            .push(collider_handle);
    }

    /// Get all colliders that are currently intersecting with any of the given collider handles
    /// Returns pairs of (our_collider_handle, intersecting_collider_handle)
    /// Uses direct intersection tests (works for sensors)
    /// Note: query_pipeline is updated in step(), so this can use &self
    pub fn get_intersecting_colliders(
        &self,
        collider_handles: &[ColliderHandle],
    ) -> Vec<(ColliderHandle, ColliderHandle)> {
        let mut intersections = Vec::new();
        
        // For each of our collider handles, check what it intersects with
        for &our_handle in collider_handles {
            if let Some(our_collider) = self.colliders.get(our_handle) {
                // Get the isometry (position + rotation) of our collider
                let our_isometry = our_collider.position();
                let our_shape = our_collider.shape();
                
                // Check against all other colliders
                for (other_handle, other_collider) in self.colliders.iter() {
                    // Skip if it's one of our own colliders
                    if collider_handles.contains(&other_handle) {
                        continue;
                    }
                    
                    // Get the isometry of the other collider
                    let other_isometry = other_collider.position();
                    let other_shape = other_collider.shape();
                    
                    // Use Rapier's intersection test
                    // contact() returns Result<Option<Contact>, Unsupported>
                    if let Ok(Some(_contact)) = rapier2d::parry::query::contact(
                        &our_isometry,
                        our_shape,
                        &other_isometry,
                        other_shape,
                        0.0, // prediction
                    ) {
                        intersections.push((our_handle, other_handle));
                    }
                }
            }
        }
        
        intersections
    }

    /// Update collider transform based on node transform
    pub fn update_collider_transform(
        &mut self,
        node_id: NodeID,
        position: [f32; 2],
        rotation: f32,
    ) {
        if let Some(&handle) = self.node_to_collider.get(&node_id) {
            if let Some(collider) = self.colliders.get_mut(handle) {
                collider.set_translation(vector![position[0], position[1]]);
                collider.set_rotation(rotation);
            }
        }
    }

    /// Get the node ID for a collider handle
    pub fn get_node_id(&self, collider_handle: ColliderHandle) -> Option<NodeID> {
        self.collider_to_node.get(&collider_handle).copied()
    }
}

impl Default for PhysicsWorld2D {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert Shape2D to Rapier shape
/// Only Rectangle and Circle are supported for physics (Square and Triangle are converted to Rectangle)
pub fn shape_to_rapier_shape(shape: Shape2D) -> SharedShape {
    match shape {
        Shape2D::Rectangle { width, height } => {
            SharedShape::cuboid(width / 2.0, height / 2.0)
        }
        Shape2D::Circle { radius } => SharedShape::ball(radius),
        Shape2D::Square { size } => {
            // Convert square to rectangle
            SharedShape::cuboid(size / 2.0, size / 2.0)
        }
        Shape2D::Triangle { base, height } => {
            // Convert triangle to rectangle (approximation)
            SharedShape::cuboid(base / 2.0, height / 2.0)
        }
    }
}

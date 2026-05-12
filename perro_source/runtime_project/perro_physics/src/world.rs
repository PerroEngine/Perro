use ahash::AHashMap;
use perro_ids::NodeID;

use crate::{BodyKind, r2, r3};

const MAX_CCD_SUBSTEPS: usize = 1;

#[derive(Clone, Debug)]
pub struct BodyState2D {
    pub handle: r2::RigidBodyHandle,
    pub colliders: Vec<r2::ColliderHandle>,
    pub kind: BodyKind,
    pub shape_signature: u64,
    pub opaque_handle: u64,
}

#[derive(Clone, Debug)]
pub struct BodyState3D {
    pub handle: r3::RigidBodyHandle,
    pub colliders: Vec<r3::ColliderHandle>,
    pub kind: BodyKind,
    pub shape_signature: u64,
    pub opaque_handle: u64,
}

#[derive(Clone, Debug)]
pub struct JointState2D {
    pub handle: r2::ImpulseJointHandle,
    pub signature: u64,
    pub sync_epoch: u64,
}

#[derive(Clone, Debug)]
pub struct JointState3D {
    pub handle: r3::ImpulseJointHandle,
    pub signature: u64,
    pub sync_epoch: u64,
}

pub struct PhysicsWorld2D {
    pub pipeline: r2::PhysicsPipeline,
    pub gravity: r2::Vector<f32>,
    pub integration_parameters: r2::IntegrationParameters,
    pub islands: r2::IslandManager,
    pub broad_phase: r2::DefaultBroadPhase,
    pub narrow_phase: r2::NarrowPhase,
    pub bodies: r2::RigidBodySet,
    pub colliders: r2::ColliderSet,
    pub query_pipeline: r2::QueryPipeline,
    pub impulse_joints: r2::ImpulseJointSet,
    pub multibody_joints: r2::MultibodyJointSet,
    pub ccd_solver: r2::CCDSolver,
    pub collider_owners: AHashMap<r2::ColliderHandle, NodeID>,
    pub body_map: AHashMap<NodeID, BodyState2D>,
    pub joint_map: AHashMap<NodeID, JointState2D>,
}

pub struct PhysicsWorld3D {
    pub pipeline: r3::PhysicsPipeline,
    pub gravity: r3::Vector<f32>,
    pub integration_parameters: r3::IntegrationParameters,
    pub islands: r3::IslandManager,
    pub broad_phase: r3::DefaultBroadPhase,
    pub narrow_phase: r3::NarrowPhase,
    pub bodies: r3::RigidBodySet,
    pub colliders: r3::ColliderSet,
    pub query_pipeline: r3::QueryPipeline,
    pub impulse_joints: r3::ImpulseJointSet,
    pub multibody_joints: r3::MultibodyJointSet,
    pub ccd_solver: r3::CCDSolver,
    pub collider_owners: AHashMap<r3::ColliderHandle, NodeID>,
    pub body_map: AHashMap<NodeID, BodyState3D>,
    pub joint_map: AHashMap<NodeID, JointState3D>,
}

impl PhysicsWorld2D {
    pub fn new() -> Self {
        let integration_parameters = r2::IntegrationParameters {
            max_ccd_substeps: MAX_CCD_SUBSTEPS,
            ..r2::IntegrationParameters::default()
        };
        Self {
            pipeline: r2::PhysicsPipeline::new(),
            gravity: crate::na2::Vector2::new(0.0, -9.81),
            integration_parameters,
            islands: r2::IslandManager::new(),
            broad_phase: r2::DefaultBroadPhase::new(),
            narrow_phase: r2::NarrowPhase::new(),
            bodies: r2::RigidBodySet::new(),
            colliders: r2::ColliderSet::new(),
            query_pipeline: r2::QueryPipeline::new(),
            impulse_joints: r2::ImpulseJointSet::new(),
            multibody_joints: r2::MultibodyJointSet::new(),
            ccd_solver: r2::CCDSolver::new(),
            collider_owners: AHashMap::default(),
            body_map: AHashMap::default(),
            joint_map: AHashMap::default(),
        }
    }
}

impl Default for PhysicsWorld2D {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsWorld3D {
    pub fn new() -> Self {
        let integration_parameters = r3::IntegrationParameters {
            max_ccd_substeps: MAX_CCD_SUBSTEPS,
            ..r3::IntegrationParameters::default()
        };
        Self {
            pipeline: r3::PhysicsPipeline::new(),
            gravity: crate::na3::Vector3::new(0.0, -9.81, 0.0),
            integration_parameters,
            islands: r3::IslandManager::new(),
            broad_phase: r3::DefaultBroadPhase::new(),
            narrow_phase: r3::NarrowPhase::new(),
            bodies: r3::RigidBodySet::new(),
            colliders: r3::ColliderSet::new(),
            query_pipeline: r3::QueryPipeline::new(),
            impulse_joints: r3::ImpulseJointSet::new(),
            multibody_joints: r3::MultibodyJointSet::new(),
            ccd_solver: r3::CCDSolver::new(),
            collider_owners: AHashMap::default(),
            body_map: AHashMap::default(),
            joint_map: AHashMap::default(),
        }
    }
}

impl Default for PhysicsWorld3D {
    fn default() -> Self {
        Self::new()
    }
}

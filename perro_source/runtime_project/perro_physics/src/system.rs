use ahash::{AHashMap, AHashSet};
use perro_ids::NodeID;
use perro_nodes::{Shape2D, Shape3D};
use perro_runtime_context::sub_apis::{
    PhysicsContact2D, PhysicsContact3D, PhysicsQueryFilter, PhysicsRayHit2D, PhysicsRayHit3D,
    PhysicsShapeHit2D, PhysicsShapeHit3D,
};
use perro_structs::{Vector2, Vector3};
use rayon::prelude::*;

use crate::{
    AreaOverlap, AudioRaycastInput, AudioRaycastResult, BodyPair, PendingForce2D, PendingForce3D,
    PendingImpulse2D, PendingImpulse3D, PhysicsAssetContext, PhysicsWorld2D, PhysicsWorld3D,
    TriMeshData, helpers::*, na2, na3, r2, r3,
};

const MAX_RIGID_SPEED_2D: f32 = 80.0;
const MAX_RIGID_SPEED_3D: f32 = 80.0;
const CCD_MIN_SPEED_RATIO_OF_MAX: f32 = 0.5;
const CCD_MIN_SPEED_SQ_2D: f32 = MAX_RIGID_SPEED_2D
    * CCD_MIN_SPEED_RATIO_OF_MAX
    * MAX_RIGID_SPEED_2D
    * CCD_MIN_SPEED_RATIO_OF_MAX;
const CCD_MIN_SPEED_SQ_3D: f32 = MAX_RIGID_SPEED_3D
    * CCD_MIN_SPEED_RATIO_OF_MAX
    * MAX_RIGID_SPEED_3D
    * CCD_MIN_SPEED_RATIO_OF_MAX;

pub struct PhysicsSystem {
    pub paused: bool,
    pub world_2d: Option<PhysicsWorld2D>,
    pub world_3d: Option<PhysicsWorld3D>,
    pub active_collision_pairs_2d: AHashSet<BodyPair>,
    pub active_collision_pairs_3d: AHashSet<BodyPair>,
    pub active_area_overlaps_2d: AHashSet<AreaOverlap>,
    pub active_area_overlaps_3d: AHashSet<AreaOverlap>,
    pub pending_forces_2d: Vec<PendingForce2D>,
    pub pending_forces_3d: Vec<PendingForce3D>,
    pub pending_impulses_2d: Vec<PendingImpulse2D>,
    pub pending_impulses_3d: Vec<PendingImpulse3D>,
    pub stale_ids_2d: Vec<NodeID>,
    pub stale_ids_3d: Vec<NodeID>,
    pub trimesh_cache: AHashMap<u64, TriMeshData>,
    pub next_opaque_handle: u64,
    pub signal_name_scratch: String,
}

impl PhysicsSystem {
    pub fn new() -> Self {
        Self {
            paused: false,
            world_2d: None,
            world_3d: None,
            active_collision_pairs_2d: AHashSet::default(),
            active_collision_pairs_3d: AHashSet::default(),
            active_area_overlaps_2d: AHashSet::default(),
            active_area_overlaps_3d: AHashSet::default(),
            pending_forces_2d: Vec::new(),
            pending_forces_3d: Vec::new(),
            pending_impulses_2d: Vec::new(),
            pending_impulses_3d: Vec::new(),
            stale_ids_2d: Vec::new(),
            stale_ids_3d: Vec::new(),
            trimesh_cache: AHashMap::default(),
            next_opaque_handle: 1,
            signal_name_scratch: String::new(),
        }
    }

    pub fn clear(&mut self) {
        self.world_2d = None;
        self.world_3d = None;
        self.active_collision_pairs_2d.clear();
        self.active_collision_pairs_3d.clear();
        self.active_area_overlaps_2d.clear();
        self.active_area_overlaps_3d.clear();
        self.pending_forces_2d.clear();
        self.pending_forces_3d.clear();
        self.pending_impulses_2d.clear();
        self.pending_impulses_3d.clear();
        self.stale_ids_2d.clear();
        self.stale_ids_3d.clear();
        self.trimesh_cache.clear();
        self.next_opaque_handle = 1;
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn queue_impulse_2d(&mut self, id: NodeID, impulse: Vector2) {
        self.pending_impulses_2d
            .push(PendingImpulse2D { id, impulse });
    }

    pub fn queue_force_2d(&mut self, id: NodeID, force: Vector2) {
        self.pending_forces_2d.push(PendingForce2D { id, force });
    }

    pub fn queue_impulse_3d(&mut self, id: NodeID, impulse: Vector3) {
        self.pending_impulses_3d
            .push(PendingImpulse3D { id, impulse });
    }

    pub fn queue_force_3d(&mut self, id: NodeID, force: Vector3) {
        self.pending_forces_3d.push(PendingForce3D { id, force });
    }

    pub fn alloc_opaque_handle(&mut self) -> u64 {
        let handle = self.next_opaque_handle;
        self.next_opaque_handle = self.next_opaque_handle.saturating_add(1);
        handle
    }
}

mod audio;
mod queries;
mod step;
mod sync;

impl Default for PhysicsSystem {
    fn default() -> Self {
        Self::new()
    }
}

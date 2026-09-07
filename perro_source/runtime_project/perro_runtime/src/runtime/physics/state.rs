//! Physics integration caches and reusable scratch; no solver ownership.
use super::*;

pub(crate) struct PhysicsSyncState {
    /// arena mutation revision @ last node->world sync; match + no dirty => skip re-sync
    pub(crate) physics_synced_node_revision_2d: Option<u64>,
    pub(crate) physics_synced_node_revision_3d: Option<u64>,
    pub(crate) physics_synced_world_revisions: AHashMap<NodeID, (Option<u64>, Option<u64>)>,
    pub(crate) physics_body_descs_2d: Vec<perro_physics::BodyDesc2D>,
    pub(crate) physics_body_descs_3d: Vec<perro_physics::BodyDesc3D>,
    pub(crate) physics_joint_descs_2d: Vec<perro_physics::JointDesc2D>,
    pub(crate) physics_joint_descs_3d: Vec<perro_physics::JointDesc3D>,
    /// internal fall speed per char body 4 script-invoked apply_gravity;
    /// ! exposed on node (char body has no velocity state)
    pub(crate) character_fall_speed_2d: AHashMap<NodeID, f32>,
    pub(crate) character_fall_speed_3d: AHashMap<NodeID, f32>,
    /// last sweep hit per char body (node, point, normal); merged -> contacts_*
    /// cuz kinematic-vs-fixed pairs never activate in solver narrow phase
    pub(crate) character_sweep_hit_2d:
        AHashMap<NodeID, (NodeID, perro_structs::Vector2, perro_structs::Vector2)>,
    pub(crate) character_sweep_hit_3d:
        AHashMap<NodeID, (NodeID, perro_structs::Vector3, perro_structs::Vector3)>,
    pub(crate) water_samples: AHashMap<NodeID, perro_nodes::WaterPhysicsSample>,
    pub(crate) water_sample_times: AHashMap<NodeID, f32>,
    pub(crate) water_body_samples: AHashMap<WaterBodySampleKey, WaterBodySampleCache>,
    pub(crate) pending_water_queries_2d: AHashMap<NodeID, Vec<PendingWaterQuery>>,
    pub(crate) pending_water_queries_3d: AHashMap<NodeID, Vec<PendingWaterQuery>>,
    pub(crate) water_contacts_2d: AHashMap<NodeID, Vec<WaterBodyContact2D>>,
    pub(crate) water_contacts_3d: AHashMap<NodeID, Vec<WaterBodyContact3D>>,
    /// Entry splash gate. Bodies must stay clear of water before another
    /// surface crossing counts as a foreign-body impact.
    pub(crate) water_entry_states_3d: AHashMap<NodeID, WaterEntryState3D>,
    pub(crate) water_rigid_body_ids_2d_cache: Vec<NodeID>,
    pub(crate) water_rigid_body_ids_3d_cache: Vec<NodeID>,
    pub(crate) water_collision_body_ids_2d_cache: Vec<NodeID>,
    pub(crate) water_collision_body_ids_3d_cache: Vec<NodeID>,
    pub(crate) water_ids_2d_cache: Vec<NodeID>,
    pub(crate) water_ids_3d_cache: Vec<NodeID>,
    /// `nodes.physics_revision()` snapshot @ last fill of each cache above.
    /// `None` means unfilled. Lets empty-result scenes cache too (`is_empty`
    /// used 2 be the unfilled sentinel, so 0-water scenes rescanned forever).
    pub(crate) water_rigid_body_ids_2d_cache_version: Option<u64>,
    pub(crate) water_rigid_body_ids_3d_cache_version: Option<u64>,
    pub(crate) water_collision_body_ids_2d_cache_version: Option<u64>,
    pub(crate) water_collision_body_ids_3d_cache_version: Option<u64>,
    pub(crate) water_ids_2d_cache_version: Option<u64>,
    pub(crate) water_ids_3d_cache_version: Option<u64>,
    pub(crate) force_water_impacts_2d: Vec<ForceWaterImpact2D>,
    pub(crate) force_water_impacts_3d: Vec<ForceWaterImpact3D>,
    pub(crate) pending_force_emitters_2d: Vec<(NodeID, perro_nodes::PhysicsForceEmitter2D)>,
    pub(crate) pending_force_emitters_3d: Vec<(NodeID, perro_nodes::PhysicsForceEmitter3D)>,
    /// reusable body-handle update buf 4 sync_world_2d/3d; avoid per-frame alloc.
    pub(crate) physics_handle_updates_scratch_2d: Vec<(NodeID, Option<u64>)>,
    pub(crate) physics_handle_updates_scratch_3d: Vec<(NodeID, Option<u64>)>,
    /// reusable staged-pose buf 4 sync_world_to_nodes_2d/3d writeback.
    pub(crate) physics_writeback_scratch_2d: Vec<physics::StagedBodyPose2D>,
    pub(crate) physics_writeback_scratch_3d: Vec<physics::StagedBodyPose3D>,
    /// reusable force-emitter stage buf 4 queue_physics_force_emitters_2d/3d;
    /// stage (pos, node id) only -- emitter data re-read in apply loop, no clone.
    pub(crate) physics_force_emitters_scratch_2d: Vec<(perro_structs::Vector2, NodeID)>,
    pub(crate) physics_force_emitters_scratch_3d: Vec<(perro_structs::Vector3, NodeID)>,
    /// reusable emitter-id scan buf 4 queue_physics_force_emitters_2d/3d;
    /// avoid per-step alloc on the type-lane scan result.
    pub(crate) physics_force_emitter_ids_scratch_2d: Vec<NodeID>,
    pub(crate) physics_force_emitter_ids_scratch_3d: Vec<NodeID>,
    /// reusable water-index input buf 4 queue_water_forces_2d/3d.
    pub(crate) physics_waters_scratch_2d: Vec<physics::RuntimeWater2D>,
    pub(crate) physics_waters_scratch_3d: Vec<physics::RuntimeWater3D>,
    /// reusable rigid-body sample buf 4 queue_water_forces_2d/3d.
    pub(crate) physics_water_bodies_scratch_2d: Vec<physics::RuntimeWaterBody2D>,
    pub(crate) physics_water_bodies_scratch_3d: Vec<physics::RuntimeWaterBody3D>,
    /// reusable water spatial-bin storage 4 queue_water_forces_2d/3d.
    pub(crate) physics_water_bins_scratch_2d: Vec<Vec<usize>>,
    pub(crate) physics_water_bins_scratch_3d: Vec<Vec<usize>>,
    /// reusable per-tick water force collect buf 4 queue_water_forces_2d/3d.
    pub(crate) physics_water_forces_scratch_2d: Vec<physics::WaterBodyForce2D>,
    pub(crate) physics_water_forces_scratch_3d: Vec<physics::WaterBodyForce3D>,
    /// memo 4 physics root inverse (world id, root mat, inv); kill per-body
    /// per-tileset fold of collision tiles: (parsed tileset, hash). parsed
    /// tilesets are immutable, so Rc identity gate the memo; kp the Rc so a
    /// reused address can't read as a hit. Entries self-drop when the tileset
    /// cache releases the tileset.
    pub(crate) tileset_collision_hash_cache_2d:
        AHashMap<u64, (Rc<render_2d::ParsedTileset2D>, u64)>,
    /// matrix inversion in physics_transform_2d/3d.
    pub(crate) physics_root_inv_2d: Option<(NodeID, glam::Mat3, glam::Mat3)>,
    pub(crate) physics_root_inv_3d: Option<(NodeID, glam::Mat4, glam::Mat4)>,
    /// reusable sorted world list 4 fixed-step dispatch + stale-world prune.
    pub(crate) physics_world_ids_scratch: Vec<NodeID>,
    /// `nodes.structural_revision()` @ last build of the list above. body/joint
    /// node sets + node_world both move only on structural chg, so a match let
    /// the fixed step reuse the list + skip the 2 stale-world retain passes.
    pub(crate) physics_world_ids_revision: Option<u64>,
}

impl PhysicsSyncState {
    pub(crate) fn new() -> Self {
        Self {
            physics_synced_node_revision_2d: None,
            physics_synced_node_revision_3d: None,
            physics_synced_world_revisions: AHashMap::new(),
            physics_body_descs_2d: Vec::new(),
            physics_body_descs_3d: Vec::new(),
            physics_joint_descs_2d: Vec::new(),
            physics_joint_descs_3d: Vec::new(),
            character_fall_speed_2d: AHashMap::new(),
            character_fall_speed_3d: AHashMap::new(),
            character_sweep_hit_2d: AHashMap::new(),
            character_sweep_hit_3d: AHashMap::new(),
            water_samples: AHashMap::new(),
            water_sample_times: AHashMap::new(),
            water_body_samples: AHashMap::new(),
            pending_water_queries_2d: AHashMap::new(),
            pending_water_queries_3d: AHashMap::new(),
            water_contacts_2d: AHashMap::new(),
            water_contacts_3d: AHashMap::new(),
            water_entry_states_3d: AHashMap::new(),
            water_rigid_body_ids_2d_cache: Vec::new(),
            water_rigid_body_ids_3d_cache: Vec::new(),
            water_collision_body_ids_2d_cache: Vec::new(),
            water_collision_body_ids_3d_cache: Vec::new(),
            water_ids_2d_cache: Vec::new(),
            water_ids_3d_cache: Vec::new(),
            water_rigid_body_ids_2d_cache_version: None,
            water_rigid_body_ids_3d_cache_version: None,
            water_collision_body_ids_2d_cache_version: None,
            water_collision_body_ids_3d_cache_version: None,
            water_ids_2d_cache_version: None,
            water_ids_3d_cache_version: None,
            force_water_impacts_2d: Vec::new(),
            force_water_impacts_3d: Vec::new(),
            pending_force_emitters_2d: Vec::new(),
            pending_force_emitters_3d: Vec::new(),
            physics_handle_updates_scratch_2d: Vec::new(),
            physics_handle_updates_scratch_3d: Vec::new(),
            physics_writeback_scratch_2d: Vec::new(),
            physics_writeback_scratch_3d: Vec::new(),
            physics_force_emitters_scratch_2d: Vec::new(),
            physics_force_emitters_scratch_3d: Vec::new(),
            physics_force_emitter_ids_scratch_2d: Vec::new(),
            physics_force_emitter_ids_scratch_3d: Vec::new(),
            physics_waters_scratch_2d: Vec::new(),
            physics_waters_scratch_3d: Vec::new(),
            physics_water_bodies_scratch_2d: Vec::new(),
            physics_water_bodies_scratch_3d: Vec::new(),
            physics_water_bins_scratch_2d: Vec::new(),
            physics_water_bins_scratch_3d: Vec::new(),
            physics_water_forces_scratch_2d: Vec::new(),
            physics_water_forces_scratch_3d: Vec::new(),
            tileset_collision_hash_cache_2d: AHashMap::new(),
            physics_root_inv_2d: None,
            physics_root_inv_3d: None,
            physics_world_ids_scratch: Vec::new(),
            physics_world_ids_revision: None,
        }
    }
}

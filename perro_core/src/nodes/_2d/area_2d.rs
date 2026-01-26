use std::ops::{Deref, DerefMut};

use crate::{
    api::ScriptApi,
    nodes::_2d::node_2d::Node2D,
    nodes::node_registry::{BaseNode, NodeType, SceneNode},
    prelude::string_to_u64,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashSet;
use crate::uid32::NodeID;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Area2D {
    #[serde(rename = "type")]
    pub ty: NodeType,


    pub base: Node2D,

    /// Track which colliders were intersecting in the previous frame
    /// Used to detect enter/exit events
    #[serde(skip)]
    pub previous_collisions: HashSet<NodeID>,
}

impl Default for Area2D {
    fn default() -> Self {
        Self {
            ty: NodeType::Area2D,
            base: Node2D::default(),
            previous_collisions: HashSet::new(),
        }
    }
}

impl Area2D {
    pub fn new() -> Self {
        let mut base = Node2D::new();
        base.name = Cow::Borrowed("Area2D");
        Self {
            ty: NodeType::Area2D,
            base,
            previous_collisions: HashSet::new(),
        }
    }

    pub fn internal_fixed_update(&mut self, api: &mut ScriptApi) {
        let children = self.get_children().clone();

        // First, collect all collider handles from children (uses RefCell for immutable access)
        // IMPORTANT: Only collect handles from children that still exist and are CollisionShape2D
        let mut collider_handles = Vec::new();
        {
            let children_ids: Vec<NodeID> = children.iter().copied().collect();
            for child_id in children_ids {
                // Check if child still exists before accessing it
                if let Some(child_node) = api.scene.get_scene_node_ref(child_id) {
                    // Check if it's a CollisionShape2D
                    if let SceneNode::CollisionShape2D(shape) = &*child_node {
                        if let Some(handle) = shape.collider_handle {
                            collider_handles.push(handle);
                        }
                    }
                }
            }
        }

        if collider_handles.is_empty() {
            // No valid colliders - clear previous collisions and return
            self.previous_collisions.clear();
            return;
        }

        // Query for collisions and collect node IDs
        // Note: We pass the node ID from physics (typically CollisionShape2D)
        // The codegen will handle getting the parent node if needed when converting UUID to node type
        let (current_colliding_node_ids, _intersection_count) = {
            let physics_ref = api.scene.get_physics_2d();
            match physics_ref {
                Some(physics) => {
                    let physics = physics.borrow();
                    let intersections = physics.get_intersecting_colliders(&collider_handles);
                    
                    // Collect all colliding node IDs while we have the physics borrow
                    // Filter out any node IDs that no longer exist in the scene (were deleted)
                    // IMPORTANT: Also filter out any node IDs that don't have a valid mapping in physics
                    // (this can happen if a collider was removed but physics still has stale data)
                    // NOTE: The node_id we collect here is the CollisionShape2D node ID, which is what gets
                    // passed to signal handlers. We need to ensure this CollisionShape2D still exists.
                    let mut node_ids = HashSet::new();
                    for (_our_handle, other_handle) in &intersections {
                        // First check if the collider handle is still valid (collider wasn't removed)
                        // This prevents accessing colliders that were removed during iteration
                        if physics.colliders.get(*other_handle).is_none() {
                            continue; // Collider was removed, skip it
                        }
                        
                        // Check if physics has a valid node_id mapping for this handle
                        if let Some(id) = physics.get_node_id(*other_handle) {
                            // Triple-check: 
                            // 1. Node must exist in scene
                            // 2. Node must still be a CollisionShape2D (not removed and replaced)
                            // 3. Collider handle must still match (node wasn't recreated)
                            if let Some(node) = api.scene.get_scene_node_ref(id) {
                                // Verify it's still a CollisionShape2D with the same handle
                                if let SceneNode::CollisionShape2D(shape) = node {
                                    if shape.collider_handle == Some(*other_handle) {
                                        node_ids.insert(id);
                                    }
                                }
                            }
                        }
                        // If physics doesn't have a node_id for this handle, skip it
                        // (collider was removed but intersection query still returned it)
                    }
                    (node_ids, intersections.len())
                }
                None => {
                    // No physics - clear previous collisions and return
                    self.previous_collisions.clear();
                    return;
                }
            }
        };

        // Clean up previous_collisions - remove any nodes that no longer exist
        // This prevents trying to access deleted nodes
        self.previous_collisions.retain(|&node_id| {
            api.scene.get_scene_node_ref(node_id).is_some()
        });
        
        // Get the signal base name (e.g., "Deadzone")
        let signal_base = self.name.as_ref();
        
        // Determine which colliders entered (new collisions)
        let entered: Vec<NodeID> = current_colliding_node_ids
            .difference(&self.previous_collisions)
            .copied()
            .collect();

        // Determine which colliders exited (no longer colliding)
        let exited: Vec<NodeID> = self
            .previous_collisions
            .difference(&current_colliding_node_ids)
            .copied()
            .collect();

        // Emit AreaEntered signals (when something enters the area)
        // Only emit for nodes that still exist (might have been deleted during signal handler execution)
        // IMPORTANT: Check node existence BEFORE each emission, as previous handlers may have deleted nodes
        if !entered.is_empty() {
            let entered_signal = format!("{}_AreaEntered", signal_base);
            let entered_signal_id = string_to_u64(&entered_signal);
            
            for node_id in &entered {
                // Double-check node still exists before emitting signal
                // (it might have been deleted by a previous signal handler in this same loop)
                if api.scene.get_scene_node_ref(*node_id).is_some() {
                    let params = [Value::String(node_id.to_string())];
                    api.emit_signal_id(entered_signal_id, &params);
                }
            }
        }

        // Emit AreaExited signals (when something leaves the area)
        // Only emit for nodes that still exist
        if !exited.is_empty() {
            let exited_signal = format!("{}_AreaExited", signal_base);
            let exited_signal_id = string_to_u64(&exited_signal);
            
            for node_id in &exited {
                // Double-check node still exists before emitting signal
                if api.scene.get_scene_node_ref(*node_id).is_some() {
                    let params = [Value::String(node_id.to_string())];
                    api.emit_signal_id(exited_signal_id, &params);
                }
            }
        }

        // Emit AreaOccupied signal for all objects currently inside the area
        // (emitted every frame for all current collisions - useful for continuous detection)
        // Only emit for nodes that still exist
        if !current_colliding_node_ids.is_empty() {
            let occupied_signal = format!("{}_AreaOccupied", signal_base);
            let occupied_signal_id = string_to_u64(&occupied_signal);

            for node_id in &current_colliding_node_ids {
                // Double-check node still exists before emitting signal
                if api.scene.get_scene_node_ref(*node_id).is_some() {
                    let params = [Value::String(node_id.to_string())];
                    api.emit_signal_id(occupied_signal_id, &params);
                }
            }
        }

        // Update previous collisions for next frame
        self.previous_collisions = current_colliding_node_ids;
    }
}

impl Deref for Area2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Area2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

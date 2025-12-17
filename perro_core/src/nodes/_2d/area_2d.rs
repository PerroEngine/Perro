use std::ops::{Deref, DerefMut};

use crate::{
    api::ScriptApi,
    nodes::_2d::node_2d::Node2D,
    nodes::node_registry::{BaseNode, SceneNode},
    prelude::string_to_u64,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Area2D {
    #[serde(rename = "type")]
    pub ty: Cow<'static, str>,

    #[serde(rename = "base")]
    pub base: Node2D,

    /// Track which colliders were intersecting in the previous frame
    /// Used to detect enter/exit events
    #[serde(skip)]
    pub previous_collisions: HashSet<Uuid>,
}

impl Area2D {
    pub fn new(name: &str) -> Self {
        Self {
            ty: Cow::Borrowed("Area2D"),
            base: Node2D::new(name),
            previous_collisions: HashSet::new(),
        }
    }

    pub fn internal_fixed_update(&mut self, api: &mut ScriptApi) {
        let children = self.get_children().clone();

        // First, collect all collider handles from children (needs mutable scene access)
        let mut collider_handles = Vec::new();
        {
            let children_ids: Vec<Uuid> = children.iter().copied().collect();
            for child_id in children_ids {
                if let Some(child_node) = api.scene.get_scene_node(child_id) {
                    // Check if it's a CollisionShape2D
                    if let SceneNode::CollisionShape2D(shape) = child_node {
                        if let Some(handle) = shape.collider_handle {
                            collider_handles.push(handle);
                        }
                    }
                }
            }
        }

        if collider_handles.is_empty() {
            return;
        }

        // Now get the physics world reference (after we're done with mutable scene access)
        let physics_ref = match api.scene.get_physics_2d() {
            Some(physics) => physics,
            None => return,
        };

        // Query for collisions and collect node IDs
        let current_colliding_node_ids = {
            let physics = physics_ref.borrow();
            let intersections = physics.get_intersecting_colliders(&collider_handles);
            
            // Collect all colliding node IDs while we have the physics borrow
            let mut node_ids = HashSet::new();
            for (_our_handle, other_handle) in intersections {
                if let Some(id) = physics.get_node_id(other_handle) {
                    node_ids.insert(id);
                }
            }
            
            node_ids
        };

        // Get the signal base name (e.g., "Deadzone")
        let signal_base = self.name.as_ref();

        // Determine which colliders entered (new collisions)
        let entered: Vec<Uuid> = current_colliding_node_ids
            .difference(&self.previous_collisions)
            .copied()
            .collect();

        // Determine which colliders exited (no longer colliding)
        let exited: Vec<Uuid> = self
            .previous_collisions
            .difference(&current_colliding_node_ids)
            .copied()
            .collect();

        // Emit AreaEntered signals (when something enters the area)
        if !entered.is_empty() {
            let entered_signal = format!("{}_AreaEntered", signal_base);
            let entered_signal_id = string_to_u64(&entered_signal);
            
            for node_id in &entered {
                let params = SmallVec::from(vec![Value::String(node_id.to_string())]);
                api.emit_signal_id(entered_signal_id, params);
            }
        }

        // Emit AreaExited signals (when something leaves the area)
        if !exited.is_empty() {
            let exited_signal = format!("{}_AreaExited", signal_base);
            let exited_signal_id = string_to_u64(&exited_signal);
            
            for node_id in &exited {
                let params = SmallVec::from(vec![Value::String(node_id.to_string())]);
                api.emit_signal_id(exited_signal_id, params);
            }
        }

        // Emit AreaOccupied signal for all objects currently inside the area
        // (emitted every frame for all current collisions - useful for continuous detection)
        if !current_colliding_node_ids.is_empty() {
            let occupied_signal = format!("{}_AreaOccupied", signal_base);
            let occupied_signal_id = string_to_u64(&occupied_signal);

            for node_id in &current_colliding_node_ids {
                let params = SmallVec::from(vec![Value::String(node_id.to_string())]);
                api.emit_signal_id(occupied_signal_id, params);
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

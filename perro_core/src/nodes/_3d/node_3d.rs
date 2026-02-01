use crate::nodes::node::Node;
use crate::nodes::node_registry::NodeType;
use crate::structs3d::{Transform3D, Vector3};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

fn default_visible() -> bool {
    true
}

fn is_default_visible(v: &bool) -> bool {
    *v == default_visible()
}

fn default_transform_dirty() -> bool {
    true
}

// Optimized field order: small fields grouped together (ty, transform_dirty, visible), then larger fields
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Node3D {
    #[serde(rename = "type")]
    pub ty: NodeType,

    /// Flag indicating if the global transform needs to be recalculated
    #[serde(skip, default = "default_transform_dirty")]
    pub transform_dirty: bool,

    #[serde(
        default = "default_visible",
        skip_serializing_if = "is_default_visible"
    )]
    pub visible: bool,

    #[serde(
        skip_serializing_if = "Transform3D::is_default",
        default = "Transform3D::default"
    )]
    pub transform: Transform3D,

    /// World-space transform (calculated from parent's global_transform + local transform)
    /// Runtime-only, not serialized
    #[serde(skip, default = "Transform3D::default")]
    pub global_transform: Transform3D,

    /// Optional pivot point for scaling/rotation center (defaults to {0.5,0.5,0.5})
    #[serde(
        skip_serializing_if = "Vector3::is_half_half_half",
        default = "Vector3::default_pivot"
    )]
    pub pivot: Vector3,

    /// Cached list of child IDs that are Node3D-based (for propagation)
    #[serde(skip, default)]
    pub node3d_children_cache: Option<Vec<crate::ids::NodeID>>,

    /// Wrapped base node with name, id, parent relationship, etc.
    #[serde(rename = "base")]
    pub base: Node,
}

impl Node3D {
    /// Create a new Node3D.
    pub fn new() -> Self {
        let mut base = Node::new();
        base.name = Cow::Borrowed("Node3D");
        Self {
            ty: NodeType::Node3D,
            transform_dirty: true,
            visible: default_visible(),
            transform: Transform3D::default(),
            global_transform: Transform3D::default(),
            pivot: Vector3::new(0.5, 0.5, 0.5),
            node3d_children_cache: None,
            base,
        }
    }

    /// Mark the transform as dirty
    pub fn mark_transform_dirty(&mut self) {
        self.transform_dirty = true;
    }

    /// Check if the transform is dirty
    pub fn is_transform_dirty(&self) -> bool {
        self.transform_dirty
    }

    /// Mark the transform as clean (after recalculation)
    pub fn mark_transform_clean(&mut self) {
        self.transform_dirty = false;
    }

    /// Create a new Node3D with a nil ID (for graphics-only nodes not in the scene tree).
    pub fn new_with_nil_id() -> Self {
        use crate::ids::NodeID;
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut base = Node {
            id: NodeID::nil(),
            ty: NodeType::Node,
            name: Cow::Borrowed("Node"),
            script_path: None,
            script_exp_vars: None,
            parent: None,
            children: None,
            metadata: None,
            is_root_of: None,
            created_timestamp: timestamp,
        };
        base.name = Cow::Borrowed("Node3D");
        Self {
            ty: NodeType::Node3D,
            transform_dirty: true,
            visible: default_visible(),
            transform: Transform3D::default(),
            global_transform: Transform3D::default(),
            pivot: Vector3::new(0.5, 0.5, 0.5),
            node3d_children_cache: None,
            base,
        }
    }

    /// Returns if the node is visible
    pub fn get_visible(&self) -> bool {
        self.visible
    }

    /// Sets node visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

impl Deref for Node3D {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl Default for Node3D {
    fn default() -> Self {
        Self {
            ty: NodeType::Node3D,
            transform_dirty: true,
            visible: default_visible(),
            transform: Transform3D::default(),
            global_transform: Transform3D::default(),
            pivot: Vector3::new(0.5, 0.5, 0.5),
            node3d_children_cache: None,
            base: {
                let mut base = Node::new();
                base.name = Cow::Borrowed("Node3D");
                base
            },
        }
    }
}

impl DerefMut for Node3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

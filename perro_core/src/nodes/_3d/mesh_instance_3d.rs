use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

use crate::nodes::_3d::node_3d::Node3D;

/// A single instance of a shared 3D mesh asset within the scene.
///
/// Similar to `Sprite2D`: wraps a `Node3D` for transform/hierarchy,
/// and references mesh/material assets via resource paths or IDs.
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct MeshInstance3D {
    #[serde(rename = "type")]
    pub ty: Cow<'static, str>,

    /// Resource path for the mesh this instance uses (e.g., "res://models/cube.gltf")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh_path: Option<Cow<'static, str>>,

    /// Resource path for the material applied to this instance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material_path: Option<Cow<'static, str>>,

    /// Optional runtime material slot ID assigned by the render system.
    /// Used for batching and sorting materials.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material_id: Option<u32>,

    /// Base transform node (provides position/rotation/scale/hierarchy)
    pub node_3d: Node3D,
}

// In mesh_instance_3d.rs - update the implementation
impl MeshInstance3D {
    pub fn new(name: &str) -> Self {
        Self {
            ty: Cow::Borrowed("MeshInstance3D"),
            mesh_path: None,
            material_path: Some(Cow::Borrowed("__default__")), // Always start with default
            material_id: Some(0),                              // Default material is always slot 0
            node_3d: Node3D::new(name),
        }
    }

    /// Set material path and clear cached material_id (will be resolved on next queue)
    pub fn set_material(&mut self, material_path: impl Into<Cow<'static, str>>) {
        let new_path = material_path.into();
        if self.material_path.as_ref() != Some(&new_path) {
            self.material_path = Some(new_path);
            self.material_id = None; // Clear cached ID to force re-resolution
        }
    }

    /// Get the current material path (with fallback to default)
    pub fn get_material_path(&self) -> &str {
        self.material_path
            .as_ref()
            .map(|s| s.as_ref())
            .unwrap_or("__default__")
    }
}

impl Deref for MeshInstance3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.node_3d
    }
}

impl DerefMut for MeshInstance3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node_3d
    }
}

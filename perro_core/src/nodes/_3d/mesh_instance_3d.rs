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

impl MeshInstance3D {
    pub fn new(name: &str) -> Self {
        Self {
            ty: Cow::Borrowed("MeshInstance3D"),
            mesh_path: None,
            material_path: None,
            material_id: None,
            node_3d: Node3D::new(name),
        }
    }

    /// Convenience method to set mesh + material at once.
    pub fn with_mesh_and_material(
        mut self,
        mesh_path: impl Into<Cow<'static, str>>,
        material_path: Option<impl Into<Cow<'static, str>>>,
    ) -> Self {
        self.mesh_path = Some(mesh_path.into());
        self.material_path = material_path.map(|m| m.into());
        self
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

use crate::mesh_instance_3d::MeshSurfaceBinding;
use crate::node_3d::Node3D;
use perro_ids::MeshID;
use perro_structs::{Quaternion, Vector3};
use std::ops::{Deref, DerefMut};

impl Deref for MultiMeshInstance3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for MultiMeshInstance3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct MultiMeshInstance3D {
    pub base: Node3D,
    pub mesh: MeshID,
    pub surfaces: Vec<MeshSurfaceBinding>,
    pub instances: Vec<(Vector3, Quaternion)>,
    pub instance_scale: f32,
    // None => follow renderer default.
    // Some(true) => force meshlet draw.
    // Some(false) => force classic indexed draw.
    pub meshlet_override: Option<bool>,
}

impl MultiMeshInstance3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            mesh: MeshID::nil(),
            surfaces: Vec::new(),
            instances: Vec::new(),
            instance_scale: 1.0,
            meshlet_override: None,
        }
    }

    pub fn ensure_surface_mut(&mut self, surface_index: usize) -> &mut MeshSurfaceBinding {
        if self.surfaces.len() <= surface_index {
            self.surfaces
                .resize_with(surface_index + 1, MeshSurfaceBinding::default);
        }
        &mut self.surfaces[surface_index]
    }

    #[inline]
    pub fn set_meshlet_override(&mut self, override_enabled: Option<bool>) {
        self.meshlet_override = override_enabled;
    }
}

impl Default for MultiMeshInstance3D {
    fn default() -> Self {
        Self::new()
    }
}

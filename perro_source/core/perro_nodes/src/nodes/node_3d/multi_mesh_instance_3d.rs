use crate::mesh_instance_3d::MeshSurfaceBinding;
use crate::node_3d::Node3D;
use perro_ids::MeshID;
use perro_structs::Transform3D;
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

#[derive(Clone, Debug, Default)]
pub struct MultiMeshInstance3D {
    pub base: Node3D,
    pub mesh: MeshID,
    pub surfaces: Vec<MeshSurfaceBinding>,
    pub transforms: Vec<Transform3D>,
}

impl MultiMeshInstance3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            mesh: MeshID::nil(),
            surfaces: Vec::new(),
            transforms: Vec::new(),
        }
    }

    pub fn ensure_surface_mut(&mut self, surface_index: usize) -> &mut MeshSurfaceBinding {
        if self.surfaces.len() <= surface_index {
            self.surfaces
                .resize_with(surface_index + 1, MeshSurfaceBinding::default);
        }
        &mut self.surfaces[surface_index]
    }
}

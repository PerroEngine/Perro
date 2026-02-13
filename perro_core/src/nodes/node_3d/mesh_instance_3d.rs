use crate::node_3d::node_3d::Node3D;
use perro_ids::{MaterialID, MeshID};

#[derive(Clone, Debug, Default)]
pub struct MeshInstance3D {
    pub base: Node3D,
    pub mesh_id: MeshID,
    pub material_id: MaterialID,
}

impl MeshInstance3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            mesh_id: MeshID::nil(),
            material_id: MaterialID::nil(),
        }
    }
}

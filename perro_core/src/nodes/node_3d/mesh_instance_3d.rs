use std::borrow::Cow;

use crate::{MaterialID, MeshID, node_3d::node_3d::Node3D};

#[derive(Clone, Debug, Default)]
pub struct MeshInstance3D {
    pub base: Node3D,
    pub mesh_id: Option<MeshID>,
    pub mesh_path: Option<Cow<'static, str>>,

    pub material_id: Option<MaterialID>,
    pub material_path: Option<Cow<'static, str>>,
}

impl MeshInstance3D {
    pub fn new() -> Self {
        Self {
            base: Node3D::new(),
            mesh_id: None,
            mesh_path: None,
            material_id: None,
            material_path: None,
        }
    }
}

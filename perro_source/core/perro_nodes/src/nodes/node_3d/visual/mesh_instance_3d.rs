use crate::node_3d::Node3D;
use perro_ids::{MaterialID, MeshID, NodeID};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

impl Deref for MeshInstance3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for MeshInstance3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

pub type MaterialParamOverrideValue = perro_structs::ConstParamValue;

#[derive(Clone, Debug, Default)]
pub struct MaterialParamOverride {
    pub name: Cow<'static, str>,
    pub value: MaterialParamOverrideValue,
}

#[derive(Clone, Debug)]
pub struct MeshSurfaceBinding {
    pub material: Option<MaterialID>,
    pub overrides: Vec<MaterialParamOverride>,
    pub modulate: [f32; 4],
}

impl Default for MeshSurfaceBinding {
    fn default() -> Self {
        Self {
            material: None,
            overrides: Vec::new(),
            modulate: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MeshInstance3D {
    pub base: Node3D,
    pub mesh: MeshID,
    pub surfaces: Vec<MeshSurfaceBinding>,
    pub skeleton: NodeID,
    // None => follow renderer default.
    // Some(true) => force meshlet draw.
    // Some(false) => force classic indexed draw.
    pub meshlet_override: Option<bool>,
}

impl MeshInstance3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            mesh: MeshID::nil(),
            surfaces: Vec::new(),
            skeleton: NodeID::nil(),
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

    pub fn set_surface_material(&mut self, surface_index: usize, material: Option<MaterialID>) {
        self.ensure_surface_mut(surface_index).material = material;
    }

    #[inline]
    pub fn set_material(&mut self, material: MaterialID) {
        self.set_surface_material(0, Some(material));
    }

    #[inline]
    pub fn clear_material(&mut self) {
        self.set_surface_material(0, None);
    }

    #[inline]
    pub fn material(&self) -> Option<MaterialID> {
        self.surfaces.first().and_then(|surface| surface.material)
    }

    #[inline]
    pub fn set_meshlet_override(&mut self, override_enabled: Option<bool>) {
        self.meshlet_override = override_enabled;
    }
}

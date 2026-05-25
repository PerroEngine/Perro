use crate::node_3d::Node3D;
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_structs::{BitMask, Color};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LODOptions {
    pub min_lod: u8,
    pub max_lod: u8,
}

impl LODOptions {
    pub const MIN: u8 = 0;
    pub const LOW: u8 = 1;
    pub const MEDIUM_LOW: u8 = 2;
    pub const MEDIUM: u8 = 3;
    pub const HIGH: u8 = 4;
    pub const MAX: u8 = 5;

    pub const fn new() -> Self {
        Self {
            min_lod: Self::MIN,
            max_lod: Self::MAX,
        }
    }
}

impl Default for LODOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshBlendOptions {
    pub enabled: bool,
    pub screen_blending: bool,
    pub normal_blending: bool,
    pub blend_layers: BitMask,
    pub blend_mask: BitMask,
    pub distance: f32,
    pub min_distance: f32,
    pub noise_factor: f32,
    pub noise_scale: f32,
}

impl MeshBlendOptions {
    pub const fn new() -> Self {
        Self {
            enabled: false,
            screen_blending: true,
            normal_blending: false,
            blend_layers: BitMask::ALL,
            blend_mask: BitMask::NONE,
            distance: 1.35,
            min_distance: 0.03,
            noise_factor: 0.35,
            noise_scale: 14.0,
        }
    }
}

impl Default for MeshBlendOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct MeshSurfaceBinding {
    pub material: Option<MaterialID>,
    pub overrides: Vec<MaterialParamOverride>,
    pub modulate: Color,
}

impl Default for MeshSurfaceBinding {
    fn default() -> Self {
        Self {
            material: None,
            overrides: Vec::new(),
            modulate: Color::WHITE,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MeshInstance3D {
    pub base: Node3D,
    pub mesh: MeshID,
    pub surfaces: Vec<MeshSurfaceBinding>,
    pub skeleton: NodeID,
    pub flip_x: bool,
    pub flip_y: bool,
    pub flip_z: bool,
    // None => follow renderer default.
    // Some(true) => force meshlet draw.
    // Some(false) => force classic indexed draw.
    pub meshlet_override: Option<bool>,
    pub lod: LODOptions,
    pub blend: MeshBlendOptions,
    pub blend_shape_weights: Vec<f32>,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
}

impl MeshInstance3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            mesh: MeshID::nil(),
            surfaces: Vec::new(),
            skeleton: NodeID::nil(),
            flip_x: false,
            flip_y: false,
            flip_z: false,
            meshlet_override: None,
            lod: LODOptions::new(),
            blend: MeshBlendOptions::new(),
            blend_shape_weights: Vec::new(),
            cast_shadows: true,
            receive_shadows: true,
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

    #[inline]
    pub fn set_lod_clamp(&mut self, min_lod: u8, max_lod: u8) {
        self.lod = LODOptions {
            min_lod: min_lod.min(LODOptions::MAX),
            max_lod: max_lod.min(LODOptions::MAX),
        };
    }
}

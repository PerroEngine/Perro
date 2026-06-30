use crate::mesh_instance_3d::{LODOptions, MeshBlendOptions, MeshSurfaceBinding};
use crate::node_3d::Node3D;
use perro_ids::MeshID;
use perro_structs::{Quaternion, Transform3D, Vector3};
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

#[derive(Clone, Debug, PartialEq)]
pub struct MultiMeshInstanceTransform {
    pub transform: Transform3D,
    pub blend_shape_weights: Option<Vec<f32>>,
}

pub type MultiMeshInstancePose = MultiMeshInstanceTransform;

impl MultiMeshInstanceTransform {
    pub const fn new(transform: Transform3D) -> Self {
        Self {
            transform,
            blend_shape_weights: None,
        }
    }

    pub const fn from_pos_rot(position: Vector3, rotation: Quaternion) -> Self {
        Self::new(Transform3D::new(position, rotation, Vector3::ONE))
    }
}

#[derive(Clone, Debug)]
pub struct MultiMeshInstance3D {
    pub base: Node3D,
    pub mesh: MeshID,
    pub surfaces: Vec<MeshSurfaceBinding>,
    pub instances: Vec<MultiMeshInstanceTransform>,
    pub instance_scale: f32,
    pub blend_shape_weights: Vec<f32>,
    pub flip_x: bool,
    pub flip_y: bool,
    pub flip_z: bool,
    // None => follow renderer default.
    // Some(true) => force meshlet draw.
    // Some(false) => force classic indexed draw.
    pub meshlet_override: Option<bool>,
    pub lod: LODOptions,
    pub blend: MeshBlendOptions,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
}

impl MultiMeshInstance3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            mesh: MeshID::nil(),
            surfaces: Vec::new(),
            instances: Vec::new(),
            instance_scale: 1.0,
            blend_shape_weights: Vec::new(),
            flip_x: false,
            flip_y: false,
            flip_z: false,
            meshlet_override: None,
            lod: LODOptions::new(),
            blend: MeshBlendOptions::new(),
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

impl Default for MultiMeshInstance3D {
    fn default() -> Self {
        Self::new()
    }
}

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
    // Slope falloff exponent for the screen seam pass, 0.0..=8.0; 0 disables
    // the falloff so perpendicular contacts blend as strongly as parallel ones.
    pub slope_factor: f32,
    // Overall seam blend scale, 0.0..=1.0.
    pub strength: f32,
    // Salt multimesh instance ids so same-type instances seam against each
    // other; off shares one id per batch (no instance-vs-instance seams, but
    // many multimesh sources stop exhausting the id range).
    pub salt_instances: bool,
}

impl MeshBlendOptions {
    pub const fn new() -> Self {
        Self {
            enabled: false,
            screen_blending: true,
            normal_blending: false,
            blend_layers: BitMask::ALL,
            blend_mask: BitMask::NONE,
            distance: 0.6,
            min_distance: 0.03,
            noise_factor: 0.35,
            noise_scale: 14.0,
            slope_factor: 2.0,
            strength: 1.0,
            salt_instances: true,
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

    /// Sets one per-surface material param override.
    ///
    /// The node keeps its own value for `name` while still sharing the base
    /// material, so a crowd of nodes can each drive a param without a material
    /// (and bind group) per node. Overwrites an existing override of the same
    /// name; returns true when the stored value actually changed.
    pub fn set_surface_param_override(
        &mut self,
        surface_index: usize,
        name: &str,
        value: MaterialParamOverrideValue,
    ) -> bool {
        let surface = self.ensure_surface_mut(surface_index);
        if let Some(existing) = surface
            .overrides
            .iter_mut()
            .find(|entry| entry.name == name)
        {
            if existing.value == value {
                return false;
            }
            existing.value = value;
            return true;
        }
        surface.overrides.push(MaterialParamOverride {
            name: std::borrow::Cow::Owned(name.to_string()),
            value,
        });
        true
    }

    /// Drops a per-surface override so the surface falls back to the material's
    /// own value. Returns true when one was removed.
    pub fn clear_surface_param_override(&mut self, surface_index: usize, name: &str) -> bool {
        if self.surfaces.len() <= surface_index {
            return false;
        }
        let overrides = &mut self.surfaces[surface_index].overrides;
        let before = overrides.len();
        overrides.retain(|entry| entry.name != name);
        overrides.len() != before
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

#[cfg(test)]
mod surface_param_override_tests {
    use super::*;

    #[test]
    fn sets_updates_and_dedupes_by_name() {
        let mut mesh = MeshInstance3D::default();
        assert!(mesh.set_surface_param_override(0, "glow", 0.5f32.into()));
        // Same name replaces rather than appending a second entry.
        assert!(mesh.set_surface_param_override(0, "glow", 0.9f32.into()));
        assert_eq!(mesh.surfaces[0].overrides.len(), 1);
        assert_eq!(
            mesh.surfaces[0].overrides[0].value,
            MaterialParamOverrideValue::F32(0.9)
        );
        // Re-asserting the same value reports no change, so callers can drive
        // this every frame without churning the render extract.
        assert!(!mesh.set_surface_param_override(0, "glow", 0.9f32.into()));
    }

    #[test]
    fn overrides_are_per_surface() {
        let mut mesh = MeshInstance3D::default();
        mesh.set_surface_param_override(0, "glow", 1.0f32.into());
        mesh.set_surface_param_override(2, "glow", 2.0f32.into());
        assert_eq!(mesh.surfaces.len(), 3);
        assert_eq!(mesh.surfaces[0].overrides.len(), 1);
        assert!(mesh.surfaces[1].overrides.is_empty());
        assert_eq!(
            mesh.surfaces[2].overrides[0].value,
            MaterialParamOverrideValue::F32(2.0)
        );
    }

    #[test]
    fn clear_removes_only_the_named_override() {
        let mut mesh = MeshInstance3D::default();
        mesh.set_surface_param_override(0, "glow", 1.0f32.into());
        mesh.set_surface_param_override(0, "tint", 2.0f32.into());
        assert!(mesh.clear_surface_param_override(0, "glow"));
        assert_eq!(mesh.surfaces[0].overrides.len(), 1);
        assert_eq!(mesh.surfaces[0].overrides[0].name, "tint");
        // Absent name and out-of-range surface both report nothing removed.
        assert!(!mesh.clear_surface_param_override(0, "glow"));
        assert!(!mesh.clear_surface_param_override(9, "tint"));
    }
}

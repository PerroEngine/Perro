use crate::node_3d::Node3D;
use perro_ids::TextureID;
use perro_structs::{Color, Vector3};
use perro_ui::UiTextAlign;
use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

impl Deref for Decal3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Decal3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

/// How the decal patches the surface it lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecalSurfaceSettings {
    /// 0..1 blend of decal albedo over the surface albedo.
    pub albedo_mix: f32,
    pub emission_energy: f32,
    /// Scales the normal-map perturbation.
    pub normal_strength: f32,
    /// 0..1 fade on surfaces facing away from the projection axis
    /// (0 = soft falloff over the whole hemisphere, 1 = only head-on).
    pub normal_fade: f32,
}

impl DecalSurfaceSettings {
    pub const fn new() -> Self {
        Self {
            albedo_mix: 1.0,
            emission_energy: 1.0,
            normal_strength: 1.0,
            normal_fade: 0.3,
        }
    }
}

impl Default for DecalSurfaceSettings {
    fn default() -> Self {
        Self::new()
    }
}

/// Camera-distance opacity fade; `begin` 0 disables.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecalDistanceFade {
    pub begin: f32,
    pub length: f32,
}

impl DecalDistanceFade {
    pub const fn new() -> Self {
        Self {
            begin: 0.0,
            length: 8.0,
        }
    }
}

impl Default for DecalDistanceFade {
    fn default() -> Self {
        Self::new()
    }
}

/// Projected box decal. Projects along the node's -Z (forward) axis onto
/// lit geometry inside the `size` box; patches albedo/normal/emission before
/// lighting so decals receive shadows and light like the surface under them.
#[derive(Clone, Debug)]
pub struct Decal3D {
    pub base: Node3D,
    /// Box extents in local units (x = width, y = height, z = projection depth).
    pub size: Vector3,
    /// Nil = slot unused; with no albedo texture `modulate` paints flat.
    pub albedo_texture: TextureID,
    pub normal_texture: TextureID,
    pub emission_texture: TextureID,
    /// Albedo tint; alpha scales overall decal opacity.
    pub modulate: Color,
    pub surface: DecalSurfaceSettings,
    pub distance_fade: DecalDistanceFade,
    /// Higher priority draws over lower when decals overlap.
    pub sort_priority: i32,
    pub active: bool,
}

impl Decal3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            size: Vector3::ONE,
            albedo_texture: TextureID::nil(),
            normal_texture: TextureID::nil(),
            emission_texture: TextureID::nil(),
            modulate: Color::WHITE,
            surface: DecalSurfaceSettings::new(),
            distance_fade: DecalDistanceFade::new(),
            sort_priority: 0,
            active: true,
        }
    }
}

impl Default for Decal3D {
    fn default() -> Self {
        Self::new()
    }
}

/// Text projected as a lit decal. The runtime rasterizes `text` into an
/// albedo texture, then submits it through the Decal3D projection path.
#[derive(Clone, Debug)]
pub struct TextDecal3D {
    pub base: Node3D,
    /// Box extents in local units (x = width, y = height, z = projection depth).
    pub size: Vector3,
    pub text: Cow<'static, str>,
    /// Text tint; alpha scales overall decal opacity.
    pub color: Color,
    /// Pixel size used when rasterizing the backing texture.
    pub font_size: f32,
    pub h_align: UiTextAlign,
    pub v_align: UiTextAlign,
    /// Max backing texture dimension before upload into the decal atlas.
    pub texture_resolution: u32,
    /// Font outline thickness in texture pixels; 0 disables the outline.
    pub outline_width: f32,
    /// Outline tint, drawn under the glyph fill.
    pub outline_color: Color,
    pub surface: DecalSurfaceSettings,
    pub distance_fade: DecalDistanceFade,
    /// Higher priority draws over lower when decals overlap.
    pub sort_priority: i32,
    pub active: bool,
}

impl TextDecal3D {
    pub const fn new() -> Self {
        let mut surface = DecalSurfaceSettings::new();
        surface.emission_energy = 0.0;
        Self {
            base: Node3D::new(),
            size: Vector3::new(2.0, 0.5, 0.25),
            text: Cow::Borrowed(""),
            color: Color::WHITE,
            font_size: 64.0,
            h_align: UiTextAlign::Center,
            v_align: UiTextAlign::Center,
            texture_resolution: 512,
            outline_width: 0.0,
            outline_color: Color::BLACK,
            surface,
            distance_fade: DecalDistanceFade::new(),
            sort_priority: 0,
            active: true,
        }
    }
}

impl Default for TextDecal3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for TextDecal3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TextDecal3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

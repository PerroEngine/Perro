use super::*;

pub(super) fn materials_from_gltf_file(
    path: &Path,
    res_path: &str,
) -> io::Result<Vec<(String, MaterialLiteral, bool)>> {
    let (doc, _buffers, _images) = gltf::import(path).map_err(|err| {
        io::Error::other(format!(
            "failed to import model `{res_path}` for materials: {err}"
        ))
    })?;

    let mut out = Vec::<(String, MaterialLiteral, bool)>::new();
    for (index, material) in doc.materials().enumerate() {
        let pbr = material.pbr_metallic_roughness();
        let base_color = pbr.base_color_factor();
        let emissive_factor = material.emissive_factor();
        let derived = StandardMaterial3D {
            base_color_factor: base_color,
            roughness_factor: pbr.roughness_factor(),
            metallic_factor: pbr.metallic_factor(),
            occlusion_strength: material
                .occlusion_texture()
                .map(|occ| occ.strength())
                .unwrap_or(1.0),
            emissive_factor,
            alpha_mode: match material.alpha_mode() {
                gltf::material::AlphaMode::Opaque => 0,
                gltf::material::AlphaMode::Mask => 1,
                gltf::material::AlphaMode::Blend => 2,
            },
            alpha_cutoff: material.alpha_cutoff().unwrap_or(0.5),
            double_sided: material.double_sided(),
            flat_shading: false,
            normal_scale: material
                .normal_texture()
                .map(|normal| normal.scale())
                .unwrap_or(1.0),
            base_color_texture: pbr
                .base_color_texture()
                .map(|tex| tex.texture().index() as u32)
                .unwrap_or(u32::MAX),
            metallic_roughness_texture: pbr
                .metallic_roughness_texture()
                .map(|tex| tex.texture().index() as u32)
                .unwrap_or(u32::MAX),
            normal_texture: material
                .normal_texture()
                .map(|tex| tex.texture().index() as u32)
                .unwrap_or(u32::MAX),
            occlusion_texture: material
                .occlusion_texture()
                .map(|tex| tex.texture().index() as u32)
                .unwrap_or(u32::MAX),
            emissive_texture: material
                .emissive_texture()
                .map(|tex| tex.texture().index() as u32)
                .unwrap_or(u32::MAX),
        };
        out.push((
            format!("{res_path}:mat[{index}]"),
            MaterialLiteral::Standard(derived),
            true,
        ));
    }
    if out.is_empty() {
        out.push((
            format!("{res_path}:mat[0]"),
            MaterialLiteral::Standard(StandardMaterial3D::default()),
            true,
        ));
    }
    Ok(out)
}

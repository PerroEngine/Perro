use super::*;

pub(super) struct BuildMeshLodRangesArgs<'a> {
    pub(super) index_start: u32,
    pub(super) index_count: u32,
    pub(super) decoded_surfaces: &'a [MeshRange],
    pub(super) uploaded_surfaces: &'a Arc<[MeshRange]>,
    pub(super) decoded_meshlets: &'a [DecodedMeshlet],
    pub(super) uploaded_meshlets: &'a Arc<[MeshletRange]>,
    pub(super) decoded_lods: &'a [DecodedLod],
    pub(super) packed_lods: &'a [Option<PackedMeshLodRange>],
}

pub(super) fn build_mesh_lod_ranges(args: BuildMeshLodRangesArgs<'_>) -> Vec<MeshLodRange> {
    let BuildMeshLodRangesArgs {
        index_start,
        index_count,
        decoded_surfaces,
        uploaded_surfaces,
        decoded_meshlets,
        uploaded_meshlets,
        decoded_lods,
        packed_lods,
    } = args;
    if decoded_lods.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for (lod_index, lod) in decoded_lods.iter().enumerate() {
        if lod.index_count == 0 {
            continue;
        }
        let lod_full = MeshRange {
            index_start: index_start + lod.index_start.min(index_count),
            index_count: lod
                .index_count
                .min(index_count.saturating_sub(lod.index_start)),
            base_vertex: 0,
        };
        let surface_start = lod.surface_start as usize;
        let surface_end = surface_start
            .saturating_add(lod.surface_count as usize)
            .min(decoded_surfaces.len())
            .min(uploaded_surfaces.len());
        let surfaces = if surface_start < surface_end {
            Arc::from(uploaded_surfaces[surface_start..surface_end].to_vec())
        } else {
            Arc::from([lod_full])
        };
        let meshlet_start = lod.meshlet_start as usize;
        let meshlet_end = meshlet_start
            .saturating_add(lod.meshlet_count as usize)
            .min(decoded_meshlets.len())
            .min(uploaded_meshlets.len());
        let meshlets = if meshlet_start < meshlet_end {
            Arc::from(uploaded_meshlets[meshlet_start..meshlet_end].to_vec())
        } else {
            Arc::from([])
        };
        out.push(MeshLodRange {
            full: lod_full,
            surface_ranges: surfaces,
            meshlets,
            packed: packed_lods.get(lod_index).cloned().unwrap_or(None),
        });
    }
    out
}

pub(super) fn select_mesh_lod<'a>(
    mesh: &'a MeshAssetRange,
    model: Option<&[[f32; 4]; 4]>,
    camera_pos: [f32; 3],
    control: LODOptions3D,
) -> MeshLodView<'a> {
    if mesh.lods.len() <= 1 {
        return MeshLodView {
            full: mesh.full,
            surface_ranges: &mesh.surface_ranges,
            meshlets: &mesh.meshlets,
            packed: None,
        };
    }
    let Some(model) = model else {
        return MeshLodView {
            full: mesh.full,
            surface_ranges: &mesh.surface_ranges,
            meshlets: &mesh.meshlets,
            packed: None,
        };
    };
    let world_pos = [model[3][0], model[3][1], model[3][2]];
    let dx = world_pos[0] - camera_pos[0];
    let dy = world_pos[1] - camera_pos[1];
    let dz = world_pos[2] - camera_pos[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    let radius = mesh.bounds_radius.max(0.001);
    let ratio = dist / radius;
    let mut baked_index = LOD_DISTANCE_RADIUS_SCALES
        .iter()
        .take(mesh.lods.len().saturating_sub(1))
        .filter(|&&threshold| ratio > threshold)
        .count();
    let last = mesh.lods.len().saturating_sub(1);
    baked_index = baked_index.min(last);
    let auto_quality = usize::from(LODOptions3D::MAX).saturating_sub(baked_index);
    let min_quality = usize::from(control.min_lod.min(LODOptions3D::MAX));
    let max_quality = usize::from(control.max_lod.min(LODOptions3D::MAX)).max(min_quality);
    let quality = auto_quality.clamp(min_quality, max_quality);
    let baked_index = usize::from(LODOptions3D::MAX)
        .saturating_sub(quality)
        .min(last);
    let lod = &mesh.lods[baked_index];
    MeshLodView {
        full: lod.full,
        surface_ranges: &lod.surface_ranges,
        meshlets: &lod.meshlets,
        packed: lod.packed.as_ref(),
    }
}

pub(crate) fn validate_mesh_source(
    source: &str,
    static_mesh_lookup: Option<StaticMeshLookup>,
) -> Result<(), String> {
    perro_graphics_assets::validate_mesh_source(source, static_mesh_lookup)
}

pub(crate) fn load_mesh3d_from_source(
    source: &str,
    static_mesh_lookup: Option<StaticMeshLookup>,
) -> Option<perro_render_bridge::Mesh3D> {
    perro_graphics_assets::load_mesh3d_from_source(source, static_mesh_lookup)
}

pub(super) fn is_builtin_primitive_mesh_source(source: &str) -> bool {
    fn is_builtin_or_alias(source: &str) -> bool {
        perro_builtin_meshes::is_builtin_mesh_source(source)
    }

    let Some((base, selector)) = source.rsplit_once(':') else {
        return is_builtin_or_alias(source);
    };
    if base.is_empty() || selector.contains('/') || selector.contains('\\') {
        return is_builtin_or_alias(source);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return is_builtin_or_alias(base);
    }
    is_builtin_or_alias(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(index_start: u32) -> MeshLodRange {
        let full = MeshRange {
            index_start,
            index_count: 3,
            base_vertex: 0,
        };
        MeshLodRange {
            full,
            surface_ranges: Arc::from([full]),
            meshlets: Arc::from([]),
            packed: None,
        }
    }

    fn mesh() -> MeshAssetRange {
        let full = MeshRange {
            index_start: 0,
            index_count: 3,
            base_vertex: 0,
        };
        MeshAssetRange {
            full,
            surface_ranges: Arc::from([full]),
            meshlets: Arc::from([]),
            lods: Arc::from([
                range(0),
                range(10),
                range(20),
                range(30),
                range(40),
                range(50),
            ]),
            bounds_center: [0.0, 0.0, 0.0],
            bounds_radius: 1.0,
            blend_shape_delta_start: 0,
            blend_shape_target_count: 0,
            blend_shape_vertex_start: 0,
            blend_shape_vertex_count: 0,
        }
    }

    #[test]
    fn select_mesh_lod_uses_clamp() {
        let mesh = mesh();
        let model =
            glam::Mat4::from_translation(glam::Vec3::new(200.0, 0.0, 0.0)).to_cols_array_2d();

        let default = select_mesh_lod(
            &mesh,
            Some(&model),
            [0.0, 0.0, 0.0],
            LODOptions3D::default(),
        );
        assert_eq!(default.full.index_start, 50);

        let clamped = select_mesh_lod(
            &mesh,
            Some(&model),
            [0.0, 0.0, 0.0],
            LODOptions3D {
                min_lod: LODOptions3D::MEDIUM_LOW,
                max_lod: LODOptions3D::MAX,
            },
        );
        assert_eq!(clamped.full.index_start, 30);
    }

    #[test]
    fn plane_alias_counts_as_builtin_primitive() {
        assert!(is_builtin_primitive_mesh_source("__plane__"));
        assert!(is_builtin_primitive_mesh_source("__plane__:mesh[0]"));
    }
}

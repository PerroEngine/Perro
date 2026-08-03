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
            Arc::from(&uploaded_surfaces[surface_start..surface_end])
        } else {
            Arc::from([lod_full])
        };
        let meshlet_start = lod.meshlet_start as usize;
        let meshlet_end = meshlet_start
            .saturating_add(lod.meshlet_count as usize)
            .min(decoded_meshlets.len())
            .min(uploaded_meshlets.len());
        let meshlets = if meshlet_start < meshlet_end {
            Arc::from(&uploaded_meshlets[meshlet_start..meshlet_end])
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

/// Which baked LOD a mesh at `world_pos` resolves to for this camera.
///
/// Split out of `select_mesh_lod` so the multimesh staging-reuse check can ask
/// the same question without touching the asset: the camera enters the dense
/// multimesh packing ONLY through this index (it picks the index/surface ranges
/// the batches are built from), so staged rows stay valid across camera motion
/// until this band flips.
pub(super) fn select_mesh_lod_index(
    lod_count: usize,
    bounds_radius: f32,
    world_pos: [f32; 3],
    camera_pos: [f32; 3],
    control: LODOptions3D,
    ratio_scale: f32,
) -> usize {
    if lod_count <= 1 {
        return 0;
    }
    let dx = world_pos[0] - camera_pos[0];
    let dy = world_pos[1] - camera_pos[1];
    let dz = world_pos[2] - camera_pos[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    let radius = bounds_radius.max(0.001);
    // `ratio_scale` converts distance/radius into the reference view's terms:
    // the same mesh at the same distance projects onto fewer pixels in a short
    // target or a wide FOV, so it steps down a LOD sooner. See
    // `Gpu3D::lod_ratio_scale`. 1.0 reproduces the pre-scaling behaviour.
    let ratio = (dist / radius) * ratio_scale.max(1.0);
    let last = lod_count.saturating_sub(1);
    let baked_index = LOD_DISTANCE_RADIUS_SCALES
        .iter()
        .take(last)
        .filter(|&&threshold| ratio > threshold)
        .count()
        .min(last);
    let auto_quality = usize::from(LODOptions3D::MAX).saturating_sub(baked_index);
    let min_quality = usize::from(control.min_lod.min(LODOptions3D::MAX));
    let max_quality = usize::from(control.max_lod.min(LODOptions3D::MAX)).max(min_quality);
    let quality = auto_quality.clamp(min_quality, max_quality);
    usize::from(LODOptions3D::MAX)
        .saturating_sub(quality)
        .min(last)
}

pub(super) fn select_mesh_lod<'a>(
    mesh: &'a MeshAssetRange,
    model: Option<&[[f32; 4]; 4]>,
    camera_pos: [f32; 3],
    control: LODOptions3D,
    ratio_scale: f32,
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
    let baked_index = select_mesh_lod_index(
        mesh.lods.len(),
        mesh.bounds_radius,
        [model[3][0], model[3][1], model[3][2]],
        camera_pos,
        control,
        ratio_scale,
    );
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
            1.0,
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
            1.0,
        );
        assert_eq!(clamped.full.index_start, 30);
    }

    /// The multimesh staging-reuse check bands on `select_mesh_lod_index`
    /// while the packer bands on `select_mesh_lod`; if the two ever disagree,
    /// reuse would keep rows staged from a different LOD.
    #[test]
    fn select_mesh_lod_index_agrees_with_select_mesh_lod() {
        let mesh = mesh();
        let controls = [
            LODOptions3D::default(),
            LODOptions3D {
                min_lod: LODOptions3D::MEDIUM_LOW,
                max_lod: LODOptions3D::MAX,
            },
            LODOptions3D {
                min_lod: LODOptions3D::MIN,
                max_lod: LODOptions3D::LOW,
            },
        ];
        for control in controls {
            for scale in [1.0f32, 1.7, 4.2] {
                for step in 0..400u32 {
                    let distance = step as f32 * 0.75;
                    let model = glam::Mat4::from_translation(glam::Vec3::new(distance, 0.0, 0.0))
                        .to_cols_array_2d();
                    let camera = [0.0, 0.0, 0.0];
                    let view = select_mesh_lod(&mesh, Some(&model), camera, control, scale);
                    let index = select_mesh_lod_index(
                        mesh.lods.len(),
                        mesh.bounds_radius,
                        [distance, 0.0, 0.0],
                        camera,
                        control,
                        scale,
                    );
                    assert_eq!(
                        view.full.index_start, mesh.lods[index].full.index_start,
                        "band {index} disagrees with the selected LOD at {distance}"
                    );
                }
            }
        }
        // A single-LOD mesh has no band at all, whatever the camera does.
        assert_eq!(
            select_mesh_lod_index(
                1,
                1.0,
                [0.0; 3],
                [900.0, 0.0, 0.0],
                LODOptions3D::default(),
                1.0
            ),
            0
        );
    }

    #[test]
    fn plane_alias_counts_as_builtin_primitive() {
        assert!(is_builtin_primitive_mesh_source("__plane__"));
        assert!(is_builtin_primitive_mesh_source("__plane__:mesh[0]"));
    }
}

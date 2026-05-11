use super::*;

pub(super) fn build_mesh_lod_ranges(
    index_start: u32,
    index_count: u32,
    decoded_surfaces: &[MeshRange],
    uploaded_surfaces: &Arc<[MeshRange]>,
    decoded_meshlets: &[DecodedMeshlet],
    uploaded_meshlets: &Arc<[MeshletRange]>,
    decoded_lods: &[DecodedLod],
) -> Vec<MeshLodRange> {
    if decoded_lods.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for lod in decoded_lods {
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
        });
    }
    out
}

pub(super) fn select_mesh_lod<'a>(
    mesh: &'a MeshAssetRange,
    model: Option<&[[f32; 4]; 4]>,
    camera_pos: [f32; 3],
) -> MeshLodView<'a> {
    if mesh.lods.len() <= 1 {
        return MeshLodView {
            full: mesh.full,
            surface_ranges: &mesh.surface_ranges,
            meshlets: &mesh.meshlets,
        };
    }
    let Some(model) = model else {
        return MeshLodView {
            full: mesh.full,
            surface_ranges: &mesh.surface_ranges,
            meshlets: &mesh.meshlets,
        };
    };
    let world_pos = [model[3][0], model[3][1], model[3][2]];
    let dx = world_pos[0] - camera_pos[0];
    let dy = world_pos[1] - camera_pos[1];
    let dz = world_pos[2] - camera_pos[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    let radius = mesh.bounds_radius.max(0.001);
    let ratio = dist / radius;
    let index = if ratio > LOD3_DISTANCE_RADIUS_SCALE {
        3
    } else if ratio > LOD2_DISTANCE_RADIUS_SCALE {
        2
    } else if ratio > LOD1_DISTANCE_RADIUS_SCALE {
        1
    } else {
        0
    }
    .min(mesh.lods.len().saturating_sub(1));
    let lod = &mesh.lods[index];
    MeshLodView {
        full: lod.full,
        surface_ranges: &lod.surface_ranges,
        meshlets: &lod.meshlets,
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
    let Some((base, selector)) = source.rsplit_once(':') else {
        return perro_builtin_meshes::is_builtin_mesh_source(source);
    };
    if base.is_empty() || selector.contains('/') || selector.contains('\\') {
        return perro_builtin_meshes::is_builtin_mesh_source(source);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return perro_builtin_meshes::is_builtin_mesh_source(base);
    }
    perro_builtin_meshes::is_builtin_mesh_source(source)
}

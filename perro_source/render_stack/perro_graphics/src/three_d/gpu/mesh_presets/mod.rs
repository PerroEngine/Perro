use super::{MeshRange, MeshVertex, MeshletRange};
use ahash::AHashMap;
use std::sync::Arc;

mod capsule;
mod common;
mod cone;
mod cube;
mod cylinder;
mod sphere;
mod square_pyramid;
mod terrain64;
mod tri_prism;
mod triangular_pyramid;

type BuiltinMeshBuffer = (
    Vec<MeshVertex>,
    Vec<u32>,
    AHashMap<&'static str, MeshRange>,
    AHashMap<&'static str, Arc<[MeshletRange]>>,
);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MeshVertexKey {
    pos: [u32; 3],
    normal: [u32; 3],
    uv: [u32; 2],
}

impl From<MeshVertex> for MeshVertexKey {
    fn from(vertex: MeshVertex) -> Self {
        Self {
            pos: vertex.pos.map(f32::to_bits),
            normal: vertex.normal.map(f32::to_bits),
            uv: vertex.uv.map(f32::to_bits),
        }
    }
}

pub(super) fn build_builtin_mesh_buffer() -> BuiltinMeshBuffer {
    const ROUND_SEGMENTS: u32 = 36;
    const CYLINDER_SEGMENTS: u32 = ROUND_SEGMENTS * 3;
    const SPHERE_LATITUDE_BANDS: u32 = 24;
    const CAPSULE_HEMISPHERE_BANDS: u32 = 14;

    let presets = [
        ("__cube__", deduplicate_mesh(cube::geometry())),
        (
            "__tri_pyr__",
            deduplicate_mesh(triangular_pyramid::geometry()),
        ),
        ("__sq_pyr__", deduplicate_mesh(square_pyramid::geometry())),
        (
            "__sphere__",
            deduplicate_mesh(sphere::geometry(ROUND_SEGMENTS, SPHERE_LATITUDE_BANDS)),
        ),
        ("__tri_prism__", deduplicate_mesh(tri_prism::geometry())),
        (
            "__cylinder__",
            deduplicate_mesh(cylinder::geometry(CYLINDER_SEGMENTS)),
        ),
        ("__cone__", deduplicate_mesh(cone::geometry(ROUND_SEGMENTS))),
        (
            "__capsule__",
            deduplicate_mesh(capsule::geometry(ROUND_SEGMENTS, CAPSULE_HEMISPHERE_BANDS)),
        ),
        ("__terrain64__", deduplicate_mesh(terrain64::geometry())),
    ];

    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();
    let mut ranges = AHashMap::new();
    let mut meshlets = AHashMap::new();

    for (name, (vertices, indices)) in presets {
        let base_vertex = all_vertices.len() as i32;
        let index_start = all_indices.len() as u32;
        let packed_indices = pack_indices_spatial(&vertices, &indices);
        let index_count = packed_indices.len() as u32;
        let preset_meshlets = build_meshlets_for_range(&vertices, &packed_indices, index_start);
        all_vertices.extend(vertices);
        all_indices.extend(packed_indices);
        ranges.insert(
            name,
            MeshRange {
                index_start,
                index_count,
                base_vertex,
            },
        );
        meshlets.insert(name, Arc::from(preset_meshlets));
    }

    (all_vertices, all_indices, ranges, meshlets)
}

fn deduplicate_mesh(
    (vertices, indices): (Vec<MeshVertex>, Vec<u16>),
) -> (Vec<MeshVertex>, Vec<u32>) {
    let mut unique_vertices = Vec::with_capacity(vertices.len());
    let mut remap = vec![0u16; vertices.len()];
    let mut vertex_to_index = AHashMap::with_capacity(vertices.len());

    for (old_index, vertex) in vertices.into_iter().enumerate() {
        let key = MeshVertexKey::from(vertex);
        let new_index = match vertex_to_index.get(&key) {
            Some(index) => *index,
            None => {
                let index =
                    u16::try_from(unique_vertices.len()).expect("mesh vertex count exceeds u16");
                unique_vertices.push(vertex);
                vertex_to_index.insert(key, index);
                index
            }
        };
        remap[old_index] = new_index;
    }

    let remapped_indices = indices
        .into_iter()
        .map(|index| remap[index as usize] as u32)
        .collect();
    (unique_vertices, remapped_indices)
}

const MESHLET_TRIANGLES: usize = 64;

fn build_meshlets_for_range(
    vertices: &[MeshVertex],
    indices: &[u32],
    base_index_start: u32,
) -> Vec<MeshletRange> {
    if indices.len() < 3 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let tri_index_len = (indices.len() / 3) * 3;
    let chunk = MESHLET_TRIANGLES * 3;
    let mut start = 0usize;
    while start < tri_index_len {
        let end = (start + chunk).min(tri_index_len);
        if let Some((center, radius)) = meshlet_bounds(vertices, &indices[start..end]) {
            out.push(MeshletRange {
                index_start: base_index_start + start as u32,
                index_count: (end - start) as u32,
                center,
                radius,
            });
        }
        start = end;
    }
    out
}

fn pack_indices_spatial(vertices: &[MeshVertex], indices: &[u32]) -> Vec<u32> {
    let tri_len = (indices.len() / 3) * 3;
    if tri_len == 0 {
        return indices.to_vec();
    }

    let mut centroids = Vec::with_capacity(tri_len / 3);
    let mut cmin = [f32::INFINITY; 3];
    let mut cmax = [f32::NEG_INFINITY; 3];
    for tri in indices[..tri_len].chunks_exact(3) {
        let Some(a) = vertices.get(tri[0] as usize) else {
            return indices.to_vec();
        };
        let Some(b) = vertices.get(tri[1] as usize) else {
            return indices.to_vec();
        };
        let Some(c) = vertices.get(tri[2] as usize) else {
            return indices.to_vec();
        };
        let centroid = [
            (a.pos[0] + b.pos[0] + c.pos[0]) / 3.0,
            (a.pos[1] + b.pos[1] + c.pos[1]) / 3.0,
            (a.pos[2] + b.pos[2] + c.pos[2]) / 3.0,
        ];
        cmin[0] = cmin[0].min(centroid[0]);
        cmin[1] = cmin[1].min(centroid[1]);
        cmin[2] = cmin[2].min(centroid[2]);
        cmax[0] = cmax[0].max(centroid[0]);
        cmax[1] = cmax[1].max(centroid[1]);
        cmax[2] = cmax[2].max(centroid[2]);
        centroids.push((tri, centroid));
    }

    let span = [
        (cmax[0] - cmin[0]).max(1.0e-6),
        (cmax[1] - cmin[1]).max(1.0e-6),
        (cmax[2] - cmin[2]).max(1.0e-6),
    ];
    let mut keyed = Vec::with_capacity(centroids.len());
    for (tri, c) in centroids {
        let nx = ((c[0] - cmin[0]) / span[0]).clamp(0.0, 1.0);
        let ny = ((c[1] - cmin[1]) / span[1]).clamp(0.0, 1.0);
        let nz = ((c[2] - cmin[2]) / span[2]).clamp(0.0, 1.0);
        keyed.push((morton3(nx, ny, nz), [tri[0], tri[1], tri[2]]));
    }
    keyed.sort_unstable_by_key(|(key, _)| *key);

    let mut packed = Vec::with_capacity(indices.len());
    for (_, tri) in keyed {
        packed.extend_from_slice(&tri);
    }
    if tri_len < indices.len() {
        packed.extend_from_slice(&indices[tri_len..]);
    }
    packed
}

#[inline]
fn morton3(nx: f32, ny: f32, nz: f32) -> u64 {
    let qx = (nx * 1023.0).round() as u32;
    let qy = (ny * 1023.0).round() as u32;
    let qz = (nz * 1023.0).round() as u32;
    interleave10(qx) | (interleave10(qy) << 1) | (interleave10(qz) << 2)
}

#[inline]
fn interleave10(v: u32) -> u64 {
    let mut x = (v & 0x3ff) as u64;
    x = (x | (x << 16)) & 0x30000ff;
    x = (x | (x << 8)) & 0x300f00f;
    x = (x | (x << 4)) & 0x30c30c3;
    x = (x | (x << 2)) & 0x9249249;
    x
}

fn meshlet_bounds(vertices: &[MeshVertex], indices: &[u32]) -> Option<([f32; 3], f32)> {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for &idx in indices {
        let pos = vertices.get(idx as usize)?.pos;
        min[0] = min[0].min(pos[0]);
        min[1] = min[1].min(pos[1]);
        min[2] = min[2].min(pos[2]);
        max[0] = max[0].max(pos[0]);
        max[1] = max[1].max(pos[1]);
        max[2] = max[2].max(pos[2]);
    }
    if !min.iter().all(|v| v.is_finite()) || !max.iter().all(|v| v.is_finite()) {
        return None;
    }
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let mut radius_sq = 0.0f32;
    for &idx in indices {
        let pos = vertices.get(idx as usize)?.pos;
        let dx = pos[0] - center[0];
        let dy = pos[1] - center[1];
        let dz = pos[2] - center[2];
        radius_sq = radius_sq.max(dx * dx + dy * dy + dz * dz);
    }
    Some((center, radius_sq.sqrt()))
}

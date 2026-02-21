use super::{MeshRange, MeshVertex};
use std::collections::HashMap;

mod capsule;
mod common;
mod cone;
mod cube;
mod cylinder;
mod sphere;
mod square_pyramid;
mod tri_prism;
mod triangular_pyramid;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MeshVertexKey {
    pos: [u32; 3],
    normal: [u32; 3],
}

impl From<MeshVertex> for MeshVertexKey {
    fn from(vertex: MeshVertex) -> Self {
        Self {
            pos: vertex.pos.map(f32::to_bits),
            normal: vertex.normal.map(f32::to_bits),
        }
    }
}

pub(super) fn build_builtin_mesh_buffer()
-> (Vec<MeshVertex>, Vec<u32>, HashMap<&'static str, MeshRange>) {
    const ROUND_SEGMENTS: u32 = 36;
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
            deduplicate_mesh(cylinder::geometry(ROUND_SEGMENTS)),
        ),
        ("__cone__", deduplicate_mesh(cone::geometry(ROUND_SEGMENTS))),
        (
            "__capsule__",
            deduplicate_mesh(capsule::geometry(ROUND_SEGMENTS, CAPSULE_HEMISPHERE_BANDS)),
        ),
    ];

    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();
    let mut ranges = HashMap::new();

    for (name, (vertices, indices)) in presets {
        let base_vertex = all_vertices.len() as i32;
        let index_start = all_indices.len() as u32;
        let index_count = indices.len() as u32;
        all_vertices.extend(vertices);
        all_indices.extend(indices);
        ranges.insert(
            name,
            MeshRange {
                index_start,
                index_count,
                base_vertex,
            },
        );
    }

    (all_vertices, all_indices, ranges)
}

fn deduplicate_mesh(
    (vertices, indices): (Vec<MeshVertex>, Vec<u16>),
) -> (Vec<MeshVertex>, Vec<u32>) {
    let mut unique_vertices = Vec::with_capacity(vertices.len());
    let mut remap = vec![0u16; vertices.len()];
    let mut vertex_to_index = HashMap::with_capacity(vertices.len());

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

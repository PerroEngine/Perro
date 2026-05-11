//! Built-in primitive query meshes for runtime mesh ray/region tests.

use super::{QueryMeshData, QueryTri, build_query_mesh_data};
use glam::Vec3;

pub(super) fn decode_builtin_query_mesh(source: &str) -> Option<QueryMeshData> {
    let mesh = perro_builtin_meshes::build_builtin_mesh(source)?;
    let vertices = mesh
        .vertices
        .into_iter()
        .map(|vertex| Vec3::from(vertex.pos))
        .collect();
    let triangles = mesh
        .indices
        .chunks_exact(3)
        .map(|tri| QueryTri {
            a: tri[0] as u32,
            b: tri[1] as u32,
            c: tri[2] as u32,
            surface_index: 0,
        })
        .collect();
    build_query_mesh_data(vertices, triangles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_builtins_use_shared_mesh_source_set() {
        for source in perro_builtin_meshes::BUILTIN_MESH_SOURCES {
            let mesh = decode_builtin_query_mesh(source).expect("query builtin mesh");
            assert!(!mesh.vertices.is_empty(), "{source} vertices");
            assert!(!mesh.triangles.is_empty(), "{source} triangles");
            assert!(mesh.triangles.iter().all(|tri| tri.surface_index == 0));
        }
    }
}

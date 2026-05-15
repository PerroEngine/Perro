use super::*;
use perro_structs::Unorm8x4;

#[test]
fn runtime_mesh_data_builds_query_surfaces() {
    let vertex = |position| perro_render_bridge::RuntimeMeshVertex {
        position,
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
        joints: [0; 4],
        weights: Unorm8x4::ZERO,
    };
    let mesh = Mesh3D {
        vertices: vec![
            vertex([0.0, 0.0, 0.0]),
            vertex([1.0, 0.0, 0.0]),
            vertex([0.0, 1.0, 0.0]),
            vertex([0.0, 0.0, 1.0]),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        surface_ranges: vec![
            perro_render_bridge::MeshSurfaceRange {
                index_start: 0,
                index_count: 3,
            },
            perro_render_bridge::MeshSurfaceRange {
                index_start: 3,
                index_count: 3,
            },
        ],
    };

    let query = build_query_mesh_from_runtime_mesh(&mesh).expect("query mesh");

    assert_eq!(query.triangles.len(), 2);
    assert_eq!(query.triangles[0].surface_index, 0);
    assert_eq!(query.triangles[1].surface_index, 1);
}

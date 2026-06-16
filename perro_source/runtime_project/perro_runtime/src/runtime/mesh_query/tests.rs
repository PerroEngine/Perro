use super::*;
use perro_structs::UnitVector4;

#[test]
fn runtime_mesh_data_builds_query_surfaces() {
    let vertex = |position| perro_render_bridge::RuntimeMeshVertex {
        position,
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
        joints: [0; 4],
        weights: UnitVector4::ZERO,
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
        blend_shapes: Vec::new(),
    };

    let query = build_query_mesh_from_runtime_mesh(&mesh).expect("query mesh");

    assert_eq!(query.triangles.len(), 2);
    assert_eq!(query.triangles[0].surface_index, 0);
    assert_eq!(query.triangles[1].surface_index, 1);
}

#[test]
fn batch_global_ray_query_preserves_order_and_surface_index() {
    let vertex = |position| perro_render_bridge::RuntimeMeshVertex {
        position,
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
        joints: [0; 4],
        weights: UnitVector4::ZERO,
    };
    let mesh = Mesh3D {
        vertices: vec![
            vertex([-1.0, 0.0, -1.0]),
            vertex([1.0, 0.0, -1.0]),
            vertex([0.0, 0.0, 1.0]),
        ],
        indices: vec![0, 1, 2],
        surface_ranges: vec![],
        blend_shapes: Vec::new(),
    };
    let query = build_query_mesh_from_runtime_mesh(&mesh).expect("query mesh");
    let rays = [
        MeshSurfaceRay3D {
            origin: Vector3::new(0.0, 1.0, 0.0),
            direction: Vector3::new(0.0, -1.0, 0.0),
            max_distance: 4.0,
        },
        MeshSurfaceRay3D {
            origin: Vector3::new(4.0, 1.0, 4.0),
            direction: Vector3::new(0.0, -1.0, 0.0),
            max_distance: 4.0,
        },
    ];

    let hits: Vec<_> = rays
        .iter()
        .map(|ray| {
            query_global_ray_candidates_for_node_mesh(
                &query,
                &[Mat4::IDENTITY],
                Mat4::IDENTITY,
                *ray,
                false,
            )
        })
        .collect();

    assert_eq!(hits[0].as_ref().map(|hit| hit.surface_index), Some(0));
    assert!(hits[1].is_none());
}

use perro_structs::Vector3;
use perro_terrain::{ChunkCoord, ChunkError, TerrainChunk};

#[test]
fn flat_chunk_starts_with_dense_1m_grid() {
    let c = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    assert_eq!(c.vertex_count(), 65 * 65);
    assert_eq!(c.triangle_count(), 64 * 64 * 2);
    assert!(c.validate(1.0e-6).is_ok());
}

#[test]
fn flat_chunk_is_centered_around_origin() {
    let c = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let p0 = c.vertices()[0].position;
    let plast = c.vertices()[c.vertex_count() - 1].position;
    assert_eq!(p0, Vector3::new(-32.0, 0.0, -32.0));
    assert_eq!(plast, Vector3::new(32.0, 0.0, 32.0));
}

#[test]
fn add_vertex_and_triangle_works() {
    let mut c = TerrainChunk::new_flat_64m(ChunkCoord::new(2, -1));
    let center = c.add_vertex(Vector3::new(32.0, 2.0, 32.0));
    let tri_id = c
        .add_triangle(0, 1, center)
        .expect("triangle should be valid");
    assert_eq!(tri_id, 64 * 64 * 2);
    assert_eq!(c.vertex_count(), 65 * 65 + 1);
    assert_eq!(c.triangle_count(), 64 * 64 * 2 + 1);
}

#[test]
fn add_triangle_rejects_bad_indices() {
    let mut c = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let bad_index = c.vertex_count() + 100;
    let err = c
        .add_triangle(0, 1, bad_index)
        .expect_err("invalid index should fail");
    assert_eq!(err, ChunkError::InvalidVertexID { vertex_id: bad_index });
}

#[test]
fn validate_rejects_degenerate_triangles() {
    let mut c = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let first_extra_tri = c.triangle_count();
    let a = c.add_vertex(Vector3::new(10.0, 0.0, 10.0));
    let b = c.add_vertex(Vector3::new(20.0, 0.0, 20.0));
    let d = c.add_vertex(Vector3::new(30.0, 0.0, 30.0));
    c.add_triangle(a, b, d)
        .expect("indices are valid, triangle insert should succeed");

    let err = c.validate(1.0e-6).expect_err("validation should fail");
    assert_eq!(err, ChunkError::DegenerateTriangle { triangle_id: first_extra_tri });
}

#[test]
fn set_vertex_position_updates_existing_vertex() {
    let mut c = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    c.set_vertex_position(0, Vector3::new(-3.0, 5.0, 7.0))
        .expect("vertex should exist");
    assert_eq!(c.vertices()[0].position, Vector3::new(-3.0, 5.0, 7.0));
}

use perro_structs::Vector3;
use perro_terrain::{ChunkCoord, TerrainChunk};

#[test]
fn center_vertex_on_flat_plane_gets_optimized_away() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let result = chunk
        .insert_vertex(Vector3::new(0.0, 0.0, 0.0))
        .expect("insert should succeed");

    assert!(result.removed_as_coplanar);
    assert_eq!(chunk.vertex_count(), 4);
    assert_eq!(chunk.triangle_count(), 2);
    assert!(chunk.validate(1.0e-6).is_ok());
}

#[test]
fn center_vertex_with_height_is_kept_and_creates_peak_topology() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let result = chunk
        .insert_vertex(Vector3::new(0.0, 8.0, 0.0))
        .expect("insert should succeed");

    assert!(!result.removed_as_coplanar);
    assert_eq!(chunk.vertex_count(), 5);
    assert_eq!(chunk.triangle_count(), 4);
    assert!(chunk.validate(1.0e-6).is_ok());
}

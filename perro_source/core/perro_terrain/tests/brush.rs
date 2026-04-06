use perro_structs::Vector3;
use perro_terrain::{BrushShape, ChunkCoord, TerrainChunk};

#[test]
fn insert_brush_touches_existing_grid_vertices_only() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let results = chunk
        .insert_brush(Vector3::new(0.0, 0.0, 0.0), 10.0, BrushShape::Square)
        .expect("square brush insert should succeed");

    assert!(!results.is_empty());
    assert!(results.iter().all(|r| r.removed_as_coplanar));
    assert_eq!(chunk.vertex_count(), 65 * 65);
    assert_eq!(chunk.triangle_count(), 64 * 64 * 2);
    assert!(chunk.validate(1.0e-6).is_ok());
}

#[test]
fn larger_brush_touches_more_vertices() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let small = chunk
        .insert_brush(Vector3::new(0.0, 0.0, 0.0), 6.0, BrushShape::Circle)
        .expect("small circle insert should succeed");
    let large = chunk
        .insert_brush(Vector3::new(0.0, 0.0, 0.0), 20.0, BrushShape::Circle)
        .expect("large circle insert should succeed");

    assert!(large.len() > small.len());
}

#[test]
fn triangle_brush_touches_some_vertices_and_keeps_topology() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let result = chunk
        .insert_brush(Vector3::new(0.0, 0.0, 0.0), 12.0, BrushShape::Triangle)
        .expect("triangle brush insert should succeed");

    assert!(!result.is_empty());
    assert_eq!(chunk.vertex_count(), 65 * 65);
    assert_eq!(chunk.triangle_count(), 64 * 64 * 2);
    assert!(chunk.validate(1.0e-6).is_ok());
}

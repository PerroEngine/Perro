use perro_structs::Vector3;
use perro_terrain::{BrushShape, ChunkCoord, TerrainChunk};

#[test]
fn square_brush_generates_four_corner_inserts_and_stays_lean_on_flat_plane() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let results = chunk
        .insert_brush(Vector3::new(0.0, 0.0, 0.0), 10.0, BrushShape::Square)
        .expect("square brush insert should succeed");

    assert_eq!(results.len(), 4);
    assert!(results.iter().all(|r| r.removed_as_coplanar));
    assert_eq!(chunk.vertex_count(), 4);
    assert_eq!(chunk.triangle_count(), 2);
    assert!(chunk.validate(1.0e-6).is_ok());
}

#[test]
fn square_brush_snaps_to_size_grid_without_decimal_coords_for_size_gt_one() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let _ = chunk
        .insert_brush(Vector3::new(3.2, 0.0, -7.4), 10.0, BrushShape::Square)
        .expect("square brush insert should succeed");

    for v in chunk.vertices() {
        assert!((v.position.x.fract()).abs() <= 1.0e-6);
        assert!((v.position.z.fract()).abs() <= 1.0e-6);
    }
}

#[test]
fn circle_brush_adapts_sample_count_by_size() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let s3 = chunk
        .insert_brush(Vector3::new(0.0, 0.0, 0.0), 3.0, BrushShape::Circle)
        .expect("3m circle insert should succeed");
    let s5 = chunk
        .insert_brush(Vector3::new(0.0, 0.0, 0.0), 5.0, BrushShape::Circle)
        .expect("5m circle insert should succeed");
    let small = chunk
        .insert_brush(Vector3::new(0.0, 0.0, 0.0), 10.0, BrushShape::Circle)
        .expect("small circle insert should succeed");
    let medium = chunk
        .insert_brush(Vector3::new(0.0, 0.0, 0.0), 20.0, BrushShape::Circle)
        .expect("medium circle insert should succeed");
    let large = chunk
        .insert_brush(Vector3::new(0.0, 0.0, 0.0), 30.0, BrushShape::Circle)
        .expect("large circle insert should succeed");

    assert_eq!(s3.len(), 5);
    assert_eq!(s5.len(), 6);
    assert_eq!(small.len(), 8);
    assert_eq!(medium.len(), 12);
    assert_eq!(large.len(), 16);
}

#[test]
fn triangle_brush_inserts_three_points_and_keeps_mesh_lean_on_flat() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let result = chunk
        .insert_brush(Vector3::new(0.0, 0.0, 0.0), 12.0, BrushShape::Triangle)
        .expect("triangle brush insert should succeed");

    assert_eq!(result.len(), 3);
    assert!(result.iter().all(|r| r.removed_as_coplanar));
    assert_eq!(chunk.vertex_count(), 4);
    assert_eq!(chunk.triangle_count(), 2);
}

#[test]
fn square_brush_on_raised_plane_keeps_non_coplanar_vertices() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let results = chunk
        .insert_brush(Vector3::new(0.0, 4.0, 0.0), 10.0, BrushShape::Square)
        .expect("square brush insert should succeed");

    assert_eq!(results.len(), 4);
    assert!(results.iter().any(|r| !r.removed_as_coplanar));
    assert!(chunk.vertex_count() > 4);
    assert!(chunk.triangle_count() > 2);
    assert!(chunk.validate(1.0e-6).is_ok());
}

#[test]
fn circle_brush_on_raised_plane_keeps_non_coplanar_vertices() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let results = chunk
        .insert_brush(Vector3::new(0.0, 4.0, 0.0), 10.0, BrushShape::Circle)
        .expect("circle brush insert should succeed");

    assert_eq!(results.len(), 8);
    assert!(results.iter().any(|r| !r.removed_as_coplanar));
    assert!(chunk.vertex_count() > 4);
    assert!(chunk.triangle_count() > 2);
    assert!(chunk.validate(1.0e-6).is_ok());
}

#[test]
fn triangle_brush_on_raised_plane_keeps_non_coplanar_vertices() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let results = chunk
        .insert_brush(Vector3::new(0.0, 4.0, 0.0), 12.0, BrushShape::Triangle)
        .expect("triangle brush insert should succeed");

    assert_eq!(results.len(), 3);
    assert!(results.iter().any(|r| !r.removed_as_coplanar));
    assert!(chunk.vertex_count() > 4);
    assert!(chunk.triangle_count() > 2);
    assert!(chunk.validate(1.0e-6).is_ok());
}

#[test]
fn detail_points_snap_to_half_meter_grid_for_sub_one_meter_brushes() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let _ = chunk
        .insert_brush(Vector3::new(0.0, 2.0, 0.0), 0.8, BrushShape::Circle)
        .expect("circle brush insert should succeed");

    for v in chunk.vertices() {
        let x_scaled = v.position.x * 2.0;
        let z_scaled = v.position.z * 2.0;
        assert!((x_scaled - x_scaled.round()).abs() <= 1.0e-6);
        assert!((z_scaled - z_scaled.round()).abs() <= 1.0e-6);
    }
}

#[test]
fn detail_points_snap_to_tenth_meter_grid_for_very_small_brushes() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let _ = chunk
        .insert_brush(Vector3::new(0.0, 2.0, 0.0), 0.2, BrushShape::Circle)
        .expect("circle brush insert should succeed");

    for v in chunk.vertices() {
        let x_scaled = v.position.x * 10.0;
        let z_scaled = v.position.z * 10.0;
        assert!((x_scaled - x_scaled.round()).abs() <= 1.0e-6);
        assert!((z_scaled - z_scaled.round()).abs() <= 1.0e-6);
    }
}

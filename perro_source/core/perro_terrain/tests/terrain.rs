use perro_structs::Vector3;
use perro_terrain::{BrushOp, BrushShape, ChunkCoord, TerrainData};

const EPS: f32 = 1.0e-4;

#[test]
fn brush_spanning_two_chunks_touches_both_chunks() {
    let mut terrain = TerrainData::new(64.0);
    terrain.ensure_chunk(ChunkCoord::new(0, 0));
    terrain.ensure_chunk(ChunkCoord::new(1, 0));

    let summary = terrain
        .insert_brush_world(Vector3::new(32.0, 4.0, 0.0), 10.0, BrushShape::Circle)
        .expect("multi-chunk brush should succeed");

    assert!(summary.touched_chunks.contains(&ChunkCoord::new(0, 0)));
    assert!(summary.touched_chunks.contains(&ChunkCoord::new(1, 0)));
}

#[test]
fn seam_vertices_align_after_cross_chunk_edit() {
    let mut terrain = TerrainData::new(64.0);
    terrain.ensure_chunk(ChunkCoord::new(0, 0));
    terrain.ensure_chunk(ChunkCoord::new(1, 0));

    let _ = terrain
        .insert_brush_world(Vector3::new(32.0, 6.0, 0.0), 12.0, BrushShape::Circle)
        .expect("multi-chunk brush should succeed");

    let left = terrain.chunk(ChunkCoord::new(0, 0)).expect("left chunk exists");
    let right = terrain.chunk(ChunkCoord::new(1, 0)).expect("right chunk exists");

    let mut left_border = border_world_points(left.vertices(), 32.0, 0.0);
    let mut right_border = border_world_points(right.vertices(), -32.0, 64.0);

    left_border.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    right_border.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    for (lz, ly) in &left_border {
        let Some((_, ry)) = right_border.iter().find(|(rz, _)| (*rz - *lz).abs() <= EPS) else {
            continue;
        };
        assert!((ly - ry).abs() <= 1.0e-3);
    }
}

#[test]
fn brush_three_meters_from_edge_with_radius_over_three_spans_both_chunks() {
    let mut terrain = TerrainData::new(64.0);
    terrain.ensure_chunk(ChunkCoord::new(0, 0));
    terrain.ensure_chunk(ChunkCoord::new(1, 0));

    // Edge between chunk (0,0) and (1,0) is at world X=32.
    // Center is 3m from edge (x=29), brush radius is 4m (size=8), so it must cross.
    let summary = terrain
        .insert_brush_world(Vector3::new(29.0, 3.0, 0.0), 8.0, BrushShape::Circle)
        .expect("near-edge brush should succeed");

    assert!(summary.touched_chunks.contains(&ChunkCoord::new(0, 0)));
    assert!(summary.touched_chunks.contains(&ChunkCoord::new(1, 0)));
}

#[test]
fn set_height_brush_op_spanning_chunks_keeps_seam_aligned() {
    let mut terrain = TerrainData::new(64.0);
    terrain.ensure_chunk(ChunkCoord::new(0, 0));
    terrain.ensure_chunk(ChunkCoord::new(1, 0));

    let summary = terrain
        .apply_brush_op_world(
            Vector3::new(29.0, 0.0, 0.0),
            8.0,
            BrushShape::Square,
            BrushOp::SetHeight {
                y: 6.0,
                basis: 1.0,
                feature_offset: 0.1,
            },
        )
        .expect("set-height op should succeed");

    assert!(summary.touched_chunks.contains(&ChunkCoord::new(0, 0)));
    assert!(summary.touched_chunks.contains(&ChunkCoord::new(1, 0)));
    assert_seam_y_aligned(&terrain);
}

#[test]
fn add_remove_brush_ops_spanning_chunks_keep_seam_aligned() {
    let mut terrain = TerrainData::new(64.0);
    terrain.ensure_chunk(ChunkCoord::new(0, 0));
    terrain.ensure_chunk(ChunkCoord::new(1, 0));

    let _ = terrain
        .apply_brush_op_world(
            Vector3::new(29.0, 0.0, 0.0),
            8.0,
            BrushShape::Circle,
            BrushOp::Add { delta: 2.5, basis: 1.0 },
        )
        .expect("add op should succeed");
    let _ = terrain
        .apply_brush_op_world(
            Vector3::new(29.0, 0.0, 0.0),
            8.0,
            BrushShape::Circle,
            BrushOp::Remove { delta: 1.0, basis: 1.0 },
        )
        .expect("remove op should succeed");

    assert_seam_y_aligned(&terrain);
}

#[test]
fn decimate_brush_op_spanning_chunks_keeps_seam_aligned() {
    let mut terrain = TerrainData::new(64.0);
    terrain.ensure_chunk(ChunkCoord::new(0, 0));
    terrain.ensure_chunk(ChunkCoord::new(1, 0));

    let _ = terrain
        .apply_brush_op_world(
            Vector3::new(29.0, 0.0, 0.0),
            8.0,
            BrushShape::Circle,
            BrushOp::Add { delta: 2.1, basis: 1.0 },
        )
        .expect("add op should succeed");
    let _ = terrain
        .apply_brush_op_world(
            Vector3::new(29.0, 0.0, 0.0),
            8.0,
            BrushShape::Circle,
            BrushOp::Decimate { basis: 0.25 },
        )
        .expect("decimate op should succeed");

    assert_seam_y_aligned(&terrain);
}

fn assert_seam_y_aligned(terrain: &TerrainData) {
    let left = terrain.chunk(ChunkCoord::new(0, 0)).expect("left chunk exists");
    let right = terrain.chunk(ChunkCoord::new(1, 0)).expect("right chunk exists");

    let mut left_border = border_world_points(left.vertices(), 32.0, 0.0);
    let mut right_border = border_world_points(right.vertices(), -32.0, 64.0);
    left_border.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    right_border.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    for (lz, ly) in &left_border {
        let Some((_, ry)) = right_border.iter().find(|(rz, _)| (*rz - *lz).abs() <= EPS) else {
            continue;
        };
        assert!((ly - ry).abs() <= 1.0e-3);
    }
}

fn border_world_points(vertices: &[perro_terrain::Vertex], border_x_local: f32, world_center_x: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    for v in vertices {
        if (v.position.x - border_x_local).abs() <= EPS {
            let world_z = v.position.z;
            let _world_x = v.position.x + world_center_x;
            out.push((world_z, v.position.y));
        }
    }
    out
}

#[test]
fn raycast_hits_flat_chunk_at_world_origin_without_half_chunk_offset() {
    let mut terrain = TerrainData::new(64.0);
    terrain.ensure_chunk(ChunkCoord::new(0, 0));

    let hit = terrain
        .raycast_world(
            Vector3::new(0.0, 20.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0,
        )
        .expect("ray should hit flat terrain");

    assert_eq!(hit.chunk, ChunkCoord::new(0, 0));
    assert!(hit.position_world.x.abs() <= 1.0e-4);
    assert!(hit.position_world.z.abs() <= 1.0e-4);
    assert!(hit.position_world.y.abs() <= 1.0e-4);
}

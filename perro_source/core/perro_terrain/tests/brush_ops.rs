use perro_structs::Vector3;
use perro_terrain::{BrushOp, BrushShape, ChunkCoord, TerrainChunk};
use std::collections::HashMap;

#[test]
fn set_height_square_builds_top_and_base_points() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let results = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 0.0),
            10.0,
            BrushShape::Square,
            BrushOp::SetHeight {
                y: 5.0,
                basis: 1.0,
                feature_offset: 0.1,
            },
        )
        .expect("set-height square should succeed");

    assert!(results.len() >= 8);
    assert!(
        results.iter().all(|r| !r.removed_as_coplanar),
        "set-height structural inserts should not collapse as coplanar"
    );
    assert!(chunk.vertex_count() >= 12);
    assert!(chunk.triangle_count() >= 10);

    let ys: Vec<f32> = chunk.vertices().iter().map(|v| v.position.y).collect();
    assert!(ys.iter().any(|y| *y >= 4.9));
    assert!(ys.iter().any(|y| y.abs() <= 1.0e-4));

    assert!(chunk.validate(1.0e-6).is_ok());
}

#[test]
fn set_height_circle_builds_double_ring_count() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let results = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 0.0),
            10.0,
            BrushShape::Circle,
            BrushOp::SetHeight {
                y: 3.0,
                basis: 1.0,
                feature_offset: 0.1,
            },
        )
        .expect("set-height circle should succeed");

    assert!(results.len() >= 16);
    assert!(chunk.vertex_count() >= 20);
}

#[test]
fn set_height_negative_is_supported() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let _ = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 0.0),
            12.0,
            BrushShape::Square,
            BrushOp::SetHeight {
                y: -4.0,
                basis: 1.0,
                feature_offset: 0.1,
            },
        )
        .expect("negative set-height should succeed");

    let min_y = chunk
        .vertices()
        .iter()
        .fold(f32::INFINITY, |m, v| m.min(v.position.y));
    assert!(min_y <= -3.9);
    assert!(chunk.vertex_count() >= 12);
}

#[test]
fn add_remove_and_decimate_ops_work() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let add_results = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 0.0),
            10.0,
            BrushShape::Square,
            BrushOp::Add {
                delta: 2.0,
                basis: 1.0,
            },
        )
        .expect("add should succeed");
    assert!(
        add_results.len() >= 100,
        "dense edit should affect many in-brush vertices"
    );
    let has_base = chunk
        .vertices()
        .iter()
        .any(|v| v.position.y.abs() <= 1.0e-3);
    let has_raised = chunk
        .vertices()
        .iter()
        .any(|v| (v.position.y - 2.0).abs() <= 1.0e-3);
    assert!(has_base, "expected base ring near y=0");
    assert!(has_raised, "expected raised interior near y=2");

    let max_after_add = chunk
        .vertices()
        .iter()
        .fold(f32::NEG_INFINITY, |m, v| m.max(v.position.y));
    assert!(max_after_add > 0.5);

    let _ = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 0.0),
            10.0,
            BrushShape::Square,
            BrushOp::Remove {
                delta: 1.25,
                basis: 1.0,
            },
        )
        .expect("remove should succeed");

    let _ = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 0.0),
            10.0,
            BrushShape::Circle,
            BrushOp::Decimate { basis: 0.25 },
        )
        .expect("decimate should succeed");

    for v in chunk.vertices() {
        let q = v.position.y / 0.25;
        assert!((q - q.round()).abs() <= 1.0e-4);
    }
}

#[test]
fn smooth_op_reduces_peak_height() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let _ = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 0.0),
            10.0,
            BrushShape::Circle,
            BrushOp::Add {
                delta: 2.0,
                basis: 1.0,
            },
        )
        .expect("add should succeed");

    let before = chunk
        .vertices()
        .iter()
        .fold(f32::NEG_INFINITY, |m, v| m.max(v.position.y));
    let _ = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 0.0),
            10.0,
            BrushShape::Circle,
            BrushOp::Smooth {
                strength: 1.0,
                basis: 1.0,
            },
        )
        .expect("smooth should succeed");
    let after = chunk
        .vertices()
        .iter()
        .fold(f32::NEG_INFINITY, |m, v| m.max(v.position.y));

    assert!(after <= before);
}

#[test]
fn set_height_reconcile_enforces_single_height_per_xz() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));

    let _ = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 0.0),
            10.0,
            BrushShape::Square,
            BrushOp::SetHeight {
                y: 5.0,
                basis: 1.0,
                feature_offset: 0.1,
            },
        )
        .expect("first set-height should succeed");

    let _ = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 0.0),
            10.0,
            BrushShape::Square,
            BrushOp::SetHeight {
                y: 8.0,
                basis: 1.0,
                feature_offset: 0.1,
            },
        )
        .expect("second set-height should succeed");

    let mut by_xz: HashMap<(i64, i64), f32> = HashMap::new();
    let eps = 1.0e-4_f32;
    for v in chunk.vertices() {
        let key = (
            (v.position.x / eps).round() as i64,
            (v.position.z / eps).round() as i64,
        );
        if let Some(prev_y) = by_xz.get(&key).copied() {
            assert!(
                (prev_y - v.position.y).abs() <= 1.0e-3,
                "same xz had two y values: {prev_y} vs {}",
                v.position.y
            );
        } else {
            by_xz.insert(key, v.position.y);
        }
    }

    assert!(chunk.validate(1.0e-6).is_ok());
    assert!(chunk.vertex_count() >= 20);
}

#[test]
fn adjacent_set_height_same_height_merges_shared_edge_and_reconciles() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));

    let _ = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 0.0),
            10.0,
            BrushShape::Square,
            BrushOp::SetHeight {
                y: 5.0,
                basis: 1.0,
                feature_offset: 0.1,
            },
        )
        .expect("first set-height should succeed");

    let _ = chunk
        .apply_brush_op(
            Vector3::new(0.0, 0.0, 10.0),
            10.0,
            BrushShape::Square,
            BrushOp::SetHeight {
                y: 5.0,
                basis: 1.0,
                feature_offset: 0.1,
            },
        )
        .expect("second adjacent set-height should succeed");

    let mut by_xz: HashMap<(i64, i64), f32> = HashMap::new();
    let eps = 1.0e-4_f32;
    for v in chunk.vertices() {
        let key = (
            (v.position.x / eps).round() as i64,
            (v.position.z / eps).round() as i64,
        );
        if let Some(prev_y) = by_xz.get(&key).copied() {
            assert!(
                (prev_y - v.position.y).abs() <= 1.0e-3,
                "same xz had two y values: {prev_y} vs {}",
                v.position.y
            );
        } else {
            by_xz.insert(key, v.position.y);
        }
    }

    assert!(chunk.vertex_count() >= 12);
    assert!(chunk.validate(1.0e-6).is_ok());
}

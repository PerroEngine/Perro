use crate::{ChunkError, InsertVertexResult, TerrainChunk};
use perro_structs::Vector3;
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrushShape {
    Square,
    Circle,
    Triangle,
}

impl TerrainChunk {
    pub fn insert_brush(
        &mut self,
        center: Vector3,
        brush_size_meters: f32,
        shape: BrushShape,
    ) -> Result<Vec<InsertVertexResult>, ChunkError> {
        if !brush_size_meters.is_finite() || brush_size_meters <= 0.0 {
            return Ok(Vec::new());
        }

        let snapped_size = normalize_size_for_snap(brush_size_meters);
        let points: Vec<Vector3> = match shape {
            BrushShape::Square => square_brush_points_snapped(center, snapped_size).into(),
            BrushShape::Circle => circle_brush_points_snapped(center, snapped_size),
            BrushShape::Triangle => triangle_brush_points_snapped(center, snapped_size).into(),
        };

        let mut out = Vec::with_capacity(points.len());
        for p in points {
            match self.insert_vertex(p) {
                Ok(r) => out.push(r),
                Err(ChunkError::PointOutsideMesh { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }
}

fn normalize_size_for_snap(size: f32) -> f32 {
    if size > 1.0 {
        size.round().max(1.0)
    } else {
        size
    }
}

fn square_brush_points_snapped(center: Vector3, size: f32) -> [Vector3; 4] {
    let min_x = snap_to_grid(center.x - size * 0.5, size);
    let min_z = snap_to_grid(center.z - size * 0.5, size);
    let max_x = min_x + size;
    let max_z = min_z + size;
    [
        Vector3::new(min_x, center.y, min_z),
        Vector3::new(max_x, center.y, min_z),
        Vector3::new(min_x, center.y, max_z),
        Vector3::new(max_x, center.y, max_z),
    ]
}

fn circle_brush_points_snapped(center: Vector3, size: f32) -> Vec<Vector3> {
    let radius = size * 0.5;
    let snap_step = detail_snap_step(size);
    let cx = snap_to_grid(center.x, snap_step);
    let cz = snap_to_grid(center.z, snap_step);
    let ring_samples = circle_ring_samples(size);

    let mut points = Vec::with_capacity(ring_samples as usize);

    // Keep cardinal points aligned to axes by offsetting angle so sample 0 is +X.
    for i in 0..ring_samples {
        let t = (i as f32) / (ring_samples as f32);
        let angle = t * (2.0 * PI);
        let x = cx + radius * angle.cos();
        let z = cz + radius * angle.sin();
        points.push(Vector3::new(
            snap_to_grid(x, snap_step),
            center.y,
            snap_to_grid(z, snap_step),
        ));
    }
    dedupe_points(points)
}

fn triangle_brush_points_snapped(center: Vector3, size: f32) -> [Vector3; 3] {
    let radius = size * 0.5;
    let snap_step = detail_snap_step(size);
    let cx = snap_to_grid(center.x, snap_step);
    let cz = snap_to_grid(center.z, snap_step);
    let mut out = [Vector3::new(0.0, center.y, 0.0); 3];
    for (i, p) in out.iter_mut().enumerate() {
        let angle = FRAC_PI_2 + (i as f32) * (2.0 * PI / 3.0);
        let x = cx + radius * angle.cos();
        let z = cz + radius * angle.sin();
        *p = Vector3::new(snap_to_grid(x, snap_step), center.y, snap_to_grid(z, snap_step));
    }
    out
}

fn circle_ring_samples(size: f32) -> u16 {
    if size <= 3.0 {
        5
    } else if size <= 5.0 {
        6
    } else if size <= 10.0 {
        8
    } else if size <= 20.0 {
        12
    } else if size <= 40.0 {
        16
    } else {
        32
    }
}

fn dedupe_points(points: Vec<Vector3>) -> Vec<Vector3> {
    let mut out: Vec<Vector3> = Vec::with_capacity(points.len());
    for p in points {
        let is_dup = out.iter().any(|e| {
            (e.x - p.x).abs() <= 1.0e-6 && (e.y - p.y).abs() <= 1.0e-6 && (e.z - p.z).abs() <= 1.0e-6
        });
        if !is_dup {
            out.push(p);
        }
    }
    out
}

fn detail_snap_step(size: f32) -> f32 {
    if size > 1.0 {
        1.0
    } else if size > 0.5 {
        0.5
    } else if size > 0.25 {
        0.25
    } else {
        0.1
    }
}

fn snap_to_grid(value: f32, step: f32) -> f32 {
    if step <= 0.0 {
        return value;
    }
    let snapped = (value / step).round() * step;
    if step > 1.0 {
        snapped.round()
    } else {
        snapped
    }
}

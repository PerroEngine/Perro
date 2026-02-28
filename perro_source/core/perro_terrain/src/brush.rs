use crate::{ChunkError, InsertVertexResult, TerrainChunk};
use perro_structs::Vector3;
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrushShape {
    Square,
    Circle,
    Triangle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BrushOp {
    SetHeight { y: f32, feature_offset: f32 },
    Add { delta: f32 },
    Remove { delta: f32 },
    Smooth { strength: f32 },
    Decimate { basis: f32 },
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

    pub fn apply_brush_op(
        &mut self,
        center: Vector3,
        brush_size_meters: f32,
        shape: BrushShape,
        op: BrushOp,
    ) -> Result<Vec<InsertVertexResult>, ChunkError> {
        if !brush_size_meters.is_finite() || brush_size_meters <= 0.0 {
            return Ok(Vec::new());
        }

        let snapped_size = normalize_size_for_snap(brush_size_meters);
        let xz_points = brush_xz_points(center, snapped_size, shape);
        if xz_points.is_empty() {
            return Ok(Vec::new());
        }

        match op {
            BrushOp::SetHeight { y, feature_offset } => {
                self.apply_set_height_feature(center, snapped_size, shape, y, feature_offset)
            }
            BrushOp::Add { delta } => {
                let targets: Vec<Vector3> = xz_points
                    .iter()
                    .map(|p| {
                        let current_y = self.sample_height_at_xz(p.x, p.z).unwrap_or(0.0);
                        Vector3::new(p.x, current_y + delta, p.z)
                    })
                    .collect();
                self.apply_points(targets)
            }
            BrushOp::Remove { delta } => {
                let targets: Vec<Vector3> = xz_points
                    .iter()
                    .map(|p| {
                        let current_y = self.sample_height_at_xz(p.x, p.z).unwrap_or(0.0);
                        Vector3::new(p.x, current_y - delta, p.z)
                    })
                    .collect();
                self.apply_points(targets)
            }
            BrushOp::Smooth { strength } => {
                let strength = strength.clamp(0.0, 1.0);
                let current: Vec<f32> = xz_points
                    .iter()
                    .map(|p| self.sample_height_at_xz(p.x, p.z).unwrap_or(0.0))
                    .collect();
                let avg = if current.is_empty() {
                    0.0
                } else {
                    current.iter().sum::<f32>() / current.len() as f32
                };
                let targets: Vec<Vector3> = xz_points
                    .iter()
                    .zip(current.iter())
                    .map(|(p, y0)| Vector3::new(p.x, *y0 + (avg - *y0) * strength, p.z))
                    .collect();
                self.apply_points(targets)
            }
            BrushOp::Decimate { basis } => {
                if basis <= 0.0 || !basis.is_finite() {
                    return Ok(Vec::new());
                }
                let targets: Vec<Vector3> = xz_points
                    .iter()
                    .map(|p| {
                        let y0 = self.sample_height_at_xz(p.x, p.z).unwrap_or(0.0);
                        let y = (y0 / basis).round() * basis;
                        Vector3::new(p.x, y, p.z)
                    })
                    .collect();
                self.apply_points(targets)
            }
        }
    }

    fn apply_set_height_feature(
        &mut self,
        center: Vector3,
        size: f32,
        shape: BrushShape,
        target_y: f32,
        feature_offset: f32,
    ) -> Result<Vec<InsertVertexResult>, ChunkError> {
        let offset = feature_offset.abs();
        let top_ring = brush_xz_points(center, size, shape);
        let mut targets = Vec::with_capacity(top_ring.len() * 2);
        for p in &top_ring {
            targets.push(Vector3::new(p.x, target_y, p.z));
        }
        for p in &top_ring {
            let dir = radial_dir_xz(center, *p);
            let base_x = p.x - dir.x * offset;
            let base_z = p.z - dir.z * offset;
            let base_y = self.sample_height_at_xz(p.x, p.z).unwrap_or(0.0);
            targets.push(Vector3::new(base_x, base_y, base_z));
        }
        self.apply_points_structural(targets)
    }

    fn apply_points(&mut self, points: Vec<Vector3>) -> Result<Vec<InsertVertexResult>, ChunkError> {
        let mut out = Vec::with_capacity(points.len());
        for p in points {
            match self.insert_vertex(p) {
                Ok(r) => out.push(r),
                Err(ChunkError::PointOutsideMesh { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        self.reconcile_after_edit();
        Ok(out)
    }

    fn apply_points_structural(
        &mut self,
        points: Vec<Vector3>,
    ) -> Result<Vec<InsertVertexResult>, ChunkError> {
        let mut out = Vec::with_capacity(points.len());
        for p in points {
            match self.insert_vertex_structural(p) {
                Ok(r) => out.push(r),
                Err(ChunkError::PointOutsideMesh { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        self.reconcile_after_edit();
        Ok(out)
    }

    fn sample_height_at_xz(&self, x: f32, z: f32) -> Option<f32> {
        for tri in &self.triangles {
            let a = self.vertices[tri.a].position;
            let b = self.vertices[tri.b].position;
            let c = self.vertices[tri.c].position;
            let Some((w0, w1, w2)) = barycentric_xz(x, z, a, b, c, 1.0e-6) else {
                continue;
            };
            if w0 >= -1.0e-5 && w1 >= -1.0e-5 && w2 >= -1.0e-5 {
                return Some(w0 * a.y + w1 * b.y + w2 * c.y);
            }
        }
        None
    }
}

fn brush_xz_points(center: Vector3, size: f32, shape: BrushShape) -> Vec<Vector3> {
    match shape {
        BrushShape::Square => square_brush_points_snapped(center, size).into(),
        BrushShape::Circle => circle_brush_points_snapped(center, size),
        BrushShape::Triangle => triangle_brush_points_snapped(center, size).into(),
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

fn radial_dir_xz(center: Vector3, point: Vector3) -> Vector3 {
    let dx = point.x - center.x;
    let dz = point.z - center.z;
    let len = (dx * dx + dz * dz).sqrt();
    if len <= 1.0e-6 {
        Vector3::new(1.0, 0.0, 0.0)
    } else {
        Vector3::new(dx / len, 0.0, dz / len)
    }
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

fn barycentric_xz(
    x: f32,
    z: f32,
    a: Vector3,
    b: Vector3,
    c: Vector3,
    eps: f32,
) -> Option<(f32, f32, f32)> {
    let p = (x, z);
    let a2 = (a.x, a.z);
    let b2 = (b.x, b.z);
    let c2 = (c.x, c.z);

    let area = cross2(sub2(b2, a2), sub2(c2, a2));
    if area.abs() <= eps {
        return None;
    }

    let w0 = cross2(sub2(b2, p), sub2(c2, p)) / area;
    let w1 = cross2(sub2(c2, p), sub2(a2, p)) / area;
    let w2 = cross2(sub2(a2, p), sub2(b2, p)) / area;
    Some((w0, w1, w2))
}

fn sub2(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0 - b.0, a.1 - b.1)
}

fn cross2(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.1 - a.1 * b.0
}

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
    /// Directly sets touched vertex heights (no falloff).
    SetHeight {
        y: f32,
        basis: f32,
        feature_offset: f32,
    },
    /// Raises touched vertex heights with radial falloff from brush center.
    Add { delta: f32, basis: f32 },
    /// Lowers touched vertex heights with radial falloff from brush center.
    Remove { delta: f32, basis: f32 },
    /// Moves touched vertex heights toward local brush average with radial falloff.
    Smooth { strength: f32, basis: f32 },
    /// Reserved for future topology LOD workflows. No-op in fixed-grid mode.
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
        let ids = self.vertices_in_brush(center, brush_size_meters, shape);
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            out.push(InsertVertexResult {
                inserted_vertex_id: id,
                removed_as_coplanar: true,
            });
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

        if brush_op_basis(op).is_none() {
            return Ok(Vec::new());
        }

        let ids = self.vertices_in_brush(center, brush_size_meters, shape);
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let avg = ids
            .iter()
            .map(|&id| self.vertices[id].position.y)
            .sum::<f32>()
            / ids.len() as f32;

        for &id in &ids {
            let old = self.vertices[id].position;
            let falloff = brush_falloff_weight(old.x, old.z, center, brush_size_meters, shape);
            let new_y = match op {
                BrushOp::SetHeight { y, .. } => y,
                BrushOp::Add { delta, .. } => old.y + delta * falloff,
                BrushOp::Remove { delta, .. } => old.y - delta * falloff,
                BrushOp::Smooth { strength, .. } => {
                    let local_strength = strength.clamp(0.0, 1.0) * falloff;
                    old.y + (avg - old.y) * local_strength
                }
                // Grid topology is fixed now; this op is intentionally a no-op.
                BrushOp::Decimate { .. } => old.y,
            };
            self.vertices[id].position = Vector3::new(old.x, new_y, old.z);
        }

        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            out.push(InsertVertexResult {
                inserted_vertex_id: id,
                removed_as_coplanar: false,
            });
        }
        Ok(out)
    }

    fn vertices_in_brush(&self, center: Vector3, size: f32, shape: BrushShape) -> Vec<usize> {
        let mut ids = Vec::new();
        for (id, v) in self.vertices.iter().enumerate() {
            if point_in_brush_xz(v.position.x, v.position.z, center, size, shape) {
                ids.push(id);
            }
        }
        ids
    }
}

fn point_in_brush_xz(x: f32, z: f32, center: Vector3, size: f32, shape: BrushShape) -> bool {
    let half = size * 0.5;
    match shape {
        BrushShape::Square => {
            let min_x = center.x - half;
            let max_x = center.x + half;
            let min_z = center.z - half;
            let max_z = center.z + half;
            x >= min_x && x <= max_x && z >= min_z && z <= max_z
        }
        BrushShape::Circle => {
            let dx = x - center.x;
            let dz = z - center.z;
            (dx * dx + dz * dz) <= (half * half)
        }
        BrushShape::Triangle => {
            let tri = triangle_brush_points(center, size);
            point_in_triangle_xz_inclusive(x, z, tri[0], tri[1], tri[2], 1.0e-5)
        }
    }
}

fn brush_falloff_weight(x: f32, z: f32, center: Vector3, size: f32, shape: BrushShape) -> f32 {
    let half = size * 0.5;
    if half <= 1.0e-6 {
        return 1.0;
    }
    let dx = (x - center.x).abs();
    let dz = (z - center.z).abs();
    let t = match shape {
        BrushShape::Circle | BrushShape::Triangle => {
            ((dx * dx + dz * dz).sqrt() / half).clamp(0.0, 1.0)
        }
        BrushShape::Square => (dx.max(dz) / half).clamp(0.0, 1.0),
    };
    1.0 - t
}

fn triangle_brush_points(center: Vector3, size: f32) -> [Vector3; 3] {
    let radius = size * 0.5;
    let mut out = [Vector3::new(0.0, center.y, 0.0); 3];
    for (i, p) in out.iter_mut().enumerate() {
        let angle = FRAC_PI_2 + (i as f32) * (2.0 * PI / 3.0);
        let x = center.x + radius * angle.cos();
        let z = center.z + radius * angle.sin();
        *p = Vector3::new(x, center.y, z);
    }
    out
}

fn point_in_triangle_xz_inclusive(
    x: f32,
    z: f32,
    a: Vector3,
    b: Vector3,
    c: Vector3,
    eps: f32,
) -> bool {
    if let Some((w0, w1, w2)) = barycentric_xz(x, z, a, b, c, eps) {
        w0 >= -eps && w1 >= -eps && w2 >= -eps
    } else {
        false
    }
}

fn brush_op_basis(op: BrushOp) -> Option<f32> {
    let basis = match op {
        BrushOp::SetHeight { basis, .. } => basis,
        BrushOp::Add { basis, .. } => basis,
        BrushOp::Remove { basis, .. } => basis,
        BrushOp::Smooth { basis, .. } => basis,
        BrushOp::Decimate { basis } => basis,
    };
    normalize_brush_basis(basis)
}

fn normalize_brush_basis(basis: f32) -> Option<f32> {
    const ALLOWED: [f32; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
    if !basis.is_finite() || basis <= 0.0 {
        return None;
    }
    ALLOWED
        .into_iter()
        .find(|allowed| (basis - allowed).abs() <= 1.0e-5)
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

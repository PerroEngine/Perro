use crate::{ChunkError, InsertVertexResult, TerrainChunk, Triangle};
use perro_structs::Vector3;
use std::f32::consts::{FRAC_PI_2, PI};

const ADD_REMOVE_FEATURE_OFFSET_METERS: f32 = 0.1;

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

        let touched_ids = self.vertices_in_brush(center, brush_size_meters, shape);
        if touched_ids.is_empty() {
            return Ok(Vec::new());
        }

        let current_heights: Vec<f32> = touched_ids
            .iter()
            .map(|&id| self.vertices[id].position.y)
            .collect();
        let avg = if current_heights.is_empty() {
            0.0
        } else {
            current_heights.iter().sum::<f32>() / current_heights.len() as f32
        };

        if let BrushOp::Decimate { basis } = op {
            if basis <= 0.0 || !basis.is_finite() {
                return Ok(Vec::new());
            }
            for &id in &touched_ids {
                let old = self.vertices[id].position;
                // Topology decimate: collapse edited vertices onto the target XY lattice.
                // Reconcile pass below merges duplicated positions and removes invalid tris.
                let snapped_x = snap_to_grid(old.x, basis);
                let snapped_z = snap_to_grid(old.z, basis);
                let snapped_y = (old.y / basis).round() * basis;
                self.vertices[id].position = Vector3::new(snapped_x, snapped_y, snapped_z);
            }
            self.reconcile_after_edit();

            let mut out = Vec::with_capacity(touched_ids.len());
            for id in touched_ids {
                out.push(InsertVertexResult {
                    inserted_vertex_id: id,
                    removed_as_coplanar: false,
                });
            }
            return Ok(out);
        }

        for &id in &touched_ids {
            let old = self.vertices[id].position;
            let new_y = match op {
                BrushOp::SetHeight { y, .. } => y,
                BrushOp::Add { delta } => old.y + delta,
                BrushOp::Remove { delta } => old.y - delta,
                BrushOp::Smooth { strength } => old.y + (avg - old.y) * strength.clamp(0.0, 1.0),
                BrushOp::Decimate { .. } => old.y,
            };
            self.vertices[id].position = Vector3::new(old.x, new_y, old.z);
        }

        let mut out = Vec::with_capacity(touched_ids.len());
        for id in touched_ids {
            out.push(InsertVertexResult {
                inserted_vertex_id: id,
                removed_as_coplanar: false,
            });
        }
        Ok(out)
    }

    fn vertices_in_brush(
        &self,
        center: Vector3,
        size: f32,
        shape: BrushShape,
    ) -> Vec<usize> {
        let mut ids = Vec::new();
        for (id, v) in self.vertices.iter().enumerate() {
            if point_in_brush_xz(v.position.x, v.position.z, center, size, shape) {
                ids.push(id);
            }
        }
        ids
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
        let top_ring = brush_xz_points_feature(center, size, shape);
        let mut base_targets = Vec::with_capacity(top_ring.len());
        let mut top_targets = Vec::with_capacity(top_ring.len());
        // Insert base ring first so ground retriangulation anchors to the widened footprint
        // before top-ring walls are introduced.
        for p in &top_ring {
            let dir = radial_dir_xz(center, *p);
            // Base ring expands outward so set-height forms a stable skirt around the top ring.
            let base_x = p.x + dir.x * offset;
            let base_z = p.z + dir.z * offset;
            let base_y = self.sample_height_at_xz(p.x, p.z).unwrap_or(0.0);
            base_targets.push(Vector3::new(base_x, base_y, base_z));
        }
        for p in &top_ring {
            top_targets.push(Vector3::new(p.x, target_y, p.z));
        }
        let base_polygon = ordered_polygon_xz(&base_targets, center);
        let mut out = Vec::with_capacity(base_targets.len() + top_targets.len());
        out.extend(self.apply_points_structural(base_targets)?);
        self.retriangulate_polygon_region(&base_polygon);
        out.extend(self.apply_points_structural(top_targets)?);
        self.enforce_top_cap_quad(&top_ring, center, target_y);
        Ok(out)
    }

    fn apply_add_remove_feature(
        &mut self,
        center: Vector3,
        size: f32,
        shape: BrushShape,
        signed_delta: f32,
    ) -> Result<Vec<InsertVertexResult>, ChunkError> {
        let top_ring = brush_xz_points_feature(center, size, shape);
        let mut base_targets = Vec::with_capacity(top_ring.len());
        let mut top_targets = Vec::with_capacity(top_ring.len());

        for p in &top_ring {
            let dir = radial_dir_xz(center, *p);
            let base_x = p.x + dir.x * ADD_REMOVE_FEATURE_OFFSET_METERS;
            let base_z = p.z + dir.z * ADD_REMOVE_FEATURE_OFFSET_METERS;
            let base_y = self.sample_height_at_xz(base_x, base_z).unwrap_or(0.0);
            let top_y = self.sample_height_at_xz(p.x, p.z).unwrap_or(base_y) + signed_delta;
            base_targets.push(Vector3::new(base_x, base_y, base_z));
            top_targets.push(Vector3::new(p.x, top_y, p.z));
        }

        let base_polygon = ordered_polygon_xz(&base_targets, center);
        let mut out = Vec::with_capacity(base_targets.len() + top_targets.len());
        out.extend(self.apply_points_structural(base_targets)?);
        self.retriangulate_polygon_region(&base_polygon);
        out.extend(self.apply_points_structural(top_targets)?);
        Ok(out)
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
        self.reconcile_structural_after_edit();
        Ok(out)
    }



    fn retriangulate_polygon_region(&mut self, polygon_xz: &[(f32, f32)]) {
        if polygon_xz.len() < 3 {
            return;
        }
        let mut remove_ids = Vec::new();
        for (tri_id, tri) in self.triangles.iter().copied().enumerate() {
            let a = self.vertices[tri.a].position;
            let b = self.vertices[tri.b].position;
            let c = self.vertices[tri.c].position;
            let centroid = ((a.x + b.x + c.x) / 3.0, (a.z + b.z + c.z) / 3.0);
            if point_in_polygon_xz(centroid, polygon_xz) {
                remove_ids.push(tri_id);
            }
        }
        remove_ids.sort_unstable_by(|a, b| b.cmp(a));
        for tri_id in remove_ids {
            if tri_id < self.triangles.len() {
                self.triangles.swap_remove(tri_id);
            }
        }

        let mut boundary_ids = Vec::new();
        for &(x, z) in polygon_xz {
            if let Some(id) = self.find_vertex_id_at_xz(x, z, 1.0e-3)
                && boundary_ids.last().copied() != Some(id)
            {
                boundary_ids.push(id);
            }
        }
        if boundary_ids.len() < 3 {
            return;
        }
        if boundary_ids.first() == boundary_ids.last() {
            let _ = boundary_ids.pop();
        }
        if boundary_ids.len() < 3 {
            return;
        }

        let root = boundary_ids[0];
        for i in 1..(boundary_ids.len() - 1) {
            self.triangles
                .push(Triangle::new(root, boundary_ids[i], boundary_ids[i + 1]));
        }
        self.reconcile_structural_after_edit();
    }

    fn find_vertex_id_at_xz(&self, x: f32, z: f32, eps: f32) -> Option<usize> {
        self.vertices().iter().enumerate().find_map(|(id, v)| {
            ((v.position.x - x).abs() <= eps && (v.position.z - z).abs() <= eps).then_some(id)
        })
    }

    fn enforce_top_cap_quad(&mut self, top_ring: &[Vector3], center: Vector3, target_y: f32) {
        let ordered_top = ordered_polygon_xz(top_ring, center);
        let mut top_ids = Vec::with_capacity(ordered_top.len());
        for (x, z) in ordered_top {
            let Some(id) = self.find_vertex_id_near_xyz(x, z, target_y, 0.25, 1.0e-3) else {
                return;
            };
            if top_ids.contains(&id) {
                return;
            }
            top_ids.push(id);
        }
        if top_ids.len() != 4 {
            return;
        }

        let top_set: std::collections::HashSet<usize> = top_ids.iter().copied().collect();
        let mut remove_ids = Vec::new();
        for (tri_id, tri) in self.triangles.iter().copied().enumerate() {
            if top_set.contains(&tri.a) && top_set.contains(&tri.b) && top_set.contains(&tri.c) {
                remove_ids.push(tri_id);
            }
        }
        remove_ids.sort_unstable_by(|a, b| b.cmp(a));
        for tri_id in remove_ids {
            if tri_id < self.triangles.len() {
                self.triangles.swap_remove(tri_id);
            }
        }

        let mut t0 = Triangle::new(top_ids[0], top_ids[1], top_ids[2]);
        orient_triangle_upward(&mut t0, &self.vertices);
        let mut t1 = Triangle::new(top_ids[0], top_ids[2], top_ids[3]);
        orient_triangle_upward(&mut t1, &self.vertices);
        self.triangles.push(t0);
        self.triangles.push(t1);
        self.reconcile_structural_after_edit();
    }

    fn find_vertex_id_near_xyz(
        &self,
        x: f32,
        z: f32,
        y: f32,
        y_eps: f32,
        xz_eps: f32,
    ) -> Option<usize> {
        self.vertices()
            .iter()
            .enumerate()
            .filter(|(_, v)| {
                (v.position.x - x).abs() <= xz_eps
                    && (v.position.z - z).abs() <= xz_eps
                    && (v.position.y - y).abs() <= y_eps
            })
            .min_by(|(_, a), (_, b)| {
                (a.position.y - y)
                    .abs()
                    .partial_cmp(&(b.position.y - y).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| id)
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

fn brush_xz_points_feature(center: Vector3, size: f32, shape: BrushShape) -> Vec<Vector3> {
    match shape {
        // Feature construction should be centered on brush center (not size-grid anchored)
        // so set-height topology is symmetric around the intended operation center.
        BrushShape::Square => square_feature_points_centered(center, size).into(),
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

fn square_feature_points_centered(center: Vector3, size: f32) -> [Vector3; 4] {
    let half = size * 0.5;
    let step = detail_snap_step(size);
    let cx = snap_to_grid(center.x, step);
    let cz = snap_to_grid(center.z, step);
    let min_x = snap_to_grid(cx - half, step);
    let min_z = snap_to_grid(cz - half, step);
    let max_x = snap_to_grid(cx + half, step);
    let max_z = snap_to_grid(cz + half, step);
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

fn ordered_polygon_xz(points: &[Vector3], center: Vector3) -> Vec<(f32, f32)> {
    let mut out: Vec<(f32, f32)> = points.iter().map(|p| (p.x, p.z)).collect();
    out.sort_by(|(ax, az), (bx, bz)| {
        let aa = (az - center.z).atan2(ax - center.x);
        let ba = (bz - center.z).atan2(bx - center.x);
        aa.partial_cmp(&ba).unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

fn point_in_polygon_xz(point: (f32, f32), polygon: &[(f32, f32)]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let (px, pz) = point;
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (xi, zi) = polygon[i];
        let (xj, zj) = polygon[j];
        let dz = zj - zi;
        let intersects = ((zi > pz) != (zj > pz))
            && (px < (xj - xi) * (pz - zi) / if dz.abs() <= 1.0e-8 { 1.0e-8 } else { dz } + xi);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
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
            let tri = triangle_brush_points_snapped(center, size);
            point_in_triangle_xz_inclusive(x, z, tri[0], tri[1], tri[2], 1.0e-5)
        }
    }
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

fn orient_triangle_upward(tri: &mut Triangle, vertices: &[crate::Vertex]) {
    let a = vertices[tri.a].position;
    let b = vertices[tri.b].position;
    let c = vertices[tri.c].position;
    let ab = Vector3::new(b.x - a.x, b.y - a.y, b.z - a.z);
    let ac = Vector3::new(c.x - a.x, c.y - a.y, c.z - a.z);
    if ab.cross(ac).y < 0.0 {
        std::mem::swap(&mut tri.b, &mut tri.c);
    }
}

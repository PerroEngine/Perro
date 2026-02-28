use crate::{ChunkError, TerrainChunk, Triangle, VertexID};
use perro_structs::Vector3;
use std::collections::{HashMap, HashSet};

pub const DEFAULT_AREA_EPSILON: f32 = 1.0e-6;
pub const DEFAULT_NORMAL_EPSILON: f32 = 1.0e-4;
pub const DEFAULT_DISTANCE_EPSILON: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InsertVertexResult {
    pub inserted_vertex_id: VertexID,
    pub removed_as_coplanar: bool,
}

impl TerrainChunk {
    pub fn insert_vertex(&mut self, position: Vector3) -> Result<InsertVertexResult, ChunkError> {
        self.insert_vertex_with_tolerances(
            position,
            DEFAULT_AREA_EPSILON,
            DEFAULT_NORMAL_EPSILON,
            DEFAULT_DISTANCE_EPSILON,
        )
    }

    pub fn insert_vertex_with_tolerances(
        &mut self,
        position: Vector3,
        area_epsilon: f32,
        normal_epsilon: f32,
        distance_epsilon: f32,
    ) -> Result<InsertVertexResult, ChunkError> {
        let hit_triangle_ids = self.find_hit_triangles_xz(position.x, position.z, area_epsilon);
        if hit_triangle_ids.is_empty() {
            return Err(ChunkError::PointOutsideMesh {
                x: position.x,
                z: position.z,
            });
        }

        if let Some(existing_id) =
            self.find_existing_vertex_in_hit_triangles(position, &hit_triangle_ids, distance_epsilon)
        {
            return Ok(InsertVertexResult {
                inserted_vertex_id: existing_id,
                removed_as_coplanar: true,
            });
        }

        let maybe_coplanar = self.is_potentially_coplanar_insert(
            position,
            &hit_triangle_ids,
            area_epsilon,
            distance_epsilon,
        );

        if maybe_coplanar && self.can_skip_coplanar_insert(position, &hit_triangle_ids, area_epsilon) {
            let fallback_id = self.triangles[hit_triangle_ids[0]].a;
            return Ok(InsertVertexResult {
                inserted_vertex_id: fallback_id,
                removed_as_coplanar: true,
            });
        }

        let inserted_vertex_id = self.add_vertex(position);
        self.split_hit_triangles(inserted_vertex_id, &hit_triangle_ids, area_epsilon);

        let removed_as_coplanar = if maybe_coplanar {
            self.try_remove_coplanar_vertex(
                inserted_vertex_id,
                area_epsilon,
                normal_epsilon,
                distance_epsilon,
            )
        } else {
            false
        };

        Ok(InsertVertexResult {
            inserted_vertex_id,
            removed_as_coplanar,
        })
    }

    fn find_existing_vertex_in_hit_triangles(
        &self,
        position: Vector3,
        hit_triangle_ids: &[usize],
        eps: f32,
    ) -> Option<VertexID> {
        let eps2 = eps * eps;
        let mut seen: HashSet<VertexID> = HashSet::new();
        for tri_id in hit_triangle_ids {
            let tri = self.triangles[*tri_id];
            for vid in [tri.a, tri.b, tri.c] {
                if seen.insert(vid) && squared_distance(self.vertices[vid].position, position) <= eps2 {
                    return Some(vid);
                }
            }
        }
        None
    }

    fn is_potentially_coplanar_insert(
        &self,
        position: Vector3,
        hit_triangle_ids: &[usize],
        area_epsilon: f32,
        distance_epsilon: f32,
    ) -> bool {
        for tri_id in hit_triangle_ids {
            let tri = self.triangles[*tri_id];
            let a = self.vertices[tri.a].position;
            let b = self.vertices[tri.b].position;
            let c = self.vertices[tri.c].position;

            let n = triangle_normal(a, b, c);
            if n.length() <= area_epsilon {
                return false;
            }
            let nn = n.normalized();
            let dist = point_plane_distance_abs(position, a, nn);
            if dist > distance_epsilon {
                return false;
            }
        }
        true
    }

    fn can_skip_coplanar_insert(
        &self,
        position: Vector3,
        hit_triangle_ids: &[usize],
        area_epsilon: f32,
    ) -> bool {
        for tri_id in hit_triangle_ids {
            let tri = self.triangles[*tri_id];
            let a = self.vertices[tri.a].position;
            let b = self.vertices[tri.b].position;
            let c = self.vertices[tri.c].position;
            if point_in_triangle_xz_strict_interior(position.x, position.z, a, b, c, area_epsilon * 4.0)
            {
                return true;
            }
        }
        false
    }

    fn find_hit_triangles_xz(&mut self, x: f32, z: f32, eps: f32) -> Vec<usize> {
        if let Some(tri_id) = self.last_hit_triangle {
            if let Some(tri) = self.triangles.get(tri_id).copied() {
                let a = self.vertices[tri.a].position;
                let b = self.vertices[tri.b].position;
                let c = self.vertices[tri.c].position;
                if point_in_triangle_xz_strict_interior(x, z, a, b, c, eps * 4.0) {
                    return vec![tri_id];
                }
            }
        }

        let mut hits = Vec::new();
        for (tri_id, tri) in self.triangles.iter().copied().enumerate() {
            let a = self.vertices[tri.a].position;
            let b = self.vertices[tri.b].position;
            let c = self.vertices[tri.c].position;

            if !point_in_triangle_aabb_xz(x, z, a, b, c, eps) {
                continue;
            }
            if point_in_triangle_xz(x, z, a, b, c, eps) {
                hits.push(tri_id);
            }
        }
        self.last_hit_triangle = hits.first().copied();
        hits
    }

    fn split_hit_triangles(&mut self, vertex_id: VertexID, hit_triangle_ids: &[usize], eps: f32) {
        let p = self.vertices[vertex_id].position;
        let hit_ids = unique_sorted_desc(hit_triangle_ids);
        if hit_ids.is_empty() {
            return;
        }

        let mut source_tris = Vec::with_capacity(hit_ids.len());
        for tri_id in &hit_ids {
            if let Some(tri) = self.triangles.get(*tri_id).copied() {
                source_tris.push(tri);
            }
        }

        remove_triangles_by_ids_desc(&mut self.triangles, &hit_ids);
        let mut replacement = Vec::with_capacity(source_tris.len() * 3);

        for tri in source_tris {
            let a = self.vertices[tri.a].position;
            let b = self.vertices[tri.b].position;
            let c = self.vertices[tri.c].position;
            let Some((w0, w1, w2)) = barycentric_xz(p.x, p.z, a, b, c, eps) else {
                continue;
            };

            let on_ab = w2.abs() <= eps;
            let on_bc = w0.abs() <= eps;
            let on_ca = w1.abs() <= eps;

            let mut candidates = [Triangle::new(0, 0, 0); 3];
            let candidate_count = if on_ab {
                candidates[0] = Triangle::new(tri.a, vertex_id, tri.c);
                candidates[1] = Triangle::new(vertex_id, tri.b, tri.c);
                2
            } else if on_bc {
                candidates[0] = Triangle::new(tri.b, vertex_id, tri.a);
                candidates[1] = Triangle::new(vertex_id, tri.c, tri.a);
                2
            } else if on_ca {
                candidates[0] = Triangle::new(tri.c, vertex_id, tri.b);
                candidates[1] = Triangle::new(vertex_id, tri.a, tri.b);
                2
            } else {
                candidates[0] = Triangle::new(tri.a, tri.b, vertex_id);
                candidates[1] = Triangle::new(tri.b, tri.c, vertex_id);
                candidates[2] = Triangle::new(tri.c, tri.a, vertex_id);
                3
            };

            for cand in candidates.into_iter().take(candidate_count) {
                if self.triangle_area2_by_positions(cand) > eps {
                    replacement.push(cand);
                }
            }
        }

        self.triangles.extend(replacement);
    }

    fn try_remove_coplanar_vertex(
        &mut self,
        vertex_id: VertexID,
        area_epsilon: f32,
        normal_epsilon: f32,
        distance_epsilon: f32,
    ) -> bool {
        let incident = self.incident_triangle_ids(vertex_id);
        if incident.len() < 3 {
            return false;
        }

        let neighbors = self.neighbor_vertices_of(vertex_id, &incident);
        if neighbors.len() < 3 {
            return false;
        }

        let base_normal = {
            let tri = self.triangles[incident[0]];
            let n = triangle_normal(
                self.vertices[tri.a].position,
                self.vertices[tri.b].position,
                self.vertices[tri.c].position,
            );
            if n.length() <= area_epsilon {
                return false;
            }
            n.normalized()
        };

        let base_point = self.vertices[vertex_id].position;
        for tri_id in &incident {
            let tri = self.triangles[*tri_id];
            let n = triangle_normal(
                self.vertices[tri.a].position,
                self.vertices[tri.b].position,
                self.vertices[tri.c].position,
            );
            if n.length() <= area_epsilon {
                return false;
            }
            let d = n.normalized().dot(base_normal).abs();
            if 1.0 - d > normal_epsilon {
                return false;
            }
        }

        for neighbor in &neighbors {
            let p = self.vertices[*neighbor].position;
            let dist = point_plane_distance_abs(p, base_point, base_normal);
            if dist > distance_epsilon {
                return false;
            }
        }

        let replacement = self.retriangulate_neighbors(vertex_id, &neighbors, base_normal, area_epsilon);
        if replacement.is_empty() {
            return false;
        }

        let incident_ids = unique_sorted_desc(&incident);
        let mut next_tris = self.triangles.clone();
        remove_triangles_by_ids_desc(&mut next_tris, &incident_ids);
        next_tris.extend(replacement.iter().copied());
        if !self.replacement_is_safe(
            vertex_id,
            &incident,
            &replacement,
            base_normal,
            area_epsilon,
            normal_epsilon,
        ) {
            return false;
        }

        self.triangles = next_tris;

        self.compact_remove_vertex(vertex_id);
        true
    }

    fn retriangulate_neighbors(
        &self,
        center_vertex_id: VertexID,
        neighbors: &[VertexID],
        normal: Vector3,
        area_epsilon: f32,
    ) -> Vec<Triangle> {
        let center = self.vertices[center_vertex_id].position;
        let mut sorted = neighbors.to_vec();
        let (u, v) = orthonormal_basis(normal);

        sorted.sort_by(|lhs, rhs| {
            let lp = self.vertices[*lhs].position;
            let rp = self.vertices[*rhs].position;
            let l = sub(lp, center);
            let r = sub(rp, center);
            let la = l.dot(u).atan2(l.dot(v));
            let ra = r.dot(u).atan2(r.dot(v));
            la.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
        });

        if sorted.len() < 3 {
            return Vec::new();
        }

        let mut out = Vec::new();
        let root = sorted[0];
        for i in 1..(sorted.len() - 1) {
            let mut tri = Triangle::new(root, sorted[i], sorted[i + 1]);
            if self.triangle_area2_by_positions(tri) <= area_epsilon {
                continue;
            }

            let tri_n = triangle_normal(
                self.vertices[tri.a].position,
                self.vertices[tri.b].position,
                self.vertices[tri.c].position,
            );
            if tri_n.dot(normal) < 0.0 {
                tri = Triangle::new(root, sorted[i + 1], sorted[i]);
            }
            out.push(tri);
        }
        out
    }

    fn incident_triangle_ids(&self, vertex_id: VertexID) -> Vec<usize> {
        let mut out = Vec::new();
        for (tri_id, tri) in self.triangles.iter().copied().enumerate() {
            if tri.a == vertex_id || tri.b == vertex_id || tri.c == vertex_id {
                out.push(tri_id);
            }
        }
        out
    }

    fn neighbor_vertices_of(&self, vertex_id: VertexID, incident: &[usize]) -> Vec<VertexID> {
        let mut uniq = HashSet::new();
        for tri_id in incident {
            let tri = self.triangles[*tri_id];
            for idx in [tri.a, tri.b, tri.c] {
                if idx != vertex_id {
                    uniq.insert(idx);
                }
            }
        }
        let mut neighbors: Vec<VertexID> = uniq.into_iter().collect();
        neighbors.sort_unstable();
        neighbors
    }

    fn triangle_area2_by_positions(&self, tri: Triangle) -> f32 {
        let a = self.vertices[tri.a].position;
        let b = self.vertices[tri.b].position;
        let c = self.vertices[tri.c].position;
        triangle_normal(a, b, c).length()
    }

    fn compact_remove_vertex(&mut self, vertex_id: VertexID) {
        self.vertices.remove(vertex_id);
        for tri in &mut self.triangles {
            if tri.a > vertex_id {
                tri.a -= 1;
            }
            if tri.b > vertex_id {
                tri.b -= 1;
            }
            if tri.c > vertex_id {
                tri.c -= 1;
            }
        }
    }

    fn replacement_is_safe(
        &self,
        center_vertex_id: VertexID,
        incident: &[usize],
        replacement: &[Triangle],
        base_normal: Vector3,
        area_epsilon: f32,
        normal_epsilon: f32,
    ) -> bool {
        if !self.boundary_preserved(center_vertex_id, incident, replacement) {
            return false;
        }
        if !self.replacement_area_consistent(incident, replacement, area_epsilon) {
            return false;
        }
        if !self.replacement_normals_consistent(replacement, base_normal, normal_epsilon, area_epsilon) {
            return false;
        }
        if !triangles_are_manifold_local(replacement) {
            return false;
        }
        true
    }

    fn replacement_normals_consistent(
        &self,
        replacement: &[Triangle],
        base_normal: Vector3,
        normal_epsilon: f32,
        area_epsilon: f32,
    ) -> bool {
        for tri in replacement {
            let n = triangle_normal(
                self.vertices[tri.a].position,
                self.vertices[tri.b].position,
                self.vertices[tri.c].position,
            );
            let len = n.length();
            if len <= area_epsilon {
                return false;
            }
            let d = n.normalized().dot(base_normal);
            if d < 1.0 - normal_epsilon {
                return false;
            }
        }
        true
    }

    fn replacement_area_consistent(
        &self,
        incident: &[usize],
        replacement: &[Triangle],
        area_epsilon: f32,
    ) -> bool {
        let mut incident_area2 = 0.0;
        for tri_id in incident {
            incident_area2 += self.triangle_area2_by_positions(self.triangles[*tri_id]);
        }

        let mut replacement_area2 = 0.0;
        for tri in replacement {
            replacement_area2 += self.triangle_area2_by_positions(*tri);
        }

        let diff = (incident_area2 - replacement_area2).abs();
        let tol = (incident_area2 + replacement_area2) * 1.0e-4 + area_epsilon * 32.0;
        diff <= tol
    }

    fn boundary_preserved(
        &self,
        center_vertex_id: VertexID,
        incident: &[usize],
        replacement: &[Triangle],
    ) -> bool {
        let expected = incident_boundary_edges_without_center(self, center_vertex_id, incident);
        if expected.is_empty() {
            return false;
        }

        let produced = boundary_edges_of(replacement);
        expected == produced
    }
}

fn sub(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn triangle_normal(a: Vector3, b: Vector3, c: Vector3) -> Vector3 {
    let ab = sub(b, a);
    let ac = sub(c, a);
    ab.cross(ac)
}

fn point_plane_distance_abs(point: Vector3, plane_point: Vector3, plane_normal: Vector3) -> f32 {
    sub(point, plane_point).dot(plane_normal).abs()
}

fn squared_distance(a: Vector3, b: Vector3) -> f32 {
    let d = sub(a, b);
    d.dot(d)
}

fn orthonormal_basis(normal: Vector3) -> (Vector3, Vector3) {
    let helper = if normal.y.abs() < 0.99 {
        Vector3::new(0.0, 1.0, 0.0)
    } else {
        Vector3::new(1.0, 0.0, 0.0)
    };
    let u = normal.cross(helper).normalized();
    let v = normal.cross(u).normalized();
    (u, v)
}

fn point_in_triangle_xz(x: f32, z: f32, a: Vector3, b: Vector3, c: Vector3, eps: f32) -> bool {
    let Some((w0, w1, w2)) = barycentric_xz(x, z, a, b, c, eps) else {
        return false;
    };
    w0 >= -eps && w1 >= -eps && w2 >= -eps
}

fn point_in_triangle_xz_strict_interior(
    x: f32,
    z: f32,
    a: Vector3,
    b: Vector3,
    c: Vector3,
    eps: f32,
) -> bool {
    let Some((w0, w1, w2)) = barycentric_xz(x, z, a, b, c, eps) else {
        return false;
    };
    w0 > eps && w1 > eps && w2 > eps
}

fn point_in_triangle_aabb_xz(x: f32, z: f32, a: Vector3, b: Vector3, c: Vector3, eps: f32) -> bool {
    let min_x = a.x.min(b.x).min(c.x) - eps;
    let max_x = a.x.max(b.x).max(c.x) + eps;
    let min_z = a.z.min(b.z).min(c.z) - eps;
    let max_z = a.z.max(b.z).max(c.z) + eps;
    x >= min_x && x <= max_x && z >= min_z && z <= max_z
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

fn incident_boundary_edges_without_center(
    chunk: &TerrainChunk,
    center_vertex_id: VertexID,
    incident: &[usize],
) -> HashSet<(VertexID, VertexID)> {
    let mut counts: HashMap<(VertexID, VertexID), usize> = HashMap::new();

    for tri_id in incident {
        let tri = chunk.triangles[*tri_id];
        for (u, v) in [(tri.a, tri.b), (tri.b, tri.c), (tri.c, tri.a)] {
            if u == center_vertex_id || v == center_vertex_id {
                continue;
            }
            *counts.entry(edge_key(u, v)).or_insert(0) += 1;
        }
    }

    counts
        .into_iter()
        .filter_map(|(e, c)| (c == 1).then_some(e))
        .collect()
}

fn boundary_edges_of(tris: &[Triangle]) -> HashSet<(VertexID, VertexID)> {
    let mut counts: HashMap<(VertexID, VertexID), usize> = HashMap::new();
    for tri in tris {
        for (u, v) in [(tri.a, tri.b), (tri.b, tri.c), (tri.c, tri.a)] {
            *counts.entry(edge_key(u, v)).or_insert(0) += 1;
        }
    }

    counts
        .into_iter()
        .filter_map(|(e, c)| (c == 1).then_some(e))
        .collect()
}

fn triangles_are_manifold_local(tris: &[Triangle]) -> bool {
    let mut edge_counts: HashMap<(VertexID, VertexID), usize> = HashMap::new();
    let mut tri_set: HashSet<[VertexID; 3]> = HashSet::new();

    for tri in tris {
        if tri.a == tri.b || tri.b == tri.c || tri.a == tri.c {
            return false;
        }

        let mut key = [tri.a, tri.b, tri.c];
        key.sort_unstable();
        if !tri_set.insert(key) {
            return false;
        }

        for (u, v) in [(tri.a, tri.b), (tri.b, tri.c), (tri.c, tri.a)] {
            let k = edge_key(u, v);
            let c = edge_counts.entry(k).or_insert(0);
            *c += 1;
            if *c > 2 {
                return false;
            }
        }
    }
    true
}

fn edge_key(a: VertexID, b: VertexID) -> (VertexID, VertexID) {
    if a < b { (a, b) } else { (b, a) }
}

fn unique_sorted_desc(ids: &[usize]) -> Vec<usize> {
    let mut out = ids.to_vec();
    out.sort_unstable();
    out.dedup();
    out.sort_unstable_by(|a, b| b.cmp(a));
    out
}

fn remove_triangles_by_ids_desc(triangles: &mut Vec<Triangle>, sorted_desc_ids: &[usize]) {
    for &id in sorted_desc_ids {
        if id < triangles.len() {
            triangles.swap_remove(id);
        }
    }
}

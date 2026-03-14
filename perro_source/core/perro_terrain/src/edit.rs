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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchInsertSummary {
    pub attempted: usize,
    pub inserted: usize,
    pub removed_as_coplanar: usize,
    pub skipped_outside_mesh: usize,
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

        if let Some(existing_id) = self.find_existing_vertex_in_hit_triangles(
            position,
            &hit_triangle_ids,
            distance_epsilon,
        ) {
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

        if maybe_coplanar {
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

    pub(crate) fn insert_vertex_structural(
        &mut self,
        position: Vector3,
    ) -> Result<InsertVertexResult, ChunkError> {
        self.insert_vertex_structural_filtered(position, None)
    }

    fn insert_vertex_structural_filtered(
        &mut self,
        position: Vector3,
        region_polygon_xz: Option<&[(f32, f32)]>,
    ) -> Result<InsertVertexResult, ChunkError> {
        let area_epsilon = DEFAULT_AREA_EPSILON;
        let distance_epsilon = DEFAULT_DISTANCE_EPSILON;

        let hit_triangle_ids = match region_polygon_xz {
            Some(region) => {
                self.find_hit_triangles_xz_in_region(position.x, position.z, area_epsilon, region)
            }
            None => self.find_hit_triangles_xz(position.x, position.z, area_epsilon),
        };
        if hit_triangle_ids.is_empty() {
            return Err(ChunkError::PointOutsideMesh {
                x: position.x,
                z: position.z,
            });
        }

        if let Some(existing_id) = self.find_existing_vertex_in_hit_triangles(
            position,
            &hit_triangle_ids,
            distance_epsilon,
        ) {
            return Ok(InsertVertexResult {
                inserted_vertex_id: existing_id,
                removed_as_coplanar: false,
            });
        }

        let inserted_vertex_id = self.add_vertex(position);
        self.split_hit_triangles(inserted_vertex_id, &hit_triangle_ids, area_epsilon);
        Ok(InsertVertexResult {
            inserted_vertex_id,
            removed_as_coplanar: false,
        })
    }

    pub fn insert_vertices_batch(
        &mut self,
        points: &[Vector3],
    ) -> Result<BatchInsertSummary, ChunkError> {
        let mut ordered: Vec<Vector3> = points.to_vec();
        ordered.sort_by_key(|p| morton_key_2d(p.x, p.z));

        let mut summary = BatchInsertSummary {
            attempted: ordered.len(),
            ..BatchInsertSummary::default()
        };

        for p in ordered {
            match self.insert_vertex(p) {
                Ok(r) => {
                    summary.inserted += 1;
                    if r.removed_as_coplanar {
                        summary.removed_as_coplanar += 1;
                    }
                }
                Err(ChunkError::PointOutsideMesh { .. }) => {
                    summary.skipped_outside_mesh += 1;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(summary)
    }

    pub(crate) fn reconcile_after_edit(&mut self) {
        self.enforce_single_height_per_xz(DEFAULT_DISTANCE_EPSILON);
        self.remove_invalid_triangles(DEFAULT_AREA_EPSILON);
        self.remove_duplicate_triangles();
        self.compact_unreferenced_vertices();
        self.global_coplanar_cleanup(
            DEFAULT_AREA_EPSILON,
            DEFAULT_NORMAL_EPSILON,
            DEFAULT_DISTANCE_EPSILON,
        );
        self.remove_invalid_triangles(DEFAULT_AREA_EPSILON);
        self.remove_duplicate_triangles();
        self.compact_unreferenced_vertices();
        self.last_hit_triangle = None;
    }

    pub(crate) fn reconcile_structural_after_edit(&mut self) {
        // Structural feature passes preserve authored topology and skip aggressive
        // coplanar cleanup. This keeps intermediate staged connectivity stable.
        self.enforce_single_height_per_xz(DEFAULT_DISTANCE_EPSILON);
        self.remove_invalid_triangles(DEFAULT_AREA_EPSILON);
        self.remove_duplicate_triangles();
        self.compact_unreferenced_vertices();
        self.improve_planar_connectivity_shortest_edges(48, 1.0e-4, DEFAULT_AREA_EPSILON);
        self.remove_invalid_triangles(DEFAULT_AREA_EPSILON);
        self.remove_duplicate_triangles();
        self.compact_unreferenced_vertices();
        self.last_hit_triangle = None;
    }

    fn improve_planar_connectivity_shortest_edges(
        &mut self,
        max_passes: usize,
        y_epsilon: f32,
        area_epsilon: f32,
    ) {
        if self.triangles.len() < 2 {
            return;
        }
        for _ in 0..max_passes {
            let mut edge_to_tris: HashMap<(VertexID, VertexID), Vec<usize>> = HashMap::new();
            for (tri_id, tri) in self.triangles.iter().copied().enumerate() {
                for (u, v) in [(tri.a, tri.b), (tri.b, tri.c), (tri.c, tri.a)] {
                    edge_to_tris.entry(edge_key(u, v)).or_default().push(tri_id);
                }
            }

            let mut flipped_any = false;
            for ((a, b), owners) in edge_to_tris {
                if owners.len() != 2 {
                    continue;
                }
                let t0_id = owners[0];
                let t1_id = owners[1];
                if t0_id >= self.triangles.len() || t1_id >= self.triangles.len() {
                    continue;
                }
                let t0 = self.triangles[t0_id];
                let t1 = self.triangles[t1_id];

                let c = opposite_vertex_for_edge(t0, a, b);
                let d = opposite_vertex_for_edge(t1, a, b);
                let (Some(c), Some(d)) = (c, d) else {
                    continue;
                };
                if c == d || c == a || c == b || d == a || d == b {
                    continue;
                }

                let pa = self.vertices[a].position;
                let pb = self.vertices[b].position;
                let pc = self.vertices[c].position;
                let pd = self.vertices[d].position;

                // Keep this pass strictly planar-like so we do not alter vertical wall topology.
                let min_y = pa.y.min(pb.y).min(pc.y).min(pd.y);
                let max_y = pa.y.max(pb.y).max(pc.y).max(pd.y);
                if (max_y - min_y) > y_epsilon {
                    continue;
                }

                if !segments_cross_strict_2d((pa.x, pa.z), (pb.x, pb.z), (pc.x, pc.z), (pd.x, pd.z))
                {
                    continue;
                }

                let current_len2 = squared_distance(pa, pb);
                let alt_len2 = squared_distance(pc, pd);
                if alt_len2 + 1.0e-7 >= current_len2 {
                    continue;
                }

                let ref_normal = {
                    let n0 = triangle_normal(pa, pb, pc);
                    let n1 = triangle_normal(pb, pa, pd);
                    let s = Vector3::new(n0.x + n1.x, n0.y + n1.y, n0.z + n1.z);
                    if s.length() <= 1.0e-8 {
                        Vector3::new(0.0, 1.0, 0.0)
                    } else {
                        s
                    }
                };

                let mut nt0 = Triangle::new(c, d, a);
                orient_triangle(&mut nt0, &self.vertices, ref_normal);
                let mut nt1 = Triangle::new(d, c, b);
                orient_triangle(&mut nt1, &self.vertices, ref_normal);

                if self.triangle_area2_by_positions(nt0) <= area_epsilon
                    || self.triangle_area2_by_positions(nt1) <= area_epsilon
                {
                    continue;
                }

                self.triangles[t0_id] = nt0;
                self.triangles[t1_id] = nt1;
                flipped_any = true;
                break;
            }
            if !flipped_any {
                break;
            }
        }
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
                if seen.insert(vid)
                    && squared_distance(self.vertices[vid].position, position) <= eps2
                {
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

    fn find_hit_triangles_xz(&mut self, x: f32, z: f32, eps: f32) -> Vec<usize> {
        if let Some(tri_id) = self.last_hit_triangle
            && let Some(tri) = self.triangles.get(tri_id).copied()
        {
            let a = self.vertices[tri.a].position;
            let b = self.vertices[tri.b].position;
            let c = self.vertices[tri.c].position;
            if point_in_triangle_xz_strict_interior(x, z, a, b, c, eps * 4.0) {
                return vec![tri_id];
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

    fn find_hit_triangles_xz_in_region(
        &mut self,
        x: f32,
        z: f32,
        eps: f32,
        region_polygon_xz: &[(f32, f32)],
    ) -> Vec<usize> {
        let mut hits = Vec::new();
        for (tri_id, tri) in self.triangles.iter().copied().enumerate() {
            let a = self.vertices[tri.a].position;
            let b = self.vertices[tri.b].position;
            let c = self.vertices[tri.c].position;
            let cx = (a.x + b.x + c.x) / 3.0;
            let cz = (a.z + b.z + c.z) / 3.0;
            if !point_in_polygon_xz((cx, cz), region_polygon_xz) {
                continue;
            }
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

        let replacement =
            self.retriangulate_neighbors(vertex_id, &neighbors, base_normal, area_epsilon);
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
        if !self.replacement_normals_consistent(
            replacement,
            base_normal,
            normal_epsilon,
            area_epsilon,
        ) {
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

    fn enforce_single_height_per_xz(&mut self, eps: f32) {
        if self.vertices.is_empty() || self.triangles.is_empty() {
            return;
        }

        let step = if eps > 0.0 { eps } else { 1.0e-4 };
        let mut canonical_by_xz: HashMap<(i64, i64), VertexID> = HashMap::new();
        for (vid, v) in self.vertices.iter().enumerate() {
            canonical_by_xz.insert(quantize_xz_key(v.position.x, v.position.z, step), vid);
        }

        let mut remap: Vec<VertexID> = (0..self.vertices.len()).collect();
        let mut had_merge = false;
        for (vid, v) in self.vertices.iter().enumerate() {
            let key = quantize_xz_key(v.position.x, v.position.z, step);
            if let Some(&canonical) = canonical_by_xz.get(&key) {
                remap[vid] = canonical;
                if canonical != vid {
                    had_merge = true;
                }
            }
        }
        if !had_merge {
            return;
        }

        for tri in &mut self.triangles {
            tri.a = remap[tri.a];
            tri.b = remap[tri.b];
            tri.c = remap[tri.c];
        }
    }

    fn remove_invalid_triangles(&mut self, area_epsilon: f32) {
        self.triangles.retain(|tri| {
            if tri.a == tri.b || tri.b == tri.c || tri.a == tri.c {
                return false;
            }
            let a = self.vertices[tri.a].position;
            let b = self.vertices[tri.b].position;
            let c = self.vertices[tri.c].position;
            triangle_normal(a, b, c).length() > area_epsilon
        });
    }

    fn remove_duplicate_triangles(&mut self) {
        let mut seen: HashSet<[VertexID; 3]> = HashSet::new();
        self.triangles.retain(|tri| {
            let mut key = [tri.a, tri.b, tri.c];
            key.sort_unstable();
            seen.insert(key)
        });
    }

    fn compact_unreferenced_vertices(&mut self) {
        if self.vertices.is_empty() {
            return;
        }
        let mut used = vec![false; self.vertices.len()];
        for tri in &self.triangles {
            used[tri.a] = true;
            used[tri.b] = true;
            used[tri.c] = true;
        }

        let mut remap = vec![usize::MAX; self.vertices.len()];
        let mut compacted = Vec::with_capacity(self.vertices.len());
        for (old, is_used) in used.iter().copied().enumerate() {
            if is_used {
                remap[old] = compacted.len();
                compacted.push(self.vertices[old]);
            }
        }

        for tri in &mut self.triangles {
            tri.a = remap[tri.a];
            tri.b = remap[tri.b];
            tri.c = remap[tri.c];
        }
        self.vertices = compacted;
    }

    fn global_coplanar_cleanup(
        &mut self,
        area_epsilon: f32,
        normal_epsilon: f32,
        distance_epsilon: f32,
    ) {
        if self.vertices.len() <= 4 || self.triangles.len() <= 2 {
            return;
        }
        let mut guard = 0usize;
        loop {
            if self.vertices.len() <= 4 {
                break;
            }

            let mut changed = false;
            let mut vid = 0usize;
            while vid < self.vertices.len() {
                if self.try_remove_coplanar_vertex(
                    vid,
                    area_epsilon,
                    normal_epsilon,
                    distance_epsilon,
                ) {
                    changed = true;
                    break;
                }
                vid += 1;
            }

            if !changed {
                break;
            }
            guard += 1;
            if guard > self.vertices.len().saturating_mul(4).max(64) {
                break;
            }
        }
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

fn opposite_vertex_for_edge(tri: Triangle, a: VertexID, b: VertexID) -> Option<VertexID> {
    let ids = [tri.a, tri.b, tri.c];
    let has_a = ids.contains(&a);
    let has_b = ids.contains(&b);
    if !has_a || !has_b {
        return None;
    }
    ids.into_iter().find(|v| *v != a && *v != b)
}

fn orient_triangle(tri: &mut Triangle, vertices: &[crate::Vertex], reference_normal: Vector3) {
    let a = vertices[tri.a].position;
    let b = vertices[tri.b].position;
    let c = vertices[tri.c].position;
    if triangle_normal(a, b, c).dot(reference_normal) < 0.0 {
        std::mem::swap(&mut tri.b, &mut tri.c);
    }
}

fn segments_cross_strict_2d(
    p1: (f32, f32),
    p2: (f32, f32),
    q1: (f32, f32),
    q2: (f32, f32),
) -> bool {
    fn orient(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    }
    let o1 = orient(p1, p2, q1);
    let o2 = orient(p1, p2, q2);
    let o3 = orient(q1, q2, p1);
    let o4 = orient(q1, q2, p2);
    (o1 > 1.0e-7 && o2 < -1.0e-7 || o1 < -1.0e-7 && o2 > 1.0e-7)
        && (o3 > 1.0e-7 && o4 < -1.0e-7 || o3 < -1.0e-7 && o4 > 1.0e-7)
}

fn quantize_xz_key(x: f32, z: f32, step: f32) -> (i64, i64) {
    let inv = 1.0 / step.max(1.0e-6);
    let qx = (x * inv).round() as i64;
    let qz = (z * inv).round() as i64;
    (qx, qz)
}

fn morton_key_2d(x: f32, z: f32) -> u64 {
    let sx = ((x * 16.0).round() as i32)
        .saturating_add(1 << 15)
        .clamp(0, u16::MAX as i32) as u16;
    let sz = ((z * 16.0).round() as i32)
        .saturating_add(1 << 15)
        .clamp(0, u16::MAX as i32) as u16;
    interleave_u16(sx, sz)
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
        let intersects = ((zi > pz) != (zj > pz))
            && (px < (xj - xi) * (pz - zi) / ((zj - zi).abs().max(1.0e-8)) + xi);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn interleave_u16(x: u16, y: u16) -> u64 {
    let mut xx = x as u64;
    let mut yy = y as u64;
    xx = (xx | (xx << 8)) & 0x00FF_00FF;
    xx = (xx | (xx << 4)) & 0x0F0F_0F0F;
    xx = (xx | (xx << 2)) & 0x3333_3333;
    xx = (xx | (xx << 1)) & 0x5555_5555;

    yy = (yy | (yy << 8)) & 0x00FF_00FF;
    yy = (yy | (yy << 4)) & 0x0F0F_0F0F;
    yy = (yy | (yy << 2)) & 0x3333_3333;
    yy = (yy | (yy << 1)) & 0x5555_5555;

    xx | (yy << 1)
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

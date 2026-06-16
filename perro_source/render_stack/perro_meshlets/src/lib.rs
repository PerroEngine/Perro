use glam::{Vec2, Vec3A};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LodVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LodSurfaceRange {
    pub index_start: u32,
    pub index_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LodSet {
    pub indices: Vec<u32>,
    pub surface_ranges: Vec<LodSurfaceRange>,
}

pub const DEFAULT_LOD_TARGET_RATIOS: [f32; 6] = [1.0, 0.8, 0.6, 0.4, 0.25, 0.125];
const PARALLEL_EDGE_SCORE_THRESHOLD: usize = 256;
const PARALLEL_EDGE_SCORE_TRI_THRESHOLD: usize = 512;
const COLLAPSE_NORMAL_DOT_MIN: f32 = 0.15;
const COLLAPSE_CHEAP_NORMAL_DOT_MIN: f32 = 0.6;
const COLLAPSE_CHEAP_UV_DIST2_MAX: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshletBounds {
    pub index_start: u32,
    pub index_count: u32,
    pub center: [f32; 3],
    pub radius: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshletPack {
    pub packed_indices: Vec<u32>,
    pub meshlets: Vec<MeshletBounds>,
}

pub fn pack_meshlets_from_positions(
    positions: &[[f32; 3]],
    indices: &[u32],
    triangles_per_meshlet: usize,
) -> MeshletPack {
    let tri_len = (indices.len() / 3) * 3;
    if tri_len == 0 || triangles_per_meshlet == 0 {
        return MeshletPack {
            packed_indices: indices.to_vec(),
            meshlets: Vec::new(),
        };
    }

    let tri_count = tri_len / 3;
    let mut cmin = [f32::INFINITY; 3];
    let mut cmax = [f32::NEG_INFINITY; 3];
    for tri_i in 0..tri_count {
        let base = tri_i * 3;
        let Some(a) = positions.get(indices[base] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let Some(b) = positions.get(indices[base + 1] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let Some(c) = positions.get(indices[base + 2] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let cx = (a[0] + b[0] + c[0]) * (1.0 / 3.0);
        let cy = (a[1] + b[1] + c[1]) * (1.0 / 3.0);
        let cz = (a[2] + b[2] + c[2]) * (1.0 / 3.0);
        cmin[0] = cmin[0].min(cx);
        cmin[1] = cmin[1].min(cy);
        cmin[2] = cmin[2].min(cz);
        cmax[0] = cmax[0].max(cx);
        cmax[1] = cmax[1].max(cy);
        cmax[2] = cmax[2].max(cz);
    }

    let span = [
        (cmax[0] - cmin[0]).max(1.0e-6),
        (cmax[1] - cmin[1]).max(1.0e-6),
        (cmax[2] - cmin[2]).max(1.0e-6),
    ];

    let mut keyed = Vec::with_capacity(tri_count);
    for tri_i in 0..tri_count {
        let base = tri_i * 3;
        let Some(a) = positions.get(indices[base] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let Some(b) = positions.get(indices[base + 1] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let Some(c) = positions.get(indices[base + 2] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let nx = (((a[0] + b[0] + c[0]) * (1.0 / 3.0) - cmin[0]) / span[0]).clamp(0.0, 1.0);
        let ny = (((a[1] + b[1] + c[1]) * (1.0 / 3.0) - cmin[1]) / span[1]).clamp(0.0, 1.0);
        let nz = (((a[2] + b[2] + c[2]) * (1.0 / 3.0) - cmin[2]) / span[2]).clamp(0.0, 1.0);
        keyed.push((morton3(nx, ny, nz), tri_i as u32));
    }
    keyed.sort_unstable_by_key(|item| item.0);

    let mut packed_indices = Vec::with_capacity(indices.len());
    for (_, tri) in keyed {
        let base = tri as usize * 3;
        packed_indices.push(indices[base]);
        packed_indices.push(indices[base + 1]);
        packed_indices.push(indices[base + 2]);
    }
    if tri_len < indices.len() {
        packed_indices.extend_from_slice(&indices[tri_len..]);
    }

    let chunk = triangles_per_meshlet * 3;
    let packed_tri_len = (packed_indices.len() / 3) * 3;
    let mut meshlets = Vec::with_capacity(packed_tri_len.div_ceil(chunk));
    let mut start = 0usize;
    while start < packed_tri_len {
        let end = (start + chunk).min(packed_tri_len);
        if let Some((center, radius)) = meshlet_bounds(positions, &packed_indices[start..end]) {
            meshlets.push(MeshletBounds {
                index_start: start as u32,
                index_count: (end - start) as u32,
                center,
                radius,
            });
        }
        start = end;
    }

    MeshletPack {
        packed_indices,
        meshlets,
    }
}

pub fn build_lod_sets(
    vertices: &[LodVertex],
    indices: &[u32],
    surface_ranges: &[LodSurfaceRange],
    target_ratios: &[f32],
) -> Vec<LodSet> {
    let base_surfaces = if surface_ranges.is_empty() {
        vec![LodSurfaceRange {
            index_start: 0,
            index_count: indices.len() as u32,
        }]
    } else {
        surface_ranges.to_vec()
    };
    let tri_count = indices.len() / 3;
    if tri_count == 0 || target_ratios.is_empty() {
        return vec![LodSet {
            indices: indices.to_vec(),
            surface_ranges: base_surfaces,
        }];
    }

    let ratios = target_ratios
        .iter()
        .map(|ratio| {
            if ratio.is_finite() {
                ratio.clamp(0.0, 1.0)
            } else {
                1.0
            }
        })
        .collect::<Vec<_>>();
    let build_surface = |range: &LodSurfaceRange| {
        let start = range.index_start as usize;
        let end = start
            .saturating_add(range.index_count as usize)
            .min(indices.len());
        build_surface_lods(vertices, &indices[start..end], &ratios)
    };
    let per_surface_lods = if base_surfaces.len() >= 4 && tri_count >= 512 {
        base_surfaces
            .par_iter()
            .map(build_surface)
            .collect::<Vec<_>>()
    } else {
        base_surfaces.iter().map(build_surface).collect::<Vec<_>>()
    };

    let mut lods = Vec::new();
    for ratio_index in 0..ratios.len() {
        let mut lod_indices = Vec::new();
        let mut lod_surfaces = Vec::new();
        for surface_lods in &per_surface_lods {
            let Some(simplified) = surface_lods.get(ratio_index) else {
                continue;
            };
            if simplified.is_empty() {
                continue;
            }
            let surface_start = lod_indices.len() as u32;
            lod_indices.extend_from_slice(simplified);
            let surface_count = (lod_indices.len() as u32).saturating_sub(surface_start);
            if surface_count > 0 {
                lod_surfaces.push(LodSurfaceRange {
                    index_start: surface_start,
                    index_count: surface_count,
                });
            }
        }
        let duplicate = lods
            .last()
            .is_some_and(|prev: &LodSet| prev.indices == lod_indices);
        if lod_indices.len() >= 3 && !duplicate {
            lods.push(LodSet {
                indices: lod_indices,
                surface_ranges: lod_surfaces,
            });
        }
    }
    if lods.is_empty() {
        lods.push(LodSet {
            indices: indices.to_vec(),
            surface_ranges: base_surfaces,
        });
    }
    lods
}

fn build_surface_lods(
    vertices: &[LodVertex],
    surface_indices: &[u32],
    ratios: &[f32],
) -> Vec<Vec<u32>> {
    let original_tri_count = surface_indices.len() / 3;
    if original_tri_count == 0 {
        return ratios.iter().map(|_| Vec::new()).collect();
    }
    let mut current = surface_indices[..original_tri_count * 3].to_vec();
    let mut lods = Vec::with_capacity(ratios.len());
    for &ratio in ratios {
        let target = ((original_tri_count as f32) * ratio).ceil() as usize;
        let target = target.clamp(1, original_tri_count);
        let current_tris = current.len() / 3;
        if target < current_tris {
            current = simplify_surface(vertices, &current, target);
        }
        lods.push(current.clone());
    }
    lods
}

fn simplify_surface(vertices: &[LodVertex], surface_indices: &[u32], keep_tris: usize) -> Vec<u32> {
    let tri_count = surface_indices.len() / 3;
    if keep_tris >= tri_count {
        return surface_indices[..tri_count * 3].to_vec();
    }
    if surface_indices
        .iter()
        .any(|&idx| (idx as usize) >= vertices.len())
    {
        return surface_indices[..tri_count * 3].to_vec();
    }

    let mut triangles = surface_indices[..tri_count * 3]
        .chunks_exact(3)
        .map(|tri| [tri[0], tri[1], tri[2]])
        .filter(|tri| !tri_degenerate(*tri))
        .collect::<Vec<_>>();
    if triangles.len() <= keep_tris {
        return flatten_tris(&triangles);
    }
    let quadrics = build_vertex_quadrics(vertices, &triangles);
    let mut scratch = SimplifyScratch::default();
    let mut guard = 0usize;
    while triangles.len() > keep_tris && guard < tri_count.saturating_mul(16).max(64) {
        guard += 1;
        scratch.prepare(vertices.len(), triangles.len());
        build_topology(vertices, &triangles, &mut scratch);
        let Some(collapse) = best_collapse(
            vertices,
            &triangles,
            &quadrics,
            &scratch.edge_list,
            &scratch.adjacency,
            &scratch.tri_normals,
        ) else {
            break;
        };
        if !apply_collapse_into(
            vertices,
            &triangles,
            collapse.keep,
            collapse.remove,
            &mut scratch.next_triangles,
            &mut scratch.seen_triangles,
            &mut scratch.candidate_edges,
        ) {
            break;
        }
        if scratch.next_triangles.len() >= triangles.len() {
            break;
        }
        std::mem::swap(&mut triangles, &mut scratch.next_triangles);
    }
    flatten_tris(&triangles)
}

#[derive(Clone, Copy)]
struct Collapse {
    keep: u32,
    remove: u32,
    cost: f64,
}

#[derive(Clone, Copy)]
struct EdgeCandidate {
    a: u32,
    b: u32,
    count: u32,
}

#[derive(Clone, Copy, Default)]
struct EdgeInfo {
    count: u32,
}

#[derive(Default)]
struct SimplifyScratch {
    edges: HashMap<(u32, u32), EdgeInfo>,
    edge_list: Vec<EdgeCandidate>,
    candidate_edges: HashMap<(u32, u32), EdgeInfo>,
    adjacency: Vec<Vec<usize>>,
    tri_normals: Vec<Option<[f32; 3]>>,
    next_triangles: Vec<[u32; 3]>,
    seen_triangles: HashSet<[u32; 3]>,
}

impl SimplifyScratch {
    fn prepare(&mut self, vertex_count: usize, tri_capacity: usize) {
        if self.adjacency.len() < vertex_count {
            self.adjacency.resize_with(vertex_count, Vec::new);
        }
        for items in &mut self.adjacency {
            items.clear();
        }
        self.edges.clear();
        self.edge_list.clear();
        self.edge_list.reserve(tri_capacity.saturating_mul(3) / 2);
        self.candidate_edges.clear();
        self.tri_normals.clear();
        self.tri_normals.reserve(tri_capacity);
        self.next_triangles.clear();
        self.next_triangles.reserve(tri_capacity);
        self.seen_triangles.clear();
        self.seen_triangles.reserve(tri_capacity);
    }
}

fn best_collapse(
    vertices: &[LodVertex],
    triangles: &[[u32; 3]],
    quadrics: &[[f64; 10]],
    edges: &[EdgeCandidate],
    adjacency: &[Vec<usize>],
    tri_normals: &[Option<[f32; 3]>],
) -> Option<Collapse> {
    let score_edge = |edge: &EdgeCandidate| {
        let mut best = None;
        let boundary_penalty = if edge.count <= 1 { 10_000.0 } else { 1.0 };
        for (keep, remove) in [(edge.a, edge.b), (edge.b, edge.a)] {
            if !collapse_passes_cheap_prune(vertices, adjacency, keep, remove) {
                continue;
            }
            if !collapse_preserves_normals(
                vertices,
                triangles,
                adjacency,
                tri_normals,
                keep,
                remove,
            ) {
                continue;
            }
            let cost = collapse_cost(vertices, quadrics, keep, remove) * boundary_penalty;
            best = better_collapse(best, Some(Collapse { keep, remove, cost }));
        }
        best
    };
    if edges.len() >= PARALLEL_EDGE_SCORE_THRESHOLD
        && triangles.len() >= PARALLEL_EDGE_SCORE_TRI_THRESHOLD
    {
        edges
            .par_iter()
            .map(score_edge)
            .reduce(|| None, better_collapse)
    } else {
        let mut best = None;
        for edge in edges {
            best = better_collapse(best, score_edge(edge));
        }
        best
    }
}

fn build_topology(vertices: &[LodVertex], triangles: &[[u32; 3]], scratch: &mut SimplifyScratch) {
    for (tri_index, tri) in triangles.iter().enumerate() {
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let key = if a <= b { (a, b) } else { (b, a) };
            scratch
                .edges
                .entry(key)
                .and_modify(|edge: &mut EdgeInfo| edge.count = edge.count.saturating_add(1))
                .or_insert(EdgeInfo { count: 1 });
        }
        for idx in *tri {
            if let Some(items) = scratch.adjacency.get_mut(idx as usize) {
                items.push(tri_index);
            }
        }
        scratch.tri_normals.push(tri_normal(vertices, *tri));
    }
    scratch
        .edge_list
        .extend(scratch.edges.iter().map(|(&(a, b), info)| EdgeCandidate {
            a,
            b,
            count: info.count,
        }));
}

fn better_collapse(current: Option<Collapse>, candidate: Option<Collapse>) -> Option<Collapse> {
    match (current, candidate) {
        (None, x) => x,
        (x, None) => x,
        (Some(a), Some(b)) => {
            if b.cost < a.cost {
                Some(b)
            } else {
                Some(a)
            }
        }
    }
}

fn collapse_cost(vertices: &[LodVertex], quadrics: &[[f64; 10]], keep: u32, remove: u32) -> f64 {
    let Some(keep_v) = vertices.get(keep as usize) else {
        return f64::INFINITY;
    };
    let Some(remove_v) = vertices.get(remove as usize) else {
        return f64::INFINITY;
    };
    let mut q = quadrics.get(keep as usize).copied().unwrap_or([0.0; 10]);
    let rq = quadrics.get(remove as usize).copied().unwrap_or([0.0; 10]);
    for (dst, src) in q.iter_mut().zip(rq.iter()) {
        *dst += *src;
    }
    let p = [
        keep_v.position[0] as f64,
        keep_v.position[1] as f64,
        keep_v.position[2] as f64,
    ];
    let qem = eval_quadric(q, p);
    let uv = dist2_2(keep_v.uv, remove_v.uv) as f64;
    let normal = (1.0 - dot3(keep_v.normal, remove_v.normal).clamp(-1.0, 1.0) as f64).max(0.0);
    let len = dist2_3(keep_v.position, remove_v.position) as f64;
    qem + len * 0.001 + uv * 250.0 + normal * 250.0
}

fn collapse_preserves_normals(
    vertices: &[LodVertex],
    triangles: &[[u32; 3]],
    adjacency: &[Vec<usize>],
    tri_normals: &[Option<[f32; 3]>],
    keep: u32,
    remove: u32,
) -> bool {
    let Some(affected) = adjacency.get(remove as usize) else {
        return false;
    };
    for &tri_index in affected {
        let Some(&tri) = triangles.get(tri_index) else {
            continue;
        };
        let mapped = [
            if tri[0] == remove { keep } else { tri[0] },
            if tri[1] == remove { keep } else { tri[1] },
            if tri[2] == remove { keep } else { tri[2] },
        ];
        if tri_degenerate(mapped) {
            continue;
        }
        let before = tri_normals.get(tri_index).copied().flatten();
        if normal_dot_after(vertices, before, mapped) < COLLAPSE_NORMAL_DOT_MIN {
            return false;
        }
    }
    true
}

fn collapse_passes_cheap_prune(
    vertices: &[LodVertex],
    adjacency: &[Vec<usize>],
    keep: u32,
    remove: u32,
) -> bool {
    let Some(keep_v) = vertices.get(keep as usize) else {
        return false;
    };
    let Some(remove_v) = vertices.get(remove as usize) else {
        return false;
    };
    if dot3(keep_v.normal, remove_v.normal) < COLLAPSE_CHEAP_NORMAL_DOT_MIN {
        return false;
    }
    if dist2_2(keep_v.uv, remove_v.uv) > COLLAPSE_CHEAP_UV_DIST2_MAX {
        return false;
    }
    let Some(affected) = adjacency.get(remove as usize) else {
        return false;
    };
    !affected.is_empty()
}

fn apply_collapse_into(
    _vertices: &[LodVertex],
    triangles: &[[u32; 3]],
    keep: u32,
    remove: u32,
    next: &mut Vec<[u32; 3]>,
    seen: &mut HashSet<[u32; 3]>,
    edge_counts: &mut HashMap<(u32, u32), EdgeInfo>,
) -> bool {
    for &tri in triangles {
        let mapped = [
            if tri[0] == remove { keep } else { tri[0] },
            if tri[1] == remove { keep } else { tri[1] },
            if tri[2] == remove { keep } else { tri[2] },
        ];
        if tri_degenerate(mapped) {
            continue;
        }
        let key = canonical_tri(mapped);
        if seen.insert(key) {
            for (a, b) in [
                (mapped[0], mapped[1]),
                (mapped[1], mapped[2]),
                (mapped[2], mapped[0]),
            ] {
                let edge_key = if a <= b { (a, b) } else { (b, a) };
                let count = edge_counts
                    .entry(edge_key)
                    .and_modify(|edge: &mut EdgeInfo| edge.count = edge.count.saturating_add(1))
                    .or_insert(EdgeInfo { count: 1 })
                    .count;
                if count > 2 {
                    next.clear();
                    seen.clear();
                    edge_counts.clear();
                    return false;
                }
            }
            next.push(mapped);
        }
    }
    if next.is_empty() {
        next.clear();
        seen.clear();
        edge_counts.clear();
        return false;
    }
    edge_counts.clear();
    true
}

fn build_vertex_quadrics(vertices: &[LodVertex], triangles: &[[u32; 3]]) -> Vec<[f64; 10]> {
    let mut quadrics = vec![[0.0; 10]; vertices.len()];
    for &tri in triangles {
        let Some(plane) = tri_plane(vertices, tri) else {
            continue;
        };
        let q = plane_quadric(plane);
        for idx in tri {
            let Some(entry) = quadrics.get_mut(idx as usize) else {
                continue;
            };
            for (dst, src) in entry.iter_mut().zip(q.iter()) {
                *dst += *src;
            }
        }
    }
    quadrics
}

fn tri_plane(vertices: &[LodVertex], tri: [u32; 3]) -> Option<[f64; 4]> {
    let a = vertices.get(tri[0] as usize)?.position;
    let b = vertices.get(tri[1] as usize)?.position;
    let c = vertices.get(tri[2] as usize)?.position;
    let ab = sub3(b, a);
    let ac = sub3(c, a);
    let n = normalize3(cross3(ab, ac))?;
    let d = -dot3(n, a) as f64;
    Some([n[0] as f64, n[1] as f64, n[2] as f64, d])
}

fn plane_quadric(p: [f64; 4]) -> [f64; 10] {
    [
        p[0] * p[0],
        p[0] * p[1],
        p[0] * p[2],
        p[0] * p[3],
        p[1] * p[1],
        p[1] * p[2],
        p[1] * p[3],
        p[2] * p[2],
        p[2] * p[3],
        p[3] * p[3],
    ]
}

fn eval_quadric(q: [f64; 10], p: [f64; 3]) -> f64 {
    let x = p[0];
    let y = p[1];
    let z = p[2];
    q[0] * x * x
        + 2.0 * q[1] * x * y
        + 2.0 * q[2] * x * z
        + 2.0 * q[3] * x
        + q[4] * y * y
        + 2.0 * q[5] * y * z
        + 2.0 * q[6] * y
        + q[7] * z * z
        + 2.0 * q[8] * z
        + q[9]
}

fn normal_dot_after(vertices: &[LodVertex], before: Option<[f32; 3]>, after: [u32; 3]) -> f32 {
    let Some(a) = before else {
        return 1.0;
    };
    let Some(b) = tri_normal(vertices, after) else {
        return 0.0;
    };
    dot3(a, b)
}

fn tri_normal(vertices: &[LodVertex], tri: [u32; 3]) -> Option<[f32; 3]> {
    let a = vertices.get(tri[0] as usize)?.position;
    let b = vertices.get(tri[1] as usize)?.position;
    let c = vertices.get(tri[2] as usize)?.position;
    normalize3(cross3(sub3(b, a), sub3(c, a)))
}

fn flatten_tris(triangles: &[[u32; 3]]) -> Vec<u32> {
    let mut out = Vec::with_capacity(triangles.len() * 3);
    for tri in triangles {
        out.extend_from_slice(tri);
    }
    out
}

fn canonical_tri(mut tri: [u32; 3]) -> [u32; 3] {
    tri.sort_unstable();
    tri
}

fn tri_degenerate(tri: [u32; 3]) -> bool {
    tri[0] == tri[1] || tri[1] == tri[2] || tri[2] == tri[0]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    (Vec3A::from(a) - Vec3A::from(b)).to_array()
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    Vec3A::from(a).cross(Vec3A::from(b)).to_array()
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    Vec3A::from(a).dot(Vec3A::from(b))
}

fn normalize3(v: [f32; 3]) -> Option<[f32; 3]> {
    let v = Vec3A::from(v);
    let len_sq = v.length_squared();
    if !len_sq.is_finite() || len_sq <= 1.0e-12 {
        return None;
    }
    Some((v * len_sq.sqrt().recip()).to_array())
}

fn dist2_2(a: [f32; 2], b: [f32; 2]) -> f32 {
    Vec2::from(a).distance_squared(Vec2::from(b))
}

fn dist2_3(a: [f32; 3], b: [f32; 3]) -> f32 {
    Vec3A::from(a).distance_squared(Vec3A::from(b))
}

fn meshlet_bounds(positions: &[[f32; 3]], indices: &[u32]) -> Option<([f32; 3], f32)> {
    let mut min = Vec3A::splat(f32::INFINITY);
    let mut max = Vec3A::splat(f32::NEG_INFINITY);
    for &idx in indices {
        let p = Vec3A::from(*positions.get(idx as usize)?);
        min = min.min(p);
        max = max.max(p);
    }
    if !(min.is_finite() && max.is_finite()) {
        return None;
    }
    let center = (min + max) * 0.5;
    let mut radius_sq = 0.0f32;
    for &idx in indices {
        let p = Vec3A::from(*positions.get(idx as usize)?);
        radius_sq = radius_sq.max(p.distance_squared(center));
    }
    Some((center.to_array(), radius_sq.sqrt()))
}

#[inline]
fn morton3(nx: f32, ny: f32, nz: f32) -> u64 {
    let qx = (nx * 1023.0).round() as u32;
    let qy = (ny * 1023.0).round() as u32;
    let qz = (nz * 1023.0).round() as u32;
    interleave10(qx) | (interleave10(qy) << 1) | (interleave10(qz) << 2)
}

#[inline]
fn interleave10(v: u32) -> u64 {
    let mut x = (v & 0x3ff) as u64;
    x = (x | (x << 16)) & 0x30000ff;
    x = (x | (x << 8)) & 0x300f00f;
    x = (x | (x << 4)) & 0x30c30c3;
    x = (x | (x << 2)) & 0x9249249;
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_mesh(size: u32) -> (Vec<LodVertex>, Vec<u32>, Vec<LodSurfaceRange>) {
        let mut vertices = Vec::new();
        for y in 0..=size {
            for x in 0..=size {
                vertices.push(LodVertex {
                    position: [x as f32, y as f32, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [x as f32 / size as f32, y as f32 / size as f32],
                });
            }
        }
        let stride = size + 1;
        let mut indices = Vec::new();
        for y in 0..size {
            for x in 0..size {
                let a = y * stride + x;
                let b = a + 1;
                let c = a + stride;
                let d = c + 1;
                indices.extend_from_slice(&[a, b, d, a, d, c]);
            }
        }
        let surfaces = vec![LodSurfaceRange {
            index_start: 0,
            index_count: indices.len() as u32,
        }];
        (vertices, indices, surfaces)
    }

    #[test]
    fn lod_sets_use_requested_ratios() {
        let (vertices, indices, surfaces) = grid_mesh(6);
        let lods = build_lod_sets(&vertices, &indices, &surfaces, &[1.0, 0.5, 0.25]);
        assert_eq!(lods.len(), 3);
        assert_eq!(lods[0].indices.len(), indices.len());
        assert!(lods[1].indices.len() < lods[0].indices.len());
        assert!(lods[2].indices.len() <= lods[1].indices.len());
    }

    #[test]
    fn lod_sets_keep_surface_slots() {
        let (vertices, indices, _) = grid_mesh(4);
        let half = (indices.len() / 2) as u32;
        let surfaces = vec![
            LodSurfaceRange {
                index_start: 0,
                index_count: half,
            },
            LodSurfaceRange {
                index_start: half,
                index_count: indices.len() as u32 - half,
            },
        ];
        let lods = build_lod_sets(&vertices, &indices, &surfaces, &[1.0, 0.5]);
        assert!(lods.iter().all(|lod| lod.surface_ranges.len() == 2));
    }

    #[test]
    fn lod_sets_do_not_cross_uv_seam_vertices() {
        let vertices = vec![
            LodVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
            },
            LodVertex {
                position: [1.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
            },
            LodVertex {
                position: [0.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
            },
            LodVertex {
                position: [1.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 1.0],
            },
            LodVertex {
                position: [1.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
            },
            LodVertex {
                position: [2.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
            },
            LodVertex {
                position: [1.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
            },
            LodVertex {
                position: [2.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 1.0],
            },
        ];
        let indices = vec![0, 1, 3, 0, 3, 2, 4, 5, 7, 4, 7, 6];
        let lods = build_lod_sets(&vertices, &indices, &[], &[0.5]);
        let seam_mixed = lods[0].indices.chunks_exact(3).any(|tri| {
            tri.iter().any(|idx| *idx == 1 || *idx == 3)
                && tri.iter().any(|idx| *idx == 4 || *idx == 6)
        });
        assert!(!seam_mixed);
    }
}

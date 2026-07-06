use super::*;

pub(super) fn mesh_query_cache() -> &'static RwLock<QueryMeshCache> {
    static CACHE: OnceLock<RwLock<QueryMeshCache>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(AHashMap::default()))
}

#[inline]
pub(super) fn runtime_mesh_query_cache_key(mesh_id: MeshID, revision: u64) -> u64 {
    mesh_id
        .as_u64()
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .rotate_left(17)
        ^ revision
        ^ 0x5bf0_3635_6ad6_22d5
}

thread_local! {
    pub(super) static GLTF_POS_SCRATCH: RefCell<Vec<[f32; 3]>> = const { RefCell::new(Vec::new()) };
    pub(super) static GLTF_INDEX_SCRATCH: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

pub(super) struct QueryNodeData {
    pub(super) source: String,
    pub(super) surfaces: Vec<MeshSurfaceBinding>,
    pub(super) instance_local: Vec<Mat4>,
}

/// Per-node cache entry for [`QueryNodeData`]. `instance_local` is the
/// expensive-to-rebuild part on `MultiMeshInstance3D` (one
/// `Mat4::from_scale_rotation_translation` per instance, plus a
/// `surfaces.clone()`), so point/ray/region queries reuse it across calls
/// instead of rebuilding on every query.
pub(crate) struct QueryNodeDataCacheEntry {
    pub(super) data: Arc<QueryNodeData>,
    /// `nodes.mutation_revision()` snapshot taken when `data` was built. Any
    /// node mutation anywhere bumps this (see `NodeArena::bump_mutation_revision`),
    /// so it's a conservative-but-correct invalidation signal: no false
    /// cache hits, occasional false invalidations when unrelated nodes
    /// change between queries.
    pub(super) built_at_version: u64,
}

pub(crate) type QueryNodeDataCache = AHashMap<NodeID, QueryNodeDataCacheEntry>;

#[derive(Clone, Copy)]
pub(super) struct QueryHitCandidate {
    pub(super) instance_index: u32,
    pub(super) surface_index: u32,
    pub(super) global_point: Vec3,
    pub(super) local_point: Vec3,
    pub(super) global_normal: Vec3,
    pub(super) local_normal: Vec3,
    pub(super) metric: f32,
}

#[derive(Clone, Copy)]
pub(super) struct QueryRegionAcc {
    pub(super) tri_count: u32,
    pub(super) sum_local: Vec3,
    pub(super) sum_global: Vec3,
    pub(super) local_min: Vec3,
    pub(super) local_max: Vec3,
    pub(super) global_min: Vec3,
    pub(super) global_max: Vec3,
}

pub(super) struct QueryBvhStack {
    inline: [u32; 64],
    len: usize,
    spill: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QueryMeshStrategy {
    Linear,
    Bvh,
}

#[inline]
pub(super) fn query_mesh_strategy(tri_count: usize) -> QueryMeshStrategy {
    if tri_count <= QUERY_LINEAR_TRI_THRESHOLD {
        QueryMeshStrategy::Linear
    } else {
        QueryMeshStrategy::Bvh
    }
}

impl QueryBvhStack {
    #[inline]
    pub(super) fn root() -> Self {
        let mut inline = [0; 64];
        inline[0] = 0;
        Self {
            inline,
            len: 1,
            spill: Vec::new(),
        }
    }

    #[inline]
    pub(super) fn push(&mut self, node: u32) {
        if self.len < self.inline.len() {
            self.inline[self.len] = node;
            self.len += 1;
        } else {
            self.spill.push(node);
        }
    }

    #[inline]
    pub(super) fn pop(&mut self) -> Option<u32> {
        if let Some(node) = self.spill.pop() {
            return Some(node);
        }
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.inline[self.len])
    }
}

pub(super) fn query_point_tri_local(
    mesh: &QueryMeshData,
    tri_idx: usize,
    p_local: Vec3,
    best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let tri = *mesh.triangles.get(tri_idx)?;
    let acc = *mesh.tri_accel.get(tri_idx)?;
    if let Some(hit) = best {
        let tri_d2 = aabb_distance2(p_local, acc.aabb_min, acc.aabb_max);
        if tri_d2 >= hit.metric {
            return Some(best);
        }
    }
    let a = mesh.vertices[tri.a as usize];
    let b = mesh.vertices[tri.b as usize];
    let c = mesh.vertices[tri.c as usize];
    let nearest_local = closest_point_on_triangle(p_local, a, b, c);
    let d2 = nearest_local.distance_squared(p_local);
    if let Some(hit) = best
        && d2 >= hit.metric
    {
        return Some(best);
    }
    Some(Some(QueryHitCandidate {
        instance_index: 0,
        surface_index: tri.surface_index,
        global_point: nearest_local,
        local_point: nearest_local,
        global_normal: acc.normal,
        local_normal: acc.normal,
        metric: d2,
    }))
}

pub(super) fn query_point_tri_global(
    mesh: &QueryMeshData,
    tri_idx: usize,
    p_local: Vec3,
    instance_index: u32,
    global_from_mesh: Mat4,
    global_normal_basis: Mat3,
    best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let tri = *mesh.triangles.get(tri_idx)?;
    let acc = *mesh.tri_accel.get(tri_idx)?;
    if let Some(hit) = best {
        let tri_d2 = aabb_distance2(p_local, acc.aabb_min, acc.aabb_max);
        if tri_d2 >= hit.metric {
            return Some(best);
        }
    }
    let a = mesh.vertices[tri.a as usize];
    let b = mesh.vertices[tri.b as usize];
    let c = mesh.vertices[tri.c as usize];
    let nearest_local = closest_point_on_triangle(p_local, a, b, c);
    let d2 = nearest_local.distance_squared(p_local);
    if let Some(hit) = best
        && d2 >= hit.metric
    {
        return Some(best);
    }
    let nearest_global = global_from_mesh.transform_point3(nearest_local);
    let global_normal = (global_normal_basis * acc.normal).normalize_or_zero();
    Some(Some(QueryHitCandidate {
        instance_index,
        surface_index: tri.surface_index,
        global_point: nearest_global,
        local_point: nearest_local,
        global_normal,
        local_normal: acc.normal,
        metric: d2,
    }))
}

pub(super) fn query_point_mesh_bvh(
    mesh: &QueryMeshData,
    p_local: Vec3,
    mut best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let mut stack = QueryBvhStack::root();
    while let Some(node_idx) = stack.pop() {
        let bvh = *mesh.bvh_nodes.get(node_idx as usize)?;
        let node_d2 = aabb_distance2(p_local, bvh.aabb_min, bvh.aabb_max);
        if let Some(hit) = best
            && node_d2 >= hit.metric
        {
            continue;
        }
        if bvh.left == u32::MAX || bvh.right == u32::MAX {
            let start = bvh.tri_start as usize;
            let end = start + bvh.tri_count as usize;
            for &tri_idx in &mesh.bvh_tri_indices[start..end] {
                best = query_point_tri_local(mesh, tri_idx as usize, p_local, best)?;
            }
        } else {
            let left = *mesh.bvh_nodes.get(bvh.left as usize)?;
            let right = *mesh.bvh_nodes.get(bvh.right as usize)?;
            let ld2 = aabb_distance2(p_local, left.aabb_min, left.aabb_max);
            let rd2 = aabb_distance2(p_local, right.aabb_min, right.aabb_max);
            if ld2 < rd2 {
                stack.push(bvh.right);
                stack.push(bvh.left);
            } else {
                stack.push(bvh.left);
                stack.push(bvh.right);
            }
        }
    }
    Some(best)
}

pub(super) fn query_point_mesh_bvh_global(
    mesh: &QueryMeshData,
    p_local: Vec3,
    instance_index: u32,
    global_from_mesh: Mat4,
    global_normal_basis: Mat3,
    mut best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let mut stack = QueryBvhStack::root();
    while let Some(node_idx) = stack.pop() {
        let bvh = *mesh.bvh_nodes.get(node_idx as usize)?;
        let node_d2 = aabb_distance2(p_local, bvh.aabb_min, bvh.aabb_max);
        if let Some(hit) = best
            && node_d2 >= hit.metric
        {
            continue;
        }
        if bvh.left == u32::MAX || bvh.right == u32::MAX {
            let start = bvh.tri_start as usize;
            let end = start + bvh.tri_count as usize;
            for &tri_idx in &mesh.bvh_tri_indices[start..end] {
                best = query_point_tri_global(
                    mesh,
                    tri_idx as usize,
                    p_local,
                    instance_index,
                    global_from_mesh,
                    global_normal_basis,
                    best,
                )?;
            }
        } else {
            let left = *mesh.bvh_nodes.get(bvh.left as usize)?;
            let right = *mesh.bvh_nodes.get(bvh.right as usize)?;
            let ld2 = aabb_distance2(p_local, left.aabb_min, left.aabb_max);
            let rd2 = aabb_distance2(p_local, right.aabb_min, right.aabb_max);
            if ld2 < rd2 {
                stack.push(bvh.right);
                stack.push(bvh.left);
            } else {
                stack.push(bvh.left);
                stack.push(bvh.right);
            }
        }
    }
    Some(best)
}

pub(super) fn query_ray_tri_local(
    mesh: &QueryMeshData,
    tri_idx: usize,
    ray_origin_local: Vec3,
    ray_dir_local: Vec3,
    max_t: f32,
    best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let tri = *mesh.triangles.get(tri_idx)?;
    let acc = *mesh.tri_accel.get(tri_idx)?;
    let limit = best.map_or(max_t, |h| h.metric.min(max_t));
    if ray_aabb_tmin(
        ray_origin_local,
        ray_dir_local,
        acc.aabb_min,
        acc.aabb_max,
        limit,
    )
    .is_none()
    {
        return Some(best);
    }
    let a = mesh.vertices[tri.a as usize];
    let b = mesh.vertices[tri.b as usize];
    let c = mesh.vertices[tri.c as usize];
    let t = ray_intersect_triangle(ray_origin_local, ray_dir_local, a, b, c)?;
    if t > max_t {
        return Some(best);
    }
    if let Some(hit) = best
        && t >= hit.metric
    {
        return Some(best);
    }
    let hit_local = ray_origin_local + ray_dir_local * t;
    Some(Some(QueryHitCandidate {
        instance_index: 0,
        surface_index: tri.surface_index,
        global_point: hit_local,
        local_point: hit_local,
        global_normal: acc.normal,
        local_normal: acc.normal,
        metric: t,
    }))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn query_ray_tri_global(
    mesh: &QueryMeshData,
    tri_idx: usize,
    ray_origin_local: Vec3,
    ray_dir_local: Vec3,
    ray_origin_global: Vec3,
    max_t: f32,
    instance_index: u32,
    global_from_mesh: Mat4,
    global_normal_basis: Mat3,
    best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let tri = *mesh.triangles.get(tri_idx)?;
    let acc = *mesh.tri_accel.get(tri_idx)?;
    let limit = best.map_or(max_t, |h| h.metric.min(max_t));
    if ray_aabb_tmin(
        ray_origin_local,
        ray_dir_local,
        acc.aabb_min,
        acc.aabb_max,
        limit,
    )
    .is_none()
    {
        return Some(best);
    }
    let a = mesh.vertices[tri.a as usize];
    let b = mesh.vertices[tri.b as usize];
    let c = mesh.vertices[tri.c as usize];
    let t = ray_intersect_triangle(ray_origin_local, ray_dir_local, a, b, c)?;
    if t > max_t {
        return Some(best);
    }
    let hit_local = ray_origin_local + ray_dir_local * t;
    let hit_global = global_from_mesh.transform_point3(hit_local);
    let global_t = (hit_global - ray_origin_global).length();
    if global_t > max_t {
        return Some(best);
    }
    if let Some(hit) = best
        && global_t >= hit.metric
    {
        return Some(best);
    }
    let global_normal = (global_normal_basis * acc.normal).normalize_or_zero();
    Some(Some(QueryHitCandidate {
        instance_index,
        surface_index: tri.surface_index,
        global_point: hit_global,
        local_point: hit_local,
        global_normal,
        local_normal: acc.normal,
        metric: global_t,
    }))
}

pub(super) fn query_ray_mesh_bvh(
    mesh: &QueryMeshData,
    ray_origin_local: Vec3,
    ray_dir_local: Vec3,
    max_t: f32,
    mut best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let mut stack = QueryBvhStack::root();
    while let Some(node_idx) = stack.pop() {
        let bvh = *mesh.bvh_nodes.get(node_idx as usize)?;
        let limit = best.map_or(max_t, |h| h.metric.min(max_t));
        if ray_aabb_tmin(
            ray_origin_local,
            ray_dir_local,
            bvh.aabb_min,
            bvh.aabb_max,
            limit,
        )
        .is_none()
        {
            continue;
        }
        if bvh.left == u32::MAX || bvh.right == u32::MAX {
            let start = bvh.tri_start as usize;
            let end = start + bvh.tri_count as usize;
            for &tri_idx in &mesh.bvh_tri_indices[start..end] {
                best = query_ray_tri_local(
                    mesh,
                    tri_idx as usize,
                    ray_origin_local,
                    ray_dir_local,
                    max_t,
                    best,
                )?;
            }
        } else {
            let limit = best.map_or(max_t, |h| h.metric.min(max_t));
            let left = *mesh.bvh_nodes.get(bvh.left as usize)?;
            let right = *mesh.bvh_nodes.get(bvh.right as usize)?;
            let lt = ray_aabb_tmin(
                ray_origin_local,
                ray_dir_local,
                left.aabb_min,
                left.aabb_max,
                limit,
            )
            .unwrap_or(f32::INFINITY);
            let rt = ray_aabb_tmin(
                ray_origin_local,
                ray_dir_local,
                right.aabb_min,
                right.aabb_max,
                limit,
            )
            .unwrap_or(f32::INFINITY);
            if lt < rt {
                stack.push(bvh.right);
                stack.push(bvh.left);
            } else {
                stack.push(bvh.left);
                stack.push(bvh.right);
            }
        }
    }
    Some(best)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn query_ray_mesh_bvh_global(
    mesh: &QueryMeshData,
    ray_origin_local: Vec3,
    ray_dir_local: Vec3,
    ray_origin_global: Vec3,
    max_t: f32,
    instance_index: u32,
    global_from_mesh: Mat4,
    global_normal_basis: Mat3,
    mut best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let mut stack = QueryBvhStack::root();
    while let Some(node_idx) = stack.pop() {
        let bvh = *mesh.bvh_nodes.get(node_idx as usize)?;
        let limit = best.map_or(max_t, |h| h.metric.min(max_t));
        if ray_aabb_tmin(
            ray_origin_local,
            ray_dir_local,
            bvh.aabb_min,
            bvh.aabb_max,
            limit,
        )
        .is_none()
        {
            continue;
        }
        if bvh.left == u32::MAX || bvh.right == u32::MAX {
            let start = bvh.tri_start as usize;
            let end = start + bvh.tri_count as usize;
            for &tri_idx in &mesh.bvh_tri_indices[start..end] {
                best = query_ray_tri_global(
                    mesh,
                    tri_idx as usize,
                    ray_origin_local,
                    ray_dir_local,
                    ray_origin_global,
                    max_t,
                    instance_index,
                    global_from_mesh,
                    global_normal_basis,
                    best,
                )?;
            }
        } else {
            let limit = best.map_or(max_t, |h| h.metric.min(max_t));
            let left = *mesh.bvh_nodes.get(bvh.left as usize)?;
            let right = *mesh.bvh_nodes.get(bvh.right as usize)?;
            let lt = ray_aabb_tmin(
                ray_origin_local,
                ray_dir_local,
                left.aabb_min,
                left.aabb_max,
                limit,
            )
            .unwrap_or(f32::INFINITY);
            let rt = ray_aabb_tmin(
                ray_origin_local,
                ray_dir_local,
                right.aabb_min,
                right.aabb_max,
                limit,
            )
            .unwrap_or(f32::INFINITY);
            if lt < rt {
                stack.push(bvh.right);
                stack.push(bvh.left);
            } else {
                stack.push(bvh.left);
                stack.push(bvh.right);
            }
        }
    }
    Some(best)
}

impl QueryRegionAcc {
    pub(super) fn empty() -> Self {
        Self {
            tri_count: 0,
            sum_local: Vec3::ZERO,
            sum_global: Vec3::ZERO,
            local_min: Vec3::splat(f32::INFINITY),
            local_max: Vec3::splat(f32::NEG_INFINITY),
            global_min: Vec3::splat(f32::INFINITY),
            global_max: Vec3::splat(f32::NEG_INFINITY),
        }
    }
}

pub(super) fn nearer_hit(
    a: Option<QueryHitCandidate>,
    b: Option<QueryHitCandidate>,
) -> Option<QueryHitCandidate> {
    match (a, b) {
        (Some(left), Some(right)) => {
            if right.metric < left.metric {
                Some(right)
            } else {
                Some(left)
            }
        }
        (Some(hit), None) | (None, Some(hit)) => Some(hit),
        (None, None) => None,
    }
}

pub(super) fn merge_region_acc(a: QueryRegionAcc, b: QueryRegionAcc) -> QueryRegionAcc {
    if a.tri_count == 0 {
        return b;
    }
    if b.tri_count == 0 {
        return a;
    }
    QueryRegionAcc {
        tri_count: a.tri_count.saturating_add(b.tri_count),
        sum_local: a.sum_local + b.sum_local,
        sum_global: a.sum_global + b.sum_global,
        local_min: a.local_min.min(b.local_min),
        local_max: a.local_max.max(b.local_max),
        global_min: a.global_min.min(b.global_min),
        global_max: a.global_max.max(b.global_max),
    }
}

#[inline]
pub(super) fn should_parallel_instances(instance_count: usize, tri_count: usize) -> bool {
    instance_count >= QUERY_INSTANCE_PAR_THRESHOLD
        && tri_count >= QUERY_TRI_PAR_THRESHOLD
        && instance_count.saturating_mul(tri_count) >= QUERY_PAR_WORK_THRESHOLD
}

#[inline]
pub(super) fn should_parallel_triangles(instance_parallel: bool, tri_count: usize) -> bool {
    !instance_parallel && tri_count >= QUERY_TRI_PAR_THRESHOLD
}

#[inline]
pub(super) fn should_parallel_regions(
    instance_count: usize,
    tri_count: usize,
    surface_count: usize,
) -> bool {
    if instance_count >= QUERY_INSTANCE_PAR_THRESHOLD && tri_count >= QUERY_TRI_PAR_THRESHOLD {
        return true;
    }
    let surface_gate = surface_count >= QUERY_REGION_SURFACE_PAR_THRESHOLD;
    surface_gate
        && tri_count >= QUERY_TRI_PAR_THRESHOLD
        && instance_count
            .saturating_mul(tri_count)
            .saturating_mul(surface_count)
            >= QUERY_PAR_WORK_THRESHOLD
}

#[inline]
pub(super) fn aabb_distance2(p: Vec3, min: Vec3, max: Vec3) -> f32 {
    simd::aabb_distance2(p, min, max)
}

#[inline]
pub(super) fn ray_aabb_tmin(
    origin: Vec3,
    dir: Vec3,
    min: Vec3,
    max: Vec3,
    max_t: f32,
) -> Option<f32> {
    simd::ray_aabb_tmin(origin, dir, min, max, max_t)
}

pub(super) fn build_query_mesh_data(
    vertices: Vec<Vec3>,
    triangles: Vec<QueryTri>,
) -> Option<QueryMeshData> {
    if vertices.is_empty() || triangles.is_empty() {
        return None;
    }
    let mut tri_accel = Vec::with_capacity(triangles.len());
    for tri in &triangles {
        let a = *vertices.get(tri.a as usize)?;
        let b = *vertices.get(tri.b as usize)?;
        let c = *vertices.get(tri.c as usize)?;
        let normal = (b - a).cross(c - a).normalize_or_zero();
        let aabb_min = a.min(b).min(c);
        let aabb_max = a.max(b).max(c);
        tri_accel.push(QueryTriAccel {
            normal,
            aabb_min,
            aabb_max,
            centroid: (a + b + c) * (1.0 / 3.0),
        });
    }
    let mut bvh_tri_indices: Vec<u32> = (0..triangles.len() as u32).collect();
    let mut bvh_nodes = Vec::new();
    build_bvh_recursive(
        &tri_accel,
        &mut bvh_tri_indices,
        &mut bvh_nodes,
        0,
        triangles.len(),
    );
    Some(QueryMeshData {
        vertices,
        triangles,
        tri_accel,
        bvh_nodes,
        bvh_tri_indices,
    })
}

pub(super) fn build_bvh_recursive(
    tri_accel: &[QueryTriAccel],
    tri_indices: &mut [u32],
    nodes: &mut Vec<QueryBvhNode>,
    start: usize,
    count: usize,
) -> u32 {
    let node_index = nodes.len() as u32;
    nodes.push(QueryBvhNode {
        aabb_min: Vec3::splat(f32::INFINITY),
        aabb_max: Vec3::splat(f32::NEG_INFINITY),
        left: u32::MAX,
        right: u32::MAX,
        tri_start: start as u32,
        tri_count: count as u32,
    });
    let mut node_min = Vec3::splat(f32::INFINITY);
    let mut node_max = Vec3::splat(f32::NEG_INFINITY);
    let mut cmin = Vec3::splat(f32::INFINITY);
    let mut cmax = Vec3::splat(f32::NEG_INFINITY);
    for &idx in &tri_indices[start..start + count] {
        let acc = tri_accel[idx as usize];
        node_min = node_min.min(acc.aabb_min);
        node_max = node_max.max(acc.aabb_max);
        cmin = cmin.min(acc.centroid);
        cmax = cmax.max(acc.centroid);
    }
    if count <= 12 {
        nodes[node_index as usize].aabb_min = node_min;
        nodes[node_index as usize].aabb_max = node_max;
        return node_index;
    }
    let extent = cmax - cmin;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    tri_indices[start..start + count].sort_unstable_by(|a, b| {
        let ca = tri_accel[*a as usize].centroid;
        let cb = tri_accel[*b as usize].centroid;
        let va = if axis == 0 {
            ca.x
        } else if axis == 1 {
            ca.y
        } else {
            ca.z
        };
        let vb = if axis == 0 {
            cb.x
        } else if axis == 1 {
            cb.y
        } else {
            cb.z
        };
        va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
    });
    let left_count = count / 2;
    let right_count = count - left_count;
    let left = build_bvh_recursive(tri_accel, tri_indices, nodes, start, left_count);
    let right = build_bvh_recursive(
        tri_accel,
        tri_indices,
        nodes,
        start + left_count,
        right_count,
    );
    nodes[node_index as usize] = QueryBvhNode {
        aabb_min: node_min,
        aabb_max: node_max,
        left,
        right,
        tri_start: start as u32,
        tri_count: count as u32,
    };
    node_index
}

use super::*;

pub(super) fn mesh_query_cache() -> &'static RwLock<QueryMeshCache> {
    static CACHE: OnceLock<RwLock<QueryMeshCache>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(AHashMap::default()))
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
    let dx = if p.x < min.x {
        min.x - p.x
    } else if p.x > max.x {
        p.x - max.x
    } else {
        0.0
    };
    let dy = if p.y < min.y {
        min.y - p.y
    } else if p.y > max.y {
        p.y - max.y
    } else {
        0.0
    };
    let dz = if p.z < min.z {
        min.z - p.z
    } else if p.z > max.z {
        p.z - max.z
    } else {
        0.0
    };
    dx * dx + dy * dy + dz * dz
}

#[inline]
pub(super) fn ray_aabb_tmin(
    origin: Vec3,
    dir: Vec3,
    min: Vec3,
    max: Vec3,
    max_t: f32,
) -> Option<f32> {
    let inv_x = if dir.x.abs() > 1e-8 {
        1.0 / dir.x
    } else {
        f32::INFINITY
    };
    let inv_y = if dir.y.abs() > 1e-8 {
        1.0 / dir.y
    } else {
        f32::INFINITY
    };
    let inv_z = if dir.z.abs() > 1e-8 {
        1.0 / dir.z
    } else {
        f32::INFINITY
    };

    let mut t1 = (min.x - origin.x) * inv_x;
    let mut t2 = (max.x - origin.x) * inv_x;
    if t1 > t2 {
        std::mem::swap(&mut t1, &mut t2);
    }
    let mut tmin = t1;
    let mut tmax = t2;

    t1 = (min.y - origin.y) * inv_y;
    t2 = (max.y - origin.y) * inv_y;
    if t1 > t2 {
        std::mem::swap(&mut t1, &mut t2);
    }
    tmin = tmin.max(t1);
    tmax = tmax.min(t2);

    t1 = (min.z - origin.z) * inv_z;
    t2 = (max.z - origin.z) * inv_z;
    if t1 > t2 {
        std::mem::swap(&mut t1, &mut t2);
    }
    tmin = tmin.max(t1);
    tmax = tmax.min(t2);

    if tmax < 0.0 || tmin > tmax || tmin > max_t {
        None
    } else {
        Some(tmin.max(0.0))
    }
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

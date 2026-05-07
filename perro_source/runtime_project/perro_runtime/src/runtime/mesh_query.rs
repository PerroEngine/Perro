use super::Runtime;
use ahash::AHashMap;
use glam::{Mat3, Mat4, Vec3};
use perro_ids::{MaterialID, NodeID, parse_hashed_source_uri, string_to_u64};
use perro_io::decompress_zlib;
use perro_nodes::{MeshSurfaceBinding, SceneNodeData};
use perro_runtime_context::sub_apis::{MeshMaterialRegion3D, MeshSurfaceHit3D};
use perro_structs::Vector3;
use rayon::prelude::*;
use std::cell::RefCell;
use std::sync::{Arc, OnceLock, RwLock};

#[derive(Clone, Copy)]
struct QueryTri {
    a: u32,
    b: u32,
    c: u32,
    surface_index: u32,
}

struct QueryMeshData {
    vertices: Vec<Vec3>,
    triangles: Vec<QueryTri>,
    tri_accel: Vec<QueryTriAccel>,
    bvh_nodes: Vec<QueryBvhNode>,
    bvh_tri_indices: Vec<u32>,
}

#[derive(Clone, Copy)]
struct QueryTriAccel {
    normal: Vec3,
    aabb_min: Vec3,
    aabb_max: Vec3,
    centroid: Vec3,
}

#[derive(Clone, Copy)]
struct QueryBvhNode {
    aabb_min: Vec3,
    aabb_max: Vec3,
    left: u32,
    right: u32,
    tri_start: u32,
    tri_count: u32,
}

const QUERY_TRI_PAR_THRESHOLD: usize = 4096;
const QUERY_INSTANCE_PAR_THRESHOLD: usize = 8;
const QUERY_REGION_SURFACE_PAR_THRESHOLD: usize = 8;
const QUERY_PAR_WORK_THRESHOLD: usize = 32768;

type QueryMeshCache = AHashMap<u64, Arc<QueryMeshData>>;

fn mesh_query_cache() -> &'static RwLock<QueryMeshCache> {
    static CACHE: OnceLock<RwLock<QueryMeshCache>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(AHashMap::default()))
}

thread_local! {
    static GLTF_POS_SCRATCH: RefCell<Vec<[f32; 3]>> = const { RefCell::new(Vec::new()) };
    static GLTF_INDEX_SCRATCH: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

struct QueryNodeData {
    source: String,
    surfaces: Vec<MeshSurfaceBinding>,
    instance_local: Vec<Mat4>,
}

#[derive(Clone, Copy)]
struct QueryHitCandidate {
    instance_index: u32,
    surface_index: u32,
    world_point: Vec3,
    local_point: Vec3,
    world_normal: Vec3,
    local_normal: Vec3,
    metric: f32,
}

#[derive(Clone, Copy)]
struct QueryRegionAcc {
    tri_count: u32,
    sum_local: Vec3,
    sum_world: Vec3,
    local_min: Vec3,
    local_max: Vec3,
    world_min: Vec3,
    world_max: Vec3,
}

impl QueryRegionAcc {
    fn empty() -> Self {
        Self {
            tri_count: 0,
            sum_local: Vec3::ZERO,
            sum_world: Vec3::ZERO,
            local_min: Vec3::splat(f32::INFINITY),
            local_max: Vec3::splat(f32::NEG_INFINITY),
            world_min: Vec3::splat(f32::INFINITY),
            world_max: Vec3::splat(f32::NEG_INFINITY),
        }
    }
}

fn nearer_hit(
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

fn merge_region_acc(a: QueryRegionAcc, b: QueryRegionAcc) -> QueryRegionAcc {
    if a.tri_count == 0 {
        return b;
    }
    if b.tri_count == 0 {
        return a;
    }
    QueryRegionAcc {
        tri_count: a.tri_count.saturating_add(b.tri_count),
        sum_local: a.sum_local + b.sum_local,
        sum_world: a.sum_world + b.sum_world,
        local_min: a.local_min.min(b.local_min),
        local_max: a.local_max.max(b.local_max),
        world_min: a.world_min.min(b.world_min),
        world_max: a.world_max.max(b.world_max),
    }
}

#[inline]
fn should_parallel_instances(instance_count: usize, tri_count: usize) -> bool {
    instance_count >= QUERY_INSTANCE_PAR_THRESHOLD
        && tri_count >= QUERY_TRI_PAR_THRESHOLD
        && instance_count.saturating_mul(tri_count) >= QUERY_PAR_WORK_THRESHOLD
}

#[inline]
fn should_parallel_triangles(instance_parallel: bool, tri_count: usize) -> bool {
    !instance_parallel && tri_count >= QUERY_TRI_PAR_THRESHOLD
}

#[inline]
fn should_parallel_regions(instance_count: usize, tri_count: usize, surface_count: usize) -> bool {
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
fn aabb_distance2(p: Vec3, min: Vec3, max: Vec3) -> f32 {
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
fn ray_aabb_tmin(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3, max_t: f32) -> Option<f32> {
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

fn build_query_mesh_data(vertices: Vec<Vec3>, triangles: Vec<QueryTri>) -> Option<QueryMeshData> {
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

fn build_bvh_recursive(
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

impl Runtime {
    pub(crate) fn query_mesh_instance_surface_at_world_point(
        &mut self,
        node_id: NodeID,
        world_point: Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        self.query_mesh_surface_at_world_point_impl(node_id, world_point, true)
    }

    pub(crate) fn query_mesh_data_surface_at_world_point(
        &mut self,
        node_id: NodeID,
        world_point: Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        self.query_mesh_surface_at_world_point_impl(node_id, world_point, false)
    }

    fn query_mesh_surface_at_world_point_impl(
        &mut self,
        node_id: NodeID,
        world_point: Vector3,
        resolve_material: bool,
    ) -> Option<MeshSurfaceHit3D> {
        let node = self.query_node_mesh_data(node_id)?;
        let mesh = self.load_query_mesh_data(node.source.as_str())?;
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return None;
        }

        let node_world = self.get_global_transform_3d(node_id)?.to_mat4();
        let p_world: Vec3 = world_point.into();
        let instance_parallel =
            should_parallel_instances(node.instance_local.len(), mesh.triangles.len());
        let best_for_instance = |(instance_index, local): (usize, &Mat4)| {
            let world_from_mesh = node_world * *local;
            let mesh_from_world = world_from_mesh.inverse();
            let world_normal_basis = Mat3::from_mat4(world_from_mesh).inverse().transpose();
            let p_local = mesh_from_world.transform_point3(p_world);
            let mut best: Option<QueryHitCandidate> = None;
            let mut stack = vec![0_u32];
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
                        let tri = *mesh.triangles.get(tri_idx as usize)?;
                        let acc = *mesh.tri_accel.get(tri_idx as usize)?;
                        let tri_d2 = aabb_distance2(p_local, acc.aabb_min, acc.aabb_max);
                        if let Some(hit) = best
                            && tri_d2 >= hit.metric
                        {
                            continue;
                        }
                        let a = mesh.vertices[tri.a as usize];
                        let b = mesh.vertices[tri.b as usize];
                        let c = mesh.vertices[tri.c as usize];
                        let nearest_local = closest_point_on_triangle(p_local, a, b, c);
                        let d2 = nearest_local.distance_squared(p_local);
                        if let Some(hit) = best
                            && d2 >= hit.metric
                        {
                            continue;
                        }
                        let nearest_world = world_from_mesh.transform_point3(nearest_local);
                        let world_normal = (world_normal_basis * acc.normal).normalize_or_zero();
                        best = Some(QueryHitCandidate {
                            instance_index: instance_index as u32,
                            surface_index: tri.surface_index,
                            world_point: nearest_world,
                            local_point: nearest_local,
                            world_normal,
                            local_normal: acc.normal,
                            metric: d2,
                        });
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
            best
        };
        let best = if instance_parallel {
            node.instance_local
                .par_iter()
                .enumerate()
                .map(best_for_instance)
                .reduce(|| None, nearer_hit)
        } else {
            node.instance_local
                .iter()
                .enumerate()
                .map(best_for_instance)
                .fold(None, nearer_hit)
        }?;
        let material = if resolve_material {
            self.query_surface_material(node_id, &node, best.surface_index)
        } else {
            None
        };
        Some(MeshSurfaceHit3D {
            instance_index: best.instance_index,
            surface_index: best.surface_index,
            material,
            world_point: best.world_point.into(),
            local_point: best.local_point.into(),
            world_normal: best.world_normal.into(),
            local_normal: best.local_normal.into(),
            distance: best.metric.sqrt(),
        })
    }

    pub(crate) fn query_mesh_instance_surface_on_world_ray(
        &mut self,
        node_id: NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
    ) -> Option<MeshSurfaceHit3D> {
        self.query_mesh_surface_on_world_ray_impl(
            node_id,
            ray_origin,
            ray_direction,
            max_distance,
            true,
        )
    }

    pub(crate) fn query_mesh_data_surface_on_world_ray(
        &mut self,
        node_id: NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
    ) -> Option<MeshSurfaceHit3D> {
        self.query_mesh_surface_on_world_ray_impl(
            node_id,
            ray_origin,
            ray_direction,
            max_distance,
            false,
        )
    }

    fn query_mesh_surface_on_world_ray_impl(
        &mut self,
        node_id: NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
        resolve_material: bool,
    ) -> Option<MeshSurfaceHit3D> {
        let node = self.query_node_mesh_data(node_id)?;
        let mesh = self.load_query_mesh_data(node.source.as_str())?;
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return None;
        }

        let ray_origin_world: Vec3 = ray_origin.into();
        let ray_dir_world_raw: Vec3 = ray_direction.into();
        let ray_dir_len = ray_dir_world_raw.length();
        if ray_dir_len <= 0.000001 {
            return None;
        }
        let ray_dir_world = ray_dir_world_raw / ray_dir_len;
        let max_t = if max_distance.is_finite() && max_distance > 0.0 {
            max_distance
        } else {
            f32::INFINITY
        };

        let node_world = self.get_global_transform_3d(node_id)?.to_mat4();
        let instance_parallel =
            should_parallel_instances(node.instance_local.len(), mesh.triangles.len());
        let best_for_instance = |(instance_index, local): (usize, &Mat4)| {
            let world_from_mesh = node_world * *local;
            let mesh_from_world = world_from_mesh.inverse();
            let world_normal_basis = Mat3::from_mat4(world_from_mesh).inverse().transpose();
            let ray_origin_local = mesh_from_world.transform_point3(ray_origin_world);
            let ray_dir_local = mesh_from_world
                .transform_vector3(ray_dir_world)
                .normalize_or_zero();
            if ray_dir_local.length_squared() <= 1e-10 {
                return None;
            }
            let mut best: Option<QueryHitCandidate> = None;
            let mut stack = vec![0_u32];
            while let Some(node_idx) = stack.pop() {
                let bvh = *mesh.bvh_nodes.get(node_idx as usize)?;
                if ray_aabb_tmin(
                    ray_origin_local,
                    ray_dir_local,
                    bvh.aabb_min,
                    bvh.aabb_max,
                    best.map_or(max_t, |h| h.metric.min(max_t)),
                )
                .is_none()
                {
                    continue;
                }
                if bvh.left == u32::MAX || bvh.right == u32::MAX {
                    let start = bvh.tri_start as usize;
                    let end = start + bvh.tri_count as usize;
                    for &tri_idx in &mesh.bvh_tri_indices[start..end] {
                        let tri = *mesh.triangles.get(tri_idx as usize)?;
                        let acc = *mesh.tri_accel.get(tri_idx as usize)?;
                        if ray_aabb_tmin(
                            ray_origin_local,
                            ray_dir_local,
                            acc.aabb_min,
                            acc.aabb_max,
                            best.map_or(max_t, |h| h.metric.min(max_t)),
                        )
                        .is_none()
                        {
                            continue;
                        }
                        let a = mesh.vertices[tri.a as usize];
                        let b = mesh.vertices[tri.b as usize];
                        let c = mesh.vertices[tri.c as usize];
                        let t = ray_intersect_triangle(ray_origin_local, ray_dir_local, a, b, c)?;
                        if t > max_t {
                            continue;
                        }
                        if let Some(hit) = best
                            && t >= hit.metric
                        {
                            continue;
                        }
                        let hit_local = ray_origin_local + ray_dir_local * t;
                        let hit_world = world_from_mesh.transform_point3(hit_local);
                        let world_t = (hit_world - ray_origin_world).length();
                        if world_t > max_t {
                            continue;
                        }
                        let world_normal = (world_normal_basis * acc.normal).normalize_or_zero();
                        best = Some(QueryHitCandidate {
                            instance_index: instance_index as u32,
                            surface_index: tri.surface_index,
                            world_point: hit_world,
                            local_point: hit_local,
                            world_normal,
                            local_normal: acc.normal,
                            metric: world_t,
                        });
                    }
                } else {
                    let left = *mesh.bvh_nodes.get(bvh.left as usize)?;
                    let right = *mesh.bvh_nodes.get(bvh.right as usize)?;
                    let lt = ray_aabb_tmin(
                        ray_origin_local,
                        ray_dir_local,
                        left.aabb_min,
                        left.aabb_max,
                        best.map_or(max_t, |h| h.metric.min(max_t)),
                    )
                    .unwrap_or(f32::INFINITY);
                    let rt = ray_aabb_tmin(
                        ray_origin_local,
                        ray_dir_local,
                        right.aabb_min,
                        right.aabb_max,
                        best.map_or(max_t, |h| h.metric.min(max_t)),
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
            best
        };
        let best = if instance_parallel {
            node.instance_local
                .par_iter()
                .enumerate()
                .map(best_for_instance)
                .reduce(|| None, nearer_hit)
        } else {
            node.instance_local
                .iter()
                .enumerate()
                .map(best_for_instance)
                .fold(None, nearer_hit)
        }?;
        let material = if resolve_material {
            self.query_surface_material(node_id, &node, best.surface_index)
        } else {
            None
        };
        Some(MeshSurfaceHit3D {
            instance_index: best.instance_index,
            surface_index: best.surface_index,
            material,
            world_point: best.world_point.into(),
            local_point: best.local_point.into(),
            world_normal: best.world_normal.into(),
            local_normal: best.local_normal.into(),
            distance: best.metric,
        })
    }

    pub(crate) fn query_mesh_instance_material_regions(
        &mut self,
        node_id: NodeID,
        material: MaterialID,
    ) -> Vec<MeshMaterialRegion3D> {
        let Some(node) = self.query_node_mesh_data(node_id) else {
            return Vec::new();
        };
        let Some(mesh) = self.load_query_mesh_data(node.source.as_str()) else {
            return Vec::new();
        };
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return Vec::new();
        }

        let node_world = match self.get_global_transform_3d(node_id) {
            Some(transform) => transform.to_mat4(),
            None => return Vec::new(),
        };

        let vertices = &mesh.vertices;
        let triangles = &mesh.triangles;

        let instance_parallel = should_parallel_regions(
            node.instance_local.len(),
            triangles.len(),
            node.surfaces.len(),
        );
        let tri_parallel = should_parallel_triangles(instance_parallel, triangles.len());
        let regions_for_instance = |(instance_index, local): (usize, &Mat4)| {
            let world_from_mesh = node_world * *local;
            node.surfaces
                .iter()
                .enumerate()
                .filter(move |(_, surface)| surface.material == Some(material))
                .filter_map(move |(surface_index, surface)| {
                    let tri_map = |tri: QueryTri| {
                        let Some(a) = vertices.get(tri.a as usize).copied() else {
                            return QueryRegionAcc::empty();
                        };
                        let Some(b) = vertices.get(tri.b as usize).copied() else {
                            return QueryRegionAcc::empty();
                        };
                        let Some(c) = vertices.get(tri.c as usize).copied() else {
                            return QueryRegionAcc::empty();
                        };

                        let tri_local_center = (a + b + c) / 3.0;
                        let tri_world_center = world_from_mesh.transform_point3(tri_local_center);
                        let mut local_min = Vec3::splat(f32::INFINITY);
                        let mut local_max = Vec3::splat(f32::NEG_INFINITY);
                        let mut world_min = Vec3::splat(f32::INFINITY);
                        let mut world_max = Vec3::splat(f32::NEG_INFINITY);

                        for p in [a, b, c] {
                            local_min = local_min.min(p);
                            local_max = local_max.max(p);
                            let pw = world_from_mesh.transform_point3(p);
                            world_min = world_min.min(pw);
                            world_max = world_max.max(pw);
                        }

                        QueryRegionAcc {
                            tri_count: 1,
                            sum_local: tri_local_center,
                            sum_world: tri_world_center,
                            local_min,
                            local_max,
                            world_min,
                            world_max,
                        }
                    };
                    let acc = if tri_parallel {
                        triangles
                            .par_iter()
                            .copied()
                            .filter(|tri| tri.surface_index as usize == surface_index)
                            .map(tri_map)
                            .reduce(QueryRegionAcc::empty, merge_region_acc)
                    } else {
                        triangles
                            .iter()
                            .copied()
                            .filter(|tri| tri.surface_index as usize == surface_index)
                            .map(tri_map)
                            .fold(QueryRegionAcc::empty(), merge_region_acc)
                    };

                    if acc.tri_count == 0 {
                        return None;
                    }
                    let inv = 1.0 / acc.tri_count as f32;
                    Some(MeshMaterialRegion3D {
                        instance_index: instance_index as u32,
                        surface_index: surface_index as u32,
                        material: surface.material,
                        triangle_count: acc.tri_count,
                        center_world: (acc.sum_world * inv).into(),
                        center_local: (acc.sum_local * inv).into(),
                        aabb_min_world: acc.world_min.into(),
                        aabb_max_world: acc.world_max.into(),
                        aabb_min_local: acc.local_min.into(),
                        aabb_max_local: acc.local_max.into(),
                    })
                })
                .collect::<Vec<_>>()
        };
        if instance_parallel {
            node.instance_local
                .par_iter()
                .enumerate()
                .flat_map_iter(regions_for_instance)
                .collect()
        } else {
            node.instance_local
                .iter()
                .enumerate()
                .flat_map(regions_for_instance)
                .collect()
        }
    }

    pub(crate) fn query_mesh_data_surface_regions(
        &mut self,
        node_id: NodeID,
        surface_index: u32,
    ) -> Vec<MeshMaterialRegion3D> {
        let Some(node) = self.query_node_mesh_data(node_id) else {
            return Vec::new();
        };
        let Some(mesh) = self.load_query_mesh_data(node.source.as_str()) else {
            return Vec::new();
        };
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return Vec::new();
        }

        let node_world = match self.get_global_transform_3d(node_id) {
            Some(transform) => transform.to_mat4(),
            None => return Vec::new(),
        };

        let vertices = &mesh.vertices;
        let triangles = &mesh.triangles;

        let instance_parallel =
            should_parallel_instances(node.instance_local.len(), triangles.len());
        let tri_parallel = should_parallel_triangles(instance_parallel, triangles.len());
        let region_for_instance = |(instance_index, local): (usize, &Mat4)| {
            let world_from_mesh = node_world * *local;
            let tri_map = |tri: QueryTri| {
                let Some(a) = vertices.get(tri.a as usize).copied() else {
                    return QueryRegionAcc::empty();
                };
                let Some(b) = vertices.get(tri.b as usize).copied() else {
                    return QueryRegionAcc::empty();
                };
                let Some(c) = vertices.get(tri.c as usize).copied() else {
                    return QueryRegionAcc::empty();
                };

                let tri_local_center = (a + b + c) / 3.0;
                let tri_world_center = world_from_mesh.transform_point3(tri_local_center);
                let mut local_min = Vec3::splat(f32::INFINITY);
                let mut local_max = Vec3::splat(f32::NEG_INFINITY);
                let mut world_min = Vec3::splat(f32::INFINITY);
                let mut world_max = Vec3::splat(f32::NEG_INFINITY);

                for p in [a, b, c] {
                    local_min = local_min.min(p);
                    local_max = local_max.max(p);
                    let pw = world_from_mesh.transform_point3(p);
                    world_min = world_min.min(pw);
                    world_max = world_max.max(pw);
                }

                QueryRegionAcc {
                    tri_count: 1,
                    sum_local: tri_local_center,
                    sum_world: tri_world_center,
                    local_min,
                    local_max,
                    world_min,
                    world_max,
                }
            };
            let acc = if tri_parallel {
                triangles
                    .par_iter()
                    .copied()
                    .filter(|tri| tri.surface_index == surface_index)
                    .map(tri_map)
                    .reduce(QueryRegionAcc::empty, merge_region_acc)
            } else {
                triangles
                    .iter()
                    .copied()
                    .filter(|tri| tri.surface_index == surface_index)
                    .map(tri_map)
                    .fold(QueryRegionAcc::empty(), merge_region_acc)
            };

            if acc.tri_count == 0 {
                return None;
            }
            let inv = 1.0 / acc.tri_count as f32;
            Some(MeshMaterialRegion3D {
                instance_index: instance_index as u32,
                surface_index,
                material: None,
                triangle_count: acc.tri_count,
                center_world: (acc.sum_world * inv).into(),
                center_local: (acc.sum_local * inv).into(),
                aabb_min_world: acc.world_min.into(),
                aabb_max_world: acc.world_max.into(),
                aabb_min_local: acc.local_min.into(),
                aabb_max_local: acc.local_max.into(),
            })
        };
        if instance_parallel {
            node.instance_local
                .par_iter()
                .enumerate()
                .filter_map(region_for_instance)
                .collect()
        } else {
            node.instance_local
                .iter()
                .enumerate()
                .filter_map(region_for_instance)
                .collect()
        }
    }

    fn query_node_mesh_data(&self, node_id: NodeID) -> Option<QueryNodeData> {
        let node = self.nodes.get(node_id)?;
        let source = self.render_3d.mesh_sources.get(&node_id)?.clone();
        match &node.data {
            SceneNodeData::MeshInstance3D(mesh) => Some(QueryNodeData {
                source,
                surfaces: mesh.surfaces.clone(),
                instance_local: vec![Mat4::IDENTITY],
            }),
            SceneNodeData::MultiMeshInstance3D(mesh) => {
                let instance_local = if mesh.instances.is_empty() {
                    vec![Mat4::IDENTITY]
                } else {
                    mesh.instances
                        .iter()
                        .map(|instance| {
                            Mat4::from_scale_rotation_translation(
                                Vec3::splat(mesh.instance_scale.max(0.0001)),
                                glam::Quat::from_xyzw(
                                    instance.1.x,
                                    instance.1.y,
                                    instance.1.z,
                                    instance.1.w,
                                ),
                                Vec3::new(instance.0.x, instance.0.y, instance.0.z),
                            )
                        })
                        .collect()
                };
                Some(QueryNodeData {
                    source,
                    surfaces: mesh.surfaces.clone(),
                    instance_local,
                })
            }
            _ => None,
        }
    }

    fn query_surface_material(
        &self,
        node_id: NodeID,
        node: &QueryNodeData,
        surface_index: u32,
    ) -> Option<MaterialID> {
        let index = surface_index as usize;
        node.surfaces
            .get(index)
            .and_then(|surface| surface.material)
            .or_else(|| {
                self.render_3d
                    .retained_mesh_draws
                    .get(&node_id)
                    .and_then(|draw| draw.surfaces.get(index))
                    .and_then(|surface| surface.material)
            })
    }

    fn load_query_mesh_data(&self, source: &str) -> Option<Arc<QueryMeshData>> {
        let source = source.trim();
        if source.is_empty() {
            return None;
        }
        let cache_key = string_to_u64(source);
        if let Ok(cache) = mesh_query_cache().read()
            && let Some(mesh) = cache.get(&cache_key)
        {
            return Some(mesh.clone());
        }
        let mut loaded = if source.starts_with("__") {
            decode_builtin_query_mesh(source)
        } else {
            None
        };

        if loaded.is_none() {
            let normalized = normalize_source_slashes(source);
            let source_hash =
                parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
            if self.provider_mode() == crate::runtime_project::ProviderMode::Static
                && let Some(lookup) = self
                    .project()
                    .and_then(|project| project.static_mesh_lookup)
            {
                let bytes = lookup(source_hash);
                if !bytes.is_empty()
                    && let Some(mesh) = decode_pmesh_query(bytes)
                {
                    loaded = Some(mesh);
                }
                if loaded.is_none() && normalized.as_ref() != source {
                    let bytes = lookup(string_to_u64(normalized.as_ref()));
                    if !bytes.is_empty()
                        && let Some(mesh) = decode_pmesh_query(bytes)
                    {
                        loaded = Some(mesh);
                    }
                }
                if loaded.is_none()
                    && let Some(alias) = normalized_static_mesh_lookup_alias(source)
                {
                    let bytes = lookup(string_to_u64(alias.as_str()));
                    if !bytes.is_empty()
                        && let Some(mesh) = decode_pmesh_query(bytes)
                    {
                        loaded = Some(mesh);
                    }
                }
                if loaded.is_none()
                    && normalized.as_ref() != source
                    && let Some(alias) = normalized_static_mesh_lookup_alias(normalized.as_ref())
                {
                    let bytes = lookup(string_to_u64(alias.as_str()));
                    if !bytes.is_empty()
                        && let Some(mesh) = decode_pmesh_query(bytes)
                    {
                        loaded = Some(mesh);
                    }
                }
            }

            if loaded.is_none() {
                let (path, fragment) = split_source_fragment(source);
                let mesh_index = parse_fragment_index(fragment, "mesh").unwrap_or(0);
                let bytes = perro_io::load_asset(path).ok()?;
                if path.ends_with(".glb") || path.ends_with(".gltf") {
                    loaded = decode_gltf_query_mesh(&bytes, mesh_index);
                } else if path.ends_with(".pmesh") {
                    loaded = decode_pmesh_query(&bytes);
                }
            }
        }

        let mesh = Arc::new(loaded?);
        if let Ok(mut cache) = mesh_query_cache().write() {
            cache.insert(cache_key, mesh.clone());
        }
        Some(mesh)
    }
}

fn decode_builtin_query_mesh(source: &str) -> Option<QueryMeshData> {
    let (verts, tris) = match source {
        "__cube__" => builtin_cube_mesh(),
        "__tri_pyr__" => builtin_tri_pyramid_mesh(),
        "__sq_pyr__" => builtin_square_pyramid_mesh(),
        "__sphere__" => builtin_octa_sphere_mesh(),
        "__tri_prism__" => builtin_tri_prism_mesh(),
        "__cylinder__" => builtin_cylinder_mesh(12),
        "__cone__" => builtin_cone_mesh(12),
        "__capsule__" => builtin_capsule_mesh(12),
        _ => return None,
    };
    let triangles = tris
        .into_iter()
        .map(|[a, b, c]| QueryTri {
            a,
            b,
            c,
            surface_index: 0,
        })
        .collect();
    build_query_mesh_data(verts, triangles)
}

fn builtin_cube_mesh() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let h = 0.5_f32;
    let v = vec![
        Vec3::new(-h, -h, -h),
        Vec3::new(h, -h, -h),
        Vec3::new(h, h, -h),
        Vec3::new(-h, h, -h),
        Vec3::new(-h, -h, h),
        Vec3::new(h, -h, h),
        Vec3::new(h, h, h),
        Vec3::new(-h, h, h),
    ];
    let t = vec![
        [0, 1, 2],
        [0, 2, 3],
        [4, 6, 5],
        [4, 7, 6],
        [0, 4, 5],
        [0, 5, 1],
        [1, 5, 6],
        [1, 6, 2],
        [2, 6, 7],
        [2, 7, 3],
        [3, 7, 4],
        [3, 4, 0],
    ];
    (v, t)
}

fn builtin_tri_pyramid_mesh() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let h = 0.5_f32;
    let v = vec![
        Vec3::new(-h, -h, -h),
        Vec3::new(h, -h, -h),
        Vec3::new(0.0, -h, h),
        Vec3::new(0.0, h, 0.0),
    ];
    let t = vec![[0, 1, 2], [0, 2, 3], [2, 1, 3], [1, 0, 3]];
    (v, t)
}

fn builtin_square_pyramid_mesh() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let h = 0.5_f32;
    let v = vec![
        Vec3::new(-h, -h, -h),
        Vec3::new(h, -h, -h),
        Vec3::new(h, -h, h),
        Vec3::new(-h, -h, h),
        Vec3::new(0.0, h, 0.0),
    ];
    let t = vec![
        [0, 1, 2],
        [0, 2, 3],
        [0, 4, 1],
        [1, 4, 2],
        [2, 4, 3],
        [3, 4, 0],
    ];
    (v, t)
}

fn builtin_octa_sphere_mesh() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let h = 0.5_f32;
    let v = vec![
        Vec3::new(0.0, h, 0.0),
        Vec3::new(0.0, -h, 0.0),
        Vec3::new(h, 0.0, 0.0),
        Vec3::new(-h, 0.0, 0.0),
        Vec3::new(0.0, 0.0, h),
        Vec3::new(0.0, 0.0, -h),
    ];
    let t = vec![
        [0, 2, 4],
        [0, 4, 3],
        [0, 3, 5],
        [0, 5, 2],
        [1, 4, 2],
        [1, 3, 4],
        [1, 5, 3],
        [1, 2, 5],
    ];
    (v, t)
}

fn builtin_tri_prism_mesh() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let h = 0.5_f32;
    let v = vec![
        Vec3::new(-h, -h, -h),
        Vec3::new(h, -h, -h),
        Vec3::new(0.0, h, -h),
        Vec3::new(-h, -h, h),
        Vec3::new(h, -h, h),
        Vec3::new(0.0, h, h),
    ];
    let t = vec![
        [0, 1, 2],
        [3, 5, 4],
        [0, 3, 4],
        [0, 4, 1],
        [1, 4, 5],
        [1, 5, 2],
        [2, 5, 3],
        [2, 3, 0],
    ];
    (v, t)
}

fn builtin_cylinder_mesh(segments: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let seg = segments.max(3);
    let r = 0.5_f32;
    let h = 0.5_f32;
    let mut v = Vec::with_capacity((seg * 2 + 2) as usize);
    for i in 0..seg {
        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
        v.push(Vec3::new(a.cos() * r, h, a.sin() * r));
    }
    for i in 0..seg {
        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
        v.push(Vec3::new(a.cos() * r, -h, a.sin() * r));
    }
    let top_center = v.len() as u32;
    v.push(Vec3::new(0.0, h, 0.0));
    let bot_center = v.len() as u32;
    v.push(Vec3::new(0.0, -h, 0.0));

    let mut t = Vec::new();
    for i in 0..seg {
        let n = (i + 1) % seg;
        let a = i;
        let b = n;
        let c = i + seg;
        let d = n + seg;
        t.push([a, c, b]);
        t.push([b, c, d]);
        t.push([top_center, b, a]);
        t.push([bot_center, c, d]);
    }
    (v, t)
}

fn builtin_cone_mesh(segments: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let seg = segments.max(3);
    let r = 0.5_f32;
    let h = 0.5_f32;
    let mut v = Vec::with_capacity((seg + 2) as usize);
    for i in 0..seg {
        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
        v.push(Vec3::new(a.cos() * r, -h, a.sin() * r));
    }
    let apex = v.len() as u32;
    v.push(Vec3::new(0.0, h, 0.0));
    let base_center = v.len() as u32;
    v.push(Vec3::new(0.0, -h, 0.0));

    let mut t = Vec::new();
    for i in 0..seg {
        let n = (i + 1) % seg;
        t.push([i, n, apex]);
        t.push([base_center, n, i]);
    }
    (v, t)
}

fn builtin_capsule_mesh(segments: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    // Cheap query proxy: use cylinder hull footprint for location/material lookup.
    builtin_cylinder_mesh(segments)
}

fn decode_gltf_query_mesh(bytes: &[u8], mesh_index: usize) -> Option<QueryMeshData> {
    let (doc, buffers, _images) = gltf::import_slice(bytes).ok()?;
    let mesh = doc.meshes().nth(mesh_index)?;

    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for (surface_index, primitive) in mesh.primitives().enumerate() {
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|d| d.0.as_slice()));
        let positions = reader.read_positions()?;
        let mut local_positions =
            GLTF_POS_SCRATCH.with(|scratch| std::mem::take(&mut *scratch.borrow_mut()));
        local_positions.clear();
        local_positions.extend(positions);
        if local_positions.len() < 3 {
            GLTF_POS_SCRATCH.with(|scratch| {
                local_positions.clear();
                *scratch.borrow_mut() = local_positions;
            });
            continue;
        }

        let base = vertices.len() as u32;
        for p in local_positions.iter().copied() {
            vertices.push(Vec3::new(p[0], p[1], p[2]));
        }

        if let Some(indices_reader) = reader.read_indices() {
            let mut flat =
                GLTF_INDEX_SCRATCH.with(|scratch| std::mem::take(&mut *scratch.borrow_mut()));
            flat.clear();
            flat.extend(indices_reader.into_u32());
            for tri in flat.chunks_exact(3) {
                let ia = tri[0] as usize;
                let ib = tri[1] as usize;
                let ic = tri[2] as usize;
                if ia >= local_positions.len()
                    || ib >= local_positions.len()
                    || ic >= local_positions.len()
                    || ia == ib
                    || ib == ic
                    || ia == ic
                {
                    continue;
                }
                triangles.push(QueryTri {
                    a: base + tri[0],
                    b: base + tri[1],
                    c: base + tri[2],
                    surface_index: surface_index as u32,
                });
            }
            GLTF_INDEX_SCRATCH.with(|scratch| {
                flat.clear();
                *scratch.borrow_mut() = flat;
            });
        } else {
            let tri_count = local_positions.len() / 3;
            for i in 0..tri_count {
                let idx = (i * 3) as u32;
                triangles.push(QueryTri {
                    a: base + idx,
                    b: base + idx + 1,
                    c: base + idx + 2,
                    surface_index: surface_index as u32,
                });
            }
        }
        GLTF_POS_SCRATCH.with(|scratch| {
            local_positions.clear();
            *scratch.borrow_mut() = local_positions;
        });
    }

    build_query_mesh_data(vertices, triangles)
}

fn split_source_fragment(source: &str) -> (&str, Option<&str>) {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return (source, None);
    };
    if path.is_empty() || selector.contains('/') || selector.contains('\\') {
        return (source, None);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return (path, Some(selector));
    }
    (source, None)
}

fn parse_fragment_index(fragment: Option<&str>, key: &str) -> Option<usize> {
    let fragment = fragment?;
    let (name, rest) = fragment.split_once('[')?;
    if name.trim() != key {
        return None;
    }
    let value = rest.strip_suffix(']')?.trim();
    value.parse::<usize>().ok()
}

fn normalize_source_slashes(source: &str) -> std::borrow::Cow<'_, str> {
    if source.contains('\\') {
        std::borrow::Cow::Owned(source.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(source)
    }
}

fn normalized_static_mesh_lookup_alias(source: &str) -> Option<String> {
    let (path, fragment) = split_source_fragment(source);
    if !(path.ends_with(".glb") || path.ends_with(".gltf")) {
        return None;
    }
    match parse_fragment_index(fragment, "mesh") {
        Some(0) => Some(path.to_string()),
        Some(_) => None,
        None => Some(format!("{path}:mesh[0]")),
    }
}

fn decode_pmesh_query(bytes: &[u8]) -> Option<QueryMeshData> {
    if bytes.len() < 33 || &bytes[0..5] != b"PMESH" {
        return None;
    }
    let version = u32::from_le_bytes(bytes[5..9].try_into().ok()?);
    if version != 6 {
        return None;
    }

    let flags = u32::from_le_bytes(bytes[9..13].try_into().ok()?);
    let vertex_count = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
    let index_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let surface_count = u32::from_le_bytes(bytes[21..25].try_into().ok()?) as usize;
    let raw_len = u32::from_le_bytes(bytes[29..33].try_into().ok()?) as usize;
    let payload_start = 33usize;

    let raw = decompress_zlib(&bytes[payload_start..]).ok()?;
    if raw.len() != raw_len {
        return None;
    }

    let has_normal = (flags & (1 << 0)) != 0;
    let has_uv0 = (flags & (1 << 1)) != 0;
    let has_joints = (flags & (1 << 2)) != 0;
    let has_weights = (flags & (1 << 3)) != 0;
    let vertex_stride = 12
        + if has_normal { 12 } else { 0 }
        + if has_uv0 { 8 } else { 0 }
        + if has_joints { 8 } else { 0 }
        + if has_weights { 16 } else { 0 };

    let vertex_bytes = vertex_count.checked_mul(vertex_stride)?;
    let index_bytes = index_count.checked_mul(4)?;
    let surface_bytes = surface_count.checked_mul(8)?;
    if raw.len() < vertex_bytes + index_bytes + surface_bytes {
        return None;
    }

    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let off = i * vertex_stride;
        let x = f32::from_le_bytes(raw[off..off + 4].try_into().ok()?);
        let y = f32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?);
        let z = f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?);
        vertices.push(Vec3::new(x, y, z));
    }

    let mut indices = Vec::with_capacity(index_count);
    let index_start = vertex_bytes;
    for i in 0..index_count {
        let off = index_start + i * 4;
        indices.push(u32::from_le_bytes(raw[off..off + 4].try_into().ok()?));
    }

    let mut surface_ranges = Vec::with_capacity(surface_count);
    let surface_start = vertex_bytes + index_bytes;
    for i in 0..surface_count {
        let off = surface_start + i * 8;
        let start = u32::from_le_bytes(raw[off..off + 4].try_into().ok()?) as usize;
        let count = u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?) as usize;
        surface_ranges.push((start, count));
    }
    if surface_ranges.is_empty() {
        surface_ranges.push((0, indices.len()));
    }

    let mut triangles = Vec::new();
    for (surface_index, (start, count)) in surface_ranges.into_iter().enumerate() {
        let end = start.saturating_add(count).min(indices.len());
        let slice = &indices[start..end];
        for tri in slice.chunks_exact(3) {
            let ia = tri[0] as usize;
            let ib = tri[1] as usize;
            let ic = tri[2] as usize;
            if ia >= vertices.len()
                || ib >= vertices.len()
                || ic >= vertices.len()
                || ia == ib
                || ib == ic
                || ia == ic
            {
                continue;
            }
            triangles.push(QueryTri {
                a: tri[0],
                b: tri[1],
                c: tri[2],
                surface_index: surface_index as u32,
            });
        }
    }

    build_query_mesh_data(vertices, triangles)
}

fn ray_intersect_triangle(origin: Vec3, direction: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let ab = b - a;
    let ac = c - a;
    let pvec = direction.cross(ac);
    let det = ab.dot(pvec);
    if det.abs() <= 0.000001 {
        return None;
    }
    let inv_det = 1.0 / det;

    let tvec = origin - a;
    let u = tvec.dot(pvec) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let qvec = tvec.cross(ab);
    let v = direction.dot(qvec) * inv_det;
    if v < 0.0 || (u + v) > 1.0 {
        return None;
    }

    let t = ac.dot(qvec) * inv_det;
    if t < 0.0 {
        return None;
    }
    Some(t)
}

fn closest_point_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;

    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }

    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return a + ab * v;
    }

    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return a + ac * w;
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let bc = c - b;
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return b + bc * w;
    }

    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    a + ab * v + ac * w
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;
    use std::time::{Duration, Instant};

    fn tri_workload(tri_count: usize, surface_count: usize) -> f32 {
        let mut out = 0.0_f32;
        let p = Vec3::new(0.13, 0.21, -0.37);
        for tri in 0..tri_count {
            let s = (tri % surface_count.max(1)) as f32 * 0.0001;
            let a = Vec3::new(s, 0.1 + s, -0.2);
            let b = Vec3::new(0.4 + s, 0.3, 0.15);
            let c = Vec3::new(-0.2, 0.5 + s, 0.35);
            out += closest_point_on_triangle(p, a, b, c).length_squared();
        }
        out
    }

    fn build_synth_query_mesh(tri_count: usize, surface_count: usize) -> QueryMeshData {
        let surface_count = surface_count.max(1);
        let mut vertices = Vec::with_capacity(tri_count.saturating_mul(3));
        let mut triangles = Vec::with_capacity(tri_count);
        for tri_index in 0..tri_count {
            let base = (tri_index * 3) as u32;
            let s = (tri_index % surface_count) as f32;
            let x = tri_index as f32 * 0.0003;
            vertices.push(Vec3::new(x, s * 0.0002, 0.0));
            vertices.push(Vec3::new(x + 0.0001, 0.001 + s * 0.0001, 0.0));
            vertices.push(Vec3::new(x, 0.0002 + s * 0.0001, 0.001));
            triangles.push(QueryTri {
                a: base,
                b: base + 1,
                c: base + 2,
                surface_index: (tri_index % surface_count) as u32,
            });
        }
        build_query_mesh_data(vertices, triangles).expect("synth query mesh")
    }

    fn point_query_workload(mesh: &QueryMeshData, p_local: Vec3) -> f32 {
        if mesh.bvh_nodes.is_empty() {
            return 0.0;
        }
        let mut best_metric = f32::INFINITY;
        let mut best_surface = 0_u32;
        let mut stack = vec![0_u32];
        while let Some(node_idx) = stack.pop() {
            let Some(bvh) = mesh.bvh_nodes.get(node_idx as usize).copied() else {
                continue;
            };
            let node_d2 = aabb_distance2(p_local, bvh.aabb_min, bvh.aabb_max);
            if node_d2 >= best_metric {
                continue;
            }
            if bvh.left == u32::MAX || bvh.right == u32::MAX {
                let start = bvh.tri_start as usize;
                let end = start + bvh.tri_count as usize;
                for &tri_idx in &mesh.bvh_tri_indices[start..end] {
                    let tri_idx = tri_idx as usize;
                    let tri = mesh.triangles[tri_idx];
                    let acc = mesh.tri_accel[tri_idx];
                    let tri_d2 = aabb_distance2(p_local, acc.aabb_min, acc.aabb_max);
                    if tri_d2 >= best_metric {
                        continue;
                    }
                    let a = mesh.vertices[tri.a as usize];
                    let b = mesh.vertices[tri.b as usize];
                    let c = mesh.vertices[tri.c as usize];
                    let nearest_local = closest_point_on_triangle(p_local, a, b, c);
                    let d2 = nearest_local.distance_squared(p_local);
                    if d2 < best_metric {
                        best_metric = d2;
                        best_surface = tri.surface_index;
                    }
                }
            } else {
                let left = mesh.bvh_nodes[bvh.left as usize];
                let right = mesh.bvh_nodes[bvh.right as usize];
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
        if best_metric.is_finite() {
            best_metric + best_surface as f32 * 1e-6
        } else {
            0.0
        }
    }

    fn measure_us_per_query(mesh: &QueryMeshData) -> f64 {
        let points = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.2, 0.1, -0.1),
            Vec3::new(-0.1, 0.3, 0.2),
            Vec3::new(0.4, -0.2, 0.15),
            Vec3::new(0.05, 0.07, -0.11),
            Vec3::new(-0.33, 0.44, 0.12),
            Vec3::new(0.6, -0.1, 0.05),
            Vec3::new(-0.25, -0.15, 0.4),
        ];
        let mut iters = 64usize;
        loop {
            let start = Instant::now();
            let mut acc = 0.0_f32;
            for i in 0..iters {
                acc += point_query_workload(mesh, points[i % points.len()]);
            }
            let elapsed = start.elapsed();
            black_box(acc);
            if elapsed >= Duration::from_millis(25) || iters >= (1 << 22) {
                return elapsed.as_secs_f64() * 1_000_000.0 / iters as f64;
            }
            iters *= 2;
        }
    }

    #[test]
    #[ignore]
    fn bench_mesh_query_parallel_threshold_sweep() {
        let instances = [1usize, 2, 4, 8, 16, 32];
        let triangles = [128usize, 512, 2048, 4096, 8192, 16384];
        let surfaces = [1usize, 2, 4, 8, 16];
        let rounds = 20usize;
        println!("inst,tri,surface,serial_us,parallel_us,speedup");
        for &inst in &instances {
            for &tri in &triangles {
                for &surface in &surfaces {
                    let mut serial_acc = 0.0_f32;
                    let serial_start = Instant::now();
                    for _ in 0..rounds {
                        for _ in 0..inst {
                            serial_acc += tri_workload(tri, surface);
                        }
                    }
                    let serial_us = serial_start.elapsed().as_micros();

                    let mut par_acc = 0.0_f32;
                    let parallel_start = Instant::now();
                    for _ in 0..rounds {
                        par_acc += (0..inst)
                            .into_par_iter()
                            .map(|_| tri_workload(tri, surface))
                            .sum::<f32>();
                    }
                    let parallel_us = parallel_start.elapsed().as_micros();
                    black_box(serial_acc);
                    black_box(par_acc);
                    let speedup = serial_us as f64 / parallel_us.max(1) as f64;
                    println!("{inst},{tri},{surface},{serial_us},{parallel_us},{speedup:.3}");
                }
            }
        }
    }

    #[test]
    #[ignore]
    fn bench_mesh_query_fixed_vertex_count_latency() {
        const TARGET_VERTICES: usize = 1_000_000;
        let surface_counts = [1usize, 4, 16, 64, 256];
        let tri_count = (TARGET_VERTICES / 3).max(1);
        let vertex_count = tri_count.saturating_mul(3);
        println!("running tests w/ vertices={vertex_count}, triangles={tri_count}");
        println!("surfaces,vertices,triangles,time_to_query_us");
        for &surface_count in &surface_counts {
            let mesh = build_synth_query_mesh(tri_count, surface_count);
            let mut samples = [
                measure_us_per_query(&mesh),
                measure_us_per_query(&mesh),
                measure_us_per_query(&mesh),
            ];
            samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let time_to_query_us = samples[1];
            println!("{surface_count},{vertex_count},{tri_count},{time_to_query_us:.6}");
        }
    }
}

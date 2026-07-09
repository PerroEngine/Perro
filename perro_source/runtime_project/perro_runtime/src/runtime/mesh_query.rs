//! Runtime mesh query cache, BVH build, and ray/region intersection paths.
//!
//! Query execution stays here. BVH/cache helpers and asset decode paths live
//! in child modules.

use super::Runtime;
use ahash::AHashMap;
use glam::{Mat3, Mat4, Vec3};
use perro_asset_formats::pmesh::{
    FLAG_PAYLOAD_RAW as PMESH_FLAG_PAYLOAD_RAW, VERSION as PMESH_VERSION,
};
use perro_ids::{MaterialID, MeshID, NodeID, parse_hashed_source_uri, string_to_u64};
use perro_io::decompress_zlib;
use perro_nodes::{MeshSurfaceBinding, SceneNodeData};
use perro_render_bridge::Mesh3D;
use perro_runtime_api::sub_apis::{
    MeshDataSurfaceHit3D, MeshDataSurfaceRegion3D, MeshMaterialRegion3D, MeshSurfaceHit3D,
    MeshSurfaceRay3D,
};
use perro_structs::Vector3;
use rayon::prelude::*;
use std::cell::RefCell;
use std::sync::{Arc, OnceLock, RwLock};

mod builtins;

use builtins::decode_builtin_query_mesh;

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
const QUERY_LINEAR_TRI_THRESHOLD: usize = 32;

type QueryMeshCache = AHashMap<u64, Arc<QueryMeshData>>;

mod accel;
mod decode;
mod simd;
use accel::*;
use decode::*;

pub(crate) use accel::QueryNodeDataCache;

impl Runtime {
    pub(crate) fn query_mesh_instance_surface_at_global_point(
        &mut self,
        node_id: NodeID,
        global_point: Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        self.query_mesh_surface_at_global_point_impl(node_id, global_point, true)
    }

    pub(crate) fn query_mesh_data_surface_at_local_point(
        &mut self,
        mesh_id: MeshID,
        local_point: Vector3,
    ) -> Option<MeshDataSurfaceHit3D> {
        let mesh = self.load_query_mesh_data_by_id(mesh_id)?;
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return None;
        }

        let p_local: Vec3 = local_point.into();
        let mut best: Option<QueryHitCandidate> = None;
        match query_mesh_strategy(mesh.triangles.len()) {
            QueryMeshStrategy::Linear => {
                for tri_idx in 0..mesh.triangles.len() {
                    best = query_point_tri_local(mesh.as_ref(), tri_idx, p_local, best)?;
                }
            }
            QueryMeshStrategy::Bvh => {
                best = query_point_mesh_bvh(mesh.as_ref(), p_local, best)?;
            }
        }
        let best = best?;
        Some(MeshDataSurfaceHit3D {
            surface_index: best.surface_index,
            local_point: best.local_point.into(),
            local_normal: best.local_normal.into(),
            distance: best.metric.sqrt(),
        })
    }

    fn query_mesh_surface_at_global_point_impl(
        &mut self,
        node_id: NodeID,
        global_point: Vector3,
        resolve_material: bool,
    ) -> Option<MeshSurfaceHit3D> {
        let node = self.query_node_mesh_data(node_id)?;
        let mesh = self.load_query_node_mesh_data(&node)?;
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return None;
        }

        let node_global = self.get_global_transform_3d(node_id)?.to_mat4();
        let p_global: Vec3 = global_point.into();
        let instance_parallel =
            should_parallel_instances(node.instance_local.len(), mesh.triangles.len());
        let best_for_instance = |(instance_index, local): (usize, &Mat4)| {
            let global_from_mesh = node_global * *local;
            let mesh_from_global = global_from_mesh.inverse();
            let global_normal_basis = Mat3::from_mat4(global_from_mesh).inverse().transpose();
            let p_local = mesh_from_global.transform_point3(p_global);
            let mut best: Option<QueryHitCandidate> = None;
            match query_mesh_strategy(mesh.triangles.len()) {
                QueryMeshStrategy::Linear => {
                    for tri_idx in 0..mesh.triangles.len() {
                        best = query_point_tri_global(
                            mesh.as_ref(),
                            tri_idx,
                            p_local,
                            instance_index as u32,
                            global_from_mesh,
                            global_normal_basis,
                            best,
                        )?;
                    }
                }
                QueryMeshStrategy::Bvh => {
                    best = query_point_mesh_bvh_global(
                        mesh.as_ref(),
                        p_local,
                        instance_index as u32,
                        global_from_mesh,
                        global_normal_basis,
                        best,
                    )?;
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
            global_point: best.global_point.into(),
            local_point: best.local_point.into(),
            global_normal: best.global_normal.into(),
            local_normal: best.local_normal.into(),
            distance: best.metric.sqrt(),
        })
    }

    pub(crate) fn query_mesh_instance_surface_on_global_ray(
        &mut self,
        node_id: NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
    ) -> Option<MeshSurfaceHit3D> {
        self.query_mesh_surface_on_global_ray_impl(
            node_id,
            ray_origin,
            ray_direction,
            max_distance,
            true,
        )
    }

    pub(crate) fn query_mesh_instance_surfaces_on_global_rays(
        &mut self,
        node_id: NodeID,
        rays: &[MeshSurfaceRay3D],
        resolve_material: bool,
    ) -> Vec<Option<MeshSurfaceHit3D>> {
        self.query_mesh_surfaces_on_global_rays_impl(node_id, rays, resolve_material)
    }

    pub(crate) fn query_mesh_data_surface_on_local_ray(
        &mut self,
        mesh_id: MeshID,
        ray_origin_local: Vector3,
        ray_direction_local: Vector3,
        max_distance: f32,
    ) -> Option<MeshDataSurfaceHit3D> {
        let mesh = self.load_query_mesh_data_by_id(mesh_id)?;
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return None;
        }

        let ray_origin_local: Vec3 = ray_origin_local.into();
        let ray_dir_raw: Vec3 = ray_direction_local.into();
        let ray_dir_len = ray_dir_raw.length();
        if ray_dir_len <= 0.000001 {
            return None;
        }
        let ray_dir_local = ray_dir_raw / ray_dir_len;
        let max_t = if max_distance.is_finite() && max_distance > 0.0 {
            max_distance
        } else {
            f32::INFINITY
        };

        let mut best: Option<QueryHitCandidate> = None;
        match query_mesh_strategy(mesh.triangles.len()) {
            QueryMeshStrategy::Linear => {
                for tri_idx in 0..mesh.triangles.len() {
                    best = query_ray_tri_local(
                        mesh.as_ref(),
                        tri_idx,
                        ray_origin_local,
                        ray_dir_local,
                        max_t,
                        best,
                    )?;
                }
            }
            QueryMeshStrategy::Bvh => {
                best = query_ray_mesh_bvh(
                    mesh.as_ref(),
                    ray_origin_local,
                    ray_dir_local,
                    max_t,
                    best,
                )?;
            }
        }
        let best = best?;
        Some(MeshDataSurfaceHit3D {
            surface_index: best.surface_index,
            local_point: best.local_point.into(),
            local_normal: best.local_normal.into(),
            distance: best.metric,
        })
    }

    fn query_mesh_surface_on_global_ray_impl(
        &mut self,
        node_id: NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
        resolve_material: bool,
    ) -> Option<MeshSurfaceHit3D> {
        let node = self.query_node_mesh_data(node_id)?;
        let mesh = self.load_query_node_mesh_data(&node)?;
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return None;
        }

        let ray_origin_global: Vec3 = ray_origin.into();
        let ray_dir_global_raw: Vec3 = ray_direction.into();
        let ray_dir_len = ray_dir_global_raw.length();
        if ray_dir_len <= 0.000001 {
            return None;
        }
        let ray_dir_global = ray_dir_global_raw / ray_dir_len;
        let max_t = if max_distance.is_finite() && max_distance > 0.0 {
            max_distance
        } else {
            f32::INFINITY
        };

        let node_global = self.get_global_transform_3d(node_id)?.to_mat4();
        let instance_parallel =
            should_parallel_instances(node.instance_local.len(), mesh.triangles.len());
        let best_for_instance = |(instance_index, local): (usize, &Mat4)| {
            let global_from_mesh = node_global * *local;
            let mesh_from_global = global_from_mesh.inverse();
            let global_normal_basis = Mat3::from_mat4(global_from_mesh).inverse().transpose();
            let ray_origin_local = mesh_from_global.transform_point3(ray_origin_global);
            let ray_dir_local = mesh_from_global
                .transform_vector3(ray_dir_global)
                .normalize_or_zero();
            if ray_dir_local.length_squared() <= 1e-10 {
                return None;
            }
            let mut best: Option<QueryHitCandidate> = None;
            match query_mesh_strategy(mesh.triangles.len()) {
                QueryMeshStrategy::Linear => {
                    for tri_idx in 0..mesh.triangles.len() {
                        best = query_ray_tri_global(
                            mesh.as_ref(),
                            tri_idx,
                            ray_origin_local,
                            ray_dir_local,
                            ray_origin_global,
                            max_t,
                            instance_index as u32,
                            global_from_mesh,
                            global_normal_basis,
                            best,
                        )?;
                    }
                }
                QueryMeshStrategy::Bvh => {
                    best = query_ray_mesh_bvh_global(
                        mesh.as_ref(),
                        ray_origin_local,
                        ray_dir_local,
                        ray_origin_global,
                        max_t,
                        instance_index as u32,
                        global_from_mesh,
                        global_normal_basis,
                        best,
                    )?;
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
            global_point: best.global_point.into(),
            local_point: best.local_point.into(),
            global_normal: best.global_normal.into(),
            local_normal: best.local_normal.into(),
            distance: best.metric,
        })
    }

    fn query_mesh_surfaces_on_global_rays_impl(
        &mut self,
        node_id: NodeID,
        rays: &[MeshSurfaceRay3D],
        resolve_material: bool,
    ) -> Vec<Option<MeshSurfaceHit3D>> {
        if rays.is_empty() {
            return Vec::new();
        }
        let Some(node) = self.query_node_mesh_data(node_id) else {
            return vec![None; rays.len()];
        };
        let Some(mesh) = self.load_query_node_mesh_data(&node) else {
            return vec![None; rays.len()];
        };
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return vec![None; rays.len()];
        }
        let Some(node_global) = self.get_global_transform_3d(node_id).map(|t| t.to_mat4()) else {
            return vec![None; rays.len()];
        };

        let instance_parallel =
            should_parallel_instances(node.instance_local.len(), mesh.triangles.len());
        let ray_parallel = rays.len() >= QUERY_INSTANCE_PAR_THRESHOLD
            && rays.len() * mesh.triangles.len() >= QUERY_PAR_WORK_THRESHOLD;

        let best_for_ray = |ray: &MeshSurfaceRay3D| {
            query_global_ray_candidates_for_node_mesh(
                mesh.as_ref(),
                &node.instance_local,
                node_global,
                *ray,
                instance_parallel,
            )
        };

        let best_hits: Vec<Option<QueryHitCandidate>> = if ray_parallel {
            rays.par_iter().map(best_for_ray).collect()
        } else {
            rays.iter().map(best_for_ray).collect()
        };

        best_hits
            .into_iter()
            .map(|best| {
                let best = best?;
                let material = if resolve_material {
                    self.query_surface_material(node_id, &node, best.surface_index)
                } else {
                    None
                };
                Some(MeshSurfaceHit3D {
                    instance_index: best.instance_index,
                    surface_index: best.surface_index,
                    material,
                    global_point: best.global_point.into(),
                    local_point: best.local_point.into(),
                    global_normal: best.global_normal.into(),
                    local_normal: best.local_normal.into(),
                    distance: best.metric,
                })
            })
            .collect()
    }

    pub(crate) fn query_mesh_instance_material_regions(
        &mut self,
        node_id: NodeID,
        material: MaterialID,
    ) -> Vec<MeshMaterialRegion3D> {
        let Some(node) = self.query_node_mesh_data(node_id) else {
            return Vec::new();
        };
        let Some(mesh) = self.load_query_node_mesh_data(&node) else {
            return Vec::new();
        };
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return Vec::new();
        }

        let node_global = match self.get_global_transform_3d(node_id) {
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
            let global_from_mesh = node_global * *local;
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
                        let tri_global_center = global_from_mesh.transform_point3(tri_local_center);
                        let mut local_min = Vec3::splat(f32::INFINITY);
                        let mut local_max = Vec3::splat(f32::NEG_INFINITY);
                        let mut global_min = Vec3::splat(f32::INFINITY);
                        let mut global_max = Vec3::splat(f32::NEG_INFINITY);

                        for p in [a, b, c] {
                            local_min = local_min.min(p);
                            local_max = local_max.max(p);
                            let pw = global_from_mesh.transform_point3(p);
                            global_min = global_min.min(pw);
                            global_max = global_max.max(pw);
                        }

                        QueryRegionAcc {
                            tri_count: 1,
                            sum_local: tri_local_center,
                            sum_global: tri_global_center,
                            local_min,
                            local_max,
                            global_min,
                            global_max,
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
                        center_global: (acc.sum_global * inv).into(),
                        center_local: (acc.sum_local * inv).into(),
                        aabb_min_global: acc.global_min.into(),
                        aabb_max_global: acc.global_max.into(),
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
        mesh_id: MeshID,
        surface_index: u32,
    ) -> Vec<MeshDataSurfaceRegion3D> {
        let Some(mesh) = self.load_query_mesh_data_by_id(mesh_id) else {
            return Vec::new();
        };
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return Vec::new();
        }

        let vertices = &mesh.vertices;
        let triangles = &mesh.triangles;

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
            let mut local_min = Vec3::splat(f32::INFINITY);
            let mut local_max = Vec3::splat(f32::NEG_INFINITY);

            for p in [a, b, c] {
                local_min = local_min.min(p);
                local_max = local_max.max(p);
            }

            QueryRegionAcc {
                tri_count: 1,
                sum_local: tri_local_center,
                sum_global: tri_local_center,
                local_min,
                local_max,
                global_min: local_min,
                global_max: local_max,
            }
        };

        let acc = if should_parallel_triangles(false, triangles.len()) {
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
            return Vec::new();
        }
        let inv = 1.0 / acc.tri_count as f32;
        vec![MeshDataSurfaceRegion3D {
            surface_index,
            triangle_count: acc.tri_count,
            center_local: (acc.sum_local * inv).into(),
            aabb_min_local: acc.local_min.into(),
            aabb_max_local: acc.local_max.into(),
        }]
    }

    /// Cached lookup for the per-node query snapshot (source path, surfaces,
    /// per-instance transforms). Rebuilding this is the expensive part for
    /// `MultiMeshInstance3D` (a `surfaces.clone()` plus one
    /// `Mat4::from_scale_rotation_translation` per instance), so repeated
    /// point/ray/region queries against an unchanged node reuse the cached
    /// `Arc` instead of rebuilding every call. Cache entries are validated
    /// against `nodes.mutation_revision()`, which bumps on ANY node mutation
    /// (not just this node) -- conservative but always correct, never a
    /// stale hit.
    fn query_node_mesh_data(&mut self, node_id: NodeID) -> Option<Arc<QueryNodeData>> {
        let current_revision = self.nodes.mutation_revision();
        if let Some(entry) = self.mesh_query_node_cache.get(&node_id)
            && entry.built_at_revision == current_revision
        {
            return Some(entry.data.clone());
        }

        let data = Arc::new(self.build_query_node_mesh_data(node_id)?);
        #[cfg(any(test, feature = "bench"))]
        self.mesh_query_node_rebuilds
            .set(self.mesh_query_node_rebuilds.get() + 1);
        self.mesh_query_node_cache.insert(
            node_id,
            QueryNodeDataCacheEntry {
                data: data.clone(),
                built_at_revision: current_revision,
            },
        );
        Some(data)
    }

    fn build_query_node_mesh_data(&self, node_id: NodeID) -> Option<QueryNodeData> {
        let node = self.nodes.get(node_id)?;
        match &node.data {
            SceneNodeData::MeshInstance3D(mesh) => Some(QueryNodeData {
                mesh_id: mesh.mesh,
                source: self.render_3d.mesh_sources.get(&node_id).cloned(),
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
                            let scale = instance.transform.scale;
                            Mat4::from_scale_rotation_translation(
                                Vec3::new(
                                    scale.x * mesh.instance_scale.max(0.0001),
                                    scale.y * mesh.instance_scale.max(0.0001),
                                    scale.z * mesh.instance_scale.max(0.0001),
                                ),
                                glam::Quat::from_xyzw(
                                    instance.transform.rotation.x,
                                    instance.transform.rotation.y,
                                    instance.transform.rotation.z,
                                    instance.transform.rotation.w,
                                ),
                                Vec3::new(
                                    instance.transform.position.x,
                                    instance.transform.position.y,
                                    instance.transform.position.z,
                                ),
                            )
                        })
                        .collect()
                };
                Some(QueryNodeData {
                    mesh_id: mesh.mesh,
                    source: self.render_3d.mesh_sources.get(&node_id).cloned(),
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

    fn load_query_node_mesh_data(&self, node: &QueryNodeData) -> Option<Arc<QueryMeshData>> {
        if !node.mesh_id.is_nil()
            && let Some(mesh) = self.load_query_mesh_data_by_id(node.mesh_id)
        {
            return Some(mesh);
        }
        node.source
            .as_deref()
            .and_then(|source| self.load_query_mesh_data(source))
    }

    fn load_query_mesh_data_by_id(&self, mesh_id: MeshID) -> Option<Arc<QueryMeshData>> {
        if mesh_id.is_nil() {
            return None;
        }
        let mesh_id = self.resource_api.canonical_mesh_id(mesh_id);
        if let Some(revision) = self.resource_api.mesh_revision(mesh_id) {
            let cache_key = runtime_mesh_query_cache_key(mesh_id, revision);
            if let Ok(cache) = mesh_query_cache().read()
                && let Some(mesh) = cache.get(&cache_key)
            {
                return Some(mesh.clone());
            }
            let (cache_key, mesh) = self
                .resource_api
                .with_mesh_data_and_revision(mesh_id, |data, revision| {
                    build_query_mesh_from_runtime_mesh(data)
                        .map(Arc::new)
                        .map(|mesh| (runtime_mesh_query_cache_key(mesh_id, revision), mesh))
                })
                .flatten()?;
            if let Ok(mut cache) = mesh_query_cache().write() {
                cache.insert(cache_key, mesh.clone());
            }
            return Some(mesh);
        }
        self.resource_api
            .mesh_source(mesh_id)
            .and_then(|source| self.load_query_mesh_data(source.as_str()))
    }
}

fn query_global_ray_candidates_for_node_mesh(
    mesh: &QueryMeshData,
    instance_local: &[Mat4],
    node_global: Mat4,
    ray: MeshSurfaceRay3D,
    instance_parallel: bool,
) -> Option<QueryHitCandidate> {
    let ray_origin_global: Vec3 = ray.origin.into();
    let ray_dir_global_raw: Vec3 = ray.direction.into();
    let ray_dir_len = ray_dir_global_raw.length();
    if ray_dir_len <= 0.000001 {
        return None;
    }
    let ray_dir_global = ray_dir_global_raw / ray_dir_len;
    let max_t = if ray.max_distance.is_finite() && ray.max_distance > 0.0 {
        ray.max_distance
    } else {
        f32::INFINITY
    };

    let best_for_instance = |(instance_index, local): (usize, &Mat4)| {
        let global_from_mesh = node_global * *local;
        let mesh_from_global = global_from_mesh.inverse();
        let global_normal_basis = Mat3::from_mat4(global_from_mesh).inverse().transpose();
        let ray_origin_local = mesh_from_global.transform_point3(ray_origin_global);
        let ray_dir_local = mesh_from_global
            .transform_vector3(ray_dir_global)
            .normalize_or_zero();
        if ray_dir_local.length_squared() <= 1e-10 {
            return None;
        }
        let mut best: Option<QueryHitCandidate> = None;
        match query_mesh_strategy(mesh.triangles.len()) {
            QueryMeshStrategy::Linear => {
                for tri_idx in 0..mesh.triangles.len() {
                    best = query_ray_tri_global(
                        mesh,
                        tri_idx,
                        ray_origin_local,
                        ray_dir_local,
                        ray_origin_global,
                        max_t,
                        instance_index as u32,
                        global_from_mesh,
                        global_normal_basis,
                        best,
                    )?;
                }
            }
            QueryMeshStrategy::Bvh => {
                best = query_ray_mesh_bvh_global(
                    mesh,
                    ray_origin_local,
                    ray_dir_local,
                    ray_origin_global,
                    max_t,
                    instance_index as u32,
                    global_from_mesh,
                    global_normal_basis,
                    best,
                )?;
            }
        }
        best
    };
    if instance_parallel {
        instance_local
            .par_iter()
            .enumerate()
            .map(best_for_instance)
            .reduce(|| None, nearer_hit)
    } else {
        instance_local
            .iter()
            .enumerate()
            .map(best_for_instance)
            .fold(None, nearer_hit)
    }
}

#[cfg(test)]
mod tests;

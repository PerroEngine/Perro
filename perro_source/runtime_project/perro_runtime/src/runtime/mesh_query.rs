use super::Runtime;
use glam::{Mat3, Mat4, Vec3};
use perro_ids::{MaterialID, NodeID, parse_hashed_source_uri, string_to_u64};
use perro_io::decompress_zlib;
use perro_nodes::{MeshSurfaceBinding, SceneNodeData};
use perro_runtime_context::sub_apis::{MeshMaterialRegion3D, MeshSurfaceHit3D};
use perro_structs::Vector3;
use rayon::prelude::*;

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

fn nearer_hit(a: Option<QueryHitCandidate>, b: Option<QueryHitCandidate>) -> Option<QueryHitCandidate> {
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
        let best = node
            .instance_local
            .par_iter()
            .enumerate()
            .map(|(instance_index, local)| {
                let world_from_mesh = node_world * *local;
                let mesh_from_world = world_from_mesh.inverse();
                let world_normal_basis = Mat3::from_mat4(world_from_mesh).inverse().transpose();

                mesh.triangles
                    .par_iter()
                    .copied()
                    .filter_map(|tri| {
                        let a = mesh.vertices.get(tri.a as usize).copied()?;
                        let b = mesh.vertices.get(tri.b as usize).copied()?;
                        let c = mesh.vertices.get(tri.c as usize).copied()?;
                        let local_normal = (b - a).cross(c - a).normalize_or_zero();
                        let world_normal = (world_normal_basis * local_normal).normalize_or_zero();

                        let aw = world_from_mesh.transform_point3(a);
                        let bw = world_from_mesh.transform_point3(b);
                        let cw = world_from_mesh.transform_point3(c);
                        let nearest_world = closest_point_on_triangle(p_world, aw, bw, cw);
                        let d2 = nearest_world.distance_squared(p_world);
                        let nearest_local = mesh_from_world.transform_point3(nearest_world);
                        Some(QueryHitCandidate {
                            instance_index: instance_index as u32,
                            surface_index: tri.surface_index,
                            world_point: nearest_world,
                            local_point: nearest_local,
                            world_normal,
                            local_normal,
                            metric: d2,
                        })
                    })
                    .reduce_with(|left, right| {
                        if right.metric < left.metric {
                            right
                        } else {
                            left
                        }
                    })
            })
            .reduce(|| None, nearer_hit)?;
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
        let best = node
            .instance_local
            .par_iter()
            .enumerate()
            .map(|(instance_index, local)| {
                let world_from_mesh = node_world * *local;
                let mesh_from_world = world_from_mesh.inverse();
                let world_normal_basis = Mat3::from_mat4(world_from_mesh).inverse().transpose();

                mesh.triangles
                    .par_iter()
                    .copied()
                    .filter_map(|tri| {
                        let a = mesh.vertices.get(tri.a as usize).copied()?;
                        let b = mesh.vertices.get(tri.b as usize).copied()?;
                        let c = mesh.vertices.get(tri.c as usize).copied()?;

                        let aw = world_from_mesh.transform_point3(a);
                        let bw = world_from_mesh.transform_point3(b);
                        let cw = world_from_mesh.transform_point3(c);
                        let t = ray_intersect_triangle(ray_origin_world, ray_dir_world, aw, bw, cw)?;
                        if t > max_t {
                            return None;
                        }

                        let local_normal = (b - a).cross(c - a).normalize_or_zero();
                        let world_normal = (world_normal_basis * local_normal).normalize_or_zero();
                        let hit_world = ray_origin_world + ray_dir_world * t;
                        let hit_local = mesh_from_world.transform_point3(hit_world);
                        Some(QueryHitCandidate {
                            instance_index: instance_index as u32,
                            surface_index: tri.surface_index,
                            world_point: hit_world,
                            local_point: hit_local,
                            world_normal,
                            local_normal,
                            metric: t,
                        })
                    })
                    .reduce_with(|left, right| {
                        if right.metric < left.metric {
                            right
                        } else {
                            left
                        }
                    })
            })
            .reduce(|| None, nearer_hit)?;
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

        node.instance_local
            .par_iter()
            .enumerate()
            .flat_map_iter(|(instance_index, local)| {
                let world_from_mesh = node_world * *local;
                node.surfaces
                    .iter()
                    .enumerate()
                    .filter(move |(_, surface)| surface.material == Some(material))
                    .filter_map(move |(surface_index, surface)| {
                        let acc = triangles
                            .par_iter()
                            .copied()
                            .filter(|tri| tri.surface_index as usize == surface_index)
                            .map(|tri| {
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
                            })
                            .reduce(QueryRegionAcc::empty, merge_region_acc);

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
            })
            .collect()
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

        node.instance_local
            .par_iter()
            .enumerate()
            .filter_map(|(instance_index, local)| {
                let world_from_mesh = node_world * *local;
                let acc = triangles
                    .par_iter()
                    .copied()
                    .filter(|tri| tri.surface_index == surface_index)
                    .map(|tri| {
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
                    })
                    .reduce(QueryRegionAcc::empty, merge_region_acc);

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
            })
            .collect()
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

    fn load_query_mesh_data(&self, source: &str) -> Option<QueryMeshData> {
        let source = source.trim();
        if source.is_empty() {
            return None;
        }
        if source.starts_with("__") {
            return decode_builtin_query_mesh(source);
        }

        let normalized = normalize_source_slashes(source);
        let source_hash = parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
        if self.provider_mode() == crate::runtime_project::ProviderMode::Static
            && let Some(lookup) = self
                .project()
                .and_then(|project| project.static_mesh_lookup)
        {
            let bytes = lookup(source_hash);
            if !bytes.is_empty()
                && let Some(mesh) = decode_pmesh_query(bytes)
            {
                return Some(mesh);
            }
            if normalized.as_ref() != source {
                let bytes = lookup(string_to_u64(normalized.as_ref()));
                if !bytes.is_empty()
                    && let Some(mesh) = decode_pmesh_query(bytes)
                {
                    return Some(mesh);
                }
            }
            if let Some(alias) = normalized_static_mesh_lookup_alias(source) {
                let bytes = lookup(string_to_u64(alias.as_str()));
                if !bytes.is_empty()
                    && let Some(mesh) = decode_pmesh_query(bytes)
                {
                    return Some(mesh);
                }
            }
            if normalized.as_ref() != source
                && let Some(alias) = normalized_static_mesh_lookup_alias(normalized.as_ref())
            {
                let bytes = lookup(string_to_u64(alias.as_str()));
                if !bytes.is_empty()
                    && let Some(mesh) = decode_pmesh_query(bytes)
                {
                    return Some(mesh);
                }
            }
        }

        let (path, fragment) = split_source_fragment(source);
        let mesh_index = parse_fragment_index(fragment, "mesh").unwrap_or(0);
        let bytes = perro_io::load_asset(path).ok()?;
        if path.ends_with(".glb") || path.ends_with(".gltf") {
            return decode_gltf_query_mesh(&bytes, mesh_index);
        }
        if path.ends_with(".pmesh") {
            return decode_pmesh_query(&bytes);
        }
        None
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
    Some(QueryMeshData {
        vertices: verts,
        triangles,
    })
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
        let local_positions: Vec<[f32; 3]> = positions.collect();
        if local_positions.len() < 3 {
            continue;
        }

        let base = vertices.len() as u32;
        for p in local_positions.iter().copied() {
            vertices.push(Vec3::new(p[0], p[1], p[2]));
        }

        if let Some(indices_reader) = reader.read_indices() {
            let flat: Vec<u32> = indices_reader.into_u32().collect();
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
    }

    if vertices.is_empty() || triangles.is_empty() {
        return None;
    }

    Some(QueryMeshData {
        vertices,
        triangles,
    })
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

    if vertices.is_empty() || triangles.is_empty() {
        return None;
    }
    Some(QueryMeshData {
        vertices,
        triangles,
    })
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

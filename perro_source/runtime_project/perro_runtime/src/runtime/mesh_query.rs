use super::Runtime;
use glam::{Mat3, Mat4, Vec3};
use perro_ids::{MaterialID, NodeID};
use perro_nodes::{MeshSurfaceBinding, SceneNodeData};
use perro_runtime_context::sub_apis::{MeshMaterialRegion3D, MeshSurfaceHit3D};
use perro_structs::Vector3;

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

impl Runtime {
    pub(crate) fn query_mesh_surface_at_world_point(
        &mut self,
        node_id: NodeID,
        world_point: Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        let node = self.query_node_mesh_data(node_id)?;
        let mesh = load_query_mesh_data(node.source.as_str())?;
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return None;
        }

        let node_world = self.get_global_transform_3d(node_id)?.to_mat4();
        let mut best: Option<(u32, u32, Vec3, Vec3, Vec3, Vec3, f32)> = None;
        let p_world: Vec3 = world_point.into();

        for (instance_index, local) in node.instance_local.iter().enumerate() {
            let world_from_mesh = node_world * *local;
            let mesh_from_world = world_from_mesh.inverse();
            let world_normal_basis = Mat3::from_mat4(world_from_mesh).inverse().transpose();

            for tri in mesh.triangles.iter().copied() {
                let a = *mesh.vertices.get(tri.a as usize)?;
                let b = *mesh.vertices.get(tri.b as usize)?;
                let c = *mesh.vertices.get(tri.c as usize)?;
                let local_normal = (b - a).cross(c - a).normalize_or_zero();
                let world_normal = (world_normal_basis * local_normal).normalize_or_zero();

                let aw = world_from_mesh.transform_point3(a);
                let bw = world_from_mesh.transform_point3(b);
                let cw = world_from_mesh.transform_point3(c);
                let nearest_world = closest_point_on_triangle(p_world, aw, bw, cw);
                let d2 = nearest_world.distance_squared(p_world);

                match best {
                    Some((_, _, _, _, _, _, best_d2)) if d2 >= best_d2 => {}
                    _ => {
                        let nearest_local = mesh_from_world.transform_point3(nearest_world);
                        best = Some((
                            instance_index as u32,
                            tri.surface_index,
                            nearest_world,
                            nearest_local,
                            world_normal,
                            local_normal,
                            d2,
                        ));
                    }
                }
            }
        }

        let (
            instance_index,
            surface_index,
            nearest_world,
            nearest_local,
            world_normal,
            local_normal,
            d2,
        ) = best?;
        let material = node
            .surfaces
            .get(surface_index as usize)
            .and_then(|surface| surface.material);
        Some(MeshSurfaceHit3D {
            instance_index,
            surface_index,
            material,
            world_point: nearest_world.into(),
            local_point: nearest_local.into(),
            world_normal: world_normal.into(),
            local_normal: local_normal.into(),
            distance: d2.sqrt(),
        })
    }

    pub(crate) fn query_mesh_material_regions(
        &mut self,
        node_id: NodeID,
        material: MaterialID,
    ) -> Vec<MeshMaterialRegion3D> {
        let Some(node) = self.query_node_mesh_data(node_id) else {
            return Vec::new();
        };
        let Some(mesh) = load_query_mesh_data(node.source.as_str()) else {
            return Vec::new();
        };
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return Vec::new();
        }

        let node_world = match self.get_global_transform_3d(node_id) {
            Some(transform) => transform.to_mat4(),
            None => return Vec::new(),
        };

        let mut out = Vec::new();

        for (instance_index, local) in node.instance_local.iter().enumerate() {
            let world_from_mesh = node_world * *local;
            for (surface_index, surface) in node.surfaces.iter().enumerate() {
                if surface.material != Some(material) {
                    continue;
                }

                let mut tri_count = 0_u32;
                let mut sum_local = Vec3::ZERO;
                let mut sum_world = Vec3::ZERO;
                let mut local_min = Vec3::splat(f32::INFINITY);
                let mut local_max = Vec3::splat(f32::NEG_INFINITY);
                let mut world_min = Vec3::splat(f32::INFINITY);
                let mut world_max = Vec3::splat(f32::NEG_INFINITY);

                for tri in mesh.triangles.iter().copied() {
                    if tri.surface_index as usize != surface_index {
                        continue;
                    }
                    let (Some(a), Some(b), Some(c)) = (
                        mesh.vertices.get(tri.a as usize).copied(),
                        mesh.vertices.get(tri.b as usize).copied(),
                        mesh.vertices.get(tri.c as usize).copied(),
                    ) else {
                        continue;
                    };

                    tri_count = tri_count.saturating_add(1);

                    let tri_local_center = (a + b + c) / 3.0;
                    let tri_world_center = world_from_mesh.transform_point3(tri_local_center);
                    sum_local += tri_local_center;
                    sum_world += tri_world_center;

                    for p in [a, b, c] {
                        local_min = local_min.min(p);
                        local_max = local_max.max(p);
                        let pw = world_from_mesh.transform_point3(p);
                        world_min = world_min.min(pw);
                        world_max = world_max.max(pw);
                    }
                }

                if tri_count == 0 {
                    continue;
                }
                let inv = 1.0 / tri_count as f32;
                out.push(MeshMaterialRegion3D {
                    instance_index: instance_index as u32,
                    surface_index: surface_index as u32,
                    material: surface.material,
                    triangle_count: tri_count,
                    center_world: (sum_world * inv).into(),
                    center_local: (sum_local * inv).into(),
                    aabb_min_world: world_min.into(),
                    aabb_max_world: world_max.into(),
                    aabb_min_local: local_min.into(),
                    aabb_max_local: local_max.into(),
                });
            }
        }

        out
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
                let instance_local = if mesh.transforms.is_empty() {
                    vec![Mat4::IDENTITY]
                } else {
                    mesh.transforms
                        .iter()
                        .map(|transform| transform.to_mat4())
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
}

fn load_query_mesh_data(source: &str) -> Option<QueryMeshData> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    if source.starts_with("__") {
        return decode_builtin_query_mesh(source);
    }

    let (path, fragment) = split_source_fragment(source);
    let mesh_index = parse_fragment_index(fragment, "mesh").unwrap_or(0);
    let bytes = perro_io::load_asset(path).ok()?;
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        return decode_gltf_query_mesh(&bytes, mesh_index);
    }
    None
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

use super::*;

pub(crate) fn build_skeleton_palette(
    nodes: &crate::cns::NodeArena,
    skeleton_id: NodeID,
    global: &mut Vec<Mat4>,
    out: &mut Vec<[[f32; 4]; 3]>,
) -> Option<()> {
    let skeleton_node = nodes.get(skeleton_id)?;
    let skeleton = match &skeleton_node.data {
        SceneNodeData::Skeleton3D(skeleton) => skeleton,
        _ => return None,
    };
    if skeleton.bones.is_empty() {
        return None;
    }

    global.clear();
    global.resize(skeleton.bones.len(), Mat4::IDENTITY);
    for (i, bone) in skeleton.bones.iter().enumerate() {
        let local = bone.pose.to_mat4();
        if bone.parent >= 0 {
            let parent = bone.parent as usize;
            if parent < global.len() {
                global[i] = global[parent] * local;
            } else {
                global[i] = local;
            }
        } else {
            global[i] = local;
        }
    }

    out.clear();
    if out.capacity() < skeleton.bones.len() {
        out.reserve(skeleton.bones.len() - out.capacity());
    }
    // Precomputed inverse-bind lane (constant bind pose); fall back to an inline
    // TRS→matrix conversion when the lane is absent/stale for this bone set.
    let inv_bind_mats = skeleton.inv_bind_mats();
    let use_cache = inv_bind_mats.len() == skeleton.bones.len();
    for (i, bone) in skeleton.bones.iter().enumerate() {
        let inv_bind = if use_cache {
            inv_bind_mats[i].0
        } else {
            bone.inv_bind.to_mat4()
        };
        let joint = global[i] * inv_bind;
        out.push(pack_bone_affine_rows(&joint));
    }
    Some(())
}

/// Pack a column-major bone matrix into its 3 affine rows (row-major). The
/// bottom row of a skinning matrix is always `(0,0,0,1)`, so it is dropped —
/// this is the exact layout the skinning shaders read, uploaded without a
/// second repack on the GPU staging path.
#[inline]
pub(super) fn pack_bone_affine_rows(joint: &Mat4) -> [[f32; 4]; 3] {
    let c = joint.to_cols_array_2d();
    [
        [c[0][0], c[1][0], c[2][0], c[3][0]],
        [c[0][1], c[1][1], c[2][1], c[3][1]],
        [c[0][2], c[1][2], c[2][2], c[3][2]],
    ]
}

pub(super) fn collision_debug_edge_node(node: NodeID, index: u32) -> NodeID {
    // Synthetic retained debug ID namespace: top byte 0xD3 for collision edges.
    NodeID::from_u64((0xD3u64 << 56) ^ (node.as_u64() << 16) ^ index as u64)
}

pub(super) fn is_physics_body_3d(runtime: &Runtime, node: NodeID) -> bool {
    runtime.nodes.get(node).is_some_and(|scene_node| {
        matches!(
            scene_node.data,
            SceneNodeData::StaticBody3D(_)
                | SceneNodeData::RigidBody3D(_)
                | SceneNodeData::CharacterBody3D(_)
                | SceneNodeData::Area3D(_)
        )
    })
}

pub(super) fn transform_no_scale_mat4(transform: perro_structs::Transform3D) -> Mat4 {
    let rotation = Quat::from_xyzw(
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
    );
    Mat4::from_scale_rotation_translation(
        Vec3::ONE,
        rotation,
        Vec3::new(
            transform.position.x,
            transform.position.y,
            transform.position.z,
        ),
    )
}

pub(super) fn shape_scaled_by_local_scale(
    shape: Shape3D,
    scale: perro_structs::Vector3,
) -> Shape3D {
    let sx = scale.x.abs().max(0.0001);
    let sy = scale.y.abs().max(0.0001);
    let sz = scale.z.abs().max(0.0001);
    match shape {
        Shape3D::Cube { size } => Shape3D::Cube {
            size: perro_structs::Vector3::new(size.x * sx, size.y * sy, size.z * sz),
        },
        Shape3D::Sphere { radius } => Shape3D::Sphere {
            radius: radius * sx.max(sy).max(sz),
        },
        Shape3D::Capsule {
            radius,
            half_height,
        } => Shape3D::Capsule {
            radius: radius * sx.max(sz),
            half_height: half_height * sy,
        },
        Shape3D::Cylinder {
            radius,
            half_height,
        } => Shape3D::Cylinder {
            radius: radius * sx.max(sz),
            half_height: half_height * sy,
        },
        Shape3D::Cone {
            radius,
            half_height,
        } => Shape3D::Cone {
            radius: radius * sx.max(sz),
            half_height: half_height * sy,
        },
        Shape3D::TriPrism { size } => Shape3D::TriPrism {
            size: perro_structs::Vector3::new(size.x * sx, size.y * sy, size.z * sz),
        },
        Shape3D::TriangularPyramid { size } => Shape3D::TriangularPyramid {
            size: perro_structs::Vector3::new(size.x * sx, size.y * sy, size.z * sz),
        },
        Shape3D::SquarePyramid { size } => Shape3D::SquarePyramid {
            size: perro_structs::Vector3::new(size.x * sx, size.y * sy, size.z * sz),
        },
        Shape3D::TriMesh { source } => Shape3D::TriMesh { source },
    }
}

pub(super) fn collision_debug_signature(shape: &Shape3D, world_from_shape: Mat4) -> u64 {
    let mut h = 0xC011_1510_0D3B_9A77u64;
    hash_shape3d(&mut h, shape);
    for col in world_from_shape.to_cols_array_2d() {
        for value in col {
            h ^= value.to_bits() as u64;
            h = h.rotate_left(9).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    h
}

pub(super) fn hash_shape3d(h: &mut u64, shape: &Shape3D) {
    match shape {
        Shape3D::Cube { size } => {
            *h ^= 1;
            mix_hash_f32(h, size.x);
            mix_hash_f32(h, size.y);
            mix_hash_f32(h, size.z);
        }
        Shape3D::Sphere { radius } => {
            *h ^= 2;
            mix_hash_f32(h, *radius);
        }
        Shape3D::Capsule {
            radius,
            half_height,
        } => {
            *h ^= 3;
            mix_hash_f32(h, *radius);
            mix_hash_f32(h, *half_height);
        }
        Shape3D::Cylinder {
            radius,
            half_height,
        } => {
            *h ^= 4;
            mix_hash_f32(h, *radius);
            mix_hash_f32(h, *half_height);
        }
        Shape3D::Cone {
            radius,
            half_height,
        } => {
            *h ^= 5;
            mix_hash_f32(h, *radius);
            mix_hash_f32(h, *half_height);
        }
        Shape3D::TriPrism { size } => {
            *h ^= 6;
            mix_hash_f32(h, size.x);
            mix_hash_f32(h, size.y);
            mix_hash_f32(h, size.z);
        }
        Shape3D::TriangularPyramid { size } => {
            *h ^= 7;
            mix_hash_f32(h, size.x);
            mix_hash_f32(h, size.y);
            mix_hash_f32(h, size.z);
        }
        Shape3D::SquarePyramid { size } => {
            *h ^= 8;
            mix_hash_f32(h, size.x);
            mix_hash_f32(h, size.y);
            mix_hash_f32(h, size.z);
        }
        Shape3D::TriMesh { source } => {
            *h ^= 9;
            for b in source.as_bytes() {
                *h ^= *b as u64;
                *h = h.rotate_left(11).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            }
        }
    }
}

#[inline]
pub(super) fn mix_hash_f32(h: &mut u64, value: f32) {
    *h ^= value.to_bits() as u64;
    *h = h.rotate_left(11).wrapping_mul(0xBF58_476D_1CE4_E5B9);
}

pub(super) fn collision_shape_wire_segments(shape: Shape3D) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    match shape {
        Shape3D::Cube { size } => {
            let hx = size.x.abs().max(0.0001) * 0.5;
            let hy = size.y.abs().max(0.0001) * 0.5;
            let hz = size.z.abs().max(0.0001) * 0.5;
            let points = [
                Vec3::new(-hx, -hy, -hz),
                Vec3::new(hx, -hy, -hz),
                Vec3::new(hx, hy, -hz),
                Vec3::new(-hx, hy, -hz),
                Vec3::new(-hx, -hy, hz),
                Vec3::new(hx, -hy, hz),
                Vec3::new(hx, hy, hz),
                Vec3::new(-hx, hy, hz),
            ];
            let edges = [
                (0usize, 1usize),
                (1, 2),
                (2, 3),
                (3, 0),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 4),
                (0, 4),
                (1, 5),
                (2, 6),
                (3, 7),
            ];
            push_indexed_edges(&mut out, &points, &edges);
        }
        Shape3D::Sphere { radius } => {
            let r = radius.abs().max(0.0001);
            append_circle_segments(
                &mut out,
                Vec3::ZERO,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, r, 0.0),
                20,
            );
            append_circle_segments(
                &mut out,
                Vec3::ZERO,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            append_circle_segments(
                &mut out,
                Vec3::ZERO,
                Vec3::new(0.0, r, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
        }
        Shape3D::Capsule {
            radius,
            half_height,
        } => {
            let r = radius.abs().max(0.0001);
            let h = half_height.abs().max(0.0001);
            let top = Vec3::new(0.0, h, 0.0);
            let bot = Vec3::new(0.0, -h, 0.0);
            append_circle_segments(
                &mut out,
                top,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            append_circle_segments(
                &mut out,
                bot,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            out.push((Vec3::new(r, -h, 0.0), Vec3::new(r, h, 0.0)));
            out.push((Vec3::new(-r, -h, 0.0), Vec3::new(-r, h, 0.0)));
            out.push((Vec3::new(0.0, -h, r), Vec3::new(0.0, h, r)));
            out.push((Vec3::new(0.0, -h, -r), Vec3::new(0.0, h, -r)));
            append_arc_segments(
                &mut out,
                top,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, r, 0.0),
                std::f32::consts::PI,
                16,
            );
            append_arc_segments(
                &mut out,
                top,
                Vec3::new(0.0, 0.0, r),
                Vec3::new(0.0, r, 0.0),
                std::f32::consts::PI,
                16,
            );
            append_arc_segments(
                &mut out,
                bot,
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, -r, 0.0),
                std::f32::consts::PI,
                16,
            );
            append_arc_segments(
                &mut out,
                bot,
                Vec3::new(0.0, 0.0, r),
                Vec3::new(0.0, -r, 0.0),
                std::f32::consts::PI,
                16,
            );
        }
        Shape3D::Cylinder {
            radius,
            half_height,
        } => {
            let r = radius.abs().max(0.0001);
            let h = half_height.abs().max(0.0001);
            append_circle_segments(
                &mut out,
                Vec3::new(0.0, h, 0.0),
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            append_circle_segments(
                &mut out,
                Vec3::new(0.0, -h, 0.0),
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            out.push((Vec3::new(r, -h, 0.0), Vec3::new(r, h, 0.0)));
            out.push((Vec3::new(-r, -h, 0.0), Vec3::new(-r, h, 0.0)));
            out.push((Vec3::new(0.0, -h, r), Vec3::new(0.0, h, r)));
            out.push((Vec3::new(0.0, -h, -r), Vec3::new(0.0, h, -r)));
        }
        Shape3D::Cone {
            radius,
            half_height,
        } => {
            let r = radius.abs().max(0.0001);
            let h = half_height.abs().max(0.0001);
            append_circle_segments(
                &mut out,
                Vec3::new(0.0, -h, 0.0),
                Vec3::new(r, 0.0, 0.0),
                Vec3::new(0.0, 0.0, r),
                20,
            );
            let apex = Vec3::new(0.0, h, 0.0);
            out.push((Vec3::new(r, -h, 0.0), apex));
            out.push((Vec3::new(-r, -h, 0.0), apex));
            out.push((Vec3::new(0.0, -h, r), apex));
            out.push((Vec3::new(0.0, -h, -r), apex));
        }
        Shape3D::TriPrism { size } => {
            let hw = size.x.abs().max(0.0001) * 0.5;
            let hh = size.y.abs().max(0.0001) * 0.5;
            let hd = size.z.abs().max(0.0001) * 0.5;
            let points = [
                Vec3::new(-hw, -hh, -hd),
                Vec3::new(hw, -hh, -hd),
                Vec3::new(0.0, hh, -hd),
                Vec3::new(-hw, -hh, hd),
                Vec3::new(hw, -hh, hd),
                Vec3::new(0.0, hh, hd),
            ];
            let edges = [
                (0usize, 1usize),
                (1, 2),
                (2, 0),
                (3, 4),
                (4, 5),
                (5, 3),
                (0, 3),
                (1, 4),
                (2, 5),
            ];
            push_indexed_edges(&mut out, &points, &edges);
        }
        Shape3D::TriangularPyramid { size } => {
            let hw = size.x.abs().max(0.0001) * 0.5;
            let hh = size.y.abs().max(0.0001) * 0.5;
            let hd = size.z.abs().max(0.0001) * 0.5;
            let points = [
                Vec3::new(-hw, -hh, -hd),
                Vec3::new(hw, -hh, -hd),
                Vec3::new(0.0, -hh, hd),
                Vec3::new(0.0, hh, 0.0),
            ];
            let edges = [(0usize, 1usize), (1, 2), (2, 0), (0, 3), (1, 3), (2, 3)];
            push_indexed_edges(&mut out, &points, &edges);
        }
        Shape3D::SquarePyramid { size } => {
            let hw = size.x.abs().max(0.0001) * 0.5;
            let hh = size.y.abs().max(0.0001) * 0.5;
            let hd = size.z.abs().max(0.0001) * 0.5;
            let points = [
                Vec3::new(-hw, -hh, -hd),
                Vec3::new(hw, -hh, -hd),
                Vec3::new(hw, -hh, hd),
                Vec3::new(-hw, -hh, hd),
                Vec3::new(0.0, hh, 0.0),
            ];
            let edges = [
                (0usize, 1usize),
                (1, 2),
                (2, 3),
                (3, 0),
                (0, 4),
                (1, 4),
                (2, 4),
                (3, 4),
            ];
            push_indexed_edges(&mut out, &points, &edges);
        }
        Shape3D::TriMesh { .. } => {}
    }
    out
}

pub(super) fn push_indexed_edges(
    out: &mut Vec<(Vec3, Vec3)>,
    points: &[Vec3],
    edges: &[(usize, usize)],
) {
    for (a, b) in edges.iter().copied() {
        if let (Some(pa), Some(pb)) = (points.get(a), points.get(b)) {
            out.push((*pa, *pb));
        }
    }
}

pub(super) fn append_circle_segments(
    out: &mut Vec<(Vec3, Vec3)>,
    center: Vec3,
    axis_u: Vec3,
    axis_v: Vec3,
    segments: usize,
) {
    if segments < 3 {
        return;
    }
    let mut prev = center + axis_u;
    for i in 1..=segments {
        let t = i as f32 / segments as f32;
        let a = std::f32::consts::TAU * t;
        let p = center + axis_u * a.cos() + axis_v * a.sin();
        out.push((prev, p));
        prev = p;
    }
}

pub(super) fn append_arc_segments(
    out: &mut Vec<(Vec3, Vec3)>,
    center: Vec3,
    axis_u: Vec3,
    axis_v: Vec3,
    arc_radians: f32,
    segments: usize,
) {
    if segments == 0 {
        return;
    }
    let mut prev = center + axis_u;
    for i in 1..=segments {
        let t = i as f32 / segments as f32;
        let a = arc_radians * t;
        let p = center + axis_u * a.cos() + axis_v * a.sin();
        out.push((prev, p));
        prev = p;
    }
}

pub(crate) fn derived_particle_budget(spawn_rate: f32, lifetime_max: f32) -> u32 {
    if spawn_rate <= 0.0 || lifetime_max <= 0.0 {
        return 1;
    }
    let budget = (spawn_rate * lifetime_max).ceil() as u32 + 2;
    budget.clamp(1, 1_000_000)
}

pub(super) fn dense_instance_signature(instances: &[perro_nodes::MultiMeshInstancePose]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64 ^ instances.len() as u64;
    for instance in instances {
        let position = instance.transform.position;
        let scale = instance.transform.scale;
        let rotation = instance.transform.rotation;
        hash = fnv_mix_u32(hash, position.x.to_bits());
        hash = fnv_mix_u32(hash, position.y.to_bits());
        hash = fnv_mix_u32(hash, position.z.to_bits());
        hash = fnv_mix_u32(hash, scale.x.to_bits());
        hash = fnv_mix_u32(hash, scale.y.to_bits());
        hash = fnv_mix_u32(hash, scale.z.to_bits());
        hash = fnv_mix_u32(hash, rotation.x.to_bits());
        hash = fnv_mix_u32(hash, rotation.y.to_bits());
        hash = fnv_mix_u32(hash, rotation.z.to_bits());
        hash = fnv_mix_u32(hash, rotation.w.to_bits());
        match &instance.blend_shape_weights {
            Some(weights) => {
                hash = fnv_mix_u32(hash, weights.len() as u32);
                for weight in weights {
                    hash = fnv_mix_u32(hash, weight.to_bits());
                }
            }
            None => hash = fnv_mix_u32(hash, u32::MAX),
        }
    }
    hash
}

#[inline]
pub(super) fn fnv_mix_u32(hash: u64, value: u32) -> u64 {
    (hash ^ value as u64).wrapping_mul(0x100000001b3)
}

pub(crate) fn resolve_particle_sim_mode(
    override_mode: ParticleEmitterSimMode3D,
    default_mode: perro_project::ParticleSimDefault,
) -> ParticleSimulationMode3D {
    match override_mode {
        ParticleEmitterSimMode3D::Default => match default_mode {
            perro_project::ParticleSimDefault::Cpu => ParticleSimulationMode3D::Cpu,
            perro_project::ParticleSimDefault::GpuVertex => ParticleSimulationMode3D::GpuVertex,
            perro_project::ParticleSimDefault::GpuCompute => ParticleSimulationMode3D::GpuCompute,
        },
        ParticleEmitterSimMode3D::Cpu => ParticleSimulationMode3D::Cpu,
        ParticleEmitterSimMode3D::GpuVertex => ParticleSimulationMode3D::GpuVertex,
        ParticleEmitterSimMode3D::GpuCompute => ParticleSimulationMode3D::GpuCompute,
    }
}

pub(crate) fn resolve_particle_render_mode(mode: ParticleType) -> ParticleRenderMode3D {
    match mode {
        ParticleType::Point => ParticleRenderMode3D::Point,
        ParticleType::Billboard => ParticleRenderMode3D::Billboard,
    }
}

pub(super) fn quaternion_forward(rotation: perro_structs::Quaternion) -> [f32; 3] {
    // Use glam's SIMD quaternion-vector rotate path.
    let q = Quat::from_xyzw(rotation.x, rotation.y, rotation.z, rotation.w);
    let q = if q.is_finite() && q.length_squared() > 1.0e-6 {
        q.normalize()
    } else {
        Quat::IDENTITY
    };
    let forward = q * Vec3::NEG_Z;
    [forward.x, forward.y, forward.z]
}

pub(crate) fn resolve_particle_profile(
    runtime: &mut Runtime,
    source: &str,
) -> Option<ParticleProfile3D> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    let source_key = particle_profile_source_key(source);
    while let Ok((loaded_source, profile)) = runtime.render_3d.particle_path_load_rx.try_recv() {
        let loaded_key = particle_profile_source_key(&loaded_source);
        runtime
            .render_3d
            .pending_particle_path_loads
            .remove(&loaded_key);
        if let Some(profile) = profile {
            cache_particle_profile(runtime, loaded_key, profile);
        }
    }
    if let Some(path) = runtime.render_3d.particle_path_cache.get(&source_key) {
        return Some(path.clone());
    }
    let parsed = if runtime.provider_mode() == crate::runtime_project::ProviderMode::Static {
        if let Some(inline) = source.strip_prefix("inline://") {
            parse_pparticle_source(inline)?
        } else if let Some(lookup) = runtime
            .project()
            .and_then(|project| project.static_particle_lookup)
        {
            let source_hash =
                parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
            lookup(source_hash).clone()
        } else if runtime
            .render_3d
            .pending_particle_path_loads
            .insert(source_key)
        {
            spawn_particle_profile_load(
                source.to_string(),
                runtime.render_3d.particle_path_load_tx.clone(),
            );
            return None;
        } else {
            return None;
        }
    } else if let Some(inline) = source.strip_prefix("inline://") {
        parse_pparticle_source(inline)?
    } else if runtime
        .render_3d
        .pending_particle_path_loads
        .insert(source_key)
    {
        spawn_particle_profile_load(
            source.to_string(),
            runtime.render_3d.particle_path_load_tx.clone(),
        );
        return None;
    } else {
        return None;
    };
    cache_particle_profile(runtime, source_key, parsed.clone());
    Some(parsed)
}

fn particle_profile_source_key(source: &str) -> u64 {
    parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source))
}

fn cache_particle_profile(runtime: &mut Runtime, source_key: u64, parsed: ParticleProfile3D) {
    if !runtime
        .render_3d
        .particle_path_cache
        .contains_key(&source_key)
    {
        while runtime.render_3d.particle_path_cache.len() >= PARTICLE_PATH_CACHE_MAX {
            let Some(evict_key) = runtime.render_3d.particle_path_cache_order.pop_front() else {
                break;
            };
            runtime.render_3d.particle_path_cache.remove(&evict_key);
        }
        runtime
            .render_3d
            .particle_path_cache_order
            .push_back(source_key);
    }
    runtime
        .render_3d
        .particle_path_cache
        .insert(source_key, parsed);
}

fn spawn_particle_profile_load(
    source: String,
    tx: std::sync::mpsc::Sender<(String, Option<ParticleProfile3D>)>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    rayon::spawn(move || {
        let profile = perro_io::load_asset(source.as_str())
            .ok()
            .and_then(|bytes| {
                std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(parse_pparticle_source)
            });
        let _ = tx.send((source, profile));
    });
    #[cfg(target_arch = "wasm32")]
    {
        let profile = perro_io::load_asset(source.as_str())
            .ok()
            .and_then(|bytes| {
                std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(parse_pparticle_source)
            });
        let _ = tx.send((source, profile));
    }
}

pub(super) fn parse_pparticle_source(source: &str) -> Option<ParticleProfile3D> {
    let mut profile = ParticleProfile3D::default();
    let mut preset: Option<String> = None;
    let mut preset_param_a = 1.0f32;
    let mut preset_param_b = 1.0f32;
    let mut preset_param_c = 0.0f32;
    let mut preset_param_d = 0.0f32;
    let mut expr_x = String::from("0.0");
    let mut expr_y = String::from("0.0");
    let mut expr_z = String::from("0.0");
    let mut has_expr_x = false;
    let mut has_expr_y = false;
    let mut has_expr_z = false;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let (key, value) = line.split_once('=')?;
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "preset" => {
                preset = Some(value.to_ascii_lowercase());
            }
            "preset_param_a" => {
                preset_param_a = value.parse::<f32>().ok().unwrap_or(preset_param_a);
            }
            "preset_param_b" => {
                preset_param_b = value.parse::<f32>().ok().unwrap_or(preset_param_b);
            }
            "preset_param_c" => {
                preset_param_c = value.parse::<f32>().ok().unwrap_or(preset_param_c);
            }
            "preset_param_d" => {
                preset_param_d = value.parse::<f32>().ok().unwrap_or(preset_param_d);
            }
            "x" => expr_x = value.to_string(),
            "y" => expr_y = value.to_string(),
            "z" => expr_z = value.to_string(),
            "force" => {
                if let Some(v) = parse_vec3_literal(value) {
                    profile.force = v;
                }
            }
            "force_x" => {
                let v = value.parse::<f32>().ok()?;
                profile.force[0] = v;
            }
            "force_y" => {
                let v = value.parse::<f32>().ok()?;
                profile.force[1] = v;
            }
            "force_z" => {
                let v = value.parse::<f32>().ok()?;
                profile.force[2] = v;
            }
            "lifetime_min" => {
                profile.lifetime_min = value.parse::<f32>().ok().unwrap_or(profile.lifetime_min);
            }
            "lifetime_max" => {
                profile.lifetime_max = value.parse::<f32>().ok().unwrap_or(profile.lifetime_max);
            }
            "speed_min" => {
                profile.speed_min = value.parse::<f32>().ok().unwrap_or(profile.speed_min);
            }
            "speed_max" => {
                profile.speed_max = value.parse::<f32>().ok().unwrap_or(profile.speed_max);
            }
            "spread_radians" => {
                profile.spread_radians =
                    value.parse::<f32>().ok().unwrap_or(profile.spread_radians);
            }
            "size" => {
                profile.size = value.parse::<f32>().ok().unwrap_or(profile.size);
            }
            "size_min" => {
                profile.size_min = value.parse::<f32>().ok().unwrap_or(profile.size_min);
            }
            "size_max" => {
                profile.size_max = value.parse::<f32>().ok().unwrap_or(profile.size_max);
            }
            "color_start" => {
                if let Some(v) = parse_vec4_literal(value) {
                    profile.color_start = v.into();
                }
            }
            "color_end" => {
                if let Some(v) = parse_vec4_literal(value) {
                    profile.color_end = v.into();
                }
            }
            "emissive" => {
                if let Some(v) = parse_vec3_literal(value) {
                    profile.emissive = v;
                }
            }
            "spin" => {
                profile.spin_angular_velocity = value
                    .parse::<f32>()
                    .ok()
                    .unwrap_or(profile.spin_angular_velocity);
            }
            _ => {}
        }
        match key.as_str() {
            "x" => has_expr_x = true,
            "y" => has_expr_y = true,
            "z" => has_expr_z = true,
            _ => {}
        }
    }
    profile.path = match preset.as_deref() {
        None => ParticlePath3D::None,
        Some("ballistic") => ParticlePath3D::Ballistic,
        Some("spiral") => ParticlePath3D::Spiral {
            angular_velocity: preset_param_a,
            radius: preset_param_b.abs(),
        },
        Some("orbit_y") => ParticlePath3D::OrbitY {
            angular_velocity: preset_param_a,
            radius: preset_param_b.abs(),
        },
        Some("noise_drift") => ParticlePath3D::NoiseDrift {
            amplitude: preset_param_a.abs(),
            frequency: preset_param_b.abs(),
        },
        Some("flat_disk") => ParticlePath3D::FlatDisk {
            radius: preset_param_a.abs(),
        },
        Some(_) => ParticlePath3D::None,
    };
    let _ = (preset_param_c, preset_param_d);
    if has_expr_x || has_expr_y || has_expr_z {
        profile.expr_x_ops = Some(Cow::Owned(compile_expression(&expr_x).ok()?.ops().to_vec()));
        profile.expr_y_ops = Some(Cow::Owned(compile_expression(&expr_y).ok()?.ops().to_vec()));
        profile.expr_z_ops = Some(Cow::Owned(compile_expression(&expr_z).ok()?.ops().to_vec()));
    }
    Some(profile)
}

pub(super) fn parse_vec3_literal(raw: &str) -> Option<[f32; 3]> {
    let raw = raw.trim();
    let inner = raw.strip_prefix('(')?.strip_suffix(')')?;
    let mut it = inner.split(',').map(|v| v.trim().parse::<f32>().ok());
    Some([it.next()??, it.next()??, it.next()??])
}

pub(super) fn parse_vec4_literal(raw: &str) -> Option<[f32; 4]> {
    let raw = raw.trim();
    let inner = raw.strip_prefix('(')?.strip_suffix(')')?;
    let mut it = inner.split(',').map(|v| v.trim().parse::<f32>().ok());
    Some([it.next()??, it.next()??, it.next()??, it.next()??])
}

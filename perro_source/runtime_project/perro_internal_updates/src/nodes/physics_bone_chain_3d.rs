use crate::prelude::*;
use glam::{Mat4, Vec3};
use perro_nodes::{
    BoneCollider3D, CollisionShape3D, NodeType, PhysicsBoneChain3D, Shape3D, Skeleton3D,
};
use perro_runtime_context::perro_structs::{Transform3D, Vector3};

pub fn internal_fixed_update<RT>(ctx: &mut RuntimeWindow<'_, RT>, id: NodeID)
where
    RT: RuntimeAPI + ?Sized,
{
    let Some(cfg) = with_base_node!(ctx, PhysicsBoneChain3D, id, |node| ChainCfg {
        skeleton: node.skeleton,
        bone_index: node.bone_index,
        chain_length: node.chain_length as usize,
        enabled: node.enabled,
        gravity: node.gravity,
        damping: node.damping.clamp(0.0, 1.0),
        stiffness: node.stiffness.clamp(0.0, 1.0),
        radius: node.radius.max(0.0),
        collisions: node.collisions,
        iterations: node.iterations.max(1) as usize,
    }) else {
        return;
    };
    if !cfg.enabled || cfg.skeleton.is_nil() || cfg.bone_index < 0 || cfg.chain_length == 0 {
        return;
    }

    let Some((bones_len, chain, rest_globals)) =
        with_base_node!(ctx, Skeleton3D, cfg.skeleton, |skeleton| {
            let chain = collect_chain(skeleton, cfg.bone_index as usize, cfg.chain_length);
            let rest_globals = chain_global_positions(skeleton, &chain);
            (skeleton.bones.len(), chain, rest_globals)
        })
    else {
        return;
    };
    if chain.len() < 2 || rest_globals.len() != chain.len() || bones_len == 0 {
        return;
    }

    let skeleton_global = ctx
        .Nodes()
        .get_global_transform_3d(cfg.skeleton)
        .unwrap_or(Transform3D::IDENTITY)
        .to_mat4();
    let rest_world = rest_globals
        .iter()
        .map(|p| skeleton_global.transform_point3(*p).into())
        .collect::<Vec<Vector3>>();
    let lengths = rest_world
        .windows(2)
        .map(|pair| pair[0].distance_to(pair[1]).max(0.0001))
        .collect::<Vec<_>>();
    let colliders = if cfg.collisions {
        collect_colliders(ctx)
    } else {
        Vec::new()
    };
    let dt = fixed_delta_time!(ctx).clamp(0.0001, 0.05);

    let Some(positions) = with_base_node_mut!(ctx, PhysicsBoneChain3D, id, |node| {
        step_chain(node, &chain, &rest_world, &lengths, &colliders, cfg, dt)
    }) else {
        return;
    };

    let skeleton_from_world = skeleton_global.inverse();
    let local_positions = positions
        .iter()
        .map(|p| skeleton_from_world.transform_point3((*p).into()))
        .collect::<Vec<Vec3>>();
    let changed = with_base_node_mut!(ctx, Skeleton3D, cfg.skeleton, |skeleton| {
        write_chain_positions(skeleton, &chain, &local_positions);
    });
    if changed.is_some() {
        let _ = ctx.Nodes().force_rerender(cfg.skeleton);
    }
}

#[derive(Clone, Copy)]
struct ChainCfg {
    skeleton: NodeID,
    bone_index: i32,
    chain_length: usize,
    enabled: bool,
    gravity: Vector3,
    damping: f32,
    stiffness: f32,
    radius: f32,
    collisions: bool,
    iterations: usize,
}

#[derive(Clone)]
struct Collider {
    world: Transform3D,
    shape: Shape3D,
}

fn collect_chain(skeleton: &Skeleton3D, end: usize, max_len: usize) -> Vec<usize> {
    if end >= skeleton.bones.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut current = end as i32;
    let mut hops = 0usize;
    while current >= 0 && hops < skeleton.bones.len() && out.len() < max_len.saturating_add(1) {
        let index = current as usize;
        if index >= skeleton.bones.len() {
            break;
        }
        out.push(index);
        current = skeleton.bones[index].parent;
        hops += 1;
    }
    out.reverse();
    out
}

fn chain_global_positions(skeleton: &Skeleton3D, chain: &[usize]) -> Vec<Vec3> {
    let mut out = Vec::with_capacity(chain.len());
    let mut globals = vec![Mat4::IDENTITY; skeleton.bones.len()];
    for (index, bone) in skeleton.bones.iter().enumerate() {
        let local = bone.pose.to_mat4();
        globals[index] = if bone.parent >= 0 {
            globals
                .get(bone.parent as usize)
                .copied()
                .unwrap_or(Mat4::IDENTITY)
                * local
        } else {
            local
        };
    }
    for bone in chain {
        out.push(globals[*bone].transform_point3(Vec3::ZERO));
    }
    out
}

fn collect_colliders<RT>(ctx: &mut RuntimeWindow<'_, RT>) -> Vec<Collider>
where
    RT: RuntimeAPI + ?Sized,
{
    let ids = ctx
        .Nodes()
        .query(TagQuery::new().is_node_types([NodeType::BoneCollider3D]));
    let mut out = Vec::new();
    for id in ids {
        let enabled =
            with_base_node!(ctx, BoneCollider3D, id, |node| node.enabled).unwrap_or(false);
        if !enabled {
            continue;
        }
        let Some(collider_world) = ctx.Nodes().get_global_transform_3d(id) else {
            continue;
        };
        for child in ctx.Nodes().get_children(id) {
            let Some((shape_local, shape)) =
                with_base_node!(ctx, CollisionShape3D, child, |shape| {
                    (shape.transform, shape.shape.clone())
                })
            else {
                continue;
            };
            let world = Transform3D::from_mat4(collider_world.to_mat4() * shape_local.to_mat4());
            out.push(Collider { world, shape });
        }
    }
    out
}

fn step_chain(
    node: &mut PhysicsBoneChain3D,
    chain: &[usize],
    rest_world: &[Vector3],
    lengths: &[f32],
    colliders: &[Collider],
    cfg: ChainCfg,
    dt: f32,
) -> Vec<Vector3> {
    let reset = node.internal_bones != chain
        || node.internal_positions.len() != chain.len()
        || node.internal_prev_positions.len() != chain.len();
    if reset {
        node.internal_bones = chain.to_vec();
        node.internal_positions = rest_world.to_vec();
        node.internal_prev_positions = rest_world.to_vec();
    }

    node.internal_positions[0] = rest_world[0];
    node.internal_prev_positions[0] = rest_world[0];
    let damping = (1.0 - cfg.damping).powf(dt * 60.0);
    let stiffness = 1.0 - (1.0 - cfg.stiffness).powf(dt * 60.0);
    for (i, rest) in rest_world
        .iter()
        .enumerate()
        .take(node.internal_positions.len())
        .skip(1)
    {
        let pos = node.internal_positions[i];
        let prev = node.internal_prev_positions[i];
        let velocity = (pos - prev) * damping;
        node.internal_prev_positions[i] = pos;
        node.internal_positions[i] = pos + velocity + cfg.gravity * (dt * dt);
        node.internal_positions[i] = node.internal_positions[i].lerped(*rest, stiffness);
    }

    for _ in 0..cfg.iterations {
        node.internal_positions[0] = rest_world[0];
        solve_lengths_forward(&mut node.internal_positions, lengths);
        collide_positions(&mut node.internal_positions, cfg.radius, colliders);
        solve_lengths_backward(&mut node.internal_positions, lengths);
        node.internal_positions[0] = rest_world[0];
        solve_lengths_forward(&mut node.internal_positions, lengths);
        collide_positions(&mut node.internal_positions, cfg.radius, colliders);
    }

    for i in 1..node.internal_positions.len() {
        let delta = node.internal_positions[i] - node.internal_prev_positions[i];
        let max_step = lengths.get(i - 1).copied().unwrap_or(1.0).max(0.0001) * 2.0;
        if delta.length() > max_step {
            node.internal_prev_positions[i] =
                node.internal_positions[i] - delta.normalized() * max_step;
        }
    }

    node.internal_positions.clone()
}

fn solve_lengths_forward(positions: &mut [Vector3], lengths: &[f32]) {
    for i in 1..positions.len() {
        let parent = positions[i - 1];
        let current = positions[i];
        let dir = parent.direction_to(current);
        positions[i] = parent + dir * lengths[i - 1];
    }
}

fn solve_lengths_backward(positions: &mut [Vector3], lengths: &[f32]) {
    if positions.len() < 2 {
        return;
    }
    for i in (0..positions.len() - 1).rev() {
        let child = positions[i + 1];
        let current = positions[i];
        let dir = child.direction_to(current);
        positions[i] = child + dir * lengths[i];
    }
}

fn collide_positions(positions: &mut [Vector3], radius: f32, colliders: &[Collider]) {
    for pos in positions.iter_mut().skip(1) {
        for collider in colliders {
            *pos = collide_point(*pos, radius, collider);
        }
    }
}

fn collide_point(point: Vector3, radius: f32, collider: &Collider) -> Vector3 {
    match &collider.shape {
        Shape3D::Sphere {
            radius: shape_radius,
        } => collide_sphere(point, radius, collider.world, *shape_radius),
        Shape3D::Cube { size } => collide_cube(point, radius, collider.world, *size),
        Shape3D::Capsule {
            radius: shape_radius,
            half_height,
        } => collide_capsule(point, radius, collider.world, *shape_radius, *half_height),
        Shape3D::Cylinder {
            radius: shape_radius,
            half_height,
        } => collide_cylinder(point, radius, collider.world, *shape_radius, *half_height),
        Shape3D::Cone {
            radius: shape_radius,
            half_height,
        } => collide_cone(point, radius, collider.world, *shape_radius, *half_height),
        Shape3D::TriPrism { size } => collide_tri_prism(point, radius, collider.world, *size),
        Shape3D::TriangularPyramid { size } => {
            collide_triangular_pyramid(point, radius, collider.world, *size)
        }
        Shape3D::SquarePyramid { size } => {
            collide_square_pyramid(point, radius, collider.world, *size)
        }
        Shape3D::TriMesh { .. } => collide_sphere(point, radius, collider.world, 0.5),
    }
}

fn collide_sphere(point: Vector3, radius: f32, world: Transform3D, shape_radius: f32) -> Vector3 {
    let center = world.position;
    let scale = max_abs_component(world.scale);
    let r = shape_radius.abs() * scale + radius;
    let delta = point - center;
    let len = delta.length();
    if len > 0.0001 && len < r {
        center + delta / len * r
    } else {
        point
    }
}

fn collide_cube(point: Vector3, radius: f32, world: Transform3D, size: Vector3) -> Vector3 {
    let inv = world.to_mat4().inverse();
    let local: Vector3 = inv.transform_point3(point.into()).into();
    let half = Vector3::new(
        size.x.abs() * 0.5 + radius,
        size.y.abs() * 0.5 + radius,
        size.z.abs() * 0.5 + radius,
    );
    if local.x.abs() > half.x || local.y.abs() > half.y || local.z.abs() > half.z {
        return point;
    }
    let dx = half.x - local.x.abs();
    let dy = half.y - local.y.abs();
    let dz = half.z - local.z.abs();
    let mut pushed = local;
    if dx <= dy && dx <= dz {
        pushed.x = half.x.copysign(local.x);
    } else if dy <= dz {
        pushed.y = half.y.copysign(local.y);
    } else {
        pushed.z = half.z.copysign(local.z);
    }
    world.to_mat4().transform_point3(pushed.into()).into()
}

fn collide_capsule(
    point: Vector3,
    radius: f32,
    world: Transform3D,
    shape_radius: f32,
    half_height: f32,
) -> Vector3 {
    let inv = world.to_mat4().inverse();
    let local: Vector3 = inv.transform_point3(point.into()).into();
    let scale = max_abs_component(world.scale).max(0.0001);
    let local_probe_radius = radius / scale;
    let a = Vector3::new(0.0, -half_height.abs(), 0.0);
    let b = Vector3::new(0.0, half_height.abs(), 0.0);
    let nearest = closest_point_on_segment(local, a, b);
    push_from_nearest(
        point,
        local,
        nearest,
        shape_radius.abs() + local_probe_radius,
        world,
    )
}

fn collide_cylinder(
    point: Vector3,
    radius: f32,
    world: Transform3D,
    shape_radius: f32,
    half_height: f32,
) -> Vector3 {
    let inv = world.to_mat4().inverse();
    let local: Vector3 = inv.transform_point3(point.into()).into();
    let scale = max_abs_component(world.scale).max(0.0001);
    let probe = radius / scale;
    let r = shape_radius.abs() + probe;
    let h = half_height.abs() + probe;
    if local.y.abs() > h {
        return point;
    }
    let xz = Vector3::new(local.x, 0.0, local.z);
    let len = xz.length();
    if len > r {
        return point;
    }
    let side_depth = r - len;
    let cap_depth = h - local.y.abs();
    let mut pushed = local;
    if side_depth <= cap_depth && len > 0.0001 {
        pushed.x = local.x / len * r;
        pushed.z = local.z / len * r;
    } else {
        pushed.y = h.copysign(local.y);
    }
    world.to_mat4().transform_point3(pushed.into()).into()
}

fn collide_cone(
    point: Vector3,
    radius: f32,
    world: Transform3D,
    shape_radius: f32,
    half_height: f32,
) -> Vector3 {
    let inv = world.to_mat4().inverse();
    let local: Vector3 = inv.transform_point3(point.into()).into();
    let scale = max_abs_component(world.scale).max(0.0001);
    let probe = radius / scale;
    let h = half_height.abs().max(0.0001);
    if local.y < -h - probe || local.y > h + probe {
        return point;
    }
    let t = ((h - local.y) / (2.0 * h)).clamp(0.0, 1.0);
    let allowed = shape_radius.abs() * t + probe;
    let xz = Vector3::new(local.x, 0.0, local.z);
    let len = xz.length();
    if len > allowed {
        return point;
    }
    let mut pushed = local;
    if len > 0.0001 {
        pushed.x = local.x / len * allowed;
        pushed.z = local.z / len * allowed;
    } else {
        pushed.x = allowed;
    }
    world.to_mat4().transform_point3(pushed.into()).into()
}

fn collide_tri_prism(point: Vector3, radius: f32, world: Transform3D, size: Vector3) -> Vector3 {
    collide_poly_shape(point, radius, world, tri_prism_faces(size))
}

fn collide_triangular_pyramid(
    point: Vector3,
    radius: f32,
    world: Transform3D,
    size: Vector3,
) -> Vector3 {
    collide_poly_shape(point, radius, world, triangular_pyramid_faces(size))
}

fn collide_square_pyramid(
    point: Vector3,
    radius: f32,
    world: Transform3D,
    size: Vector3,
) -> Vector3 {
    collide_poly_shape(point, radius, world, square_pyramid_faces(size))
}

fn collide_poly_shape(
    point: Vector3,
    radius: f32,
    world: Transform3D,
    faces: Vec<[Vector3; 3]>,
) -> Vector3 {
    let inv = world.to_mat4().inverse();
    let local: Vector3 = inv.transform_point3(point.into()).into();
    let scale = max_abs_component(world.scale).max(0.0001);
    let probe = radius / scale;
    let mut best_dist = f32::NEG_INFINITY;
    let mut best_normal = Vector3::ZERO;
    for face in faces {
        let normal = (face[1] - face[0]).cross(face[2] - face[0]).normalized();
        if normal.length_squared() <= f32::EPSILON {
            continue;
        }
        let dist = (local - face[0]).dot(normal);
        if dist > probe {
            return point;
        }
        if dist > best_dist {
            best_dist = dist;
            best_normal = normal;
        }
    }
    if best_dist.is_finite() && best_normal.length_squared() > f32::EPSILON {
        let pushed = local + best_normal * (probe - best_dist);
        world.to_mat4().transform_point3(pushed.into()).into()
    } else {
        point
    }
}

fn closest_point_on_segment(p: Vector3, a: Vector3, b: Vector3) -> Vector3 {
    let ab = b - a;
    let denom = ab.length_squared();
    if denom <= f32::EPSILON {
        return a;
    }
    let t = (p - a).dot(ab) / denom;
    a + ab * t.clamp(0.0, 1.0)
}

fn push_from_nearest(
    world_point: Vector3,
    local: Vector3,
    nearest: Vector3,
    combined_radius: f32,
    world: Transform3D,
) -> Vector3 {
    let delta = local - nearest;
    let len = delta.length();
    if len > 0.0001 && len < combined_radius {
        let pushed = nearest + delta / len * combined_radius;
        world.to_mat4().transform_point3(pushed.into()).into()
    } else {
        world_point
    }
}

fn max_abs_component(v: Vector3) -> f32 {
    v.x.abs().max(v.y.abs()).max(v.z.abs())
}

fn tri_prism_faces(size: Vector3) -> Vec<[Vector3; 3]> {
    let hw = size.x.abs().max(0.0001) * 0.5;
    let hh = size.y.abs().max(0.0001) * 0.5;
    let hd = size.z.abs().max(0.0001) * 0.5;
    let p = [
        Vector3::new(-hw, -hh, -hd),
        Vector3::new(hw, -hh, -hd),
        Vector3::new(0.0, hh, -hd),
        Vector3::new(-hw, -hh, hd),
        Vector3::new(hw, -hh, hd),
        Vector3::new(0.0, hh, hd),
    ];
    vec![
        [p[0], p[2], p[1]],
        [p[3], p[4], p[5]],
        [p[0], p[1], p[4]],
        [p[0], p[4], p[3]],
        [p[1], p[2], p[5]],
        [p[1], p[5], p[4]],
        [p[2], p[0], p[3]],
        [p[2], p[3], p[5]],
    ]
}

fn triangular_pyramid_faces(size: Vector3) -> Vec<[Vector3; 3]> {
    let hw = size.x.abs().max(0.0001) * 0.5;
    let hh = size.y.abs().max(0.0001) * 0.5;
    let hd = size.z.abs().max(0.0001) * 0.5;
    let p = [
        Vector3::new(-hw, -hh, -hd),
        Vector3::new(hw, -hh, -hd),
        Vector3::new(0.0, -hh, hd),
        Vector3::new(0.0, hh, 0.0),
    ];
    vec![
        [p[0], p[2], p[1]],
        [p[0], p[1], p[3]],
        [p[1], p[2], p[3]],
        [p[2], p[0], p[3]],
    ]
}

fn square_pyramid_faces(size: Vector3) -> Vec<[Vector3; 3]> {
    let hw = size.x.abs().max(0.0001) * 0.5;
    let hh = size.y.abs().max(0.0001) * 0.5;
    let hd = size.z.abs().max(0.0001) * 0.5;
    let p = [
        Vector3::new(-hw, -hh, -hd),
        Vector3::new(hw, -hh, -hd),
        Vector3::new(hw, -hh, hd),
        Vector3::new(-hw, -hh, hd),
        Vector3::new(0.0, hh, 0.0),
    ];
    vec![
        [p[0], p[3], p[2]],
        [p[0], p[2], p[1]],
        [p[0], p[1], p[4]],
        [p[1], p[2], p[4]],
        [p[2], p[3], p[4]],
        [p[3], p[0], p[4]],
    ]
}

fn write_chain_positions(skeleton: &mut Skeleton3D, chain: &[usize], local_positions: &[Vec3]) {
    let mut parent_globals = vec![Mat4::IDENTITY; skeleton.bones.len()];
    for (chain_pos, bone_index) in chain.iter().copied().enumerate() {
        if chain_pos == 0 {
            parent_globals[bone_index] = skeleton.bones[bone_index].pose.to_mat4();
            continue;
        }
        let parent_global = parent_globals[chain[chain_pos - 1]];
        let local = parent_global
            .inverse()
            .transform_point3(local_positions[chain_pos]);
        skeleton.bones[bone_index].pose.position = local.into();
        parent_globals[bone_index] = parent_global * skeleton.bones[bone_index].pose.to_mat4();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_nodes::skeleton_3d::Bone3D;
    use std::borrow::Cow;

    fn bone(parent: i32, y: f32) -> Bone3D {
        Bone3D {
            name: Cow::Borrowed("b"),
            parent,
            rest: Transform3D {
                position: Vector3::new(0.0, y, 0.0),
                ..Transform3D::IDENTITY
            },
            pose: Transform3D {
                position: Vector3::new(0.0, y, 0.0),
                ..Transform3D::IDENTITY
            },
            inv_bind: Transform3D::IDENTITY,
        }
    }

    #[test]
    fn collect_chain_caps_length() {
        let mut skeleton = Skeleton3D::new();
        skeleton.bones = vec![bone(-1, 0.0), bone(0, 1.0), bone(1, 1.0), bone(2, 1.0)];
        assert_eq!(collect_chain(&skeleton, 3, 2), vec![1, 2, 3]);
    }

    #[test]
    fn sphere_collision_pushes_point_out() {
        let collider = Collider {
            world: Transform3D::IDENTITY,
            shape: Shape3D::Sphere { radius: 1.0 },
        };
        let out = collide_point(Vector3::new(0.0, 0.5, 0.0), 0.1, &collider);
        assert!(out.length() >= 1.099);
    }

    #[test]
    fn forward_backward_solve_keeps_segment_lengths() {
        let mut positions = vec![
            Vector3::ZERO,
            Vector3::new(0.3, -0.2, 0.0),
            Vector3::new(0.8, -0.9, 0.0),
        ];
        let lengths = vec![1.0, 1.0];
        solve_lengths_forward(&mut positions, &lengths);
        solve_lengths_backward(&mut positions, &lengths);
        solve_lengths_forward(&mut positions, &lengths);
        assert!((positions[0].distance_to(positions[1]) - 1.0).abs() < 0.001);
        assert!((positions[1].distance_to(positions[2]) - 1.0).abs() < 0.001);
    }

    #[test]
    fn capsule_collision_pushes_point_out() {
        let collider = Collider {
            world: Transform3D::IDENTITY,
            shape: Shape3D::Capsule {
                radius: 0.5,
                half_height: 1.0,
            },
        };
        let out = collide_point(Vector3::new(0.2, 0.0, 0.0), 0.1, &collider);
        assert!(out.x.abs() >= 0.599);
    }

    #[test]
    fn cylinder_collision_pushes_point_out() {
        let collider = Collider {
            world: Transform3D::IDENTITY,
            shape: Shape3D::Cylinder {
                radius: 0.5,
                half_height: 1.0,
            },
        };
        let out = collide_point(Vector3::new(0.2, 0.0, 0.0), 0.1, &collider);
        assert!(out.x.abs() >= 0.599);
    }
}

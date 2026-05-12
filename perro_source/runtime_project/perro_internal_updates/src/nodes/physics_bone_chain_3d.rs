use crate::prelude::*;
use glam::{Mat4, Vec3};
use perro_nodes::{
    BoneCollider3D, CollisionShape3D, NodeType, PhysicsBoneChain3D, Shape3D, Skeleton3D,
};
use perro_runtime_api::perro_structs::{Transform3D, Vector3};
use std::cell::RefCell;

thread_local! {
    static COLLIDER_SCRATCH_3D: RefCell<Vec<Collider>> = const { RefCell::new(Vec::new()) };
}

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

    let Some((chain, rest_globals)) = with_base_node!(ctx, Skeleton3D, cfg.skeleton, |skeleton| {
        let chain = collect_chain(skeleton, cfg.bone_index as usize, cfg.chain_length);
        let rest_globals = chain_global_positions(skeleton, &chain);
        (chain, rest_globals)
    }) else {
        return;
    };
    if chain.len() < 2 || rest_globals.len() != chain.len() {
        return;
    }

    let skeleton_global = ctx
        .Nodes()
        .get_global_transform_3d(cfg.skeleton)
        .unwrap_or(Transform3D::IDENTITY)
        .to_mat4();
    let dt = fixed_delta_time!(ctx).clamp(0.0001, 0.05);

    if cfg.collisions {
        COLLIDER_SCRATCH_3D.with(|scratch| {
            let mut colliders = scratch.borrow_mut();
            collect_colliders(ctx, &mut colliders);
            update_chain_with_colliders(
                ctx,
                id,
                ChainUpdate {
                    cfg,
                    skeleton_global,
                    chain: &chain,
                    rest_globals: &rest_globals,
                    colliders: &colliders,
                    dt,
                },
            );
        });
    } else {
        update_chain_with_colliders(
            ctx,
            id,
            ChainUpdate {
                cfg,
                skeleton_global,
                chain: &chain,
                rest_globals: &rest_globals,
                colliders: &[],
                dt,
            },
        );
    }
}

struct ChainUpdate<'a> {
    cfg: ChainCfg,
    skeleton_global: Mat4,
    chain: &'a [usize],
    rest_globals: &'a [Vec3],
    colliders: &'a [Collider],
    dt: f32,
}

fn update_chain_with_colliders<RT>(
    ctx: &mut RuntimeWindow<'_, RT>,
    id: NodeID,
    update: ChainUpdate<'_>,
) where
    RT: RuntimeAPI + ?Sized,
{
    let ChainUpdate {
        cfg,
        skeleton_global,
        chain,
        rest_globals,
        colliders,
        dt,
    } = update;
    let Some(mut local_positions) = with_base_node_mut!(ctx, PhysicsBoneChain3D, id, |node| {
        let mut rest_world = std::mem::take(&mut node.internal_rest_world);
        rest_world.clear();
        rest_world.extend(
            rest_globals
                .iter()
                .map(|p| Vector3::from(skeleton_global.transform_point3(*p))),
        );
        let mut lengths = std::mem::take(&mut node.internal_lengths);
        lengths.clear();
        lengths.extend(
            rest_world
                .windows(2)
                .map(|pair| pair[0].distance_to(pair[1]).max(0.0001)),
        );
        step_chain(node, chain, &rest_world, &lengths, colliders, cfg, dt);

        let skeleton_from_world = skeleton_global.inverse();
        let mut local_positions = std::mem::take(&mut node.internal_local_positions);
        local_positions.clear();
        local_positions.extend(
            node.internal_positions
                .iter()
                .map(|p| Vector3::from(skeleton_from_world.transform_point3((*p).into()))),
        );
        node.internal_rest_world = rest_world;
        node.internal_lengths = lengths;
        local_positions
    }) else {
        return;
    };

    let changed = with_base_node_mut!(ctx, Skeleton3D, cfg.skeleton, |skeleton| {
        write_chain_positions(skeleton, chain, &local_positions);
    });
    if changed.is_some() {
        let _ = ctx.Nodes().force_rerender(cfg.skeleton);
    }
    let _ = with_base_node_mut!(ctx, PhysicsBoneChain3D, id, |node| {
        node.internal_local_positions = std::mem::take(&mut local_positions);
    });
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
    world_mat: Mat4,
    inv_world_mat: Mat4,
    max_scale: f32,
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
    let Some(first) = chain.first().copied() else {
        return Vec::new();
    };
    let mut ancestors = Vec::new();
    let mut current = first as i32;
    let mut hops = 0usize;
    while current >= 0 && hops < skeleton.bones.len() {
        let index = current as usize;
        if index >= skeleton.bones.len() {
            break;
        }
        ancestors.push(index);
        current = skeleton.bones[index].parent;
        hops += 1;
    }
    ancestors.reverse();

    let mut global = Mat4::IDENTITY;
    for bone in ancestors {
        global *= skeleton.bones[bone].pose.to_mat4();
    }

    let mut out = Vec::with_capacity(chain.len());
    out.push(global.transform_point3(Vec3::ZERO));
    for bone in chain.iter().copied().skip(1) {
        global *= skeleton.bones[bone].pose.to_mat4();
        out.push(global.transform_point3(Vec3::ZERO));
    }
    out
}

fn collect_colliders<RT>(ctx: &mut RuntimeWindow<'_, RT>, out: &mut Vec<Collider>)
where
    RT: RuntimeAPI + ?Sized,
{
    out.clear();
    let ids = ctx
        .Nodes()
        .query(TagQuery::new().is_node_types([NodeType::BoneCollider3D]));
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
            let world_mat = world.to_mat4();
            out.push(Collider {
                world,
                world_mat,
                inv_world_mat: world_mat.inverse(),
                max_scale: max_abs_component(world.scale),
                shape,
            });
        }
    }
}

#[inline(always)]
fn step_chain(
    node: &mut PhysicsBoneChain3D,
    chain: &[usize],
    rest_world: &[Vector3],
    lengths: &[f32],
    colliders: &[Collider],
    cfg: ChainCfg,
    dt: f32,
) {
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
    let step_scale = dt * 60.0;
    let (damping, stiffness) = if (step_scale - 1.0).abs() <= 1.0e-4 {
        (1.0 - cfg.damping, cfg.stiffness)
    } else {
        (
            (1.0 - cfg.damping).powf(step_scale),
            1.0 - (1.0 - cfg.stiffness).powf(step_scale),
        )
    };
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
        let sim = pos + velocity + cfg.gravity * (dt * dt);
        node.internal_positions[i] = sim + (*rest - sim) * stiffness;
    }

    if colliders.is_empty() {
        for _ in 0..cfg.iterations {
            node.internal_positions[0] = rest_world[0];
            solve_lengths_forward(&mut node.internal_positions, lengths);
            solve_lengths_backward(&mut node.internal_positions, lengths);
            node.internal_positions[0] = rest_world[0];
            solve_lengths_forward(&mut node.internal_positions, lengths);
        }
    } else {
        for _ in 0..cfg.iterations {
            node.internal_positions[0] = rest_world[0];
            solve_lengths_forward(&mut node.internal_positions, lengths);
            collide_positions(&mut node.internal_positions, cfg.radius, colliders);
            solve_lengths_backward(&mut node.internal_positions, lengths);
            node.internal_positions[0] = rest_world[0];
            solve_lengths_forward(&mut node.internal_positions, lengths);
            collide_positions(&mut node.internal_positions, cfg.radius, colliders);
        }
    }

    for i in 1..node.internal_positions.len() {
        let delta = node.internal_positions[i] - node.internal_prev_positions[i];
        let max_step = lengths.get(i - 1).copied().unwrap_or(1.0).max(0.0001) * 2.0;
        let len_sq = delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;
        if len_sq > max_step * max_step {
            node.internal_prev_positions[i] =
                node.internal_positions[i] - delta * (max_step * len_sq.sqrt().recip());
        }
    }
}

#[inline(always)]
fn solve_lengths_forward(positions: &mut [Vector3], lengths: &[f32]) {
    for i in 1..positions.len() {
        let parent = positions[i - 1];
        let current = positions[i];
        positions[i] = parent + normalized_delta_3d(current - parent) * lengths[i - 1];
    }
}

#[inline(always)]
fn solve_lengths_backward(positions: &mut [Vector3], lengths: &[f32]) {
    if positions.len() < 2 {
        return;
    }
    for i in (0..positions.len() - 1).rev() {
        let child = positions[i + 1];
        let current = positions[i];
        positions[i] = child + normalized_delta_3d(current - child) * lengths[i];
    }
}

#[inline(always)]
fn normalized_delta_3d(delta: Vector3) -> Vector3 {
    let len_sq = delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;
    if len_sq <= f32::EPSILON {
        Vector3::ZERO
    } else {
        delta * len_sq.sqrt().recip()
    }
}

#[inline(always)]
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
        } => collide_sphere(point, radius, collider, *shape_radius),
        Shape3D::Cube { size } => collide_cube(point, radius, collider, *size),
        Shape3D::Capsule {
            radius: shape_radius,
            half_height,
        } => collide_capsule(point, radius, collider, *shape_radius, *half_height),
        Shape3D::Cylinder {
            radius: shape_radius,
            half_height,
        } => collide_cylinder(point, radius, collider, *shape_radius, *half_height),
        Shape3D::Cone {
            radius: shape_radius,
            half_height,
        } => collide_cone(point, radius, collider, *shape_radius, *half_height),
        Shape3D::TriPrism { size } => collide_tri_prism(point, radius, collider, *size),
        Shape3D::TriangularPyramid { size } => {
            collide_triangular_pyramid(point, radius, collider, *size)
        }
        Shape3D::SquarePyramid { size } => collide_square_pyramid(point, radius, collider, *size),
        Shape3D::TriMesh { .. } => collide_sphere(point, radius, collider, 0.5),
    }
}

fn collide_sphere(point: Vector3, radius: f32, collider: &Collider, shape_radius: f32) -> Vector3 {
    let center = collider.world.position;
    let scale = collider.max_scale;
    let r = shape_radius.abs() * scale + radius;
    let delta = point - center;
    let len = delta.length();
    if len > 0.0001 && len < r {
        center + delta / len * r
    } else {
        point
    }
}

fn collide_cube(point: Vector3, radius: f32, collider: &Collider, size: Vector3) -> Vector3 {
    let local: Vector3 = collider.inv_world_mat.transform_point3(point.into()).into();
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
    collider.world_mat.transform_point3(pushed.into()).into()
}

fn collide_capsule(
    point: Vector3,
    radius: f32,
    collider: &Collider,
    shape_radius: f32,
    half_height: f32,
) -> Vector3 {
    let local: Vector3 = collider.inv_world_mat.transform_point3(point.into()).into();
    let scale = collider.max_scale.max(0.0001);
    let local_probe_radius = radius / scale;
    let a = Vector3::new(0.0, -half_height.abs(), 0.0);
    let b = Vector3::new(0.0, half_height.abs(), 0.0);
    let nearest = closest_point_on_segment(local, a, b);
    push_from_nearest(
        point,
        local,
        nearest,
        shape_radius.abs() + local_probe_radius,
        collider,
    )
}

fn collide_cylinder(
    point: Vector3,
    radius: f32,
    collider: &Collider,
    shape_radius: f32,
    half_height: f32,
) -> Vector3 {
    let local: Vector3 = collider.inv_world_mat.transform_point3(point.into()).into();
    let scale = collider.max_scale.max(0.0001);
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
    collider.world_mat.transform_point3(pushed.into()).into()
}

fn collide_cone(
    point: Vector3,
    radius: f32,
    collider: &Collider,
    shape_radius: f32,
    half_height: f32,
) -> Vector3 {
    let local: Vector3 = collider.inv_world_mat.transform_point3(point.into()).into();
    let scale = collider.max_scale.max(0.0001);
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
    collider.world_mat.transform_point3(pushed.into()).into()
}

fn collide_tri_prism(point: Vector3, radius: f32, collider: &Collider, size: Vector3) -> Vector3 {
    collide_poly_shape(point, radius, collider, &tri_prism_faces(size))
}

fn collide_triangular_pyramid(
    point: Vector3,
    radius: f32,
    collider: &Collider,
    size: Vector3,
) -> Vector3 {
    collide_poly_shape(point, radius, collider, &triangular_pyramid_faces(size))
}

fn collide_square_pyramid(
    point: Vector3,
    radius: f32,
    collider: &Collider,
    size: Vector3,
) -> Vector3 {
    collide_poly_shape(point, radius, collider, &square_pyramid_faces(size))
}

fn collide_poly_shape(
    point: Vector3,
    radius: f32,
    collider: &Collider,
    faces: &[[Vector3; 3]],
) -> Vector3 {
    let local: Vector3 = collider.inv_world_mat.transform_point3(point.into()).into();
    let scale = collider.max_scale.max(0.0001);
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
        collider.world_mat.transform_point3(pushed.into()).into()
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
    collider: &Collider,
) -> Vector3 {
    let delta = local - nearest;
    let len = delta.length();
    if len > 0.0001 && len < combined_radius {
        let pushed = nearest + delta / len * combined_radius;
        collider.world_mat.transform_point3(pushed.into()).into()
    } else {
        world_point
    }
}

fn max_abs_component(v: Vector3) -> f32 {
    v.x.abs().max(v.y.abs()).max(v.z.abs())
}

fn tri_prism_faces(size: Vector3) -> [[Vector3; 3]; 8] {
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
    [
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

fn triangular_pyramid_faces(size: Vector3) -> [[Vector3; 3]; 4] {
    let hw = size.x.abs().max(0.0001) * 0.5;
    let hh = size.y.abs().max(0.0001) * 0.5;
    let hd = size.z.abs().max(0.0001) * 0.5;
    let p = [
        Vector3::new(-hw, -hh, -hd),
        Vector3::new(hw, -hh, -hd),
        Vector3::new(0.0, -hh, hd),
        Vector3::new(0.0, hh, 0.0),
    ];
    [
        [p[0], p[2], p[1]],
        [p[0], p[1], p[3]],
        [p[1], p[2], p[3]],
        [p[2], p[0], p[3]],
    ]
}

fn square_pyramid_faces(size: Vector3) -> [[Vector3; 3]; 6] {
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
    [
        [p[0], p[3], p[2]],
        [p[0], p[2], p[1]],
        [p[0], p[1], p[4]],
        [p[1], p[2], p[4]],
        [p[2], p[3], p[4]],
        [p[3], p[0], p[4]],
    ]
}

fn write_chain_positions(skeleton: &mut Skeleton3D, chain: &[usize], local_positions: &[Vector3]) {
    let mut parent_global = Mat4::IDENTITY;
    for (chain_pos, bone_index) in chain.iter().copied().enumerate() {
        if chain_pos == 0 {
            parent_global = skeleton.bones[bone_index].pose.to_mat4();
            continue;
        }
        let local = parent_global
            .inverse()
            .transform_point3(local_positions[chain_pos].into());
        skeleton.bones[bone_index].pose.position = local.into();
        parent_global *= skeleton.bones[bone_index].pose.to_mat4();
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

    fn collider(shape: Shape3D) -> Collider {
        let world = Transform3D::IDENTITY;
        let world_mat = world.to_mat4();
        Collider {
            world,
            world_mat,
            inv_world_mat: world_mat.inverse(),
            max_scale: max_abs_component(world.scale),
            shape,
        }
    }

    #[test]
    fn collect_chain_caps_length() {
        let mut skeleton = Skeleton3D::new();
        skeleton.bones = vec![bone(-1, 0.0), bone(0, 1.0), bone(1, 1.0), bone(2, 1.0)];
        assert_eq!(collect_chain(&skeleton, 3, 2), vec![1, 2, 3]);
    }

    #[test]
    fn physics_bone_chain_3d_default_iterations_balanced() {
        assert_eq!(PhysicsBoneChain3D::new().iterations, 3);
    }

    #[test]
    fn sphere_collision_pushes_point_out() {
        let collider = collider(Shape3D::Sphere { radius: 1.0 });
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
        let collider = collider(Shape3D::Capsule {
            radius: 0.5,
            half_height: 1.0,
        });
        let out = collide_point(Vector3::new(0.2, 0.0, 0.0), 0.1, &collider);
        assert!(out.x.abs() >= 0.599);
    }

    #[test]
    fn cylinder_collision_pushes_point_out() {
        let collider = collider(Shape3D::Cylinder {
            radius: 0.5,
            half_height: 1.0,
        });
        let out = collide_point(Vector3::new(0.2, 0.0, 0.0), 0.1, &collider);
        assert!(out.x.abs() >= 0.599);
    }

    #[test]
    #[ignore = "bench-style timing test; run with --ignored --nocapture"]
    fn bench_physics_bone_chain_3d_release() {
        let chain: Vec<usize> = (0..8).collect();
        let rest_world: Vec<Vector3> = (0..8).map(|i| Vector3::new(0.0, i as f32, 0.0)).collect();
        let lengths = vec![1.0; 7];
        for iterations in [2usize, 3, 4] {
            let cfg = ChainCfg {
                skeleton: NodeID::nil(),
                bone_index: 7,
                chain_length: 8,
                enabled: true,
                gravity: Vector3::new(0.0, -9.81, 0.0),
                damping: 0.08,
                stiffness: 0.35,
                radius: 0.05,
                collisions: false,
                iterations,
            };
            let dt = 1.0 / 60.0;
            let samples = 50_000usize;
            let mut node = PhysicsBoneChain3D::new();
            step_chain(&mut node, &chain, &rest_world, &lengths, &[], cfg, dt);
            let start = std::time::Instant::now();
            for _ in 0..samples {
                step_chain(&mut node, &chain, &rest_world, &lengths, &[], cfg, dt);
            }
            let elapsed = start.elapsed();
            let ns_each = elapsed.as_nanos() as f64 / samples as f64;
            println!(
                "bench_physics_bone_chain_3d_release chain=8 iterations={iterations} samples={samples} ns_each={ns_each:.1}"
            );
        }
    }
}

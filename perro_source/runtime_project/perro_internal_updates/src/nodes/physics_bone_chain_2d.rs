use crate::prelude::*;
use glam::{Mat3, Vec2};
use perro_nodes::{
    BoneCollider2D, CollisionShape2D, NodeType, PhysicsBoneChain2D, Shape2D, Skeleton2D,
};
use perro_runtime_api::perro_structs::{Transform2D, Vector2};
use std::cell::RefCell;

thread_local! {
    static COLLIDER_SCRATCH_2D: RefCell<Vec<Collider>> = const { RefCell::new(Vec::new()) };
}

pub fn internal_fixed_update<RT>(ctx: &mut RuntimeWindow<'_, RT>, id: NodeID)
where
    RT: RuntimeAPI + ?Sized,
{
    let Some(cfg) = with_base_node!(ctx, PhysicsBoneChain2D, id, |node| ChainCfg {
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

    let Some((chain, rest_globals)) = with_base_node!(ctx, Skeleton2D, cfg.skeleton, |skeleton| {
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
        .get_global_transform_2d(cfg.skeleton)
        .unwrap_or(Transform2D::IDENTITY)
        .to_mat3();
    let dt = fixed_delta_time!(ctx).clamp(0.0001, 0.05);

    if cfg.collisions {
        COLLIDER_SCRATCH_2D.with(|scratch| {
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
    skeleton_global: Mat3,
    chain: &'a [usize],
    rest_globals: &'a [Vec2],
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
    let Some(mut local_positions) = with_base_node_mut!(ctx, PhysicsBoneChain2D, id, |node| {
        let mut rest_world = std::mem::take(&mut node.internal_rest_world);
        rest_world.clear();
        rest_world.extend(
            rest_globals
                .iter()
                .map(|p| Vector2::from(skeleton_global.transform_point2(*p))),
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
                .map(|p| Vector2::from(skeleton_from_world.transform_point2((*p).into()))),
        );
        node.internal_rest_world = rest_world;
        node.internal_lengths = lengths;
        local_positions
    }) else {
        return;
    };

    let changed = with_base_node_mut!(ctx, Skeleton2D, cfg.skeleton, |skeleton| {
        write_chain_positions(skeleton, chain, &local_positions);
    });
    if changed.is_some() {
        let _ = ctx.Nodes().force_rerender(cfg.skeleton);
    }
    let _ = with_base_node_mut!(ctx, PhysicsBoneChain2D, id, |node| {
        node.internal_local_positions = std::mem::take(&mut local_positions);
    });
}

#[derive(Clone, Copy)]
struct ChainCfg {
    skeleton: NodeID,
    bone_index: i32,
    chain_length: usize,
    enabled: bool,
    gravity: Vector2,
    damping: f32,
    stiffness: f32,
    radius: f32,
    collisions: bool,
    iterations: usize,
}

#[derive(Clone)]
struct Collider {
    world: Transform2D,
    world_mat: Mat3,
    inv_world_mat: Mat3,
    shape: Shape2D,
}

fn collect_chain(skeleton: &Skeleton2D, end: usize, max_len: usize) -> Vec<usize> {
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

fn chain_global_positions(skeleton: &Skeleton2D, chain: &[usize]) -> Vec<Vec2> {
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

    let mut global = Mat3::IDENTITY;
    for bone in ancestors {
        global *= skeleton.bones[bone].pose.to_mat3();
    }

    let mut out = Vec::with_capacity(chain.len());
    out.push(global.transform_point2(Vec2::ZERO));
    for bone in chain.iter().copied().skip(1) {
        global *= skeleton.bones[bone].pose.to_mat3();
        out.push(global.transform_point2(Vec2::ZERO));
    }
    out
}

fn collect_colliders<RT>(ctx: &mut RuntimeWindow<'_, RT>, out: &mut Vec<Collider>)
where
    RT: RuntimeAPI + ?Sized,
{
    out.clear();
    let query = NodeQuery::new().node_type([NodeType::BoneCollider2D]);
    let ids = ctx.NodeQuery().query(&query);
    for id in ids {
        let enabled =
            with_base_node!(ctx, BoneCollider2D, id, |node| node.enabled).unwrap_or(false);
        if !enabled {
            continue;
        }
        let Some(collider_world) = ctx.Nodes().get_global_transform_2d(id) else {
            continue;
        };
        for child in ctx.Nodes().get_children(id) {
            let Some((shape_local, shape)) =
                with_base_node!(ctx, CollisionShape2D, child, |shape| {
                    (shape.transform, shape.shape)
                })
            else {
                continue;
            };
            let world = Transform2D::from_mat3(collider_world.to_mat3() * shape_local.to_mat3());
            let world_mat = world.to_mat3();
            out.push(Collider {
                world,
                world_mat,
                inv_world_mat: world_mat.inverse(),
                shape,
            });
        }
    }
}

#[inline(always)]
fn step_chain(
    node: &mut PhysicsBoneChain2D,
    chain: &[usize],
    rest_world: &[Vector2],
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
}

#[inline(always)]
fn solve_lengths_forward(positions: &mut [Vector2], lengths: &[f32]) {
    for i in 1..positions.len() {
        let parent = positions[i - 1];
        let current = positions[i];
        positions[i] = parent + normalized_delta_2d(current - parent) * lengths[i - 1];
    }
}

#[inline(always)]
fn solve_lengths_backward(positions: &mut [Vector2], lengths: &[f32]) {
    if positions.len() < 2 {
        return;
    }
    for i in (0..positions.len() - 1).rev() {
        let child = positions[i + 1];
        let current = positions[i];
        positions[i] = child + normalized_delta_2d(current - child) * lengths[i];
    }
}

#[inline(always)]
fn normalized_delta_2d(delta: Vector2) -> Vector2 {
    let len_sq = delta.x * delta.x + delta.y * delta.y;
    if len_sq <= f32::EPSILON {
        Vector2::ZERO
    } else {
        delta * len_sq.sqrt().recip()
    }
}

#[inline(always)]
fn collide_positions(positions: &mut [Vector2], radius: f32, colliders: &[Collider]) {
    for pos in positions.iter_mut().skip(1) {
        for collider in colliders {
            *pos = collide_point(*pos, radius, collider);
        }
    }
}

fn collide_point(point: Vector2, radius: f32, collider: &Collider) -> Vector2 {
    match collider.shape {
        Shape2D::Circle {
            radius: shape_radius,
        } => collide_circle(point, radius, collider.world, shape_radius),
        Shape2D::Quad { width, height } => collide_quad(point, radius, collider, width, height),
        Shape2D::Triangle { width, height, .. } => {
            collide_quad(point, radius, collider, width, height)
        }
    }
}

fn collide_circle(point: Vector2, radius: f32, world: Transform2D, shape_radius: f32) -> Vector2 {
    let center = world.position;
    let scale = world.scale.x.abs().max(world.scale.y.abs());
    let r = shape_radius.abs() * scale + radius;
    let delta = point - center;
    let len = delta.length();
    if len > 0.0001 && len < r {
        center + delta / len * r
    } else {
        point
    }
}

fn collide_quad(
    point: Vector2,
    radius: f32,
    collider: &Collider,
    width: f32,
    height: f32,
) -> Vector2 {
    let local: Vector2 = collider.inv_world_mat.transform_point2(point.into()).into();
    let half = Vector2::new(width.abs() * 0.5 + radius, height.abs() * 0.5 + radius);
    if local.x.abs() > half.x || local.y.abs() > half.y {
        return point;
    }
    let dx = half.x - local.x.abs();
    let dy = half.y - local.y.abs();
    let mut pushed = local;
    if dx <= dy {
        pushed.x = half.x.copysign(local.x);
    } else {
        pushed.y = half.y.copysign(local.y);
    }
    collider.world_mat.transform_point2(pushed.into()).into()
}

fn write_chain_positions(skeleton: &mut Skeleton2D, chain: &[usize], local_positions: &[Vector2]) {
    let mut parent_global = Mat3::IDENTITY;
    for (chain_pos, bone_index) in chain.iter().copied().enumerate() {
        if chain_pos == 0 {
            parent_global = skeleton.bones[bone_index].pose.to_mat3();
            continue;
        }
        let local = parent_global
            .inverse()
            .transform_point2(local_positions[chain_pos].into());
        skeleton.bones[bone_index].pose.position = local.into();
        parent_global *= skeleton.bones[bone_index].pose.to_mat3();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_backward_solve_keeps_segment_lengths() {
        let mut positions = vec![
            Vector2::ZERO,
            Vector2::new(0.3, -0.2),
            Vector2::new(0.8, -0.9),
        ];
        let lengths = vec![1.0, 1.0];
        solve_lengths_forward(&mut positions, &lengths);
        solve_lengths_backward(&mut positions, &lengths);
        solve_lengths_forward(&mut positions, &lengths);
        assert!((positions[0].distance_to(positions[1]) - 1.0).abs() < 0.001);
        assert!((positions[1].distance_to(positions[2]) - 1.0).abs() < 0.001);
    }

    #[test]
    fn physics_bone_chain_2d_default_iterations_balanced() {
        assert_eq!(PhysicsBoneChain2D::new().iterations, 3);
    }

    #[test]
    #[ignore = "bench-style timing test; run with --ignored --nocapture"]
    fn bench_physics_bone_chain_2d_release() {
        let chain: Vec<usize> = (0..8).collect();
        let rest_world: Vec<Vector2> = (0..8).map(|i| Vector2::new(0.0, i as f32)).collect();
        let lengths = vec![1.0; 7];
        for iterations in [2usize, 3, 4] {
            let cfg = ChainCfg {
                skeleton: NodeID::nil(),
                bone_index: 7,
                chain_length: 8,
                enabled: true,
                gravity: Vector2::new(0.0, -9.81),
                damping: 0.08,
                stiffness: 0.35,
                radius: 0.05,
                collisions: false,
                iterations,
            };
            let dt = 1.0 / 60.0;
            let samples = 50_000usize;
            let mut node = PhysicsBoneChain2D::new();
            step_chain(&mut node, &chain, &rest_world, &lengths, &[], cfg, dt);
            let start = std::time::Instant::now();
            for _ in 0..samples {
                step_chain(&mut node, &chain, &rest_world, &lengths, &[], cfg, dt);
            }
            let elapsed = start.elapsed();
            let ns_each = elapsed.as_nanos() as f64 / samples as f64;
            println!(
                "bench_physics_bone_chain_2d_release chain=8 iterations={iterations} samples={samples} ns_each={ns_each:.1}"
            );
        }
    }
}

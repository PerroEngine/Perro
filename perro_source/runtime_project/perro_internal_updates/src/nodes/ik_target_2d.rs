use crate::prelude::*;
use glam::{Mat3, Vec2};
use perro_nodes::{IKTarget2D, Skeleton2D};
use perro_runtime_context::perro_structs::Transform2D;

pub fn internal_update<RT>(ctx: &mut RuntimeWindow<'_, RT>, id: NodeID)
where
    RT: RuntimeAPI + ?Sized,
{
    let Some(target) = with_base_node!(ctx, IKTarget2D, id, |node| {
        (
            node.skeleton,
            node.bone_index,
            node.chain_length,
            node.iterations,
            node.tolerance,
            node.weight,
            node.match_rotation,
        )
    }) else {
        return;
    };
    let (skeleton_id, bone_index, chain_length, iterations, tolerance, weight, match_rotation) =
        target;
    if skeleton_id.is_nil()
        || bone_index < 0
        || chain_length == 0
        || iterations == 0
        || weight <= 0.0
    {
        return;
    }

    let Some(target_global) = ctx.Nodes().get_global_transform_2d(id) else {
        return;
    };
    let skeleton_global = ctx
        .Nodes()
        .get_global_transform_2d(skeleton_id)
        .unwrap_or(Transform2D::IDENTITY)
        .to_mat3();
    let skeleton_from_global = skeleton_global.inverse();
    let target_local_pos = skeleton_from_global.transform_point2(target_global.position.into());
    let target_local_rot =
        Transform2D::from_mat3(skeleton_from_global * target_global.to_mat3()).rotation;

    let changed = with_base_node_mut!(ctx, Skeleton2D, skeleton_id, |skeleton| {
        solve_ccd(
            skeleton,
            CcdSolve {
                end: bone_index as usize,
                chain_length: chain_length as usize,
                iterations: iterations as usize,
                tolerance: tolerance.max(0.0),
                weight: weight.clamp(0.0, 1.0),
                match_rotation,
                target_pos: target_local_pos,
                target_rot: target_local_rot,
            },
        )
    });
    if changed.unwrap_or(false) {
        let _ = ctx.Nodes().force_rerender(skeleton_id);
    }
}

const MIN_ROT_DELTA: f32 = 1.0e-5;

#[derive(Clone, Copy)]
struct CcdSolve {
    end: usize,
    chain_length: usize,
    iterations: usize,
    tolerance: f32,
    weight: f32,
    match_rotation: bool,
    target_pos: Vec2,
    target_rot: f32,
}

fn solve_ccd(skeleton: &mut Skeleton2D, cfg: CcdSolve) -> bool {
    let CcdSolve {
        end,
        chain_length,
        iterations,
        tolerance,
        weight,
        match_rotation,
        target_pos,
        target_rot,
    } = cfg;
    if end >= skeleton.bones.len() {
        return false;
    }
    let mut chain = Vec::with_capacity(chain_length.saturating_add(1).min(skeleton.bones.len()));
    collect_root_to_end(skeleton, end, &mut chain);
    if chain.is_empty() {
        return false;
    }
    let mut changed = false;
    let joint_count = chain.len().saturating_sub(1).min(chain_length);
    if joint_count == 0 {
        if match_rotation {
            changed |= apply_angle_delta(skeleton, end, target_rot, weight);
        }
        return changed;
    }

    let joint_start = chain.len().saturating_sub(1 + joint_count);
    let mut globals = vec![Mat3::IDENTITY; chain.len()];
    let tolerance_sq = tolerance * tolerance;
    for _ in 0..iterations {
        compute_chain_globals(skeleton, &chain, &mut globals);
        let mut end_pos = globals[chain.len() - 1].transform_point2(Vec2::ZERO);
        if end_pos.distance_squared(target_pos) <= tolerance_sq {
            break;
        }
        for chain_index in (joint_start..chain.len() - 1).rev() {
            let joint = chain[chain_index];
            let joint_pos = globals[chain_index].transform_point2(Vec2::ZERO);
            let to_end = end_pos - joint_pos;
            let to_target = target_pos - joint_pos;
            if to_end.length_squared() <= f32::EPSILON || to_target.length_squared() <= f32::EPSILON
            {
                continue;
            }
            let delta = to_end.angle_to(to_target) * weight;
            if delta.abs() <= MIN_ROT_DELTA {
                continue;
            }
            skeleton.bones[joint].pose.rotation += delta;
            changed = true;
            compute_chain_globals_from(skeleton, &chain, chain_index, &mut globals);
            end_pos = globals[chain.len() - 1].transform_point2(Vec2::ZERO);
        }
    }

    if match_rotation {
        changed |= apply_angle_delta(skeleton, end, target_rot, weight);
    }
    changed
}

fn apply_angle_delta(skeleton: &mut Skeleton2D, bone: usize, target_rot: f32, weight: f32) -> bool {
    let delta = angle_delta(skeleton.bones[bone].pose.rotation, target_rot) * weight;
    if delta.abs() <= MIN_ROT_DELTA {
        return false;
    }
    skeleton.bones[bone].pose.rotation += delta;
    true
}

fn collect_root_to_end(skeleton: &Skeleton2D, end: usize, out: &mut Vec<usize>) {
    out.clear();
    let mut current = end as i32;
    let mut hops = 0usize;
    while current >= 0 && hops < skeleton.bones.len() {
        let index = current as usize;
        if index >= skeleton.bones.len() {
            break;
        }
        out.push(index);
        current = skeleton.bones[index].parent;
        hops += 1;
    }
    out.reverse();
}

fn compute_chain_globals(skeleton: &Skeleton2D, chain: &[usize], out: &mut [Mat3]) {
    let mut parent_global = Mat3::IDENTITY;
    for (chain_index, bone_index) in chain.iter().copied().enumerate() {
        let global = parent_global * skeleton.bones[bone_index].pose.to_mat3();
        out[chain_index] = global;
        parent_global = global;
    }
}

fn compute_chain_globals_from(
    skeleton: &Skeleton2D,
    chain: &[usize],
    start: usize,
    out: &mut [Mat3],
) {
    let mut parent_global = if start > 0 {
        out[start - 1]
    } else {
        Mat3::IDENTITY
    };
    for (chain_index, bone_index) in chain.iter().copied().enumerate().skip(start) {
        let global = parent_global * skeleton.bones[bone_index].pose.to_mat3();
        out[chain_index] = global;
        parent_global = global;
    }
}

fn angle_delta(from: f32, to: f32) -> f32 {
    let mut delta = to - from;
    while delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    while delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta
}

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

    let solved = with_base_node_mut!(ctx, Skeleton2D, skeleton_id, |skeleton| {
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
        );
    });
    if solved.is_some() {
        let _ = ctx.Nodes().force_rerender(skeleton_id);
    }
}

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

fn solve_ccd(skeleton: &mut Skeleton2D, cfg: CcdSolve) {
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
        return;
    }
    let mut chain = Vec::with_capacity(chain_length.saturating_add(1).min(skeleton.bones.len()));
    collect_root_to_end(skeleton, end, &mut chain);
    if chain.is_empty() {
        return;
    }
    let joint_count = chain.len().saturating_sub(1).min(chain_length);
    if joint_count == 0 {
        if match_rotation {
            skeleton.bones[end].pose.rotation +=
                angle_delta(skeleton.bones[end].pose.rotation, target_rot) * weight;
        }
        return;
    }

    let joint_start = chain.len().saturating_sub(1 + joint_count);
    let mut globals = vec![Mat3::IDENTITY; chain.len()];
    for _ in 0..iterations {
        compute_chain_globals(skeleton, &chain, &mut globals);
        let mut end_pos = globals[chain.len() - 1].transform_point2(Vec2::ZERO);
        if end_pos.distance(target_pos) <= tolerance {
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
            skeleton.bones[joint].pose.rotation += delta;
            compute_chain_globals_from(skeleton, &chain, chain_index, &mut globals);
            end_pos = globals[chain.len() - 1].transform_point2(Vec2::ZERO);
        }
    }

    if match_rotation {
        skeleton.bones[end].pose.rotation +=
            angle_delta(skeleton.bones[end].pose.rotation, target_rot) * weight;
    }
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

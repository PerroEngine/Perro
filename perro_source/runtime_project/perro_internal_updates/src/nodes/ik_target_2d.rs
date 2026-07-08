use crate::prelude::*;
use glam::{Mat3, Vec2};
use perro_nodes::{IKTarget2D, Skeleton2D};
use perro_runtime_api::perro_structs::{IKTargetSolver, Transform2D};
use std::cell::RefCell;

thread_local! {
    static FABRIK_SCRATCH_2D: RefCell<FabrikScratch> = RefCell::new(FabrikScratch::default());
}

pub fn internal_update<RT>(ctx: &mut RuntimeWindow<'_, RT>, id: NodeID)
where
    RT: RuntimeAPI + ?Sized,
{
    let Some(target) = with_base_node!(ctx, IKTarget2D, id, |node| {
        (
            node.params.skeleton,
            node.params.bone_index,
            node.params.chain_length,
            node.params.iterations,
            node.params.tolerance,
            node.params.weight,
            node.params.match_rotation,
            node.params.solver,
        )
    }) else {
        return;
    };
    let (
        skeleton_id,
        bone_index,
        chain_length,
        iterations,
        tolerance,
        weight,
        match_rotation,
        solver,
    ) = target;
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
        solve_auto(
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
                solver,
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
    solver: IKTargetSolver,
}

fn solve_auto(skeleton: &mut Skeleton2D, cfg: CcdSolve) -> bool {
    match cfg.solver {
        IKTargetSolver::CCD => solve_ccd(skeleton, cfg),
        IKTargetSolver::FABRIK => solve_fabrik(skeleton, cfg),
    }
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
        ..
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

fn solve_fabrik(skeleton: &mut Skeleton2D, cfg: CcdSolve) -> bool {
    FABRIK_SCRATCH_2D.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        solve_fabrik_with_scratch(skeleton, cfg, &mut scratch)
    })
}

fn solve_fabrik_with_scratch(
    skeleton: &mut Skeleton2D,
    cfg: CcdSolve,
    scratch: &mut FabrikScratch,
) -> bool {
    let CcdSolve {
        end,
        chain_length,
        iterations,
        tolerance,
        weight,
        match_rotation,
        target_pos,
        target_rot,
        ..
    } = cfg;
    if end >= skeleton.bones.len() {
        return false;
    }

    let chain = &mut scratch.chain;
    if chain.capacity() < chain_length.saturating_add(1).min(skeleton.bones.len()) {
        chain.reserve(chain_length.saturating_add(1).min(skeleton.bones.len()) - chain.capacity());
    }
    collect_tail_to_end(skeleton, end, chain_length, chain);
    if chain.is_empty() {
        return false;
    }

    let joint_count = chain.len().saturating_sub(1);
    if joint_count == 0 {
        return match_rotation && apply_angle_delta(skeleton, end, target_rot, weight);
    }

    let globals = &mut scratch.globals;
    if globals.len() < chain.len() {
        globals.resize(chain.len(), Mat3::IDENTITY);
    }
    let parent_global = compute_parent_global_2d(skeleton, chain[0]);
    compute_chain_globals_with_parent(skeleton, chain, parent_global, globals);

    let points = &mut scratch.points;
    points.clear();
    if points.capacity() < chain.len() {
        points.reserve(chain.len() - points.capacity());
    }
    points.extend(
        globals
            .iter()
            .take(chain.len())
            .map(|global| global.transform_point2(Vec2::ZERO)),
    );

    let root = points[0];
    let lengths = &mut scratch.lengths;
    lengths.clear();
    if lengths.capacity() < points.len().saturating_sub(1) {
        lengths.reserve(points.len().saturating_sub(1) - lengths.capacity());
    }
    lengths.extend(
        points
            .windows(2)
            .map(|pair| pair[0].distance(pair[1]).max(0.0001)),
    );
    let total_len = lengths.iter().sum::<f32>();
    let tolerance_sq = tolerance * tolerance;

    if root.distance_squared(target_pos) >= total_len * total_len {
        let dir = (target_pos - root).normalize_or_zero();
        if dir.length_squared() > f32::EPSILON {
            for i in 1..points.len() {
                points[i] = points[i - 1] + dir * lengths[i - 1];
            }
        }
    } else {
        for _ in 0..iterations {
            let last = points.len() - 1;
            points[last] = target_pos;
            for i in (0..last).rev() {
                points[i] =
                    points[i + 1] + (points[i] - points[i + 1]).normalize_or_zero() * lengths[i];
            }
            points[0] = root;
            for i in 1..points.len() {
                points[i] = points[i - 1]
                    + (points[i] - points[i - 1]).normalize_or_zero() * lengths[i - 1];
            }
            if points[last].distance_squared(target_pos) <= tolerance_sq {
                break;
            }
        }
    }

    let mut changed = false;
    let mut parent_global_rot = global_rotation_2d(parent_global);
    for i in 0..points.len() - 1 {
        let bone = chain[i];
        let child = chain[i + 1];
        let target_dir = points[i + 1] - points[i];
        if target_dir.length_squared() <= f32::EPSILON {
            continue;
        }
        let child_offset: Vec2 = skeleton.bones[child].pose.position.into();
        if child_offset.length_squared() <= f32::EPSILON {
            continue;
        }
        let desired_global =
            target_dir.y.atan2(target_dir.x) - child_offset.y.atan2(child_offset.x);
        let current = skeleton.bones[bone].pose.rotation;
        let solved = current + angle_delta(parent_global_rot + current, desired_global);
        let delta = angle_delta(current, solved) * weight;
        if delta.abs() > MIN_ROT_DELTA {
            skeleton.bones[bone].pose.rotation += delta;
            changed = true;
        }
        parent_global_rot += skeleton.bones[bone].pose.rotation;
    }

    if match_rotation {
        changed |= apply_angle_delta(skeleton, end, target_rot, weight);
    }
    changed
}

#[derive(Default)]
struct FabrikScratch {
    chain: Vec<usize>,
    globals: Vec<Mat3>,
    points: Vec<Vec2>,
    lengths: Vec<f32>,
}

fn collect_tail_to_end(skeleton: &Skeleton2D, end: usize, max_len: usize, out: &mut Vec<usize>) {
    out.clear();
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
}

fn global_rotation_2d(global: Mat3) -> f32 {
    global.x_axis.y.atan2(global.x_axis.x)
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
    compute_chain_globals_with_parent(skeleton, chain, Mat3::IDENTITY, out);
}

fn compute_chain_globals_with_parent(
    skeleton: &Skeleton2D,
    chain: &[usize],
    mut parent_global: Mat3,
    out: &mut [Mat3],
) {
    for (chain_index, bone_index) in chain.iter().copied().enumerate() {
        let global = parent_global * skeleton.bones[bone_index].pose.to_mat3();
        out[chain_index] = global;
        parent_global = global;
    }
}

fn compute_parent_global_2d(skeleton: &Skeleton2D, first: usize) -> Mat3 {
    let parent = skeleton
        .bones
        .get(first)
        .map(|bone| bone.parent)
        .unwrap_or(-1);
    if parent < 0 {
        return Mat3::IDENTITY;
    }

    let mut ancestors = Vec::new();
    let mut current = parent;
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

    let mut global = Mat3::IDENTITY;
    for bone in ancestors.iter().rev().copied() {
        global *= skeleton.bones[bone].pose.to_mat3();
    }
    global
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

#[cfg(test)]
mod tests {
    use super::*;
    use perro_nodes::skeleton_2d::Bone2D;
    use perro_runtime_api::perro_structs::Vector2;
    use std::borrow::Cow;

    fn chain_skeleton(count: usize) -> Skeleton2D {
        let mut skeleton = Skeleton2D::default();
        skeleton.bones.reserve(count);
        for i in 0..count {
            let pose = Transform2D {
                position: if i == 0 {
                    Vector2::ZERO
                } else {
                    Vector2::new(0.0, 1.0)
                },
                ..Transform2D::IDENTITY
            };
            skeleton.bones.push(Bone2D {
                name: Cow::Owned(format!("b{i}")),
                parent: if i == 0 { -1 } else { (i - 1) as i32 },
                rest: pose,
                pose,
                inv_bind: Transform2D::IDENTITY,
            });
        }
        skeleton
    }

    #[test]
    fn fabrik_moves_two_bone_end_closer_to_target() {
        let mut skeleton = chain_skeleton(2);
        let before = Vec2::new(0.0, 1.0).distance(Vec2::new(1.0, 0.0));
        let _ = solve_fabrik(
            &mut skeleton,
            CcdSolve {
                end: 1,
                chain_length: 2,
                iterations: 8,
                tolerance: 0.001,
                weight: 1.0,
                match_rotation: false,
                target_pos: Vec2::new(1.0, 0.0),
                target_rot: 0.0,
                solver: IKTargetSolver::FABRIK,
            },
        );
        let mut chain = Vec::new();
        let mut globals = Vec::new();
        collect_root_to_end(&skeleton, 1, &mut chain);
        globals.resize(chain.len(), Mat3::IDENTITY);
        compute_chain_globals(&skeleton, &chain, &mut globals);
        let after = globals[chain.len() - 1]
            .transform_point2(Vec2::ZERO)
            .distance(Vec2::new(1.0, 0.0));
        assert!(after < before);
    }

    #[test]
    #[ignore = "bench-style timing test; run with --ignored --nocapture"]
    fn bench_ik_2d_solvers_release_many_chain_sizes() {
        let cases = [
            (8usize, 2usize),
            (8, 8),
            (32, 4),
            (128, 8),
            (512, 8),
            (512, 16),
        ];
        for (bones, chain_length) in cases {
            bench_solver_case("ccd", bones, chain_length, solve_ccd);
            bench_solver_case("fabrik", bones, chain_length, solve_fabrik);
            bench_solver_case("auto", bones, chain_length, solve_auto);
        }
    }

    fn bench_solver_case(
        name: &str,
        bones: usize,
        chain_length: usize,
        solver: fn(&mut Skeleton2D, CcdSolve) -> bool,
    ) {
        let mut skeleton = chain_skeleton(bones);
        let end = chain_length.min(bones - 1);
        let samples = 20_000usize;
        let start = std::time::Instant::now();
        for i in 0..samples {
            let t = i as f32 * 0.001;
            let _ = solver(
                &mut skeleton,
                CcdSolve {
                    end,
                    chain_length,
                    iterations: 8,
                    tolerance: 0.001,
                    weight: 1.0,
                    match_rotation: true,
                    target_pos: Vec2::new(t.sin() * 2.0, chain_length as f32),
                    target_rot: t,
                    solver: match name {
                        "ccd" => IKTargetSolver::CCD,
                        _ => IKTargetSolver::FABRIK,
                    },
                },
            );
        }
        let elapsed = start.elapsed();
        let ns_each = elapsed.as_nanos() as f64 / samples as f64;
        println!(
            "bench_ik_2d_solvers_release_many_chain_sizes solver={name} bones={bones} chain={chain_length} samples={samples} ns_each={ns_each:.1}"
        );
    }
}

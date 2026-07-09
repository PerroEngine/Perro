use crate::prelude::*;
use glam::{Mat4, Quat, Vec3};
use perro_nodes::{IKTarget3D, Skeleton3D};
use perro_runtime_api::perro_structs::{IKTargetSolver, Quaternion, Transform3D};
use std::cell::RefCell;

thread_local! {
    static FABRIK_SCRATCH_3D: RefCell<FabrikScratch> = RefCell::new(FabrikScratch::default());
    static CCD_SCRATCH_3D: RefCell<CcdScratch> = RefCell::new(CcdScratch::default());
}

pub fn internal_update<RT>(ctx: &mut RuntimeWindow<'_, RT>, id: NodeID)
where
    RT: RuntimeAPI + ?Sized,
{
    let Some(target) = with_base_node!(ctx, IKTarget3D, id, |node| {
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

    let Some(target_global) = ctx.Nodes().get_global_transform_3d(id) else {
        return;
    };
    let skeleton_global = ctx
        .Nodes()
        .get_global_transform_3d(skeleton_id)
        .unwrap_or(Transform3D::IDENTITY)
        .to_mat4();
    let skeleton_from_global = skeleton_global.inverse();
    let target_local_pos = skeleton_from_global.transform_point3(target_global.position.into());
    let target_local_rot = Transform3D::from_mat4(skeleton_from_global * target_global.to_mat4())
        .rotation
        .to_quat();
    let target_local_rot = normalize_quat(target_local_rot);

    let changed = with_base_node_mut!(ctx, Skeleton3D, skeleton_id, |skeleton| {
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
    target_pos: Vec3,
    target_rot: Quat,
    solver: IKTargetSolver,
}

fn solve_auto(skeleton: &mut Skeleton3D, cfg: CcdSolve) -> bool {
    match cfg.solver {
        IKTargetSolver::CCD => solve_ccd(skeleton, cfg),
        IKTargetSolver::FABRIK => solve_fabrik(skeleton, cfg),
    }
}

fn solve_ccd(skeleton: &mut Skeleton3D, cfg: CcdSolve) -> bool {
    CCD_SCRATCH_3D.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        solve_ccd_with_scratch(skeleton, cfg, &mut scratch)
    })
}

fn solve_ccd_with_scratch(
    skeleton: &mut Skeleton3D,
    cfg: CcdSolve,
    scratch: &mut CcdScratch,
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
    collect_root_to_end(skeleton, end, &mut scratch.chain);
    if scratch.chain.is_empty() {
        return false;
    }
    let mut changed = false;
    let joint_count = scratch.chain.len().saturating_sub(1).min(chain_length);
    if joint_count == 0 {
        if match_rotation {
            let CcdScratch { chain, state } = scratch;
            compute_chain_state(skeleton, chain, state);
            changed |= blend_end_rotation(skeleton, chain, state, end, target_rot, weight);
        }
        return changed;
    }

    let joint_start = scratch.chain.len().saturating_sub(1 + joint_count);
    let CcdScratch { chain, state } = scratch;
    let tolerance_sq = tolerance * tolerance;
    for _ in 0..iterations {
        compute_chain_state(skeleton, chain, state);
        let Some(end_pos) = chain_end_pos(state) else {
            break;
        };
        if end_pos.distance_squared(target_pos) <= tolerance_sq {
            break;
        }

        for chain_index in (joint_start..chain.len() - 1).rev() {
            let joint = chain[chain_index];
            let joint_pos = state.globals[chain_index].transform_point3(Vec3::ZERO);
            let Some(end_pos) = chain_end_pos(state) else {
                break;
            };
            let to_end = (end_pos - joint_pos).normalize_or_zero();
            let to_target = (target_pos - joint_pos).normalize_or_zero();
            if to_end.length_squared() <= f32::EPSILON || to_target.length_squared() <= f32::EPSILON
            {
                continue;
            }

            let delta = Quat::from_rotation_arc(to_end, to_target);
            if !delta.is_finite() || quat_near_identity(delta) {
                continue;
            }
            if rotate_bone_world(skeleton, state, chain_index, joint, delta, weight) {
                changed = true;
            }
            compute_chain_state_from(skeleton, chain, chain_index, state);
        }
    }

    if match_rotation {
        compute_chain_state(skeleton, chain, state);
        changed |= blend_end_rotation(skeleton, chain, state, end, target_rot, weight);
    }
    changed
}

fn solve_fabrik(skeleton: &mut Skeleton3D, cfg: CcdSolve) -> bool {
    FABRIK_SCRATCH_3D.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        solve_fabrik_with_scratch(skeleton, cfg, &mut scratch)
    })
}

fn solve_fabrik_with_scratch(
    skeleton: &mut Skeleton3D,
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
        if !match_rotation {
            return false;
        }
        let (_, parent_rot, _) = compute_parent_fabrik_basis(skeleton, end);
        return blend_end_rotation_with_parent(skeleton, end, parent_rot, target_rot, weight);
    }

    let parent_basis =
        compute_parent_fabrik_basis_with_scratch(skeleton, chain[0], &mut scratch.parents);

    let points = &mut scratch.points;
    points.clear();
    if points.capacity() < chain.len() {
        points.reserve(chain.len() - points.capacity());
    }
    compute_fabrik_points(skeleton, chain, parent_basis, points);

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
    let mut parent_rotation = parent_basis.1;
    for i in 0..points.len() - 1 {
        let bone = chain[i];
        let child = chain[i + 1];
        let target_delta = points[i + 1] - points[i];
        let target_len_sq = target_delta.length_squared();
        if target_len_sq <= f32::EPSILON {
            continue;
        }
        let target_dir = target_delta * target_len_sq.sqrt().recip();
        let child_offset: Vec3 = skeleton.bones[child].pose.position.into();
        let local_len_sq = child_offset.length_squared();
        if local_len_sq <= f32::EPSILON {
            continue;
        }
        let local_dir = child_offset * local_len_sq.sqrt().recip();

        let current = normalize_quat(skeleton.bones[bone].pose.rotation.to_quat());
        let current_dir = parent_rotation * current * local_dir;
        let delta = Quat::from_rotation_arc(current_dir, target_dir);
        if !delta.is_finite() || quat_near_identity(delta) {
            parent_rotation = normalize_quat(parent_rotation * current);
            continue;
        }
        let solved = normalize_quat(parent_rotation.inverse() * delta * parent_rotation * current);
        let blended = blend_quat(current, solved, weight);
        if !quat_close(current, blended) {
            skeleton.bones[bone].pose.rotation = Quaternion::from_quat(blended);
            changed = true;
        }
        parent_rotation = normalize_quat(parent_rotation * blended);
    }

    if match_rotation {
        if points.len() <= 3 {
            let rotations = &mut scratch.rotations;
            compute_fabrik_rotations(skeleton, chain, parent_basis.1, rotations);
            changed |= blend_end_rotation_from_rotations(
                skeleton, chain, rotations, end, target_rot, weight,
            );
        } else {
            changed |=
                blend_end_rotation_with_parent(skeleton, end, parent_rotation, target_rot, weight);
        }
    }
    changed
}

fn collect_tail_to_end(skeleton: &Skeleton3D, end: usize, max_len: usize, out: &mut Vec<usize>) {
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

fn collect_root_to_end(skeleton: &Skeleton3D, end: usize, out: &mut Vec<usize>) {
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

#[derive(Default)]
struct ChainState {
    globals: Vec<Mat4>,
    rotations: Vec<Quat>,
}

#[derive(Default)]
struct FabrikScratch {
    chain: Vec<usize>,
    parents: Vec<usize>,
    points: Vec<Vec3>,
    rotations: Vec<Quat>,
    lengths: Vec<f32>,
}

#[derive(Default)]
struct CcdScratch {
    chain: Vec<usize>,
    state: ChainState,
}

fn compute_parent_fabrik_basis(skeleton: &Skeleton3D, first: usize) -> (Vec3, Quat, Vec3) {
    let mut ancestors = Vec::new();
    compute_parent_fabrik_basis_with_scratch(skeleton, first, &mut ancestors)
}

fn compute_parent_fabrik_basis_with_scratch(
    skeleton: &Skeleton3D,
    first: usize,
    ancestors: &mut Vec<usize>,
) -> (Vec3, Quat, Vec3) {
    let parent = skeleton
        .bones
        .get(first)
        .map(|bone| bone.parent)
        .unwrap_or(-1);
    if parent < 0 {
        return (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
    }

    ancestors.clear();
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

    let mut pos = Vec3::ZERO;
    let mut rot = Quat::IDENTITY;
    let mut scale = Vec3::ONE;
    for bone_index in ancestors.iter().rev().copied() {
        let bone = &skeleton.bones[bone_index];
        let local_pos: Vec3 = bone.pose.position.into();
        let local_scale: Vec3 = bone.pose.scale.into();
        let local_rot = normalize_quat(bone.pose.rotation.to_quat());
        pos += rot * (scale * local_pos);
        rot = normalize_quat(rot * local_rot);
        scale *= local_scale;
    }
    (pos, rot, scale)
}

fn compute_fabrik_points(
    skeleton: &Skeleton3D,
    chain: &[usize],
    parent_basis: (Vec3, Quat, Vec3),
    out: &mut Vec<Vec3>,
) {
    if out.capacity() < chain.len() {
        out.reserve(chain.len() - out.capacity());
    }
    out.clear();
    let (mut parent_pos, mut parent_rot, mut parent_scale) = parent_basis;
    for bone_index in chain.iter().copied() {
        let bone = &skeleton.bones[bone_index];
        let local_pos: Vec3 = bone.pose.position.into();
        let local_scale: Vec3 = bone.pose.scale.into();
        let local_rot = normalize_quat(bone.pose.rotation.to_quat());
        let global_pos = parent_pos + parent_rot * (parent_scale * local_pos);
        let global_rot = normalize_quat(parent_rot * local_rot);
        out.push(global_pos);
        parent_pos = global_pos;
        parent_rot = global_rot;
        parent_scale *= local_scale;
    }
}

fn compute_fabrik_rotations(
    skeleton: &Skeleton3D,
    chain: &[usize],
    mut parent_rot: Quat,
    out: &mut Vec<Quat>,
) {
    if out.capacity() < chain.len() {
        out.reserve(chain.len() - out.capacity());
    }
    out.clear();
    for bone_index in chain.iter().copied() {
        let local_rot = normalize_quat(skeleton.bones[bone_index].pose.rotation.to_quat());
        parent_rot = normalize_quat(parent_rot * local_rot);
        out.push(parent_rot);
    }
}

fn compute_chain_state(skeleton: &Skeleton3D, chain: &[usize], out: &mut ChainState) {
    out.globals.clear();
    out.rotations.clear();
    reserve_chain_state(out, chain.len());
    let mut parent_global = Mat4::IDENTITY;
    let mut parent_rotation = Quat::IDENTITY;
    for bone_index in chain.iter().copied() {
        let bone = &skeleton.bones[bone_index];
        let local = bone.pose.to_mat4();
        let global = parent_global * local;
        let local_rotation = normalize_quat(bone.pose.rotation.to_quat());
        let global_rotation = normalize_quat(parent_rotation * local_rotation);
        out.globals.push(global);
        out.rotations.push(global_rotation);
        parent_global = global;
        parent_rotation = global_rotation;
    }
}

fn compute_chain_state_from(
    skeleton: &Skeleton3D,
    chain: &[usize],
    start: usize,
    out: &mut ChainState,
) {
    reserve_chain_state(out, chain.len());
    let mut parent_global = if start > 0 {
        out.globals[start - 1]
    } else {
        Mat4::IDENTITY
    };
    let mut parent_rotation = if start > 0 {
        out.rotations[start - 1]
    } else {
        Quat::IDENTITY
    };
    for (chain_index, bone_index) in chain.iter().copied().enumerate().skip(start) {
        let bone = &skeleton.bones[bone_index];
        let local = bone.pose.to_mat4();
        let global = parent_global * local;
        let local_rotation = normalize_quat(bone.pose.rotation.to_quat());
        let global_rotation = normalize_quat(parent_rotation * local_rotation);
        out.globals[chain_index] = global;
        out.rotations[chain_index] = global_rotation;
        parent_global = global;
        parent_rotation = global_rotation;
    }
}

fn reserve_chain_state(out: &mut ChainState, len: usize) {
    if out.globals.capacity() < len {
        out.globals.reserve(len - out.globals.capacity());
    }
    if out.rotations.capacity() < len {
        out.rotations.reserve(len - out.rotations.capacity());
    }
    if out.globals.len() < len {
        out.globals.resize(len, Mat4::IDENTITY);
    }
    if out.rotations.len() < len {
        out.rotations.resize(len, Quat::IDENTITY);
    }
}

fn chain_end_pos(state: &ChainState) -> Option<Vec3> {
    Some(state.globals.last()?.transform_point3(Vec3::ZERO))
}

fn rotate_bone_world(
    skeleton: &mut Skeleton3D,
    state: &ChainState,
    chain_index: usize,
    bone_index: usize,
    delta: Quat,
    weight: f32,
) -> bool {
    let parent_rotation = if chain_index > 0 {
        state.rotations[chain_index - 1]
    } else {
        Quat::IDENTITY
    };
    let current = normalize_quat(skeleton.bones[bone_index].pose.rotation.to_quat());
    let solved = normalize_quat(parent_rotation.inverse() * delta * parent_rotation * current);
    let blended = blend_quat(current, solved, weight);
    if quat_close(current, blended) {
        return false;
    }
    skeleton.bones[bone_index].pose.rotation = Quaternion::from_quat(blended);
    true
}

fn blend_end_rotation(
    skeleton: &mut Skeleton3D,
    chain: &[usize],
    state: &ChainState,
    end: usize,
    target_rot: Quat,
    weight: f32,
) -> bool {
    let Some(end_chain_index) = chain.iter().position(|index| *index == end) else {
        return false;
    };
    let parent_rot = if end_chain_index > 0 {
        state.rotations[end_chain_index - 1]
    } else {
        Quat::IDENTITY
    };
    let desired_local = parent_rot.inverse() * target_rot;
    let current = normalize_quat(skeleton.bones[end].pose.rotation.to_quat());
    let blended = blend_quat(current, desired_local, weight);
    if quat_close(current, blended) {
        return false;
    }
    skeleton.bones[end].pose.rotation = Quaternion::from_quat(blended);
    true
}

fn blend_end_rotation_with_parent(
    skeleton: &mut Skeleton3D,
    end: usize,
    parent_rot: Quat,
    target_rot: Quat,
    weight: f32,
) -> bool {
    let desired_local = parent_rot.inverse() * target_rot;
    let current = normalize_quat(skeleton.bones[end].pose.rotation.to_quat());
    let blended = blend_quat(current, desired_local, weight);
    if quat_close(current, blended) {
        return false;
    }
    skeleton.bones[end].pose.rotation = Quaternion::from_quat(blended);
    true
}

fn blend_end_rotation_from_rotations(
    skeleton: &mut Skeleton3D,
    chain: &[usize],
    rotations: &[Quat],
    end: usize,
    target_rot: Quat,
    weight: f32,
) -> bool {
    let Some(end_chain_index) = chain.iter().position(|index| *index == end) else {
        return false;
    };
    let parent_rot = if end_chain_index > 0 {
        rotations[end_chain_index - 1]
    } else {
        Quat::IDENTITY
    };
    blend_end_rotation_with_parent(skeleton, end, parent_rot, target_rot, weight)
}

fn blend_quat(current: Quat, solved: Quat, weight: f32) -> Quat {
    if weight >= 1.0 {
        normalize_quat(solved)
    } else {
        normalize_quat(current.slerp(solved, weight))
    }
}

fn normalize_quat(q: Quat) -> Quat {
    if !q.is_finite() {
        return Quat::IDENTITY;
    }
    let len_sq = q.length_squared();
    if len_sq <= 1.0e-8 {
        return Quat::IDENTITY;
    }
    if (len_sq - 1.0).abs() <= 1.0e-4 {
        q
    } else {
        q * len_sq.sqrt().recip()
    }
}

fn quat_near_identity(q: Quat) -> bool {
    (1.0 - q.w.abs()) <= MIN_ROT_DELTA * MIN_ROT_DELTA
}

fn quat_close(a: Quat, b: Quat) -> bool {
    (1.0 - a.dot(b).abs()) <= MIN_ROT_DELTA * MIN_ROT_DELTA
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_nodes::skeleton_3d::Bone3D;
    use perro_runtime_api::perro_structs::Vector3;
    use std::borrow::Cow;

    fn two_bone_skeleton() -> Skeleton3D {
        let mut skeleton = Skeleton3D::default();
        let root = Transform3D::IDENTITY;
        let child = Transform3D {
            position: Vector3::new(0.0, 1.0, 0.0),
            ..Transform3D::IDENTITY
        };
        skeleton.bones = vec![
            Bone3D {
                name: Cow::Borrowed("root"),
                parent: -1,
                rest: root,
                pose: root,
                inv_bind: Transform3D::IDENTITY,
            },
            Bone3D {
                name: Cow::Borrowed("child"),
                parent: 0,
                rest: child,
                pose: child,
                inv_bind: Transform3D::IDENTITY,
            },
        ];
        skeleton
    }

    #[test]
    fn ccd_moves_two_bone_end_closer_to_target() {
        let mut skeleton = two_bone_skeleton();
        let before = Vec3::new(0.0, 1.0, 0.0).distance(Vec3::new(1.0, 0.0, 0.0));
        solve_ccd(
            &mut skeleton,
            CcdSolve {
                end: 1,
                chain_length: 2,
                iterations: 8,
                tolerance: 0.001,
                weight: 1.0,
                match_rotation: false,
                target_pos: Vec3::new(1.0, 0.0, 0.0),
                target_rot: Quat::IDENTITY,
                solver: IKTargetSolver::CCD,
            },
        );
        let mut chain = Vec::new();
        let mut state = ChainState::default();
        collect_root_to_end(&skeleton, 1, &mut chain);
        compute_chain_state(&skeleton, &chain, &mut state);
        let after = state.globals[chain.len() - 1]
            .transform_point3(Vec3::ZERO)
            .distance(Vec3::new(1.0, 0.0, 0.0));
        assert!(after < before);
    }

    #[test]
    fn zero_weight_leaves_pose_unchanged() {
        let mut skeleton = two_bone_skeleton();
        let before = skeleton.bones[0].pose.rotation;
        solve_ccd(
            &mut skeleton,
            CcdSolve {
                end: 1,
                chain_length: 2,
                iterations: 8,
                tolerance: 0.001,
                weight: 0.0,
                match_rotation: false,
                target_pos: Vec3::new(1.0, 0.0, 0.0),
                target_rot: Quat::IDENTITY,
                solver: IKTargetSolver::CCD,
            },
        );
        let after = skeleton.bones[0].pose.rotation;
        assert!((after.x - before.x).abs() < 0.000001);
        assert!((after.y - before.y).abs() < 0.000001);
        assert!((after.z - before.z).abs() < 0.000001);
        assert!((after.w - before.w).abs() < 0.000001);
    }

    #[test]
    fn match_rotation_changes_end_bone_rotation() {
        let mut skeleton = two_bone_skeleton();
        let target = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        solve_ccd(
            &mut skeleton,
            CcdSolve {
                end: 1,
                chain_length: 2,
                iterations: 1,
                tolerance: 0.001,
                weight: 1.0,
                match_rotation: true,
                target_pos: Vec3::new(0.0, 1.0, 0.0),
                target_rot: target,
                solver: IKTargetSolver::CCD,
            },
        );
        assert_ne!(skeleton.bones[1].pose.rotation, Quaternion::IDENTITY);
    }

    fn chain_skeleton(count: usize) -> Skeleton3D {
        let mut skeleton = Skeleton3D::default();
        skeleton.bones.reserve(count);
        for i in 0..count {
            let pose = Transform3D {
                position: if i == 0 {
                    Vector3::ZERO
                } else {
                    Vector3::new(0.0, 1.0, 0.0)
                },
                ..Transform3D::IDENTITY
            };
            skeleton.bones.push(Bone3D {
                name: Cow::Owned(format!("b{i}")),
                parent: if i == 0 { -1 } else { (i - 1) as i32 },
                rest: pose,
                pose,
                inv_bind: Transform3D::IDENTITY,
            });
        }
        skeleton
    }

    #[test]
    fn ccd_ignores_unrelated_bones_in_large_skeleton() {
        let mut skeleton = chain_skeleton(128);
        let unrelated_before = skeleton.bones[90].pose.rotation;
        solve_ccd(
            &mut skeleton,
            CcdSolve {
                end: 12,
                chain_length: 4,
                iterations: 8,
                tolerance: 0.001,
                weight: 1.0,
                match_rotation: false,
                target_pos: Vec3::new(2.0, 10.0, 0.0),
                target_rot: Quat::IDENTITY,
                solver: IKTargetSolver::CCD,
            },
        );
        assert_eq!(skeleton.bones[90].pose.rotation, unrelated_before);
    }

    #[test]
    #[ignore = "bench-style timing test; run with --ignored --nocapture"]
    fn bench_ccd_solver_release_many_chain_sizes() {
        let cases = [(8usize, 2usize), (32, 4), (128, 8), (512, 8)];
        for (bones, chain_length) in cases {
            let mut skeleton = chain_skeleton(bones);
            let end = chain_length.min(bones - 1);
            let samples = 20_000usize;
            let start = std::time::Instant::now();
            for i in 0..samples {
                let t = i as f32 * 0.001;
                solve_ccd(
                    &mut skeleton,
                    CcdSolve {
                        end,
                        chain_length,
                        iterations: 8,
                        tolerance: 0.001,
                        weight: 1.0,
                        match_rotation: true,
                        target_pos: Vec3::new(t.sin() * 2.0, chain_length as f32, t.cos()),
                        target_rot: Quat::from_rotation_y(t),
                        solver: IKTargetSolver::CCD,
                    },
                );
            }
            let elapsed = start.elapsed();
            let ns_each = elapsed.as_nanos() as f64 / samples as f64;
            println!(
                "bench_ccd_solver_release_many_chain_sizes bones={bones} chain={chain_length} samples={samples} ns_each={ns_each:.1}"
            );
        }
    }

    #[test]
    #[ignore = "bench-style timing test; run with --ignored --nocapture"]
    fn bench_ik_solvers_release_many_chain_sizes() {
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
        solver: fn(&mut Skeleton3D, CcdSolve) -> bool,
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
                    target_pos: Vec3::new(t.sin() * 2.0, chain_length as f32, t.cos()),
                    target_rot: Quat::from_rotation_y(t),
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
            "bench_ik_solvers_release_many_chain_sizes solver={name} bones={bones} chain={chain_length} samples={samples} ns_each={ns_each:.1}"
        );
    }
}

use crate::prelude::*;
use perro_nodes::{BoneAttachment2D, Skeleton2D};
use perro_runtime_context::perro_structs::Transform2D;

pub fn internal_update<RT>(ctx: &mut RuntimeWindow<'_, RT>, id: NodeID)
where
    RT: RuntimeAPI + ?Sized,
{
    let Some((skeleton_id, bone_index)) = with_base_node!(ctx, BoneAttachment2D, id, |node| {
        (node.skeleton, node.bone_index)
    }) else {
        return;
    };
    if skeleton_id.is_nil() || bone_index < 0 {
        return;
    }

    let bone_index = bone_index as usize;
    let Some(bones) = with_base_node!(ctx, Skeleton2D, skeleton_id, |skeleton| {
        skeleton.bones.clone()
    }) else {
        return;
    };
    let Some(bone) = bones.get(bone_index) else {
        return;
    };

    let mut bone_global = bone.pose.to_mat3();
    let mut parent = bone.parent;
    let mut hops = 0usize;
    while parent >= 0 && hops < bones.len() {
        let Some(parent_bone) = bones.get(parent as usize) else {
            break;
        };
        bone_global = parent_bone.pose.to_mat3() * bone_global;
        parent = parent_bone.parent;
        hops += 1;
    }

    let skeleton_global = ctx
        .Nodes()
        .get_global_transform_2d(skeleton_id)
        .unwrap_or(Transform2D::IDENTITY)
        .to_mat3();
    let attachment_global = Transform2D::from_mat3(skeleton_global * bone_global);
    let _ = ctx.Nodes().set_global_transform_2d(id, attachment_global);
}

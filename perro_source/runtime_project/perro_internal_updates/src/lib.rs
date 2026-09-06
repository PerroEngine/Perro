pub mod prelude;
use crate::prelude::*;
mod nodes;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UpdateRoute {
    AnimationPlayer,
    AnimationTree,
    AnimatedSprite2D,
    UiAnimatedImage,
    VideoPlayer,
    IkTarget2D,
    IkTarget3D,
    BoneAttachment2D,
    BoneAttachment3D,
    ParticleEmitter2D,
    ParticleEmitter3D,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FixedUpdateRoute {
    PhysicsBoneChain2D,
    PhysicsBoneChain3D,
    ParticleEmitter2D,
    ParticleEmitter3D,
}

#[inline]
const fn update_route(node_type: NodeType) -> Option<UpdateRoute> {
    match node_type {
        NodeType::AnimationPlayer => Some(UpdateRoute::AnimationPlayer),
        NodeType::AnimationTree => Some(UpdateRoute::AnimationTree),
        NodeType::AnimatedSprite2D => Some(UpdateRoute::AnimatedSprite2D),
        NodeType::UiAnimatedImage => Some(UpdateRoute::UiAnimatedImage),
        NodeType::VideoPlayer2D | NodeType::VideoPlayer3D | NodeType::UiVideoPlayer => {
            Some(UpdateRoute::VideoPlayer)
        }
        NodeType::IKTarget2D => Some(UpdateRoute::IkTarget2D),
        NodeType::IKTarget3D => Some(UpdateRoute::IkTarget3D),
        NodeType::BoneAttachment2D => Some(UpdateRoute::BoneAttachment2D),
        NodeType::BoneAttachment3D => Some(UpdateRoute::BoneAttachment3D),
        NodeType::ParticleEmitter2D => Some(UpdateRoute::ParticleEmitter2D),
        NodeType::ParticleEmitter3D => Some(UpdateRoute::ParticleEmitter3D),
        _ => None,
    }
}

#[inline]
const fn fixed_update_route(node_type: NodeType) -> Option<FixedUpdateRoute> {
    match node_type {
        NodeType::PhysicsBoneChain2D => Some(FixedUpdateRoute::PhysicsBoneChain2D),
        NodeType::PhysicsBoneChain3D => Some(FixedUpdateRoute::PhysicsBoneChain3D),
        NodeType::ParticleEmitter2D => Some(FixedUpdateRoute::ParticleEmitter2D),
        NodeType::ParticleEmitter3D => Some(FixedUpdateRoute::ParticleEmitter3D),
        _ => None,
    }
}

pub fn internal_update_node<RT, RS, IP>(
    ctx: &mut RuntimeWindow<'_, RT>,
    res: &ResourceWindow<'_, RS>,
    ipt: &InputWindow<'_, IP>,
    id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    RS: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    let Some(route) = get_node_type!(ctx, id).and_then(update_route) else {
        return;
    };
    match route {
        UpdateRoute::AnimationPlayer => nodes::animation_player::internal_update(ctx, res, ipt, id),
        UpdateRoute::AnimationTree => nodes::animation_tree::internal_update(ctx, res, ipt, id),
        UpdateRoute::AnimatedSprite2D => {
            nodes::animated_sprite_2d::internal_update(ctx, res, ipt, id)
        }
        UpdateRoute::UiAnimatedImage => {
            nodes::ui_animated_image::internal_update(ctx, res, ipt, id)
        }
        UpdateRoute::VideoPlayer => nodes::video_player::internal_update(ctx, res, ipt, id),
        UpdateRoute::IkTarget2D => nodes::ik_target_2d::internal_update(ctx, id),
        UpdateRoute::IkTarget3D => nodes::ik_target_3d::internal_update(ctx, id),
        UpdateRoute::BoneAttachment2D => nodes::bone_attachment_2d::internal_update(ctx, id),
        UpdateRoute::BoneAttachment3D => nodes::bone_attachment_3d::internal_update(ctx, id),
        UpdateRoute::ParticleEmitter2D => {
            nodes::particle_emitter_2d::internal_update(ctx, res, ipt, id)
        }
        UpdateRoute::ParticleEmitter3D => {
            nodes::particle_emitter_3d::internal_update(ctx, res, ipt, id)
        }
    }
}

pub fn internal_fixed_update_node<RT, RS, IP>(
    ctx: &mut RuntimeWindow<'_, RT>,
    res: &ResourceWindow<'_, RS>,
    ipt: &InputWindow<'_, IP>,
    id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    RS: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    let Some(route) = get_node_type!(ctx, id).and_then(fixed_update_route) else {
        return;
    };
    match route {
        FixedUpdateRoute::PhysicsBoneChain2D => {
            nodes::physics_bone_chain_2d::internal_fixed_update(ctx, id)
        }
        FixedUpdateRoute::PhysicsBoneChain3D => {
            nodes::physics_bone_chain_3d::internal_fixed_update(ctx, id)
        }
        FixedUpdateRoute::ParticleEmitter2D => {
            nodes::particle_emitter_2d::internal_fixed_update(ctx, res, ipt, id)
        }
        FixedUpdateRoute::ParticleEmitter3D => {
            nodes::particle_emitter_3d::internal_fixed_update(ctx, res, ipt, id)
        }
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;

    #[test]
    fn update_routes_cover_every_scheduled_node_type() {
        for &node_type in NodeType::ALL {
            assert_eq!(
                update_route(node_type).is_some(),
                matches!(node_type.get_internal_update(), InternalUpdate::True),
                "{node_type}"
            );
        }
    }

    #[test]
    fn fixed_routes_cover_runtime_dispatch_and_legacy_particle_handlers() {
        for &node_type in NodeType::ALL {
            // Runtime dispatches bone chains here. Other fixed-update registry
            // types run in the physics step. Keep the particle routes because
            // this public entry point handled them before direct dispatch.
            let runtime_dispatch = matches!(
                node_type,
                NodeType::PhysicsBoneChain2D | NodeType::PhysicsBoneChain3D
            );
            let legacy_particle_handler = matches!(
                node_type,
                NodeType::ParticleEmitter2D | NodeType::ParticleEmitter3D
            );
            assert_eq!(
                fixed_update_route(node_type).is_some(),
                runtime_dispatch || legacy_particle_handler,
                "{node_type}"
            );
        }
    }

    #[test]
    fn video_node_families_share_update_handler() {
        for node_type in [
            NodeType::VideoPlayer2D,
            NodeType::VideoPlayer3D,
            NodeType::UiVideoPlayer,
        ] {
            assert_eq!(update_route(node_type), Some(UpdateRoute::VideoPlayer));
        }
    }
}

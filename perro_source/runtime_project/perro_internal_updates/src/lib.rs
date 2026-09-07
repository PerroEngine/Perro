pub mod prelude;
use crate::prelude::*;
mod nodes;

use perro_nodes::{FixedUpdateHook, UpdateHook};

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
    let Some(route) = get_node_type!(ctx, id).map(|ty| ty.update_hook()) else {
        return;
    };
    dispatch_update(route, ctx, res, ipt, id);
}

/// Execute a resolved hook. Caller owns phase order and node lifetime checks.
#[inline]
pub fn dispatch_update<RT, RS, IP>(
    route: UpdateHook,
    ctx: &mut RuntimeWindow<'_, RT>,
    res: &ResourceWindow<'_, RS>,
    ipt: &InputWindow<'_, IP>,
    id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    RS: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    match route {
        UpdateHook::None => {}
        UpdateHook::AnimationPlayer => nodes::animation_player::internal_update(ctx, res, ipt, id),
        UpdateHook::AnimationTree => nodes::animation_tree::internal_update(ctx, res, ipt, id),
        UpdateHook::AnimatedSprite2D => {
            nodes::animated_sprite_2d::internal_update(ctx, res, ipt, id)
        }
        UpdateHook::UiAnimatedImage => nodes::ui_animated_image::internal_update(ctx, res, ipt, id),
        UpdateHook::VideoPlayer => nodes::video_player::internal_update(ctx, res, ipt, id),
        UpdateHook::IkTarget2D => nodes::ik_target_2d::internal_update(ctx, id),
        UpdateHook::IkTarget3D => nodes::ik_target_3d::internal_update(ctx, id),
        UpdateHook::BoneAttachment2D => nodes::bone_attachment_2d::internal_update(ctx, id),
        UpdateHook::BoneAttachment3D => nodes::bone_attachment_3d::internal_update(ctx, id),
        UpdateHook::ParticleEmitter2D => {
            nodes::particle_emitter_2d::internal_update(ctx, res, ipt, id)
        }
        UpdateHook::ParticleEmitter3D => {
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
    // Preserve direct-call particle behavior; particles are not fixed-scheduled.
    match get_node_type!(ctx, id) {
        Some(NodeType::ParticleEmitter2D) => {
            nodes::particle_emitter_2d::internal_fixed_update(ctx, res, ipt, id)
        }
        Some(NodeType::ParticleEmitter3D) => {
            nodes::particle_emitter_3d::internal_fixed_update(ctx, res, ipt, id)
        }
        Some(ty) => dispatch_fixed_update(ty.fixed_update_hook(), ctx, id),
        None => {}
    }
}

/// World physics runs separately; only node callbacks dispatch here.
#[inline]
pub fn dispatch_fixed_update<RT: RuntimeAPI + ?Sized>(
    route: FixedUpdateHook,
    ctx: &mut RuntimeWindow<'_, RT>,
    id: NodeID,
) {
    match route {
        FixedUpdateHook::None | FixedUpdateHook::PhysicsWorld => {}
        FixedUpdateHook::PhysicsBoneChain2D => {
            nodes::physics_bone_chain_2d::internal_fixed_update(ctx, id)
        }
        FixedUpdateHook::PhysicsBoneChain3D => {
            nodes::physics_bone_chain_3d::internal_fixed_update(ctx, id)
        }
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;

    #[test]
    fn flags_derive_from_hooks() {
        for &ty in NodeType::ALL {
            assert_eq!(
                ty.update_hook() != UpdateHook::None,
                matches!(ty.get_internal_update(), InternalUpdate::True)
            );
            assert_eq!(
                ty.fixed_update_hook() != FixedUpdateHook::None,
                matches!(ty.get_internal_fixed_update(), InternalFixedUpdate::True)
            );
        }
    }

    #[test]
    fn world_and_node_fixed_steps_are_distinct() {
        assert_eq!(
            NodeType::RigidBody2D.fixed_update_hook(),
            FixedUpdateHook::PhysicsWorld
        );
        assert!(
            NodeType::PhysicsBoneChain2D
                .fixed_update_hook()
                .is_node_callback()
        );
        assert_eq!(
            NodeType::ParticleEmitter2D.fixed_update_hook(),
            FixedUpdateHook::None
        );
    }

    #[test]
    fn video_node_families_share_update_handler() {
        for ty in [
            NodeType::VideoPlayer2D,
            NodeType::VideoPlayer3D,
            NodeType::UiVideoPlayer,
        ] {
            assert_eq!(ty.update_hook(), UpdateHook::VideoPlayer);
        }
    }
}

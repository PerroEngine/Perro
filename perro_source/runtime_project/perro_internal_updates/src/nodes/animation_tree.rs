use crate::prelude::*;
use perro_nodes::{AnimationMixer, AnimationMixerSlotState};

type SelfNodeType = AnimationMixer;

pub fn internal_update<RT, R, IP>(
    ctx: &mut RuntimeWindow<'_, RT>,
    _res: &ResourceWindow<'_, R>,
    _ipt_w: &InputWindow<'_, IP>,
    id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    let delta = delta_time!(ctx).max(0.0);
    with_node_mut!(ctx, SelfNodeType, id, |tree| {
        for slot in &mut tree.slots {
            if slot.state == AnimationMixerSlotState::Playing {
                slot.time_seconds += delta * slot.speed;
            }
        }
        if let Ok(Some(animation)) = tree.evaluate_output_animation() {
            tree.internal.last_resolved_animation = animation;
        }
    });
}

pub fn internal_fixed_update<RT, R, IP>(
    _run: &mut RuntimeWindow<'_, RT>,
    _res_w: &ResourceWindow<'_, R>,
    _ipt_w: &InputWindow<'_, IP>,
    _id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
}

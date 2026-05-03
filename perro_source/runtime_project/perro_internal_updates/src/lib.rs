pub mod prelude;
use crate::prelude::*;
mod nodes;

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
    nodes::animation_player::internal_update(ctx, res, ipt, id);
    nodes::particle_emitter_3d::internal_update(ctx, res, ipt, id);
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
    nodes::animation_player::internal_fixed_update(ctx, res, ipt, id);
    nodes::particle_emitter_3d::internal_fixed_update(ctx, res, ipt, id);
}



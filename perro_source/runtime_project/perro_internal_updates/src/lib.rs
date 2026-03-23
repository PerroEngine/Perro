pub mod prelude;
use crate::prelude::*;
mod nodes;

pub fn internal_update_node<RT, RS, IP>(
    ctx: &mut RuntimeContext<'_, RT>,
    res: &ResourceContext<'_, RS>,
    ipt: &InputContext<'_, IP>,
    self_id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    RS: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    nodes::animation_player::internal_update(ctx, res, ipt, self_id);
    nodes::particle_emitter_3d::internal_update(ctx, res, ipt, self_id);
}

pub fn internal_fixed_update_node<RT, RS, IP>(
    ctx: &mut RuntimeContext<'_, RT>,
    res: &ResourceContext<'_, RS>,
    ipt: &InputContext<'_, IP>,
    self_id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    RS: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    nodes::animation_player::internal_fixed_update(ctx, res, ipt, self_id);
    nodes::particle_emitter_3d::internal_fixed_update(ctx, res, ipt, self_id);
}

use crate::prelude::*;

pub fn internal_update<RT, R, IP>(
    _ctx: &mut RuntimeContext<'_, RT>,
    _res: &ResourceContext<'_, R>,
    _ipt: &InputContext<'_, IP>,
    _self_id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
}

pub fn internal_fixed_update<RT, R, IP>(
    _ctx: &mut RuntimeContext<'_, RT>,
    _res: &ResourceContext<'_, R>,
    _ipt: &InputContext<'_, IP>,
    _self_id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
}

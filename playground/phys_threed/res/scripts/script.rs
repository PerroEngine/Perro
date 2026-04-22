use perro_api::prelude::*;

type SelfNodeType = Node2D;

#[State]
pub struct EmptyState {}

lifecycle!({
    fn on_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_all_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_fixed_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}
});

methods!({
    fn default_method(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}
});

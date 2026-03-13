use perro::prelude::*;


type SelfNodeType = Node2D;

#[State]
pub struct EmptyState {}



lifecycle!({
fn on_init(
    &self,
    ctx: &mut RuntimeContext<'_, RT>,
    res: &ResourceContext<'_, RS>,
    _ipt: &InputContext<'_, IP>,
    _self_id: NodeID,
) {
    let music = bus!("music");
    let sfx = bus!("sfx");

    let _ = set_master_volume!(res, 1.0);
    let _ = set_bus_volume!(res, music, 0.7);
    let _ = set_bus_volume!(res, sfx, 1.0);

    let _ = play_audio!(
        res,
        Audio {
            source: "res://groantube.mp3",
            bus: music,
            looped: true,
            volume: 0.5,
            pitch: 1.8,
        }
    );


}



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

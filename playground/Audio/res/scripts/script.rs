use perro::prelude::*;

type SelfNodeType = Node2D;

#[State]
pub struct AudioState {
    #[default = 0.5]
    timer: f32,
    #[default = false]
    played: bool,
}

lifecycle!({
fn on_init(
    &self,
    _ctx: &mut RuntimeContext<'_, RT>,
    _res: &ResourceContext<'_, RS>,
    _ipt: &InputContext<'_, IP>,
    _self_id: NodeID,
) {
}

fn on_all_init(
    &self,
    _ctx: &mut RuntimeContext<'_, RT>,
    _res: &ResourceContext<'_, RS>,
    _ipt: &InputContext<'_, IP>,
    _self_id: NodeID,
) {}

fn on_update(
    &self,
    ctx: &mut RuntimeContext<'_, RT>,
    res: &ResourceContext<'_, RS>,
    _ipt: &InputContext<'_, IP>,
    self_id: NodeID,
) {
    let dt = delta_time!(ctx);

    let should_play = with_state_mut!(ctx, AudioState, self_id, |state| {
        if state.played {
            return false;
        }

        state.timer -= dt;

        if state.timer <= 0.0 {
            state.played = true;
            return true;
        }

        false
    }).unwrap_or(false);

    if should_play {
        let music = bus!("music");
        let sfx = bus!("sfx");

        let _ = set_master_volume!(res, 1.0);
        let _ = set_bus_volume!(res, music, 0.7);
        let _ = set_bus_volume!(res, sfx, 1.0);

        let bob = Audio {
            source: "res://groantube.mp3",
            bus: music,
            looped: true,
            volume: 0.5,
            speed: 1.5,
            from_start: 0.1,
            from_end: 55.0,
        };

        play_audio!(res, bob);
    }
}

fn on_fixed_update(
    &self,
    _ctx: &mut RuntimeContext<'_, RT>,
    _res: &ResourceContext<'_, RS>,
    _ipt: &InputContext<'_, IP>,
    _self_id: NodeID,
) {}

fn on_removal(
    &self,
    _ctx: &mut RuntimeContext<'_, RT>,
    _res: &ResourceContext<'_, RS>,
    _ipt: &InputContext<'_, IP>,
    _self_id: NodeID,
) {}
});

methods!({
    fn default_method(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {}
});

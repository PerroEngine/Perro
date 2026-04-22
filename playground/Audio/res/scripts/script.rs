use perro_api::prelude::*;

type SelfNodeType = Node2D;

#[State]
pub struct AudioState {
    #[default = 0.5]
    timer: f32,
    #[default = false]
    played: bool,
}

const MUSIC: AudioBusID = audio_bus!("music");
const SFX: AudioBusID = audio_bus!("sfx");

const BOB: Audio = Audio {
    source: "res://groantube.mp3",
    bus: MUSIC,
    looped: true,
    volume: 0.5,
    speed: 1.5,
    from_start: 0.1,
    from_end: 30.0,
};

lifecycle!({
    fn on_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
        let _ = audio_set_master_volume!(res, 1.0);
        let _ = audio_bus_set_volume!(res, MUSIC, 0.7);
        let _ = audio_bus_set_volume!(res, SFX, 1.0);
    }

    fn on_all_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }

    fn on_update(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
        if key_pressed!(ipt, KeyCode::Space) {
            audio_stop!(res, BOB);
        }

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
        })
        .unwrap_or(false);

        if should_play {
            audio_play!(res, BOB);
        }
    }

    fn on_fixed_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }

    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }
});

methods!({
    fn default_method(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }
});

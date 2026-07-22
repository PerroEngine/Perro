use perro_api::prelude::*;
use std::time::Duration;

#[State]
struct PatternController {
    #[default = NodeID::nil()]
    #[node_ref(Node2D)]
    pub player: NodeID,
    #[default = NodeID::nil()]
    #[node_ref(Node2D)]
    pub adapter: NodeID,
    #[default = TextureID::nil()]
    pub icon: TextureID,
}

lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let (player, icon) = with_state!(ctx.run, PatternController, ctx.id, |state| {
            (state.player, state.icon)
        }).unwrap_or_default();

        if !player.is_nil() && !icon.is_nil() {
            let _ = call_method!(
                ctx.run,
                player,
                func!("set_icon_texture"),
                params![icon]
            );
        }

        signal_connect!(
            ctx.run,
            ctx.id,
            timer_finished!("award_score"),
            func!("on_award_score")
        );
        timer_start!(ctx.run, Duration::from_secs(1), "award_score");
    }
});

methods!({
    fn on_award_score(&self, ctx: &mut ScriptContext<'_, API>) {
        let refs = with_state!(ctx.run, PatternController, ctx.id, |state| {
            (state.player, state.adapter, state.icon)
        }).unwrap_or_default();

        if !refs.0.is_nil() && !refs.2.is_nil() {
            let _score = call_method!(
                ctx.run,
                refs.0,
                func!("add_score"),
                params![1_i32]
            )
            .as_i32()
            .unwrap_or(0);
        }

        if !refs.1.is_nil() {
            let _ = call_method!(
                ctx.run,
                refs.1,
                func!("add_to_member"),
                params!["bonus", 1_i32]
            );
        }

        timer_start!(ctx.run, Duration::from_secs(1), "award_score");
    }
});

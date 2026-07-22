use perro_api::prelude::*;

#[State]
struct PatternAudit {
    #[default = 0]
    pub last_score: i32,
    #[default = 0]
    pub event_count: i32,
}

lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("score_changed"),
            func!("record_score")
        );
    }
});

methods!({
    fn record_score(&self, ctx: &mut ScriptContext<'_, API>, score: i32) {
        with_state_mut!(ctx.run, PatternAudit, ctx.id, |state| {
            state.last_score = score;
            state.event_count += 1;
        });
    }
});


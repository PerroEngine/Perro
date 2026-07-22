use perro_api::prelude::*;

type SelfNodeType = UiLabel;

#[State]
struct PatternHud {}

lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("score_changed"),
            func!("show_score")
        );
    }
});

methods!({
    fn show_score(&self, ctx: &mut ScriptContext<'_, API>, score: i32) {
        with_node_mut!(ctx.run, SelfNodeType, ctx.id, |label| {
            label.text = format!("Score: {score}").into();
        });
    }
});


use perro_api::prelude::*;

type SelfNodeType = UiPanel;

const TEXT_NODE_NAME: &str = "profiling_overlay_text";

#[State]
struct DemoProfilingOverlayState {
    #[default = NodeID::nil()]
    pub text: NodeID,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let text = get_child!(ctx.run, ctx.id, TEXT_NODE_NAME).unwrap_or(NodeID::nil());
        with_state_mut!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
            state.text = text;
        });
        self.sync_text(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        self.sync_text(ctx);
    }
});

methods!({
    fn sync_text(&self, ctx: &mut ScriptContext<'_, API>) {
        let text = with_state!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| state.text);
        if text.is_nil() {
            return;
        }

        let p = profiling!(ctx.run);
        let sim_us = p.simulation_time.as_micros();
        let render_us = p.graphics_time.as_micros();
        let frame_us = p.frame_time.as_micros();

        with_node_mut!(ctx.run, UiLabel, text, |label| {
            label.text = format!(
                "FPS {:.1}\nSim {} us\nRender {} us\nFrame {} us",
                p.fps, sim_us, render_us, frame_us
            )
            .into();
        });
    }
});

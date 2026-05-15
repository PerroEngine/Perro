use perro_api::prelude::*;

type SelfNodeType = UiPanel;

const FPS_LABEL_NODE_NAME: &str = "profiling_overlay_fps";
const SIM_LABEL_NODE_NAME: &str = "profiling_overlay_sim";
const GRAPHICS_LABEL_NODE_NAME: &str = "profiling_overlay_graphics";
const ROW_NODE_NAME: &str = "profiling_overlay_row";

const REFRESH_SECONDS: f32 = 2.0;
const PROFILE_SAMPLE_EVERY_FRAMES: u32 = 60;

#[State]
struct DemoProfilingOverlayState {
    #[default = NodeID::nil()]
    pub fps_label: NodeID,
    #[default = NodeID::nil()]
    pub sim_label: NodeID,
    #[default = NodeID::nil()]
    pub graphics_label: NodeID,
    pub refresh_timer: f32,
    #[default = 0]
    pub frame_counter: u32,
    pub fps_value: f32,
    pub dt_us_value: f32,
    pub sim_us_value: f32,
    pub graphics_us_value: f32,
    pub fps_sum: f32,
    pub dt_us_sum: f32,
    pub sim_us_sum: f32,
    pub graphics_us_sum: f32,
    pub timing_samples: u32,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let row = get_child!(ctx.run, ctx.id, ROW_NODE_NAME).unwrap_or(NodeID::nil());
        let fps_label = get_child!(ctx.run, row, FPS_LABEL_NODE_NAME).unwrap_or(NodeID::nil());
        let sim_label = get_child!(ctx.run, row, SIM_LABEL_NODE_NAME).unwrap_or(NodeID::nil());
        let graphics_label =
            get_child!(ctx.run, row, GRAPHICS_LABEL_NODE_NAME).unwrap_or(NodeID::nil());
        with_state_mut!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
            state.fps_label = fps_label;
            state.sim_label = sim_label;
            state.graphics_label = graphics_label;
        });
        self.refresh_text(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run).max(0.0);
        let (should_sample, should_refresh) =
            with_state_mut!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
                state.refresh_timer += dt;
                state.frame_counter = state.frame_counter.wrapping_add(1);
                let should_sample = state.frame_counter % PROFILE_SAMPLE_EVERY_FRAMES == 0;
                if state.refresh_timer >= REFRESH_SECONDS {
                    state.refresh_timer = 0.0;
                    if state.timing_samples > 0 {
                        let samples = state.timing_samples as f32;
                        state.fps_value = state.fps_sum / samples;
                        state.dt_us_value = state.dt_us_sum / samples;
                        state.sim_us_value = state.sim_us_sum / samples;
                        state.graphics_us_value = state.graphics_us_sum / samples;
                        state.fps_sum = 0.0;
                        state.dt_us_sum = 0.0;
                        state.sim_us_sum = 0.0;
                        state.graphics_us_sum = 0.0;
                        state.timing_samples = 0;
                    }
                    (should_sample, true)
                } else {
                    (should_sample, false)
                }
            })
            .unwrap_or((false, false));
        if should_sample {
            let p = profiling!(ctx.run);
            with_state_mut!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
                let fps = if p.fps.is_finite() && p.fps > 0.0 { p.fps } else { 0.0 };
                state.fps_sum += fps;
                state.dt_us_sum += dt * 1_000_000.0;
                state.sim_us_sum += p.simulation_time.as_micros() as f32;
                state.graphics_us_sum += p.graphics_time.as_micros() as f32;
                state.timing_samples = state.timing_samples.saturating_add(1);
            });
        }
        if should_refresh {
            self.refresh_text(ctx);
        }
    }
});

methods!({
    fn refresh_text(&self, ctx: &mut ScriptContext<'_, API>) {
        let (fps_label, sim_label, graphics_label, fps, dt_us, sim_us, graphics_us) =
            with_state!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
                (
                    state.fps_label,
                    state.sim_label,
                    state.graphics_label,
                    state.fps_value,
                    state.dt_us_value,
                    state.sim_us_value,
                    state.graphics_us_value,
                )
            });
        set_label_text(ctx, fps_label, format!("FPS {:.1}", fps));
        set_label_text(ctx, sim_label, format!("Sim {:.0} us | dt {:.0} us", sim_us, dt_us));
        set_label_text(ctx, graphics_label, format!("Gfx {:.0} us", graphics_us));
    }
});

fn set_label_text<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, id: NodeID, text: String) {
    if id.is_nil() {
        return;
    }
    with_node_mut!(ctx.run, UiLabel, id, |label| {
        label.text = text.into();
    });
}

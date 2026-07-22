use perro_api::prelude::*;

type SelfNodeType = UiPanel;

const REFRESH_SECONDS: f32 = 0.25;
const SCRIPT_FPS_WINDOW_SECONDS: f32 = 0.5;

#[State]
struct DemoProfilingOverlayState {
    #[default = NodeID::nil()]
    pub fps_label: NodeID,
    #[default = NodeID::nil()]
    pub cpu_label: NodeID,
    #[default = NodeID::nil()]
    pub delta_label: NodeID,
    #[default = NodeID::nil()]
    pub gfx_label: NodeID,
    #[default = NodeID::nil()]
    pub script_fps_label: NodeID,
    pub refresh_timer: f32,
    pub script_fps_timer: f32,
    pub script_fps_frames: i32,
    pub script_fps: f32,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        self.refresh_text(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run).max(0.0);

        let should_refresh =
            with_state_mut!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
                state.refresh_timer += dt;
                state.script_fps_timer += dt;
                state.script_fps_frames += 1;

                if state.script_fps_timer >= SCRIPT_FPS_WINDOW_SECONDS {
                    state.script_fps = state.script_fps_frames as f32 / state.script_fps_timer;
                    state.script_fps_timer = 0.0;
                    state.script_fps_frames = 0;
                }

                if state.refresh_timer >= REFRESH_SECONDS {
                    state.refresh_timer = 0.0;
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);

        if should_refresh {
            self.refresh_text(ctx);
        }
    }
});

methods!({
    fn refresh_text(&self, ctx: &mut ScriptContext<'_, API>) {
        let p = profiling!(ctx.run);

        let fps = if p.fps.is_finite() && p.fps > 0.0 {
            p.fps
        } else {
            0.0
        };
        let cpu_us = p.simulation_time.as_micros();
        let delta_us = p.frame_time.as_micros();
        let gfx_us = p.graphics_time.as_micros();

        let (fps_label, cpu_label, delta_label, gfx_label, script_fps_label, script_fps) =
            with_state!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
                (
                    state.fps_label,
                    state.cpu_label,
                    state.delta_label,
                    state.gfx_label,
                    state.script_fps_label,
                    state.script_fps,
                )
            }).unwrap_or_default();

        set_label_text(ctx, fps_label, format!("FPS {:.1}", fps));
        set_label_text(ctx, cpu_label, format!("CPU {} us", cpu_us));
        set_label_text(ctx, delta_label, format!("Delta {} us", delta_us));
        set_label_text(ctx, gfx_label, format!("Gfx {} us", gfx_us));
        set_label_text(ctx, script_fps_label, format!("Script {:.1}", script_fps.max(0.0)));
    }
});

fn set_label_text<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    id: NodeID,
    text: String,
) {
    if id.is_nil() {
        return;
    }

    with_node_mut!(ctx.run, UiLabel, id, |label| {
        label.text = text.into();
    });
}

use perro_api::prelude::*;
use perro_api::runtime_api::prelude::FrameRateCap;

type SelfNodeType = UiPanel;

const REFRESH_SECONDS: f32 = 0.25;
const SCRIPT_FPS_WINDOW_SECONDS: f32 = 0.5;
const CAP_COUNT: i32 = 6;

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
    pub cap_label: NodeID,
    #[default = NodeID::nil()]
    pub script_fps_label: NodeID,
    #[default = NodeID::nil()]
    pub strobe_label: NodeID,
    #[default = NodeID::nil()]
    pub strobe_dot: NodeID,
    pub refresh_timer: f32,
    pub script_fps_timer: f32,
    pub script_fps_frames: i32,
    pub script_fps: f32,
    pub cap_index: i32,
    pub visual_frame: i32,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        self.apply_cap(ctx, 0);
        self.refresh_text(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run).max(0.0);

        if key_pressed!(ctx.ipt, KeyCode::F6) {
            let next = with_state!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
                (state.cap_index + 1).rem_euclid(CAP_COUNT)
            });
            self.apply_cap(ctx, next);
        }
        if key_pressed!(ctx.ipt, KeyCode::F7) {
            let next = with_state!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
                (state.cap_index - 1).rem_euclid(CAP_COUNT)
            });
            self.apply_cap(ctx, next);
        }
        if key_pressed!(ctx.ipt, KeyCode::F8) {
            let next = with_state!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
                (state.cap_index + 1).rem_euclid(CAP_COUNT)
            });
            self.apply_cap(ctx, next);
        }

        self.tick_visual(ctx);

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
    fn apply_cap(&self, ctx: &mut ScriptContext<'_, API>, cap_index: i32) {
        let cap_index = cap_index.rem_euclid(CAP_COUNT);
        match cap_index {
            0 => ctx.run.Window().set_frame_rate_cap(FrameRateCap::Unlimited),
            1 => ctx.run.Window().set_frame_rate_cap(FrameRateCap::Fps(15.0)),
            2 => ctx.run.Window().set_frame_rate_cap(FrameRateCap::Fps(30.0)),
            3 => ctx.run.Window().set_frame_rate_cap(FrameRateCap::Fps(60.0)),
            4 => ctx.run.Window().set_frame_rate_cap(FrameRateCap::Fps(120.0)),
            _ => ctx.run.Window().set_frame_rate_cap(FrameRateCap::RefreshRate),
        }

        with_state_mut!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
            state.cap_index = cap_index;
        });
        self.refresh_text(ctx);
    }

    fn tick_visual(&self, ctx: &mut ScriptContext<'_, API>) {
        let (dot, label, frame) =
            with_state_mut!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
                state.visual_frame = state.visual_frame.wrapping_add(1);
                (state.strobe_dot, state.strobe_label, state.visual_frame)
            })
            .unwrap_or((NodeID::nil(), NodeID::nil(), 0));

        let phase = frame.rem_euclid(12) as f32 / 11.0;
        let odd = frame & 1 != 0;
        let x = 0.06 + phase * 0.88;
        with_node_mut!(ctx.run, UiShape, dot, |dot| {
            dot.base.transform.position = UiVector2::ratio(x, 0.5);
            dot.fill = if odd {
                Color::new(1.0, 0.86, 0.10, 1.0)
            } else {
                Color::new(0.10, 0.90, 1.0, 1.0)
            };
        });

        set_label_text(ctx, label, format!("Tick {:04}", frame.rem_euclid(10000)));
    }

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

        let (fps_label, cpu_label, delta_label, gfx_label, cap_label, script_fps_label, cap_index, script_fps) =
            with_state!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
                (
                    state.fps_label,
                    state.cpu_label,
                    state.delta_label,
                    state.gfx_label,
                    state.cap_label,
                    state.script_fps_label,
                    state.cap_index,
                    state.script_fps,
                )
            });

        set_label_text(ctx, fps_label, format!("FPS {:.1}", fps));
        set_label_text(ctx, cpu_label, format!("CPU {} us", cpu_us));
        set_label_text(ctx, delta_label, format!("Delta {} us", delta_us));
        set_label_text(ctx, gfx_label, format!("Gfx {} us", gfx_us));
        set_label_text(ctx, cap_label, format!("Cap {}", cap_name(cap_index)));
        set_label_text(ctx, script_fps_label, format!("Script {:.1}", script_fps.max(0.0)));
    }
});

fn cap_name(index: i32) -> &'static str {
    match index.rem_euclid(CAP_COUNT) {
        0 => "off",
        1 => "15",
        2 => "30",
        3 => "60",
        4 => "120",
        _ => "refresh",
    }
}

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

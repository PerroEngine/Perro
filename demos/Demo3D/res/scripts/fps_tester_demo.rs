use perro_api::prelude::*;
use perro_api::runtime_api::prelude::FrameRateCap;

type SelfNodeType = Node3D;

const FPS_WINDOW_SECONDS: f32 = 0.5;
const LABEL_REFRESH_SECONDS: f32 = 0.15;

#[State]
struct FpsTesterDemoState {
    #[default = NodeID::nil()]
    pub status_label: NodeID,
    #[default = NodeID::nil()]
    pub profiler_label: NodeID,
    #[default = NodeID::nil()]
    pub render_dot: NodeID,
    #[default = NodeID::nil()]
    pub cap_dot: NodeID,
    #[default = NodeID::nil()]
    pub smooth_dot: NodeID,
    #[default = NodeID::nil()]
    pub render_cube: NodeID,
    #[default = NodeID::nil()]
    pub cap_cube: NodeID,
    #[default = NodeID::nil()]
    pub smooth_cube: NodeID,
    pub target_fps: f32,
    pub render_frame: i32,
    pub cap_frame: i32,
    pub cap_accum: f32,
    pub sweep_time: f32,
    pub fps_timer: f32,
    pub fps_frames: i32,
    pub script_fps: f32,
    pub label_timer: f32,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        for (sig, func_name) in [
            ("fps_test_10_click", "on_cap_10_click"),
            ("fps_test_30_click", "on_cap_30_click"),
            ("fps_test_60_click", "on_cap_60_click"),
            ("fps_test_120_click", "on_cap_120_click"),
            ("fps_test_144_click", "on_cap_144_click"),
        ] {
            signal_connect!(ctx.run, ctx.id, signal!(sig), func!(func_name));
        }
        self.apply_cap(ctx, 30.0);
        self.refresh_labels(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run).clamp(0.0, 0.25);
        let (target_fps, render_frame, cap_frame, sweep_time, script_fps, refresh) =
            with_state_mut!(ctx.run, FpsTesterDemoState, ctx.id, |state| {
                state.render_frame = state.render_frame.wrapping_add(1);
                state.cap_accum += dt;
                let step = 1.0 / state.target_fps.max(1.0);
                while state.cap_accum >= step {
                    state.cap_accum -= step;
                    state.cap_frame = state.cap_frame.wrapping_add(1);
                }
                state.sweep_time = (state.sweep_time + dt).rem_euclid(2.0);
                state.fps_timer += dt;
                state.fps_frames += 1;
                if state.fps_timer >= FPS_WINDOW_SECONDS {
                    state.script_fps = state.fps_frames as f32 / state.fps_timer.max(0.001);
                    state.fps_timer = 0.0;
                    state.fps_frames = 0;
                }
                state.label_timer += dt;
                let refresh = state.label_timer >= LABEL_REFRESH_SECONDS;
                if refresh {
                    state.label_timer = 0.0;
                }
                (
                    state.target_fps,
                    state.render_frame,
                    state.cap_frame,
                    state.sweep_time,
                    state.script_fps,
                    refresh,
                )
            })
            .unwrap_or((30.0, 0, 0, 0.0, 0.0, false));

        self.move_visuals(ctx, target_fps, render_frame, cap_frame, sweep_time);
        if refresh {
            self.refresh_labels(ctx);
        } else {
            self.refresh_profiler_label(ctx, script_fps);
        }
    }
});

methods!({
    fn on_cap_10_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.apply_cap(ctx, 10.0);
    }

    fn on_cap_30_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.apply_cap(ctx, 30.0);
    }

    fn on_cap_60_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.apply_cap(ctx, 60.0);
    }

    fn on_cap_120_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.apply_cap(ctx, 120.0);
    }

    fn on_cap_144_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.apply_cap(ctx, 144.0);
    }

    fn set_info_overlay(&self, ctx: &mut ScriptContext<'_, API>, overlay: NodeID) {
        if overlay.is_nil() {
            return;
        }
        let _ = call_method!(
            ctx.run,
            overlay,
            func!("set_content"),
            params![
                "FPS Tester".to_string(),
                "btn caps 10/30/60/120/144\nrender tick vs cap tick".to_string()
            ]
        );
    }

    fn apply_cap(&self, ctx: &mut ScriptContext<'_, API>, fps: f32) {
        ctx.run.Window().set_frame_rate_cap(FrameRateCap::Fps(fps));
        with_state_mut!(ctx.run, FpsTesterDemoState, ctx.id, |state| {
            state.target_fps = fps;
            state.cap_accum = 0.0;
            state.cap_frame = 0;
        });
        self.refresh_labels(ctx);
    }

    fn move_visuals(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        target_fps: f32,
        render_frame: i32,
        cap_frame: i32,
        sweep_time: f32,
    ) {
        let render_phase = render_frame.rem_euclid(60) as f32 / 59.0;
        let cap_mod = target_fps.round().max(1.0) as i32;
        let cap_phase = cap_frame.rem_euclid(cap_mod) as f32 / cap_mod.max(1) as f32;
        let smooth_phase = sweep_time / 2.0;

        let render_dot = state_node(ctx, |s| s.render_dot);
        let cap_dot = state_node(ctx, |s| s.cap_dot);
        let smooth_dot = state_node(ctx, |s| s.smooth_dot);
        let render_cube = state_node(ctx, |s| s.render_cube);
        let cap_cube = state_node(ctx, |s| s.cap_cube);
        let smooth_cube = state_node(ctx, |s| s.smooth_cube);

        set_dot(ctx, render_dot, render_phase, render_frame);
        set_dot(ctx, cap_dot, cap_phase, cap_frame);
        set_dot(ctx, smooth_dot, smooth_phase, render_frame);
        move_cube(ctx, render_cube, render_phase, 0.6, render_frame);
        move_cube(ctx, cap_cube, cap_phase, 1.55, cap_frame);
        move_cube(ctx, smooth_cube, smooth_phase, 2.5, render_frame);
    }

    fn refresh_labels(&self, ctx: &mut ScriptContext<'_, API>) {
        let target_fps = with_state!(ctx.run, FpsTesterDemoState, ctx.id, |state| state
            .target_fps);
        let status_label = state_node(ctx, |s| s.status_label);
        set_label(
            ctx,
            status_label,
            format!("Cap {} fps | render row must look choppy", target_fps as i32),
        );
        let script_fps = with_state!(ctx.run, FpsTesterDemoState, ctx.id, |state| state
            .script_fps);
        self.refresh_profiler_label(ctx, script_fps);
    }

    fn refresh_profiler_label(&self, ctx: &mut ScriptContext<'_, API>, script_fps: f32) {
        let p = profiling!(ctx.run);
        let profiler_fps = if p.fps.is_finite() && p.fps > 0.0 {
            p.fps
        } else {
            0.0
        };
        let profiler_label = state_node(ctx, |s| s.profiler_label);
        set_label(
            ctx,
            profiler_label,
            format!("Profiler {:.1} | script {:.1}", profiler_fps, script_fps.max(0.0)),
        );
    }
});

fn state_node<API: ScriptAPI + ?Sized, F: FnOnce(&FpsTesterDemoState) -> NodeID>(
    ctx: &mut ScriptContext<'_, API>,
    read: F,
) -> NodeID {
    with_state!(ctx.run, FpsTesterDemoState, ctx.id, |state| read(state))
}

fn set_dot<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    dot: NodeID,
    phase: f32,
    frame: i32,
) {
    if dot.is_nil() {
        return;
    }
    let x = 0.06 + phase.clamp(0.0, 1.0) * 0.88;
    let odd = frame & 1 != 0;
    with_node_mut!(ctx.run, UiShape, dot, |node| {
        node.base.transform.position = UiVector2::ratio(x, 0.5);
        node.fill = if odd {
            Color::new(1.0, 0.80, 0.12, 1.0)
        } else {
            Color::new(0.08, 0.90, 1.0, 1.0)
        };
    });
}

fn move_cube<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    cube: NodeID,
    phase: f32,
    y: f32,
    frame: i32,
) {
    if cube.is_nil() {
        return;
    }
    let x = -4.0 + phase.clamp(0.0, 1.0) * 8.0;
    let _ = set_local_pos_3d!(ctx.run, cube, Vector3::new(x, y, 0.0));
    let _ = set_local_rot_3d!(
        ctx.run,
        cube,
        Quaternion::from_euler_xyz(frame as f32 * 0.07, frame as f32 * 0.11, 0.0)
    );
}

fn set_label<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, id: NodeID, text: String) {
    if id.is_nil() {
        return;
    }
    with_node_mut!(ctx.run, UiLabel, id, |label| {
        label.text = text.into();
    });
}

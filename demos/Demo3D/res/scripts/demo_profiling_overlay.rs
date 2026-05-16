use perro_api::prelude::*;

type SelfNodeType = UiPanel;

const FPS_LABEL_NODE_NAME: &str = "profiling_overlay_fps";
const SIM_LABEL_NODE_NAME: &str = "profiling_overlay_sim";
const GRAPHICS_LABEL_NODE_NAME: &str = "profiling_overlay_graphics";
const PREP_LABEL_NODE_NAME: &str = "profiling_overlay_prep";
const DRAW_LABEL_NODE_NAME: &str = "profiling_overlay_draw";
const ROOT_COLUMN_NODE_NAME: &str = "profiling_overlay_col";

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
    #[default = NodeID::nil()]
    pub prep_label: NodeID,
    #[default = NodeID::nil()]
    pub draw_label: NodeID,

    pub refresh_timer: f32,

    #[default = 0]
    pub frame_counter: u32,

    pub fps_value: f32,
    pub dt_us_value: f32,
    pub sim_us_value: f32,
    pub graphics_us_value: f32,
    pub frame_us_value: f32,
    pub prep_3d_us_value: f32,
    pub prep_frustum_us_value: f32,
    pub prep_hiz_us_value: f32,
    pub prep_indirect_us_value: f32,
    pub prep_cull_inputs_us_value: f32,
    pub draw_calls_2d_value: f32,
    pub draw_calls_3d_value: f32,
    pub draw_calls_total_value: f32,
    pub draw_instances_3d_value: f32,
    pub draw_material_refs_3d_value: f32,
    pub skip_prepare_3d_value: f32,
    pub skip_prepare_3d_frustum_value: f32,
    pub skip_prepare_3d_hiz_value: f32,
    pub skip_prepare_3d_indirect_value: f32,
    pub skip_prepare_3d_cull_inputs_value: f32,

    pub fps_sum: f32,
    pub dt_us_sum: f32,
    pub sim_us_sum: f32,
    pub graphics_us_sum: f32,
    pub frame_us_sum: f32,
    pub prep_3d_us_sum: f32,
    pub prep_frustum_us_sum: f32,
    pub prep_hiz_us_sum: f32,
    pub prep_indirect_us_sum: f32,
    pub prep_cull_inputs_us_sum: f32,
    pub draw_calls_2d_sum: f32,
    pub draw_calls_3d_sum: f32,
    pub draw_calls_total_sum: f32,
    pub draw_instances_3d_sum: f32,
    pub draw_material_refs_3d_sum: f32,
    pub skip_prepare_3d_sum: f32,
    pub skip_prepare_3d_frustum_sum: f32,
    pub skip_prepare_3d_hiz_sum: f32,
    pub skip_prepare_3d_indirect_sum: f32,
    pub skip_prepare_3d_cull_inputs_sum: f32,

    pub timing_samples: u32,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let root_col = get_child!(ctx.run, ctx.id, ROOT_COLUMN_NODE_NAME).unwrap_or(NodeID::nil());

        let fps_label =
            get_child!(ctx.run, root_col, FPS_LABEL_NODE_NAME).unwrap_or(NodeID::nil());

        let sim_label =
            get_child!(ctx.run, root_col, SIM_LABEL_NODE_NAME).unwrap_or(NodeID::nil());

        let graphics_label =
            get_child!(ctx.run, root_col, GRAPHICS_LABEL_NODE_NAME)
                .unwrap_or(NodeID::nil());

        let prep_label =
            get_child!(ctx.run, root_col, PREP_LABEL_NODE_NAME).unwrap_or(NodeID::nil());

        let draw_label =
            get_child!(ctx.run, root_col, DRAW_LABEL_NODE_NAME).unwrap_or(NodeID::nil());

        with_state_mut!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
            state.fps_label = fps_label;
            state.sim_label = sim_label;
            state.graphics_label = graphics_label;
            state.prep_label = prep_label;
            state.draw_label = draw_label;
        });

        self.refresh_text(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run).max(0.0);

        let (should_sample, should_refresh) =
            with_state_mut!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
                state.refresh_timer += dt;
                state.frame_counter = state.frame_counter.wrapping_add(1);
                let should_sample =
                    state.frame_counter % PROFILE_SAMPLE_EVERY_FRAMES == 0;

                if state.refresh_timer >= REFRESH_SECONDS {
                    state.refresh_timer = 0.0;

                    if state.timing_samples > 0 {
                        let samples = state.timing_samples as f32;

                        state.fps_value = state.fps_sum / samples;
                        state.dt_us_value = state.dt_us_sum / samples;
                        state.sim_us_value = state.sim_us_sum / samples;
                        state.graphics_us_value =
                            state.graphics_us_sum / samples;
                        state.frame_us_value = state.frame_us_sum / samples;
                        state.prep_3d_us_value = state.prep_3d_us_sum / samples;
                        state.prep_frustum_us_value =
                            state.prep_frustum_us_sum / samples;
                        state.prep_hiz_us_value = state.prep_hiz_us_sum / samples;
                        state.prep_indirect_us_value =
                            state.prep_indirect_us_sum / samples;
                        state.prep_cull_inputs_us_value =
                            state.prep_cull_inputs_us_sum / samples;
                        state.draw_calls_2d_value = state.draw_calls_2d_sum / samples;
                        state.draw_calls_3d_value = state.draw_calls_3d_sum / samples;
                        state.draw_calls_total_value =
                            state.draw_calls_total_sum / samples;
                        state.draw_instances_3d_value =
                            state.draw_instances_3d_sum / samples;
                        state.draw_material_refs_3d_value =
                            state.draw_material_refs_3d_sum / samples;
                        state.skip_prepare_3d_value =
                            state.skip_prepare_3d_sum / samples;
                        state.skip_prepare_3d_frustum_value =
                            state.skip_prepare_3d_frustum_sum / samples;
                        state.skip_prepare_3d_hiz_value =
                            state.skip_prepare_3d_hiz_sum / samples;
                        state.skip_prepare_3d_indirect_value =
                            state.skip_prepare_3d_indirect_sum / samples;
                        state.skip_prepare_3d_cull_inputs_value =
                            state.skip_prepare_3d_cull_inputs_sum / samples;

                        state.fps_sum = 0.0;
                        state.dt_us_sum = 0.0;
                        state.sim_us_sum = 0.0;
                        state.graphics_us_sum = 0.0;
                        state.frame_us_sum = 0.0;
                        state.prep_3d_us_sum = 0.0;
                        state.prep_frustum_us_sum = 0.0;
                        state.prep_hiz_us_sum = 0.0;
                        state.prep_indirect_us_sum = 0.0;
                        state.prep_cull_inputs_us_sum = 0.0;
                        state.draw_calls_2d_sum = 0.0;
                        state.draw_calls_3d_sum = 0.0;
                        state.draw_calls_total_sum = 0.0;
                        state.draw_instances_3d_sum = 0.0;
                        state.draw_material_refs_3d_sum = 0.0;
                        state.skip_prepare_3d_sum = 0.0;
                        state.skip_prepare_3d_frustum_sum = 0.0;
                        state.skip_prepare_3d_hiz_sum = 0.0;
                        state.skip_prepare_3d_indirect_sum = 0.0;
                        state.skip_prepare_3d_cull_inputs_sum = 0.0;

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
                let fps = if p.fps.is_finite() && p.fps > 0.0 {
                    p.fps
                } else {
                    0.0
                };

                state.fps_sum += fps;
                state.dt_us_sum += dt * 1_000_000.0;
                state.sim_us_sum += p.simulation_time.as_micros() as f32;
                state.graphics_us_sum += p.graphics_time.as_micros() as f32;
                state.frame_us_sum += p.frame_time.as_micros() as f32;
                state.prep_3d_us_sum += p.draw_gpu_prepare_3d.as_micros() as f32;
                state.prep_frustum_us_sum +=
                    p.draw_gpu_prepare_3d_frustum.as_micros() as f32;
                state.prep_hiz_us_sum += p.draw_gpu_prepare_3d_hiz.as_micros() as f32;
                state.prep_indirect_us_sum +=
                    p.draw_gpu_prepare_3d_indirect.as_micros() as f32;
                state.prep_cull_inputs_us_sum +=
                    p.draw_gpu_prepare_3d_cull_inputs.as_micros() as f32;
                state.draw_calls_2d_sum += p.draw_calls_2d as f32;
                state.draw_calls_3d_sum += p.draw_calls_3d as f32;
                state.draw_calls_total_sum += p.draw_calls_total as f32;
                state.draw_instances_3d_sum += p.draw_instances_3d as f32;
                state.draw_material_refs_3d_sum += p.draw_material_refs_3d as f32;
                state.skip_prepare_3d_sum += p.skip_prepare_3d as f32;
                state.skip_prepare_3d_frustum_sum += p.skip_prepare_3d_frustum as f32;
                state.skip_prepare_3d_hiz_sum += p.skip_prepare_3d_hiz as f32;
                state.skip_prepare_3d_indirect_sum += p.skip_prepare_3d_indirect as f32;
                state.skip_prepare_3d_cull_inputs_sum +=
                    p.skip_prepare_3d_cull_inputs as f32;
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
        let (
            fps_label,
            sim_label,
            graphics_label,
            prep_label,
            draw_label,
            fps,
            dt_us,
            sim_us,
            graphics_us,
            frame_us,
            prep_3d_us,
            prep_frustum_us,
        ) = with_state!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
            (
                state.fps_label,
                state.sim_label,
                state.graphics_label,
                state.prep_label,
                state.draw_label,
                state.fps_value,
                state.dt_us_value,
                state.sim_us_value,
                state.graphics_us_value,
                state.frame_us_value,
                state.prep_3d_us_value,
                state.prep_frustum_us_value,
            )
        });

        let (
            prep_hiz_us,
            prep_indirect_us,
            prep_cull_inputs_us,
            draw_calls_2d,
            draw_calls_3d,
            draw_calls_total,
            draw_instances_3d,
            draw_material_refs_3d,
            skip_prepare_3d,
            skip_prepare_3d_frustum,
            skip_prepare_3d_hiz,
            skip_prepare_3d_indirect,
        ) = with_state!(ctx.run, DemoProfilingOverlayState, ctx.id, |state| {
            (
                state.prep_hiz_us_value,
                state.prep_indirect_us_value,
                state.prep_cull_inputs_us_value,
                state.draw_calls_2d_value,
                state.draw_calls_3d_value,
                state.draw_calls_total_value,
                state.draw_instances_3d_value,
                state.draw_material_refs_3d_value,
                state.skip_prepare_3d_value,
                state.skip_prepare_3d_frustum_value,
                state.skip_prepare_3d_hiz_value,
                state.skip_prepare_3d_indirect_value,
            )
        });

        let skip_prepare_3d_cull_inputs = with_state!(
            ctx.run,
            DemoProfilingOverlayState,
            ctx.id,
            |state| state.skip_prepare_3d_cull_inputs_value
        );

        set_label_text(
            ctx,
            fps_label,
            format!("FPS {:.1}", fps),
        );

        set_label_text(
            ctx,
            sim_label,
            format!("Sim {:.0} us | dt {:.0} us", sim_us, dt_us),
        );

        set_label_text(
            ctx,
            graphics_label,
            format!("Gfx {:.0} us | frame {:.0} us", graphics_us, frame_us),
        );

        set_label_text(
            ctx,
            prep_label,
            format!(
                "3D prep {:.0} us | fr {:.0} | hiz {:.0} | ind {:.0} | cin {:.0}",
                prep_3d_us,
                prep_frustum_us,
                prep_hiz_us,
                prep_indirect_us,
                prep_cull_inputs_us,
            ),
        );

        set_label_text(
            ctx,
            draw_label,
            format!(
                "Draw 2d {:.0} | 3d {:.0} | all {:.0} | inst {:.0} | mat {:.0} | skip {:.0}/{:.0}/{:.0}/{:.0}/{:.0}",
                draw_calls_2d,
                draw_calls_3d,
                draw_calls_total,
                draw_instances_3d,
                draw_material_refs_3d,
                skip_prepare_3d,
                skip_prepare_3d_frustum,
                skip_prepare_3d_hiz,
                skip_prepare_3d_indirect,
                skip_prepare_3d_cull_inputs,
            ),
        );
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

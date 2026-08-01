use perro_api::prelude::*;

// Bench-only: spins the node it is attached to so the sub-view it lives in
// never hits the per-stream idle skip. Used by res://scenes/bench/sv_live.scn
// to measure a continuously re-rendered sub view.

#[State]
struct BenchSubViewSpinState {
    #[default = 0.8]
    pub speed: f32,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let speed =
            with_state!(ctx.run, BenchSubViewSpinState, ctx.id, |state| state.speed).unwrap_or(0.8);
        let angle = elapsed_time!(ctx.run) * speed;
        let rot = Quaternion::from_euler_xyz(0.0, angle, 0.0);
        let _ = with_node_mut!(ctx.run, Node3D, ctx.id, |node| {
            node.transform.rotation = rot;
        });
    }
});

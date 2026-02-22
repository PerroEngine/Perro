use perro_context::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;

type SelfNodeType = Camera3D;

#[State]
pub struct CameraState {
    #[default = 1.0]
    job: f32
}


lifecycle!({
    fn on_init(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {

        let j = with_state!(ctx, CameraState, self_id, |state| {
            state.job
        }).unwrap_or_default();
        log_info!(j);
    }

    fn on_all_init(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}

    fn on_update(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        let dt = delta_time!(ctx);
        call_method!(ctx, NodeID(4), smid!("bob"), params![7123_i32, "bodsasb"]);
           let j2 = with_state!(ctx, CameraState, self_id, |state| {
            state.job
        }).unwrap_or_default();

    }

    fn on_fixed_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}

    fn on_removal(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
});

methods!({
    fn bob(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID, param1: i32, j: &str) {
        let j = with_state_mut!(ctx, CameraState, self_id, |state| {
            state.job += 1.0;
            state.job
        }).unwrap_or_default();
    }
});


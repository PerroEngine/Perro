use perro_context::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;

type SelfNodeType = Camera3D;

///@State
#[derive(Default)]
pub struct CameraState {
    job: i32
}

///@Script
pub struct CameraScript;

const SPEED: f32 = 5.0;

impl<R: RuntimeAPI + ?Sized> ScriptLifecycle<R> for CameraScript {
    fn init(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        let j = with_state_mut!(ctx, CameraState, self_id, |state| {
            state.job = 123;
            state.job
        }).unwrap_or_default();
    }

    fn update(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        let dt = delta_time!(ctx);
        call_method!(ctx, NodeID(4), ScriptMemberID::from_string("bob"), params![7123_i32, "bodsasb"]);
           let j2 = with_state!(ctx, CameraState, self_id, |state| {
            state.job
        }).unwrap_or_default();

    }

    fn fixed_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
}

impl CameraScript {
    pub fn bob<R: RuntimeAPI + ?Sized>(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID, param1: i32, j: &str) {
        let j = with_state_mut!(ctx, CameraState, self_id, |state| {
            state.job += 1;
            state.job
        }).unwrap_or_default();
    }
}

use perro_context::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;

type SelfNodeType = Camera3D;

///@State
#[derive(Default)]
pub struct CameraState {
}

///@Script
pub struct CameraScript;

const SPEED: f32 = 5.0;

impl<R: RuntimeAPI + ?Sized> ScriptLifecycle<R> for CameraScript {
    fn init(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
    }

    fn update(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        let dt = delta_time!(ctx);

    }

    fn fixed_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
}

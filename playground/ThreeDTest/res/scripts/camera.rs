use perro_api::prelude::*;
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
    fn init(&self, api: &mut API<'_, R>, self_id: NodeID) {
    }

    fn update(&self, api: &mut API<'_, R>, self_id: NodeID) {
        let dt = api.Time().get_delta();

    }

    fn fixed_update(&self, _api: &mut API<'_, R>, _self_id: NodeID) {}
}

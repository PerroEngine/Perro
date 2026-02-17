use perro_api::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_scripting::prelude::*;

///@State
#[derive(Default)]
pub struct ExampleState {
    speed: f32,
}

///@Script
pub struct ExampleScript;

impl<R: RuntimeAPI + ?Sized> ScriptLifecycle<R> for ExampleScript {
    fn init(&self, api: &mut API<'_, R>, self_id: NodeID) {
        let _origin = Vector2::new(0.0, 0.0);
        let _ = api
            .Scripts()
            .with_state_mut::<ExampleState, _, _>(self_id, |state| {
                state.speed = 240.0;
            });
    }

    fn update(&self, api: &mut API<'_, R>, self_id: NodeID) {
        let dt = api.Time().get_delta();
        let _ = api
            .Scripts()
            .with_state_mut::<ExampleState, _, _>(self_id, |state| {
                state.speed += dt;
            });
            self.say_hi(api, self_id);
    }

    fn fixed_update(&self, _api: &mut API<'_, R>, _self_id: NodeID) {}
}

impl ExampleScript {
    fn say_hi<R: RuntimeAPI + ?Sized>(&self, api: &mut API<'_, R>, _self_id: NodeID) {
        let time = api.Time().get_delta();
        println!("Hi! fps: {}", 1.0/time);
    }
}

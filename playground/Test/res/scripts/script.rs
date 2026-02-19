use perro_api::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
#[allow(unused_imports)]
use perro_modules::prelude::*;
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
                state.speed = 5.0;
            });
    }

    fn update(&self, api: &mut API<'_, R>, self_id: NodeID) {
        let dt = api.Time().get_delta();
        let speed = api
            .Scripts()
            .with_state_mut::<ExampleState, _, _>(self_id, |state| {
                state.speed
            })
            .unwrap_or_default();
        api.Nodes().mutate::<MeshInstance3D, _>(self_id, |mesh| {
            mesh.base.transform.rotation.rotate_y(dt * speed);
            mesh.base.transform.rotation.rotate_z(dt * speed / 2.0);
        });
    }

    fn fixed_update(&self, _api: &mut API<'_, R>, _self_id: NodeID) {}
}

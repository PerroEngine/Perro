use perro_context::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_modules::{log, prelude::*};
use perro_scripting::prelude::*;

type SelfNodeType = Node2D;

#[State]
pub struct ExampleState {
    #[default = 50.0]
    speed: f32,

    #[default = true]
    bool_var: bool,
}

///@Script
pub struct ExampleScript;

impl<R: RuntimeAPI + ?Sized> ScriptLifecycle<R> for ExampleScript {
    fn on_init(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        let _origin = Vector2::new(0.0, 0.0);
        log_info!("Script initialized!");
        let b = ctx
            .Scripts()
            .with_state::<ExampleState, _, _>(self_id, |state| {
                state.bool_var
            }).unwrap_or_default();

        log_info!(b);
    }

    fn on_update(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        let dt = delta_time!(ctx);
        let _ = ctx
            .Scripts()
            .with_state_mut::<ExampleState, _, _>(self_id, |state| {
                state.speed += dt;
            });
    }

    fn on_fixed_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
}

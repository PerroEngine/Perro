use perro_context::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;

type SelfNodeType = MeshInstance3D;

///@State
#[derive(Default)]
pub struct ExampleState {
    speed: f32,
    bob: i32
}

///@Script
pub struct ExampleScript;

impl<R: RuntimeAPI + ?Sized> ScriptLifecycle<R> for ExampleScript {
    fn init(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        with_state_mut!(ctx, ExampleState, self_id, |state| {
            state.speed = 5.0;
            state.bob = 42;
        });
    }

    fn update(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        let dt = delta_time!(ctx);
        let speed = with_state!(ctx, ExampleState, self_id, |state| state.speed).unwrap_or_default();
        mutate_node!(ctx, SelfNodeType, self_id, |mesh| {
            mesh.scale.x += dt * speed;
            mesh.rotation.rotate_z(dt * speed / 2.0);
        });

    }

    fn fixed_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
}



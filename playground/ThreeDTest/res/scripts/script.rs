use perro_context::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;

type SelfNodeType = MeshInstance3D;

#[State]
pub struct ExampleState {
    #[default = 5.0]
    speed: f32,
    #[default = 422]
    bob: i32
}


lifecycle!({
    fn on_init(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        set_var!(ctx, self_id, var_id!("speed"), variant!(12.0));
        connect_signal!(ctx, self_id, sig_id!("test_signal"), func_id!("set_speed"));
    }

    fn on_all_init(&self, ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {
        emit_signal!(ctx, sig_id!("test_signal"), params![42_f32]);
    }

    fn on_update(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        let dt = delta_time!(ctx);
        let speed = with_state!(ctx, ExampleState, self_id, |state| {
            state.speed
        }).unwrap_or_default();
        let b = with_node_mut!(ctx, SelfNodeType, self_id, |mesh| {
            mesh.scale.x += dt * speed;
            mesh.rotation.rotate_z(dt * speed / 2.0);
            mesh.position
        }).unwrap_or_default();
    }

    fn on_fixed_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}

    fn on_removal(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
});

methods!({
    fn set_speed(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID, speed: f32) {
        log_info!(format!("Setting speed to {}", speed));
        with_state_mut!(ctx, ExampleState, self_id, |state| {
            state.speed = speed;
        });
    }

    fn get_speed(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) -> f32 {
        with_state!(ctx, ExampleState, self_id, |state| {
            state.speed
        }).unwrap_or_default()
    }
});


use perro_runtime_context::prelude::*;
use perro_nodes::prelude::*;
use perro_structs::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_resource_context::prelude::*;
use perro_scripting::prelude::*;

type SelfNodeType = MeshInstance3D;

#[State]
pub struct ExampleState {
    #[default = 5.0]
    speed: f32,
    #[default = 0.0]
    timer: f32
}


lifecycle!({
    fn on_init(&self, ctx: &mut RuntimeContext<'_, RT>, res: &ResourceContext<'_, RS>, self_id: NodeID) {
        self.set_speed(ctx, res, self_id, 12.0);
        with_node_mut!(ctx, SelfNodeType, self_id, |mesh| {
                   let id = res.Meshes().load("res://models/2Noses.glb:mesh[3]");
                   let id = res.Meshes().load("res://models/2Noses.glb:mesh[2]");
                   log_info!(format!("Loaded mesh with id: {}", id));
                   let id2 = res.Meshes().load("res://models/2Noses.glb:mesh[0]");
                    log_info!(format!("Loaded mesh with id: {}", id2));
                    mesh.mesh = id2;
                    log_info!(mesh.mesh);
                });
        connect_signal!(ctx, self_id, signal!("test_signal1"), func!("set_speed"));
    }

    fn on_all_init(&self, ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, _self: NodeID) {
        emit_signal!(ctx, signal!("test_signal1"), params![7_f32]);
    }

    fn on_update(&self, ctx: &mut RuntimeContext<'_, RT>, res: &ResourceContext<'_, RS>, self_id: NodeID) {
        let dt = delta_time!(ctx);
        let (speed, timer) = with_state_mut!(ctx, ExampleState, self_id, |state| {
            if state.timer >= 0.0 {
                state.timer += dt; 
            }
            (state.speed, state.timer)
        }).unwrap_or_default();

        if timer > 5.0 {
            with_node_mut!(ctx, SelfNodeType, self_id, |mesh| {
                mesh.mesh = res.Meshes().load("res://models/2Noses.glb:mesh[1]");
            }).unwrap_or_default();
            with_state_mut!(ctx, ExampleState, self_id, |state| {
                state.timer = -1.0;
            });
        }


        let b = with_node_mut!(ctx, SelfNodeType, self_id, |mesh| {
            mesh.rotation.rotate_z(dt * speed / 2.0);
            mesh.position;
        }).unwrap_or_default();
    }

    fn on_fixed_update(&self, _ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, _self_id: NodeID) {}

    fn on_removal(&self, _ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, _self_id: NodeID) {}
});

methods!({
    fn set_speed(&self, ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, self_id: NodeID, speed: f32) {
        log_info!(format!("Setting speed to {}", speed));
        with_state_mut!(ctx, ExampleState, self_id, |state| {
            state.speed = speed;
        });
    }

    fn get_speed(&self, ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, self_id: NodeID) -> f32 {
        with_state!(ctx, ExampleState, self_id, |state| {
            state.speed
        }).unwrap_or_default()
    }
});












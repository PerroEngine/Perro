use perro_nodes::prelude::*;
use perro_structs::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;

type SelfNodeType = MeshInstance3D;

#[State]
pub struct ExampleState {
    #[default = 0.0]
    ///@Expose
    speed: f32,

    #[default = 0.0]
    timer: f32,


}


lifecycle!({
    fn on_init(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
        self.set_speed(ctx, res, ipt, self_id, 5.0);
        signal_connect!(ctx, self_id, signal!("test_signal1"), func!("set_speed"));

    }

    fn on_all_init(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self: NodeID,
    ) {
        signal_emit!(ctx, signal!("test_signal1"), params![7_f32]);
    }

    fn on_update(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
        let dt = delta_time!(ctx);
        let (speed, timer) = with_state_mut!(ctx, ExampleState, self_id, |state| {
            
            if state.timer >= 0.0 {
                state.timer += dt; 
            }
            (state.speed, state.timer)
        }).unwrap_or_default();

        if timer > 3.0 {
            let tags = get_node_tags!(ctx, self_id).unwrap_or_default();
            if tags.contains(&tag!("mesh_change")) {
  
            
            with_node_mut!(ctx, SelfNodeType, self_id, |mesh| {
                mesh.mesh = res.Meshes().load("res://models/2Noses.glb:mesh[1]");
                mesh.material = res.Materials().load("res://materials/mat.pmat");
            }).unwrap_or_default(); }
            with_state_mut!(ctx, ExampleState, self_id, |state| {
                state.timer = -1.0;
            });
        }


        let b = with_node_mut!(ctx, SelfNodeType, self_id, |mesh| {
            mesh.rotation.rotate_y(dt * speed / 2.0);
            mesh.position;
        }).unwrap_or_default();
    }

    fn on_fixed_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {}

    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {}
});

methods!({
    fn set_speed(&self, ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, _ipt: &InputContext<'_, IP>, self_id: NodeID, speed: f32) {
        log_info!(format!("Setting speed to {}", speed));
        with_state_mut!(ctx, ExampleState, self_id, |state| {
            state.speed = speed;
        });
    }

    fn get_speed(&self, ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, _ipt: &InputContext<'_, IP>, self_id: NodeID) -> f32 {
        with_state!(ctx, ExampleState, self_id, |state| {
            state.speed
        }).unwrap_or_default()
    }
});











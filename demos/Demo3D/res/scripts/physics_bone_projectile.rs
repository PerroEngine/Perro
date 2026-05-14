use perro_api::prelude::*;

type SelfNodeType = BoneCollider3D;

const PROJECTILE_MESH_NODE_NAME: &str = "ProjectileMesh";
const PROJECTILE_SHAPE_NODE_NAME: &str = "ProjectileShape";

#[State]
struct PhysicsBoneProjectileState {
    #[default = Vector3::ZERO]
    pub velocity: Vector3,
    #[default = 2.5]
    pub life: f32,
    #[default = 0.35]
    pub radius: f32,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let (velocity, alive) =
            with_state_mut!(ctx.run, PhysicsBoneProjectileState, ctx.id, |state| {
                state.life -= dt;
                state.velocity.y -= 2.2 * dt;
                (state.velocity, state.life > 0.0)
            })
            .unwrap_or((Vector3::ZERO, false));

        if !alive {
            remove_node!(ctx.run, ctx.id);
            return;
        }

        let pos = get_global_pos_3d!(ctx.run, ctx.id).unwrap_or(Vector3::ZERO);
        let _ = set_global_pos_3d!(ctx.run, ctx.id, pos + velocity * dt);
    }
});

methods!({
    fn launch(&self, ctx: &mut ScriptContext<'_, API>, velocity: Vector3, radius: f32) {
        with_state_mut!(ctx.run, PhysicsBoneProjectileState, ctx.id, |state| {
            state.velocity = velocity;
            state.radius = radius;
            state.life = 2.5;
        });

        if let Some(mesh) = get_child!(ctx.run, ctx.id, PROJECTILE_MESH_NODE_NAME) {
            let diameter = radius * 2.0;
            let _ = set_local_scale_3d!(ctx.run, mesh, Vector3::new(diameter, diameter, diameter));
        }
        if let Some(shape) = get_child!(ctx.run, ctx.id, PROJECTILE_SHAPE_NODE_NAME) {
            with_node_mut!(ctx.run, CollisionShape3D, shape, |shape| {
                shape.shape = Shape3D::Sphere { radius };
            });
        }
    }
});

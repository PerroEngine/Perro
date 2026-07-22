use perro_api::prelude::*;

type SelfNodeType = BoneCollider3D;

#[State]
struct PhysicsBoneProjectileState {
    #[default = NodeID::nil()]
    pub projectile_mesh: NodeID,
    #[default = NodeID::nil()]
    pub projectile_shape: NodeID,
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

        let (mesh, shape) = with_state!(ctx.run, PhysicsBoneProjectileState, ctx.id, |state| {
            (state.projectile_mesh, state.projectile_shape)
        }).unwrap_or_default();
        if !mesh.is_nil() {
            let diameter = radius * 2.0;
            let _ = set_local_scale_3d!(ctx.run, mesh, Vector3::new(diameter, diameter, diameter));
        }
        if !shape.is_nil() {
            with_node_mut!(ctx.run, CollisionShape3D, shape, |shape| {
                shape.shape = Shape3D::Sphere { radius };
            });
        }
    }
});

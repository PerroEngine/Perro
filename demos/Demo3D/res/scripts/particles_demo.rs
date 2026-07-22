use perro_api::prelude::*;

type SelfNodeType = Node3D;

#[State]
struct ParticlesDemoState {
    #[default = NodeID::nil()]
    pub overlay: NodeID,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        self.push_overlay(ctx);
    }
});

methods!({
    fn set_info_overlay(&self, ctx: &mut ScriptContext<'_, API>, overlay: NodeID) {
        with_state_mut!(ctx.run, ParticlesDemoState, ctx.id, |state| {
            state.overlay = overlay;
        });
        self.push_overlay(ctx);
    }

    fn push_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let overlay = with_state!(ctx.run, ParticlesDemoState, ctx.id, |state| state.overlay).unwrap_or_default();
        if overlay.is_nil() {
            return;
        }
        let emitters = query!(
            ctx.run,
            all(node_type[ParticleEmitter3D]),
            in_subtree(ctx.id)
        )
        .len();
        let players = query!(ctx.run, all(node_type[AnimationPlayer]), in_subtree(ctx.id)).len();
        let lights = query!(ctx.run, all(node_type[PointLight3D]), in_subtree(ctx.id)).len()
            + query!(ctx.run, all(node_type[AmbientLight3D]), in_subtree(ctx.id)).len();
        let body = format!(
            "emitters {}\nanim rigs {} | lights {}",
            emitters, players, lights
        );
        let _ = call_method!(
            ctx.run,
            overlay,
            func!("set_content"),
            params!["Particles".to_string(), body]
        );
    }
});

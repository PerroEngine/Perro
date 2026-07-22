use perro_api::prelude::*;

type SelfNodeType = Node3D;

#[State]
struct AnimationsDemoState {
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
        with_state_mut!(ctx.run, AnimationsDemoState, ctx.id, |state| {
            state.overlay = overlay;
        });
        self.push_overlay(ctx);
    }

    fn push_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let overlay = with_state!(ctx.run, AnimationsDemoState, ctx.id, |state| state.overlay).unwrap_or_default();
        if overlay.is_nil() {
            return;
        }
        let players = query!(ctx.run, all(node_type[AnimationPlayer]), in_subtree(ctx.id)).len();
        let meshes = query!(ctx.run, all(node_type[MeshInstance3D]), in_subtree(ctx.id)).len();
        let body = format!(
            "anim players {}\nanimated props {}",
            players,
            meshes.saturating_sub(1)
        );
        let _ = call_method!(
            ctx.run,
            overlay,
            func!("set_content"),
            params!["Animations".to_string(), body]
        );
    }
});

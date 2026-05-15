use perro_api::prelude::*;

type SelfNodeType = Node3D;

#[State]
struct MeshBlendingDemoState {
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
        with_state_mut!(ctx.run, MeshBlendingDemoState, ctx.id, |state| {
            state.overlay = overlay;
        });
        self.push_overlay(ctx);
    }

    fn push_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let overlay = with_state!(ctx.run, MeshBlendingDemoState, ctx.id, |state| state.overlay);
        if overlay.is_nil() {
            return;
        }
        let meshes = query!(ctx.run, all(node_type[MeshInstance3D]), in_subtree(ctx.id)).len();
        let lights = query!(ctx.run, all(node_type[AmbientLight3D]), in_subtree(ctx.id)).len()
            + query!(ctx.run, all(node_type[RayLight3D]), in_subtree(ctx.id)).len();
        let body = format!("blend targets {}\nlight rigs {}", meshes, lights);
        let _ = call_method!(
            ctx.run,
            overlay,
            func!("set_content"),
            params!["Mesh Blending".to_string(), body]
        );
    }
});

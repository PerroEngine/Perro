use perro_api::prelude::*;

type SelfNodeType = Node3D;

#[State]
struct SkyDemoState {
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
        with_state_mut!(ctx.run, SkyDemoState, ctx.id, |state| {
            state.overlay = overlay;
        });
        self.push_overlay(ctx);
    }

    fn push_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let overlay = with_state!(ctx.run, SkyDemoState, ctx.id, |state| state.overlay).unwrap_or_default();
        if overlay.is_nil() {
            return;
        }
        let sky = query!(ctx.run, all(node_type[Sky3D]), in_subtree(ctx.id)).len();
        let meshes = query!(ctx.run, all(node_type[MeshInstance3D]), in_subtree(ctx.id)).len();
        let body = format!("sky nodes {}\nterrain props {}", sky, meshes);
        let _ = call_method!(
            ctx.run,
            overlay,
            func!("set_content"),
            params!["Sky".to_string(), body]
        );
    }
});

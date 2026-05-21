use perro_api::prelude::*;

type SelfNodeType = Node3D;

#[State]
struct MultiMeshDemoState {
    #[default = NodeID::nil()]
    pub overlay: NodeID,
    #[default = String::new()]
    pub last_body: String,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        self.push_overlay(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        self.push_overlay(ctx);
    }
});

methods!({
    fn set_info_overlay(&self, ctx: &mut ScriptContext<'_, API>, overlay: NodeID) {
        with_state_mut!(ctx.run, MultiMeshDemoState, ctx.id, |state| {
            state.overlay = overlay;
        });
        self.push_overlay(ctx);
    }

    fn push_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let overlay = with_state!(ctx.run, MultiMeshDemoState, ctx.id, |state| state.overlay);
        if overlay.is_nil() {
            return;
        }

        let multimeshes = query!(
            ctx.run,
            all(node_type[MultiMeshInstance3D]),
            in_subtree(ctx.id)
        );
        let mut total = 0usize;
        let mut per_mesh = Vec::new();
        for node in multimeshes.iter().copied() {
            let count = with_node!(ctx.run, MultiMeshInstance3D, node, |mesh| mesh
                .instances
                .len());
            total += count;
            per_mesh.push(count.to_string());
        }
        let body = format!(
            "multimeshes {} | total inst {}\ninst/mesh {}",
            multimeshes.len(),
            total,
            if per_mesh.is_empty() {
                "0".into()
            } else {
                per_mesh.join(", ")
            }
        );
        let changed = with_state_mut!(ctx.run, MultiMeshDemoState, ctx.id, |state| {
            if state.last_body == body {
                false
            } else {
                state.last_body = body.clone();
                true
            }
        })
        .unwrap_or(false);
        if !changed {
            return;
        }

        let _ = call_method!(
            ctx.run,
            overlay,
            func!("set_content"),
            params!["MultiMesh".to_string(), body]
        );
    }
});

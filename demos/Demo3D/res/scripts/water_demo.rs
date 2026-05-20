use perro_api::prelude::*;

type SelfNodeType = Node3D;

#[State]
struct WaterDemoState {
    #[default = NodeID::nil()]
    pub overlay: NodeID,
    #[default = NodeID::nil()]
    pub projectiles: NodeID,
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
        with_state_mut!(ctx.run, WaterDemoState, ctx.id, |state| {
            state.overlay = overlay;
        });
        self.push_overlay(ctx);
    }

    fn push_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let overlay = with_state!(ctx.run, WaterDemoState, ctx.id, |state| state.overlay);
        if overlay.is_nil() {
            return;
        }

        let water = query!(ctx.run, all(node_type[WaterBody3D]), in_subtree(ctx.id));
        let rigid = query!(ctx.run, all(node_type[RigidBody3D]), in_subtree(ctx.id));
        let projectiles = with_state!(ctx.run, WaterDemoState, ctx.id, |state| state
            .projectiles);
        let projectile_cnt = if projectiles.is_nil() {
            0
        } else {
            query!(ctx.run, all(node_type[RigidBody3D]), in_subtree(projectiles)).len()
        };
        let mut depths = Vec::new();
        for node in water.iter().copied() {
            let depth = with_node!(ctx.run, WaterBody3D, node, |body| body.water.depth);
            depths.push(format!("{depth:.1}"));
        }
        let body = format!(
            "water bodies {} | float bodies {}\nprojectiles {} | depth {}",
            water.len(),
            rigid.len().saturating_sub(projectile_cnt),
            projectile_cnt,
            if depths.is_empty() {
                "0".into()
            } else {
                depths.join(", ")
            }
        );
        let changed = with_state_mut!(ctx.run, WaterDemoState, ctx.id, |state| {
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
            params!["Water".to_string(), body]
        );
    }
});

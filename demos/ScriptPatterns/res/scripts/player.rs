use perro_api::prelude::*;

#[State]
struct PatternPlayer {
    #[default = 0]
    pub score: i32,
    #[default = 0]
    pub bonus: i32,
    #[default = NodeID::nil()]
    #[node_ref(Sprite2D)]
    pub icon: NodeID,
}

lifecycle!({});

methods!({
    fn set_icon_texture(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        texture: TextureID,
    ) -> bool {
        let icon = with_state!(ctx.run, PatternPlayer, ctx.id, |state| state.icon);
        if icon.is_nil() || texture.is_nil() {
            return false;
        }

        with_node_mut!(ctx.run, Sprite2D, icon, |sprite| {
            sprite.texture = texture;
        })
        .is_some()
    }

    fn add_score(&self, ctx: &mut ScriptContext<'_, API>, amount: i32) -> i32 {
        let result = with_state_mut!(ctx.run, PatternPlayer, ctx.id, |state| {
            state.score += amount.max(0);
            (state.score, state.icon)
        });

        let Some((score, icon)) = result else {
            return 0;
        };

        if !icon.is_nil() {
            with_node_mut!(ctx.run, Sprite2D, icon, |sprite| {
                let scale = 0.75 + score.min(10) as f32 * 0.03;
                sprite.transform.scale = Vector2::new(scale, scale);
            });
        }

        signal_emit!(
            ctx.run,
            signal!("score_changed"),
            params![score]
        );
        score
    }
});

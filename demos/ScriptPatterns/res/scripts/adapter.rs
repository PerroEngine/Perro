use perro_api::prelude::*;

#[State]
struct DynamicAdapter {
    #[default = NodeID::nil()]
    #[node_ref(Node2D)]
    pub target: NodeID,
}

lifecycle!({});

methods!({
    fn add_to_member(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        member: String,
        amount: i32,
    ) -> i32 {
        let target = with_state!(ctx.run, DynamicAdapter, ctx.id, |state| state.target).unwrap_or_default();
        if target.is_nil() {
            return 0;
        }

        let old = get_var!(ctx.run, target, member.as_str())
            .as_i32()
            .unwrap_or(0);
        let next = old + amount;
        set_var!(ctx.run, target, member.as_str(), variant!(next));
        next
    }
});


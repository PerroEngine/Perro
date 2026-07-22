use perro_api::prelude::*;

type SelfNodeType = UiPanel;

#[State]
struct DemoInfoOverlayState {
    #[default = NodeID::nil()]
    pub title_label: NodeID,
    #[default = NodeID::nil()]
    pub body_label: NodeID,
}

lifecycle!({});

methods!({
    fn set_content(&self, ctx: &mut ScriptContext<'_, API>, title: String, body: String) {
        let (title_label, body_label) =
            with_state!(ctx.run, DemoInfoOverlayState, ctx.id, |state| {
                (state.title_label, state.body_label)
            });
        set_label_text(ctx, title_label, title);
        set_label_text(ctx, body_label, body);
    }
});

fn set_label_text<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    id: NodeID,
    text: String,
) {
    if id.is_nil() {
        return;
    }
    with_node_mut!(ctx.run, UiLabel, id, |label| {
        label.text = text.into();
    });
}

use perro_api::prelude::*;

type SelfNodeType = UiPanel;

const TITLE_LABEL_NODE_NAME: &str = "info_overlay_title";
const BODY_LABEL_NODE_NAME: &str = "info_overlay_body";

#[State]
struct DemoInfoOverlayState {
    #[default = NodeID::nil()]
    pub title_label: NodeID,
    #[default = NodeID::nil()]
    pub body_label: NodeID,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let title = get_child!(ctx.run, ctx.id, TITLE_LABEL_NODE_NAME).unwrap_or(NodeID::nil());
        let body = get_child!(ctx.run, ctx.id, BODY_LABEL_NODE_NAME).unwrap_or(NodeID::nil());
        with_state_mut!(ctx.run, DemoInfoOverlayState, ctx.id, |state| {
            state.title_label = title;
            state.body_label = body;
        });
    }
});

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

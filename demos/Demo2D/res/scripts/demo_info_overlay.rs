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
        let title = find_descendant_by_name(ctx, ctx.id, TITLE_LABEL_NODE_NAME);
        let body = find_descendant_by_name(ctx, ctx.id, BODY_LABEL_NODE_NAME);
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

fn find_descendant_by_name<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
    name: &str,
) -> NodeID {
    if root.is_nil() {
        return NodeID::nil();
    }
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if let Some(child) = get_child!(ctx.run, id, name) {
            return child;
        }
        if let Some(children) = get_node_children_ids!(ctx.run, id) {
            for child in children {
                if !child.is_nil() {
                    stack.push(child);
                }
            }
        }
    }
    NodeID::nil()
}

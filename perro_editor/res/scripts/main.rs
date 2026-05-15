mod editor_tab;

use editor_tab::EditorTab;
use perro_api::prelude::*;

type SelfNodeType = UiPanel;

#[State]
#[derive(Clone, Copy)]
struct EditorShellState {
    #[default = EditorTab::Scene]
    active_tab: EditorTab,
}

lifecycle!({
    fn on_all_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_tab_scene"), func!("on_tab_click"));
        let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_tab_script"), func!("on_tab_click"));
        let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_tab_anim"), func!("on_tab_click"));
        set_active_tab(ctx, EditorTab::Scene);
    }
});

methods!({
    fn on_tab_click(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        button: NodeID,
    ) {
        let next = match get_node_name!(ctx.run, button).as_deref() {
            Some("tab_scene_button") => EditorTab::Scene,
            Some("tab_script_button") => EditorTab::Script,
            Some("tab_anim_button") => EditorTab::AnimTree,
            _ => return,
        };
        set_active_tab(ctx, next);
    }

    fn set_active_tab(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        next: String,
    ) {
        let next = match next.as_str() {
            "scene" => EditorTab::Scene,
            "script" => EditorTab::Script,
            "anim" => EditorTab::AnimTree,
            _ => return,
        };
        set_active_tab(ctx, next);
    }
});

fn set_active_tab<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    next: EditorTab,
) {
    let _ = with_state_mut!(ctx.run, EditorShellState, ctx.id, |state| {
        state.active_tab = next;
    });

    let side_bar = get_child!(ctx.run, ctx.id, "side_bar").unwrap_or(NodeID::nil());

    for tab in EditorTab::all() {
        let Some(root) = get_child!(ctx.run, ctx.id, tab.root_name()) else {
            continue;
        };
        with_base_node_mut!(ctx.run, UiBox, root, |node| {
            node.visible = tab == next;
        });

        let Some(button) = get_child!(ctx.run, side_bar, tab.button_name()) else {
            continue;
        };
        with_node_mut!(ctx.run, UiButton, button, |node| {
            let fill = if tab == next {
                tab.accent_fill()
            } else {
                "#343A46"
            };
            if let Some(color) = Color::from_hex(fill) {
                node.style.fill = color;
            }
        });

        if let Some(label) = get_child!(ctx.run, button, tab.label_name()) {
            with_node_mut!(ctx.run, UiLabel, label, |node| {
                let color = if tab == next {
                    tab.accent_text()
                } else {
                    "#EEF0F3"
                };
                if let Some(color) = Color::from_hex(color) {
                    node.color = color;
                }
            });
        }
    }
}

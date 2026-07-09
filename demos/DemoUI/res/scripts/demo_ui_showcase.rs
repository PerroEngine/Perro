use perro_api::prelude::*;

type SelfNodeType = Node2D;

#[derive(Variant, Clone, Copy, Default)]
struct DemoUiShowcaseState {
    time: f32,
    tick: i32,
}

#[derive(Clone, Copy, Default)]
struct UiRefs {
    shell: NodeID,
    left_panel: NodeID,
    mid_panel: NodeID,
    right_panel: NodeID,
    title: NodeID,
    subtitle: NodeID,
    scroll_view: NodeID,
    scroll_items: NodeID,
    grid: NodeID,
    grid_a: NodeID,
    grid_b: NodeID,
    grid_c: NodeID,
    nested_panel: NodeID,
    nested_grid: NodeID,
}

#[State]
struct DemoUiShowcase {
    #[default = NodeID::nil()]
    shell: NodeID,
    #[default = NodeID::nil()]
    left_panel: NodeID,
    #[default = NodeID::nil()]
    mid_panel: NodeID,
    #[default = NodeID::nil()]
    right_panel: NodeID,
    #[default = NodeID::nil()]
    title: NodeID,
    #[default = NodeID::nil()]
    subtitle: NodeID,
    #[default = NodeID::nil()]
    scroll_view: NodeID,
    #[default = NodeID::nil()]
    scroll_items: NodeID,
    #[default = NodeID::nil()]
    grid: NodeID,
    #[default = NodeID::nil()]
    grid_a: NodeID,
    #[default = NodeID::nil()]
    grid_b: NodeID,
    #[default = NodeID::nil()]
    grid_c: NodeID,
    #[default = NodeID::nil()]
    nested_panel: NodeID,
    #[default = NodeID::nil()]
    nested_grid: NodeID,
    #[default = NodeID::nil()]
    shape_a: NodeID,
    #[default = DemoUiShowcaseState::default()]
    runtime: DemoUiShowcaseState,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let Some((time, tick)) = with_state_mut!(ctx.run, DemoUiShowcase, ctx.id, |state| {
            state.runtime.time += dt;
            state.runtime.tick += 1;
            (state.runtime.time, state.runtime.tick)
        }) else {
            return;
        };

        let wave = (time * 1.4).sin() * 0.5 + 0.5;
        let fast = (time * 3.1).sin() * 0.5 + 0.5;
        let refs = ui_refs(ctx, ctx.id);
        resize_columns(ctx, refs, wave);
        animate_nested(ctx, refs, wave, fast);
        animate_scroll(ctx, refs, time);
        animate_grid(ctx, refs, time);
        refresh_text(ctx, refs, tick, wave);
    }
});

methods!({});

fn resize_columns<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, refs: UiRefs, t: f32) {
    // widths trade around a constant 0.95 sum; + h_spacing 0.025*2 = exact 1.0 fit
    let left = 0.31 + 0.06 * t;
    let mid = 0.35 - 0.04 * t;
    let right = 0.29 - 0.02 * t;
    set_ui_size(ctx, refs.left_panel, left, 1.0);
    set_ui_size(ctx, refs.mid_panel, mid, 1.0);
    set_ui_size(ctx, refs.right_panel, right, 1.0);
    force_rerender!(ctx.run, refs.shell);
}

fn animate_nested<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    refs: UiRefs,
    t: f32,
    fast: f32,
) {
    // nested_panel grows within budget; scroll_box (fill row) absorbs the delta
    set_ui_size(ctx, refs.nested_panel, 1.0, 0.12 + 0.03 * t);
    // nested_grid grows in width; nested_note (h fill) absorbs the delta
    set_ui_size(ctx, refs.nested_grid, 0.44 + 0.14 * fast, 1.0);
    with_node_mut!(ctx.run, UiGrid, refs.nested_grid, |grid| {
        grid.columns = if fast > 0.55 { 3 } else { 2 };
        grid.h_spacing = 0.012 + 0.018 * t;
        grid.v_spacing = 0.012 + 0.018 * (1.0 - t);
    });
}

fn animate_scroll<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    refs: UiRefs,
    time: f32,
) {
    let y = (time * 0.25).sin() * 0.5 + 0.5;
    with_node_mut!(ctx.run, UiScrollContainer, refs.scroll_view, |scroll| {
        scroll.scroll = Vector2::new(0.0, y);
    });
    set_ui_size(ctx, refs.scroll_items, 1.0, 1.45 + 0.45 * y);
}

fn animate_grid<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, refs: UiRefs, time: f32) {
    let columns = 2 + ((time * 0.8).sin() > 0.0) as u32;
    with_node_mut!(ctx.run, UiGrid, refs.grid, |grid| {
        grid.columns = columns;
        grid.h_spacing = 0.012 + 0.012 * (time * 2.0).sin().abs();
        grid.v_spacing = 0.012 + 0.012 * (time * 2.3).cos().abs();
    });
    set_ui_scale(ctx, refs.grid_a, 0.92 + 0.08 * (time * 2.4).sin().abs());
    set_ui_scale(ctx, refs.grid_b, 0.92 + 0.08 * (time * 2.0).cos().abs());
    set_ui_scale(ctx, refs.grid_c, 0.92 + 0.08 * (time * 1.7).sin().abs());
}

fn refresh_text<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    refs: UiRefs,
    tick: i32,
    wave: f32,
) {
    if tick % 12 != 0 {
        return;
    }
    with_node_mut!(ctx.run, UiLabel, refs.subtitle, |label| {
        label.text = format!("live layout {:.0}%", wave * 100.0).into();
    });
    with_node_mut!(ctx.run, UiLabel, refs.title, |label| {
        label.text = "DemoUI".into();
    });
}

fn ui_refs<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, script: NodeID) -> UiRefs {
    with_state!(ctx.run, DemoUiShowcase, script, |state| UiRefs {
        shell: state.shell,
        left_panel: state.left_panel,
        mid_panel: state.mid_panel,
        right_panel: state.right_panel,
        title: state.title,
        subtitle: state.subtitle,
        scroll_view: state.scroll_view,
        scroll_items: state.scroll_items,
        grid: state.grid,
        grid_a: state.grid_a,
        grid_b: state.grid_b,
        grid_c: state.grid_c,
        nested_panel: state.nested_panel,
        nested_grid: state.nested_grid,
    })
}

fn set_ui_size<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    node: NodeID,
    x: f32,
    y: f32,
) {
    if node.is_nil() {
        return;
    }
    with_base_node_mut!(ctx.run, UiNode, node, |ui| {
        ui.layout.size = UiVector2::ratio(x, y);
    });
}

fn set_ui_scale<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, node: NodeID, scale: f32) {
    if node.is_nil() {
        return;
    }
    with_base_node_mut!(ctx.run, UiNode, node, |ui| {
        ui.transform.scale = Vector2::new(scale, scale);
    });
}

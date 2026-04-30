use perro_api::prelude::*;
use std::borrow::Cow;

type SelfNodeType = UiPanel;

#[State]
#[derive(Clone, Copy)]
struct UiTestState {
    top_bar: NodeID,
    title_label: NodeID,
    main_row: NodeID,
    left_stack: NodeID,
    left_button_b: NodeID,
    center_grid: NodeID,
    grid_c: NodeID,
    grid_c_layout: NodeID,
    grid_c_note: NodeID,
    grid_c_progress_bg: NodeID,
    grid_c_progress_fill: NodeID,
    grid_d: NodeID,
    grid_f: NodeID,
    grid_g_cell_3: NodeID,
    right_layout: NodeID,
}

const TITLE_A: &str = "UiTest: panels / layouts / anchors";
const TITLE_B: &str = "UiTest: layout churn / text resize";

lifecycle!({
    fn on_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
    }

    fn on_all_init(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
        let top_bar = ctx
            .Nodes()
            .get_child_by_name(self_id, "top_bar")
            .unwrap_or_default();
        let main_row = ctx
            .Nodes()
            .get_child_by_name(self_id, "main_row")
            .unwrap_or_default();
        let left_panel = ctx
            .Nodes()
            .get_child_by_name(main_row, "left_panel")
            .unwrap_or_default();
        let left_stack = ctx
            .Nodes()
            .get_child_by_name(left_panel, "left_stack")
            .unwrap_or_default();
        let center_panel = ctx
            .Nodes()
            .get_child_by_name(main_row, "center_panel")
            .unwrap_or_default();
        let center_grid = ctx
            .Nodes()
            .get_child_by_name(center_panel, "center_grid")
            .unwrap_or_default();
        let grid_c = ctx
            .Nodes()
            .get_child_by_name(center_grid, "grid_c")
            .unwrap_or_default();
        let grid_c_layout = ctx
            .Nodes()
            .get_child_by_name(grid_c, "grid_c_layout")
            .unwrap_or_default();
        let grid_c_progress_bg = ctx
            .Nodes()
            .get_child_by_name(grid_c_layout, "grid_c_progress_bg")
            .unwrap_or_default();
        let right_panel = ctx
            .Nodes()
            .get_child_by_name(main_row, "right_panel")
            .unwrap_or_default();
        let right_layout = ctx
            .Nodes()
            .get_child_by_name(right_panel, "right_layout")
            .unwrap_or_default();
        let title_label = ctx
            .Nodes()
            .get_child_by_name(top_bar, "title_label")
            .unwrap_or_default();
        let left_button_b = ctx
            .Nodes()
            .get_child_by_name(left_stack, "left_button_b")
            .unwrap_or_default();
        let grid_c_note = ctx
            .Nodes()
            .get_child_by_name(grid_c_layout, "grid_c_note")
            .unwrap_or_default();
        let grid_c_progress_fill = ctx
            .Nodes()
            .get_child_by_name(grid_c_progress_bg, "grid_c_progress_fill")
            .unwrap_or_default();
        let grid_d = ctx
            .Nodes()
            .get_child_by_name(center_grid, "grid_d")
            .unwrap_or_default();
        let grid_f = ctx
            .Nodes()
            .get_child_by_name(center_grid, "grid_f")
            .unwrap_or_default();
        let grid_g = ctx
            .Nodes()
            .get_child_by_name(center_grid, "grid_g")
            .unwrap_or_default();
        let grid_g_stack = ctx
            .Nodes()
            .get_child_by_name(grid_g, "grid_g_stack")
            .unwrap_or_default();
        let grid_g_inner = ctx
            .Nodes()
            .get_child_by_name(grid_g_stack, "grid_g_inner")
            .unwrap_or_default();
        let grid_g_cell_3 = ctx
            .Nodes()
            .get_child_by_name(grid_g_inner, "grid_g_cell_3")
            .unwrap_or_default();

        let _ = with_state_mut!(ctx, UiTestState, self_id, |state| {
            state.top_bar = top_bar;
            state.title_label = title_label;
            state.main_row = main_row;
            state.left_stack = left_stack;
            state.left_button_b = left_button_b;
            state.center_grid = center_grid;
            state.grid_c = grid_c;
            state.grid_c_layout = grid_c_layout;
            state.grid_c_note = grid_c_note;
            state.grid_c_progress_bg = grid_c_progress_bg;
            state.grid_c_progress_fill = grid_c_progress_fill;
            state.grid_d = grid_d;
            state.grid_f = grid_f;
            state.grid_g_cell_3 = grid_g_cell_3;
            state.right_layout = right_layout;
        });
    }

    fn on_update(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
        let time = elapsed_time!(ctx);
        let size_pulse = ((time * 2.2).sin() * 0.5 + 0.5).max(0.0);
        let slow_pulse = ((time * 0.9).sin() * 0.5 + 0.5).max(0.0);
        let fast_pulse = ((time * 3.1).cos() * 0.5 + 0.5).max(0.0);
        let show_progress = (time * 0.8).sin() >= 0.0;
        let show_stats = (time * 0.65).sin() >= 0.0;
        let show_inner_cell = (time * 1.1).cos() >= 0.0;
        let use_alt_text = ((time * 0.7) as i32) % 2 == 0;
        let quest_count = 1 + ((time * 1.4) as i32).rem_euclid(5);
        let ids = with_state!(ctx, UiTestState, self_id, |state| { *state });

        if !ids.title_label.is_nil() {
            let text = if use_alt_text { TITLE_A } else { TITLE_B };
            let _ = with_node_mut!(ctx, UiLabel, ids.title_label, |label| {
                label.text = Cow::Owned(text.to_string());
            });
        }

        if !ids.top_bar.is_nil() {
            let _ = with_base_node_mut!(ctx, UiBox, ids.top_bar, |ui| {
                ui.transform.translation.x = (slow_pulse - 0.5) * 44.0;
                ui.layout.padding = UiRect::symmetric(18.0 + fast_pulse * 10.0, 12.0);
            });
        }

        if ids.main_row.is_nil() {
            return;
        }
        let _ = with_node_mut!(ctx, UiHLayout, ids.main_row, |layout| {
            layout.inner.spacing = 52.0 + slow_pulse * 48.0;
        });
        if ids.left_stack.is_nil() {
            return;
        }

        if !ids.left_button_b.is_nil() {
            let width = 240.0 + size_pulse * 180.0;
            let _ = with_base_node_mut!(ctx, UiBox, ids.left_button_b, |ui| {
                ui.layout.size = UiVector2::pixels(width, 52.0);
            });
        }

        if ids.center_grid.is_nil() {
            return;
        }
        let _ = with_node_mut!(ctx, UiGrid, ids.center_grid, |grid| {
            grid.h_spacing = 10.0 + fast_pulse * 18.0;
            grid.v_spacing = 10.0 + slow_pulse * 18.0;
        });

        if !ids.grid_c.is_nil() {
            let _ = with_base_node_mut!(ctx, UiBox, ids.grid_c, |ui| {
                ui.layout.size =
                    UiVector2::pixels(140.0 + slow_pulse * 36.0, 96.0 + fast_pulse * 34.0);
                ui.transform.translation.y = (fast_pulse - 0.5) * 30.0;
            });
            if !ids.grid_c_layout.is_nil() {
                let _ = with_node_mut!(ctx, UiLayout, ids.grid_c_layout, |layout| {
                    layout.inner.spacing = 3.0 + fast_pulse * 9.0;
                });
                if !ids.grid_c_note.is_nil() {
                    let _ = with_node_mut!(ctx, UiLabel, ids.grid_c_note, |label| {
                        label.text = Cow::Owned(format!("{quest_count}/5"));
                    });
                }
                if !ids.grid_c_progress_bg.is_nil() {
                    let _ = with_base_node_mut!(ctx, UiBox, ids.grid_c_progress_bg, |ui| {
                        ui.visible = show_progress;
                        ui.transform.translation.y = (slow_pulse - 0.5) * 22.0;
                        ui.layout.size =
                            UiVector2::pixels(104.0 + fast_pulse * 28.0, 16.0 + slow_pulse * 8.0);
                    });
                    if !ids.grid_c_progress_fill.is_nil() {
                        let fill_width = 18.0 + quest_count as f32 * 18.0 + fast_pulse * 12.0;
                        let _ = with_base_node_mut!(ctx, UiBox, ids.grid_c_progress_fill, |ui| {
                            ui.layout.size = UiVector2::pixels(fill_width, 10.0);
                        });
                    }
                }
            }
        }

        if !ids.grid_d.is_nil() {
            let _ = with_base_node_mut!(ctx, UiBox, ids.grid_d, |ui| {
                ui.transform.scale =
                    Vector2::new(0.9 + slow_pulse * 0.35, 0.78 + fast_pulse * 0.38);
            });
        }

        if !ids.grid_f.is_nil() {
            let _ = with_base_node_mut!(ctx, UiBox, ids.grid_f, |ui| {
                ui.visible = show_stats;
            });
        }

        if !ids.grid_g_cell_3.is_nil() {
            let _ = with_base_node_mut!(ctx, UiBox, ids.grid_g_cell_3, |ui| {
                ui.visible = show_inner_cell;
            });
        }

        if !ids.right_layout.is_nil() {
            let _ = with_node_mut!(ctx, UiLayout, ids.right_layout, |layout| {
                layout.inner.spacing = 8.0 + slow_pulse * 14.0;
            });
            let _ = with_base_node_mut!(ctx, UiBox, ids.right_layout, |ui| {
                ui.transform.translation.y = (fast_pulse - 0.5) * 24.0;
            });
        }
    }

    fn on_fixed_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
    }

    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
    }
});

methods!({
    fn default_method(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
    }
});

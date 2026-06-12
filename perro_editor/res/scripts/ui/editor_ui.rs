use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_app_editor_manager_rs as editor_manager;
use crate::scripts_app_editor_project_rs as editor_project;
use crate::scripts_assets_editor_assets_rs::*;
use crate::scripts_assets_editor_file_watch_rs as editor_file_watch;
use crate::scripts_assets_editor_files_rs as editor_files;
use crate::scripts_editor_main_rs::{
    EditorState, FILE_WATCH_INTERVAL_FRAMES, MAX_FILES, MAX_INSPECTOR_PICKER_ROWS,
    MAX_NODE_PICKER_ROWS, MAX_NODES, MAX_RECENT, MAX_TABS, RECENT_PROJECTS_PATH,
};
use crate::scripts_scene_editor_animation_rs::*;
use crate::scripts_scene_editor_gizmos_rs as editor_gizmos;
use crate::scripts_scene_editor_nav_rs::*;
use crate::scripts_scene_editor_nodes_rs::*;
use crate::scripts_scene_editor_scene_deps_rs as editor_scene_deps;
use crate::scripts_scene_editor_scene_rs as editor_scene;
use crate::scripts_scene_editor_viewport_rs::*;
use crate::scripts_ui_editor_inspector_values_rs::*;
use crate::scripts_ui_editor_view_rs as editor_view;
use perro_api::prelude::*;
use perro_api::scene::{
    SceneDoc, SceneFieldName, SceneKey, SceneNodeData, SceneNodeEntry, SceneValue, SceneValueKey,
};
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
pub fn refresh_all<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let view = with_state!(ctx.run, EditorState, ctx.id, EditorView::from_state);

    set_label(
        ctx,
        "project_status",
        &format!("{}  {}", view.project_name, view.project_root),
    );
    set_label(ctx, "status_bar", &view.status);
    set_label(ctx, "log_text", &view.log);
    set_label(ctx, "viewport_label", &view.viewport);
    let glb_mode = view.activity_mode == "glb";
    set_button_fill(
        ctx,
        "activity_scene_button",
        if glb_mode { "#2D323C" } else { "#478CBF" },
    );
    set_button_fill(
        ctx,
        "activity_glb_button",
        if glb_mode { "#478CBF" } else { "#2D323C" },
    );
    set_image_tint(
        ctx,
        "activity_scene_icon",
        if glb_mode { "#B8C1CCFF" } else { "#FFFFFFFF" },
    );
    set_image_tint(
        ctx,
        "activity_glb_icon",
        if glb_mode { "#FFFFFFFF" } else { "#B8C1CCFF" },
    );
    set_button_fill(
        ctx,
        "mode_ui_button",
        if view.viewport_mode == "UI" {
            "#478CBF"
        } else {
            "#2D323C"
        },
    );
    set_button_fill(
        ctx,
        "mode_2d_button",
        if view.viewport_mode == "2D" {
            "#478CBF"
        } else {
            "#2D323C"
        },
    );
    set_button_fill(
        ctx,
        "mode_3d_button",
        if view.viewport_mode == "3D" {
            "#478CBF"
        } else {
            "#2D323C"
        },
    );
    set_ui_display(ctx, "bottom_tab_bar", true);
    set_button_fill(
        ctx,
        "bottom_log_button",
        if view.anim_drawer_open {
            "#2D323C"
        } else {
            "#478CBF"
        },
    );
    set_button_fill(
        ctx,
        "bottom_anim_button",
        if view.anim_drawer_open {
            "#478CBF"
        } else {
            "#2D323C"
        },
    );
    set_ui_display(ctx, "log_title", false);
    set_ui_display(ctx, "log_text", !view.anim_drawer_open);
    set_ui_display(ctx, "anim_drawer", view.anim_drawer_open);
    set_ui_display(ctx, "anim_create_button", view.anim_can_create);
    set_ui_display(ctx, "anim_add_track_button", view.anim_can_add_track);
    set_label(ctx, "anim_drawer_title", &view.anim_title);
    set_label(ctx, "anim_status_text", &view.anim_status);
    set_label(ctx, "anim_tracks_text", &view.anim_tracks);
    set_ui_display(ctx, "left_panel", true);
    set_ui_display(ctx, "inspector_panel", !glb_mode);
    set_ui_display(ctx, "scene_tabs", !glb_mode);
    set_ui_display(ctx, "viewport_panel", !glb_mode);
    set_ui_display(ctx, "bottom_panel", !glb_mode);
    set_ui_display(ctx, "glb_viewer_panel", glb_mode);
    set_label(ctx, "glb_viewer_title", &view.glb_title);
    set_label(ctx, "glb_viewer_summary", &view.glb_summary);
    set_ui_display(ctx, "scene_tree_title", !glb_mode);
    set_ui_display(ctx, "scene_action_row", !glb_mode);
    set_ui_display(ctx, "scene_order_row", !glb_mode);
    set_ui_display(ctx, "scene_tools_row", !glb_mode);
    set_ui_display(ctx, "scene_filter_box", !glb_mode);
    set_text_box(ctx, "scene_filter_box", &view.scene_filter);
    set_ui_display(ctx, "scene_rows", !glb_mode);
    set_ui_display(ctx, "file_title", true);
    set_label(ctx, "file_title", &view.file_title);
    set_ui_display(ctx, "file_action_row", true);
    set_ui_display(ctx, "file_tools_row", true);
    set_ui_display(ctx, "file_ops_row", true);
    set_ui_display(ctx, "file_filter_box", true);
    set_text_box(ctx, "file_filter_box", &view.file_filter);
    set_ui_display(ctx, "file_rows", true);
    set_ui_box_size(ctx, "scene_tools_row", (1.0, 0.032));
    set_ui_box_size(ctx, "scene_rows", (1.0, if glb_mode { 0.0 } else { 0.312 }));
    set_ui_box_size(ctx, "file_action_row", (1.0, 0.034));
    set_ui_box_size(ctx, "file_tools_row", (1.0, 0.032));
    set_ui_box_size(ctx, "file_ops_row", (1.0, 0.032));
    set_ui_box_size(ctx, "file_rows", (1.0, if glb_mode { 0.776 } else { 0.296 }));

    for idx in 0..MAX_RECENT {
        let has_recent = view.recent_projects.get(idx).is_some();
        let text = view
            .recent_projects
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        set_label(
            ctx,
            &format!("manager_recent_{idx}_label"),
            &editor_view::short_path(&text, 44),
        );
        set_ui_display(ctx, &format!("manager_recent_{idx}"), has_recent);
    }
    set_label(
        ctx,
        "create_location_label",
        &format!(
            "location: {}",
            editor_view::short_path(&view.create_parent_dir, 34)
        ),
    );

    set_label(ctx, "add_node_page_label", &view.node_picker_page);
    set_label(ctx, "add_node_parent_label", &view.node_picker_parent);
    set_text_box(ctx, "add_node_search_box", &view.node_picker_filter);
    for idx in 0..MAX_NODE_PICKER_ROWS {
        let text = view
            .node_picker_rows
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        set_label(ctx, &format!("add_node_type_{idx}_label"), &text);
    }

    for idx in 0..MAX_FILES {
        let has_file = view.file_paths.get(idx).is_some();
        let text = view
            .file_paths
            .get(idx)
            .map(|path| {
                format!(
                    "{}{} {}",
                    file_row_state_prefix(path, &view.open_paths, &view.dirty_scene_paths),
                    file_row_icon(path, &view.file_expanded_paths),
                    file_row_label_for_filter(path, !view.file_filter.is_empty()),
                )
            })
            .unwrap_or_else(|| "-".to_string());
        set_label(ctx, &format!("file_row_{idx}_label"), &text);
        set_ui_display(ctx, &format!("file_row_{idx}"), has_file);
        set_button_fill(
            ctx,
            &format!("file_row_{idx}"),
            if view.file_paths.get(idx) == Some(&view.active_asset_path) {
                "#478CBF"
            } else {
                "#00000000"
            },
        );
    }
    apply_file_tree_layout(ctx);

    for idx in 0..MAX_TABS {
        let has_tab = view.open_paths.get(idx).is_some();
        let text = view
            .open_paths
            .get(idx)
            .map(|path| {
                let mark = if view.dirty_scene_paths.iter().any(|dirty| dirty == path) {
                    "* "
                } else {
                    ""
                };
                format!("{mark}{}", editor_files::rel_label(path))
            })
            .unwrap_or_else(|| "-".to_string());
        set_label(
            ctx,
            &format!("scene_tab_{idx}_label"),
            &editor_view::short_path(&text, 24),
        );
        set_ui_display(ctx, &format!("scene_tab_{idx}"), has_tab);
        set_ui_display(ctx, &format!("scene_tab_close_{idx}"), has_tab);
        set_button_fill(
            ctx,
            &format!("scene_tab_{idx}"),
            if idx == view.active_open {
                "#478CBF"
            } else {
                "#2D323C"
            },
        );
    }

    for idx in 0..MAX_NODES {
        let has_node = view.nodes.get(idx).is_some();
        let text = view
            .nodes
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        set_label(ctx, &format!("scene_row_{idx}_label"), &text);
        set_ui_display(ctx, &format!("scene_row_{idx}"), has_node);
        set_button_fill(
            ctx,
            &format!("scene_row_{idx}"),
            if view.selected_row == Some(idx) {
                "#478CBF"
            } else {
                "#00000000"
            },
        );
    }
    apply_scene_list_layout(ctx);
    apply_viewport_mode(ctx, &view.viewport_mode);
    apply_editor_gizmos(ctx, &view.gizmo, &view.viewport_mode);
    apply_selected_ui_overlay(ctx, view.selected_ui_rect);
    sync_selected_preview_gizmo(ctx);

    if take_inspector_layout_pass(ctx) {
        apply_inspector_static_layout(ctx);
    }
    apply_inspector_dynamic_layout(ctx, &view.inspector);
    set_label(ctx, "inspector_title", &view.inspector.title);
    set_label(ctx, "inspector_name", &view.inspector.name);
    set_ui_display(
        ctx,
        "inspector_name_box",
        view.inspector.node_actions || view.inspector.asset_selected,
    );
    set_text_box(ctx, "inspector_name_box", &view.inspector.name_edit);
    set_label(ctx, "inspector_type", &view.inspector.kind);
    set_label(ctx, "inspector_parent", &view.inspector.parent);
    set_label(ctx, "inspector_script_top", &view.inspector.script);
    set_ui_box_size(
        ctx,
        "inspector_script_top",
        (
            1.0,
            if view.inspector.node_actions {
                0.034
            } else if view.inspector.script.len() > 80 {
                0.12
            } else {
                0.034
            },
        ),
    );
    set_ui_display(
        ctx,
        "inspector_script_top",
        view.inspector.node_actions || view.inspector.asset_actions,
    );
    set_ui_display(ctx, "inspector_action_row", view.inspector.node_actions);
    set_label(
        ctx,
        "inspector_visible_label",
        &view.inspector.visible_action_label,
    );
    set_ui_display(ctx, "asset_action_row", view.inspector.asset_selected);
    set_ui_display(ctx, "asset_open_button", view.inspector.asset_selected);
    set_ui_display(ctx, "asset_use_button", view.inspector.asset_use_action);
    set_ui_display(ctx, "asset_make_node_button", view.inspector.asset_make_action);
    set_ui_display(ctx, "asset_user_button", view.inspector.asset_actions);
    set_label(ctx, "asset_make_node_label", &view.inspector.asset_make_label);
    set_ui_display(
        ctx,
        "asset_glb_anim_button",
        view.inspector.glb_asset_actions,
    );
    set_ui_display(
        ctx,
        "asset_glb_mat_button",
        view.inspector.glb_asset_actions,
    );
    let transform_closed =
        inspector_section_collapsed(&view.inspector.collapsed_sections, "transform");
    let refs_closed = inspector_section_collapsed(&view.inspector.collapsed_sections, "refs");
    let vars_closed = inspector_section_collapsed(&view.inspector.collapsed_sections, "vars");
    set_label(
        ctx,
        "inspector_pos_label",
        if transform_closed {
            "> Transform"
        } else {
            &view.inspector.pos_label
        },
    );
    set_text_box(
        ctx,
        "inspector_position_box",
        &view.inspector.pos.join(", "),
    );
    apply_component_row(
        ctx,
        "inspector_position",
        &["x", "y", "z", "w"],
        &view.inspector.pos,
        view.inspector.node_actions && !transform_closed,
    );
    set_label(
        ctx,
        "inspector_rotation_header_label",
        &view.inspector.rotation_label,
    );
    set_ui_display(
        ctx,
        "inspector_rotation_label",
        view.inspector.node_actions && !transform_closed,
    );
    set_ui_display(
        ctx,
        "inspector_rotation_mode_row",
        view.inspector.rotation_mode_buttons && !transform_closed,
    );
    set_button_fill(
        ctx,
        "inspector_rotation_quat_button",
        if view.inspector.rotation_mode == "quat" {
            "#478CBF"
        } else {
            "#2D323C"
        },
    );
    set_button_fill(
        ctx,
        "inspector_rotation_euler_button",
        if view.inspector.rotation_mode == "euler" {
            "#478CBF"
        } else {
            "#2D323C"
        },
    );
    set_text_box(
        ctx,
        "inspector_rotation_box",
        &view.inspector.rotation.join(", "),
    );
    apply_component_row(
        ctx,
        "inspector_rotation",
        &view.inspector.rotation_components,
        &view.inspector.rotation,
        view.inspector.node_actions && !transform_closed,
    );
    set_label(ctx, "inspector_scale_header_label", &view.inspector.scale_label);
    set_ui_display(
        ctx,
        "inspector_scale_label",
        view.inspector.node_actions && !transform_closed,
    );
    set_text_box(ctx, "inspector_scale_box", &view.inspector.scale.join(", "));
    apply_component_row(
        ctx,
        "inspector_scale",
        &["x", "y", "z", "w"],
        &view.inspector.scale,
        view.inspector.node_actions && !transform_closed,
    );
    set_label(
        ctx,
        "inspector_script_label",
        if refs_closed { "> References" } else { "v References" },
    );
    set_ui_display(
        ctx,
        "inspector_script",
        view.inspector.node_actions && !view.inspector.resource_fields.is_empty(),
    );
    set_label(
        ctx,
        "inspector_vars_label",
        if vars_closed {
            "> Script Variables"
        } else {
            "v Script Variables"
        },
    );
    set_ui_display(
        ctx,
        "inspector_vars",
        view.inspector.node_actions && !view.inspector.script_vars.is_empty(),
    );
    set_ui_display(ctx, "inspector_vars_box", false);
    set_text_box(ctx, "inspector_vars_box", &view.inspector.vars_text);
    for idx in 0..MAX_SCRIPT_VARS {
        let row = view.inspector.script_vars.get(idx);
        set_ui_display(
            ctx,
            &format!("inspector_var_row_{idx}"),
            view.inspector.node_actions && row.is_some() && !vars_closed,
        );
        set_label(
            ctx,
            &format!("inspector_var_{idx}_name"),
            row.map(|item| item.name.as_str()).unwrap_or("-"),
        );
        set_label(
            ctx,
            &format!("inspector_var_{idx}_type"),
            row.map(|item| item.kind.as_str()).unwrap_or("-"),
        );
        set_text_box(
            ctx,
            &format!("inspector_var_{idx}_value"),
            row.map(|item| item.value.as_str()).unwrap_or(""),
        );
        let bool_row = row.is_some_and(|item| item.kind == "Bool");
        let picker_button_name = format!("inspector_var_{idx}_pick_button");
        let picker_row = row.is_some_and(|item| item.kind == "Node" || item.expandable)
            && find_named(ctx, &picker_button_name).is_some();
        set_ui_display(
            ctx,
            &format!("inspector_var_{idx}_value"),
            view.inspector.node_actions && row.is_some() && !picker_row && !bool_row && !vars_closed,
        );
        let checkbox_name = format!("inspector_var_{idx}_check");
        set_ui_display(
            ctx,
            &checkbox_name,
            view.inspector.node_actions && bool_row && !vars_closed,
        );
        set_ui_display(
            ctx,
            &picker_button_name,
            view.inspector.node_actions && picker_row && !vars_closed,
        );
        set_label(
            ctx,
            &format!("inspector_var_{idx}_pick_label"),
            row.map(inspector_var_button_label)
                .as_deref()
                .unwrap_or("Select"),
        );
        set_checkbox_checked(
            ctx,
            &checkbox_name,
            row.is_some_and(|item| item.kind == "Bool" && item.value == "true"),
        );
    }
    for idx in 0..MAX_RESOURCE_FIELDS {
        let row = view.inspector.resource_fields.get(idx);
        set_ui_display(
            ctx,
            &format!("inspector_resource_row_{idx}"),
            view.inspector.node_actions && row.is_some() && !refs_closed,
        );
        set_label(
            ctx,
            &format!("inspector_resource_{idx}_name"),
            row.map(|item| item.name.as_str()).unwrap_or("-"),
        );
        set_label(
            ctx,
            &format!("inspector_resource_{idx}_button_label"),
            row.map(|item| item.value.as_str()).unwrap_or("Select"),
        );
        let has_value = row.is_some_and(|item| !item.value.starts_with("[Select"));
        set_ui_display(
            ctx,
            &format!("inspector_resource_{idx}_clear_button"),
            view.inspector.node_actions && has_value && !refs_closed,
        );
    }

    set_ui_display(ctx, "inspector_pick_popup", view.inspector_picker_open);
    set_label(ctx, "inspector_pick_title", &view.inspector_picker_title);
    set_text_box(ctx, "inspector_pick_filter_box", &view.inspector_picker_filter);
    set_label(ctx, "inspector_pick_page_label", &view.inspector_picker_page);
    for idx in 0..MAX_INSPECTOR_PICKER_ROWS {
        let has_row = view.inspector_picker_rows.get(idx).is_some();
        let text = view
            .inspector_picker_rows
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        set_label(ctx, &format!("inspector_pick_row_{idx}_label"), &text);
        set_ui_display(
            ctx,
            &format!("inspector_pick_row_{idx}"),
            view.inspector_picker_open && has_row,
        );
    }
    if view.inspector_picker_open {
        set_label(ctx, "add_node_popup_title", &view.inspector_picker_title);
        set_text_box(ctx, "add_node_search_box", &view.inspector_picker_filter);
        set_label(ctx, "add_node_parent_label", "current scene");
        set_label(ctx, "add_node_page_label", &view.inspector_picker_page);
        for idx in 0..MAX_NODE_PICKER_ROWS {
            let has_row = view.inspector_picker_rows.get(idx).is_some();
            let text = view
                .inspector_picker_rows
                .get(idx)
                .cloned()
                .unwrap_or_else(|| "-".to_string());
            set_label(ctx, &format!("add_node_type_{idx}_label"), &text);
            set_ui_display(ctx, &format!("add_node_type_{idx}"), has_row);
        }
        set_label(ctx, "add_node_cancel_label", "Cancel");
    }
}

pub const MAX_SCRIPT_VARS: usize = 8;
pub const MAX_RESOURCE_FIELDS: usize = 4;

pub fn apply_component_row<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    prefix: &str,
    labels: &[&str],
    values: &[String],
    visible: bool,
) {
    let row_id = format!("{prefix}_row");
    set_ui_display(ctx, &row_id, visible);
    for idx in 0..4 {
        let has_value = values.get(idx).is_some();
        set_ui_display(ctx, &format!("{prefix}_{idx}_label"), visible && has_value);
        set_ui_display(ctx, &format!("{prefix}_{idx}_box"), visible && has_value);
        set_label(
            ctx,
            &format!("{prefix}_{idx}_label"),
            labels.get(idx).copied().unwrap_or(""),
        );
        set_text_box(
            ctx,
            &format!("{prefix}_{idx}_box"),
            values.get(idx).map(String::as_str).unwrap_or(""),
        );
    }
}

pub fn toggle_inspector_section<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    section: &str,
) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if let Some(pos) = state
            .inspector_collapsed_sections
            .iter()
            .position(|item| item == section)
        {
            state.inspector_collapsed_sections.remove(pos);
            state.log = format!("inspector expand\n{section}");
        } else {
            state.inspector_collapsed_sections.push(section.to_string());
            state.log = format!("inspector collapse\n{section}");
        }
    });
    refresh_all(ctx);
}

pub fn inspector_section_collapsed(sections: &[String], section: &str) -> bool {
    sections.iter().any(|item| item == section)
}

fn take_inspector_layout_pass<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> bool {
    with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.inspector_layout_applied {
            false
        } else {
            state.inspector_layout_applied = true;
            true
        }
    })
    .unwrap_or(false)
}

fn apply_inspector_static_layout<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    for name in ["add_node_popup", "inspector_pick_popup"] {
        set_ui_box_z_index(ctx, name, 200);
    }

    set_ui_box_size(ctx, "inspector_panel", (0.20, 1.0));
    set_ui_box_size(ctx, "inspector_content", (1.0, 1.12));
    set_label_text_ratio(ctx, "inspector_title", 0.22);
    set_label_text_ratio(ctx, "inspector_name", 0.18);
    set_label_text_ratio(ctx, "inspector_type", 0.16);
    set_label_text_ratio(ctx, "inspector_parent", 0.15);
    set_label_text_ratio(ctx, "inspector_script_top", 0.15);
    set_label_text_ratio(ctx, "inspector_pos_label", 0.17);
    set_label_text_ratio(ctx, "inspector_rotation_header_label", 0.17);
    set_label_text_ratio(ctx, "inspector_scale_header_label", 0.17);
    set_label_text_ratio(ctx, "inspector_script_label", 0.17);
    set_label_text_ratio(ctx, "inspector_vars_label", 0.17);

    for name in [
        "inspector_action_row",
        "asset_action_row",
        "inspector_position_row",
        "inspector_rotation_mode_row",
        "inspector_rotation_row",
        "inspector_scale_row",
    ] {
        set_ui_box_size(ctx, name, (1.0, 0.024));
    }
    for name in [
        "inspector_position_row",
        "inspector_rotation_row",
        "inspector_scale_row",
    ] {
        set_ui_box_padding(ctx, name, UiRect::new(0.030, 0.0, 0.0, 0.0));
        set_hlayout_spacing(ctx, name, 0.002);
    }

    for name in [
        "inspector_duplicate_button",
        "inspector_delete_button",
        "inspector_reset_button",
        "inspector_open_ref_button",
        "inspector_visible_button",
        "inspector_clear_ref_button",
    ] {
        set_button_size(ctx, name, (0.16, 0.70));
    }
    for name in [
        "inspector_rotation_quat_button",
        "inspector_rotation_euler_button",
    ] {
        set_button_size(ctx, name, (0.50, 0.70));
    }

    for name in [
        "inspector_name_box",
        "inspector_position_box",
        "inspector_rotation_box",
        "inspector_scale_box",
    ] {
        set_ui_box_size(ctx, name, (1.0, 0.024));
        set_text_box_text_ratio(ctx, name, 0.42);
        set_text_box_padding(ctx, name, 4.0, 1.0);
    }

    for prefix in [
        "inspector_position",
        "inspector_rotation",
        "inspector_scale",
    ] {
        for idx in 0..4 {
            let name = format!("{prefix}_{idx}_box");
            set_ui_box_size(ctx, &name, (0.185, 0.72));
            set_text_box_text_ratio(ctx, &name, 0.42);
            set_text_box_padding(ctx, &name, 4.0, 1.0);
        }
    }

    for idx in 0..MAX_SCRIPT_VARS {
        set_ui_box_size(ctx, &format!("inspector_var_row_{idx}"), (1.0, 0.027));
        set_ui_box_size(ctx, &format!("inspector_var_{idx}_value"), (0.50, 0.70));
        set_text_box_text_ratio(ctx, &format!("inspector_var_{idx}_value"), 0.42);
        set_text_box_padding(ctx, &format!("inspector_var_{idx}_value"), 5.0, 1.0);
        set_ui_box_padding(
            ctx,
            &format!("inspector_var_row_{idx}"),
            UiRect::new(0.025, 0.0, 0.0, 0.0),
        );
        set_hlayout_spacing(ctx, &format!("inspector_var_row_{idx}"), 0.002);
        set_ui_box_size(ctx, &format!("inspector_var_{idx}_check"), (0.055, 0.50));
        set_button_size(ctx, &format!("inspector_var_{idx}_pick_button"), (0.50, 0.70));
        set_label_text_ratio(ctx, &format!("inspector_var_{idx}_name"), 0.18);
        set_label_text_ratio(ctx, &format!("inspector_var_{idx}_type"), 0.16);
    }
    for idx in 0..MAX_RESOURCE_FIELDS {
        set_ui_box_size(ctx, &format!("inspector_resource_row_{idx}"), (1.0, 0.027));
        set_ui_box_padding(
            ctx,
            &format!("inspector_resource_row_{idx}"),
            UiRect::new(0.025, 0.0, 0.0, 0.0),
        );
        set_hlayout_spacing(ctx, &format!("inspector_resource_row_{idx}"), 0.002);
        set_button_size(ctx, &format!("inspector_resource_{idx}_button"), (0.60, 0.70));
        set_button_size(
            ctx,
            &format!("inspector_resource_{idx}_clear_button"),
            (0.08, 0.60),
        );
        set_label_text_ratio(ctx, &format!("inspector_resource_{idx}_name"), 0.18);
        set_label_text_ratio(ctx, &format!("inspector_resource_{idx}_button_label"), 0.16);
        set_label_text_ratio(ctx, &format!("inspector_resource_{idx}_clear_label"), 0.20);
    }
}

fn apply_inspector_dynamic_layout<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    inspector: &InspectorViewData,
) {
    let asset_button_w = if inspector.glb_asset_actions {
        0.166
    } else if inspector.asset_actions {
        0.25
    } else {
        1.0
    };
    for name in [
        "asset_open_button",
        "asset_use_button",
        "asset_make_node_button",
        "asset_user_button",
    ] {
        set_button_size(ctx, name, (asset_button_w, 0.70));
    }
    for name in ["asset_glb_anim_button", "asset_glb_mat_button"] {
        set_button_size(ctx, name, (0.166, 0.70));
    }
}

#[derive(Default)]
pub struct EditorView {
    project_root: String,
    project_name: String,
    create_parent_dir: String,
    recent_projects: Vec<String>,
    file_paths: Vec<String>,
    file_filter: String,
    file_scope: String,
    file_expanded_paths: Vec<String>,
    file_title: String,
    active_asset_path: String,
    scene_paths: Vec<String>,
    open_paths: Vec<String>,
    dirty_scene_paths: Vec<String>,
    active_open: usize,
    nodes: Vec<String>,
    selected_row: Option<usize>,
    inspector: InspectorViewData,
    glb_title: String,
    glb_summary: String,
    viewport: String,
    status: String,
    log: String,
    viewport_mode: String,
    activity_mode: String,
    sidebar_mode: String,
    scene_filter: String,
    anim_drawer_open: bool,
    anim_title: String,
    anim_status: String,
    anim_tracks: String,
    anim_can_create: bool,
    anim_can_add_track: bool,
    gizmo: editor_gizmos::GizmoView,
    selected_ui_rect: Option<EditorUiRect>,
    node_picker_rows: Vec<String>,
    node_picker_page: String,
    node_picker_filter: String,
    node_picker_parent: String,
    inspector_picker_open: bool,
    inspector_picker_title: String,
    inspector_picker_rows: Vec<String>,
    inspector_picker_page: String,
    inspector_picker_filter: String,
}

#[derive(Default, Clone)]
pub struct ResourceFieldView {
    pub name: String,
    pub value: String,
    pub picker_kind: String,
}

pub struct InspectorViewData {
    title: String,
    name: String,
    name_edit: String,
    kind: String,
    parent: String,
    node_actions: bool,
    asset_selected: bool,
    asset_actions: bool,
    asset_use_action: bool,
    asset_make_action: bool,
    asset_make_label: String,
    glb_asset_actions: bool,
    pos_label: String,
    pos: Vec<String>,
    rotation_label: String,
    rotation: Vec<String>,
    rotation_components: [&'static str; 4],
    rotation_mode: String,
    rotation_mode_buttons: bool,
    scale_label: String,
    scale: Vec<String>,
    script: String,
    visible_action_label: String,
    vars_text: String,
    script_vars: Vec<InspectorValueRow>,
    resource_fields: Vec<ResourceFieldView>,
    collapsed_sections: Vec<String>,
}

impl Default for InspectorViewData {
    fn default() -> Self {
        Self {
            title: "Inspector".to_string(),
            name: "No selection".to_string(),
            name_edit: "-".to_string(),
            kind: "Select node or asset".to_string(),
            parent: String::new(),
            node_actions: false,
            asset_selected: false,
            asset_actions: false,
            asset_use_action: false,
            asset_make_action: false,
            asset_make_label: "Node".to_string(),
            glb_asset_actions: false,
            pos_label: "v Transform".to_string(),
            pos: Vec::new(),
            rotation_label: "Rotation".to_string(),
            rotation: Vec::new(),
            rotation_components: ["x", "y", "z", "w"],
            rotation_mode: "quat".to_string(),
            rotation_mode_buttons: false,
            scale_label: "Scale".to_string(),
            scale: Vec::new(),
            script: "Script  -".to_string(),
            visible_action_label: "Vis".to_string(),
            vars_text: String::new(),
            script_vars: Vec::new(),
            resource_fields: Vec::new(),
            collapsed_sections: Vec::new(),
        }
    }
}

impl InspectorViewData {
    fn for_node(doc: &SceneDoc, node: &SceneNodeEntry, state: &EditorState) -> Self {
        let mut view = Self::default();
        let path = scene_node_path(doc, node.key);
        let child_count = scene_child_count(doc, node.key.as_u32());
        let script = node
            .script
            .as_ref()
            .map(|value| value.as_ref())
            .unwrap_or("-");
        let is_3d = node.data.node_type.is_a(perro_scene::NodeType::Node3D);
        let rotation_mode = if is_3d && state.inspector_rotation_mode == "euler" {
            "euler"
        } else {
            "quat"
        };
        let rotation = if rotation_mode == "euler" {
            scene_rotation_deg_components(&node.data)
        } else {
            scene_value_components(&node.data, "rotation")
        };

        view.name = "Name".to_string();
        view.name_edit = doc.scene.key_name_or_id(node.key).to_string();
        view.kind = format!("Type  {}", node_hierarchy_text(node.data.node_type));
        view.parent = format!("Path  {path}\nChildren  {child_count}");
        view.node_actions = true;
        view.pos = scene_value_components(&node.data, "position");
        view.rotation_label = if rotation_mode == "euler" {
            "Rotation Degrees".to_string()
        } else {
            "Rotation".to_string()
        };
        view.rotation_components = if rotation_mode == "euler" {
            ["x", "y", "z", ""]
        } else if rotation.len() == 1 {
            ["r", "", "", ""]
        } else {
            ["x", "y", "z", "w"]
        };
        view.rotation = rotation;
        view.rotation_mode = rotation_mode.to_string();
        view.rotation_mode_buttons = is_3d;
        view.scale = scene_value_components(&node.data, "scale");
        view.script = format!("Script  {script}");
        view.visible_action_label = if scene_field_bool(&node.data, "visible") == Some(false) {
            "Show".to_string()
        } else {
            "Hide".to_string()
        };
        view.collapsed_sections = state.inspector_collapsed_sections.clone();
        let script_fields = inspector_script_var_fields_for_node(state, node);
        view.vars_text = script_vars_edit_text(&script_fields);
        view.script_vars = inspector_script_var_rows(&script_fields, &state.inspector_expanded_paths);
        view.resource_fields = resource_field_rows(node);
        view.apply_asset_actions(state);
        view
    }

    fn for_asset(state: &EditorState) -> Self {
        let mut view = Self::default();
        if state.active_asset_path.is_empty() {
            return view;
        }
        let asset = asset_inspector_text(state);
        view.title = "Asset".to_string();
        view.name = "Name".to_string();
        view.name_edit = asset_edit_name(&state.active_asset_path);
        view.kind = asset.kind;
        view.parent = format!("{}\n{}", asset.path, asset.size);
        view.script = format!(
            "State  {}\n{}\nRefs\n{}\nActions\n{}",
            asset.state, asset.detail, asset.refs, asset.actions
        );
        view.collapsed_sections = state.inspector_collapsed_sections.clone();
        view.apply_asset_actions(state);
        view
    }

    fn apply_asset_actions(&mut self, state: &EditorState) {
        let path = state.active_asset_path.as_str();
        let kind = editor_files::kind_label(path);
        self.asset_selected = !path.is_empty();
        self.asset_actions = !path.is_empty() && !path.ends_with('/') && !state.doc_text.is_empty();
        self.asset_use_action = self.asset_actions && kind != "scene";
        self.asset_make_action = self.asset_actions;
        self.asset_make_label = if kind == "scene" {
            "Instance".to_string()
        } else {
            "Node".to_string()
        };
        self.glb_asset_actions = self.asset_actions && is_gltf_path(path);
    }
}

impl EditorView {
    fn from_state(state: &EditorState) -> Self {
        let mut nodes = Vec::new();
        let mut selected_row = None;
        let mut inspector = InspectorViewData::for_asset(state);
        let mut gizmo = editor_gizmos::GizmoView::default();
        let mut selected_ui_rect = None;
        let mut glb_title = "GLB Viewer".to_string();
        let mut glb_summary = "select .glb asset".to_string();

        if !state.doc_text.is_empty() {
            let doc = SceneDoc::parse(&state.doc_text);
            let tree = scene_tree_view(
                &doc,
                state.selected_key,
                &state.scene_filter,
                &state.collapsed_scene_keys,
            );
            gizmo = editor_gizmos::gizmo_view(&doc, state.selected_key);
            selected_ui_rect = state.selected_key.and_then(|key| doc_ui_rect(&doc, key));
            nodes = tree.labels;
            selected_row = tree.selected_row;

            if state.sidebar_mode != "files"
                && let Some(key) = state.selected_key.and_then(|raw| {
                doc.scene
                    .nodes
                    .iter()
                    .find(|node| node.key.as_u32() == raw)
                    .map(|node| node.key)
            }) && let Some(node) = doc.scene.nodes.iter().find(|node| node.key == key)
            {
                inspector = InspectorViewData::for_node(&doc, node, state);
            }
        }

        if state.activity_mode == "glb" {
            glb_title = if state.active_glb_path.is_empty() {
                "GLB Viewer".to_string()
            } else {
                format!(
                    "GLB Viewer  {}",
                    editor_files::rel_label(&state.active_glb_path)
                )
            };
            glb_summary = if state.active_glb_summary.is_empty() {
                "select .glb or .gltf from left list".to_string()
            } else {
                state.active_glb_summary.clone()
            };
        }

        let status = editor_status_text(state);
        let viewport = format!(
            "Viewport  mode={}  cam=({:.1}, {:.1}, {:.1})",
            state.viewport_mode, state.cam_x, state.cam_y, state.cam_z
        );
        let (anim_title, anim_status, anim_tracks, anim_can_create, anim_can_add_track) =
            animation_drawer_text(state);
        let node_picker_rows =
            picker_rows(state, &state.node_picker_filter, state.node_picker_offset);
        let page = (state.node_picker_offset / MAX_NODE_PICKER_ROWS) + 1;
        let picker_count = picker_node_types(state, &state.node_picker_filter)
            .len()
            .max(1);
        let page_count = picker_count.div_ceil(MAX_NODE_PICKER_ROWS);
        let node_picker_parent = picker_parent_text(state);
        let inspector_picker_rows = inspector_picker_rows(state);
        let inspector_picker_count = inspector_picker_entries(state).len().max(1);
        let inspector_picker_page =
            (state.inspector_picker_offset / MAX_INSPECTOR_PICKER_ROWS) + 1;
        let inspector_picker_page_count =
            inspector_picker_count.div_ceil(MAX_INSPECTOR_PICKER_ROWS);
        let inspector_picker_title = inspector_picker_title(state);
        Self {
            project_root: state.project_root.clone(),
            project_name: if state.project_name.is_empty() {
                "No project".to_string()
            } else {
                state.project_name.clone()
            },
            create_parent_dir: if state.create_parent_dir.is_empty() {
                "-".to_string()
            } else {
                state.create_parent_dir.clone()
            },
            recent_projects: state.recent_projects.clone(),
            file_paths: filtered_file_paths(state),
            file_filter: state.file_filter.clone(),
            file_scope: state.file_scope.clone(),
            file_expanded_paths: state.file_expanded_paths.clone(),
            file_title: file_panel_title(state),
            active_asset_path: state.active_asset_path.clone(),
            scene_paths: state.scene_paths.clone(),
            open_paths: state.open_paths.clone(),
            dirty_scene_paths: state.dirty_scene_paths.clone(),
            active_open: state.active_open,
            nodes,
            selected_row,
            inspector,
            glb_title,
            glb_summary,
            viewport,
            status,
            log: state.log.clone(),
            viewport_mode: state.viewport_mode.clone(),
            activity_mode: state.activity_mode.clone(),
            sidebar_mode: state.sidebar_mode.clone(),
            scene_filter: state.scene_filter.clone(),
            anim_drawer_open: state.anim_drawer_open,
            anim_title,
            anim_status,
            anim_tracks,
            anim_can_create,
            anim_can_add_track,
            gizmo,
            selected_ui_rect,
            node_picker_rows,
            node_picker_page: format!("page {page}/{page_count}"),
            node_picker_filter: state.node_picker_filter.clone(),
            node_picker_parent,
            inspector_picker_open: state.inspector_picker_open,
            inspector_picker_title,
            inspector_picker_rows,
            inspector_picker_page: format!(
                "page {inspector_picker_page}/{inspector_picker_page_count}"
            ),
            inspector_picker_filter: state.inspector_picker_filter.clone(),
        }
    }
}

pub struct AssetInspectorText {
    name: String,
    kind: String,
    path: String,
    size: String,
    refs: String,
    state: String,
    detail: String,
    actions: String,
}

pub fn editor_status_text(state: &EditorState) -> String {
    if state.project_root.is_empty() {
        return "ready | open project".to_string();
    }
    let node_count = if state.doc_text.is_empty() {
        0
    } else {
        SceneDoc::parse(&state.doc_text).scene.nodes.len()
    };
    let file_count = filtered_file_paths(state).len();
    let scope = if state.file_scope.is_empty() {
        "res://".to_string()
    } else {
        editor_view::short_path(&state.file_scope, 22)
    };
    let filters = [
        (!state.scene_filter.is_empty()).then_some("scene-filter"),
        (!state.file_filter.is_empty()).then_some("file-filter"),
        (!state.node_picker_filter.is_empty()).then_some("node-filter"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let filters = if filters.is_empty() {
        "-".to_string()
    } else {
        filters.join(",")
    };
    format!(
        "ready | {} | tabs={} dirty={} nodes={} files={} scope={} filters={} quick=CtrlAlt1-7",
        state.project_name,
        state.open_paths.len(),
        state.dirty,
        node_count,
        file_count,
        scope,
        filters
    )
}

pub fn asset_inspector_text(state: &EditorState) -> AssetInspectorText {
    let path = state.active_asset_path.as_str();
    let kind = editor_files::kind_label(path);
    let rel = editor_files::rel_label(path);
    let abs = res_to_abs(&state.project_root, path);
    let size = if path.ends_with('/') {
        "folder".to_string()
    } else {
        fs::metadata(&abs)
            .map(|meta| format!("{} bytes", meta.len()))
            .unwrap_or_else(|_| "missing".to_string())
    };
    let refs = asset_ref_text(state, path, kind);
    let detail = asset_detail_text(state, path, kind);
    let actions = match kind {
        "scene" => {
            "Enter -> open scene\nCtrl+Enter / Ctrl+Shift+Enter -> instance scene\nCtrl+Shift+G -> find user\nCtrl+E -> reveal tab".to_string()
        }
        "mesh" if is_gltf_path(path) => {
            "Use -> bind ref\nNode -> mesh node\nCtrl+Shift+G -> find user\n[] -> mesh  Shift+[] -> mat\nAnim -> .panim\nMat -> .pmat".to_string()
        }
        "mesh" => "Ctrl+Enter -> use\nCtrl+Shift+Enter -> node\nCtrl+Shift+G -> find user".to_string(),
        "resource" if path.ends_with(".panim") => {
            "Ctrl+Enter -> bind clip\nCtrl+Shift+G -> find user\nNew Track -> bind node".to_string()
        }
        "folder" => "Enter -> scope folder\nBackspace -> parent\nEsc -> root/filter clear".to_string(),
        _ => "Enter -> inspect\nCtrl+Enter -> use\nCtrl+Shift+Enter -> node\nCtrl+Shift+G -> find user".to_string(),
    };
    AssetInspectorText {
        name: format!("name: {rel}"),
        kind: format!("type: {kind}"),
        path: format!("path: {path}"),
        size,
        refs,
        state: if Path::new(&abs).exists() || path.ends_with('/') {
            "ok".to_string()
        } else {
            "missing".to_string()
        },
        detail,
        actions,
    }
}

pub fn asset_edit_name(path: &str) -> String {
    let rel = editor_files::rel_label(path);
    let trimmed = rel.trim_end_matches('/');
    Path::new(trimmed)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("-")
        .to_string()
}

pub fn asset_ref_text(state: &EditorState, path: &str, kind: &str) -> String {
    let users = asset_user_text(state, path);
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        return format!(
            "{}\n{}\n{}\n{}",
            indexed_ref(path, "mesh", usize::MAX, state.active_glb_mesh_index),
            indexed_ref(path, "mat", usize::MAX, state.active_glb_mat_index),
            indexed_ref(path, "animation", usize::MAX, state.active_glb_anim_index),
            users
        );
    }
    match kind {
        "scene" | "script" | "resource" | "mesh" | "image" | "audio" => {
            format!("{path}\n{users}")
        }
        _ => users,
    }
}

pub fn script_vars_edit_text(fields: &[(SceneFieldName, SceneValue)]) -> String {
    if fields.is_empty() {
        return String::new();
    }
    fields
        .iter()
        .map(|(name, value)| format!("{} = {}", name.as_ref(), scene_value_edit_text(value)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn inspector_var_button_label(row: &InspectorValueRow) -> String {
    row.value.clone()
}

pub fn resource_field_rows(node: &SceneNodeEntry) -> Vec<ResourceFieldView> {
    let mut rows = Vec::new();
    if let Some(script) = node.script.as_ref() {
        rows.push(ResourceFieldView {
            name: "script".to_string(),
            value: if script.trim().is_empty() {
                "[Select Script]".to_string()
            } else {
                editor_view::short_path(script, 22)
            },
            picker_kind: "asset".to_string(),
        });
    } else {
        rows.push(ResourceFieldView {
            name: "script".to_string(),
            value: "[Select Script]".to_string(),
            picker_kind: "asset".to_string(),
        });
    }
    rows.extend(
        perro_scene::scene_inspector_fields(node.data.node_type)
        .into_iter()
        .filter(|field| {
            matches!(
                field.kind,
                perro_scene::SceneInspectorValueKind::Asset(_)
                    | perro_scene::SceneInspectorValueKind::NodeRef
            )
        })
        .filter_map(|field| resource_field_row(&node.data, &field)),
    );
    rows
}

pub fn resource_field_row(
    data: &SceneNodeData,
    field: &perro_scene::SceneInspectorField,
) -> Option<ResourceFieldView> {
    let raw_value = scene_field_value_text(data, field.name);
    let value_text = raw_value
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let picker_kind = match field.kind {
        perro_scene::SceneInspectorValueKind::NodeRef => "node",
        perro_scene::SceneInspectorValueKind::Asset(_) => "asset",
        _ => return None,
    };
    let value = match field.kind {
        perro_scene::SceneInspectorValueKind::NodeRef => value_text
            .map(inspector_node_ref_label)
            .unwrap_or_else(|| "[Select Node]".to_string()),
        perro_scene::SceneInspectorValueKind::Asset(kind) => value_text
            .map(|value| editor_view::short_path(value, 22))
            .unwrap_or_else(|| format!("[Select {}]", inspector_asset_kind_label(kind))),
        _ => return None,
    };
    Some(ResourceFieldView {
        name: field.name.to_string(),
        value,
        picker_kind: picker_kind.to_string(),
    })
}

fn inspector_asset_kind_label(kind: perro_scene::SceneAssetKind) -> &'static str {
    match kind {
        perro_scene::SceneAssetKind::Scene => "Scene",
        perro_scene::SceneAssetKind::Script => "Script",
        perro_scene::SceneAssetKind::Texture => "Texture",
        perro_scene::SceneAssetKind::Mesh | perro_scene::SceneAssetKind::Model => "Mesh",
        perro_scene::SceneAssetKind::Material => "Material",
        perro_scene::SceneAssetKind::Animation => "Animation",
        perro_scene::SceneAssetKind::AnimationTree => "Animation Tree",
        perro_scene::SceneAssetKind::Skeleton => "Skeleton",
        perro_scene::SceneAssetKind::ParticleProfile => "Particle",
        perro_scene::SceneAssetKind::TileSet => "Tile Set",
        perro_scene::SceneAssetKind::UiStyle => "UI Style",
    }
}

#[derive(Clone)]
pub struct InspectorPickerEntry {
    pub value: String,
    pub label: String,
}

pub fn inspector_picker_entries(state: &EditorState) -> Vec<InspectorPickerEntry> {
    match state.inspector_picker_kind.as_str() {
        "node" | "script_node" => inspector_node_picker_entries(state),
        "asset" => inspector_asset_picker_entries(state),
        _ => Vec::new(),
    }
}

pub fn inspector_picker_rows(state: &EditorState) -> Vec<String> {
    inspector_picker_entries(state)
        .into_iter()
        .skip(state.inspector_picker_offset)
        .take(MAX_INSPECTOR_PICKER_ROWS)
        .map(|entry| entry.label)
        .collect()
}

pub fn inspector_picker_title(state: &EditorState) -> String {
    if state.inspector_picker_field.is_empty() {
        return "Pick".to_string();
    }
    match state.inspector_picker_kind.as_str() {
        "node" => format!("Pick Node  {}", state.inspector_picker_field),
        "script_node" => "Pick Node  script var".to_string(),
        "asset" => format!("Pick Asset  {}", state.inspector_picker_field),
        _ => "Pick".to_string(),
    }
}

fn inspector_node_picker_entries(state: &EditorState) -> Vec<InspectorPickerEntry> {
    if state.doc_text.is_empty() {
        return Vec::new();
    }
    let filter = NodePickerFilter::parse(&state.inspector_picker_filter);
    let doc = SceneDoc::parse(&state.doc_text);
    doc.scene
        .nodes
        .iter()
        .filter_map(|node| {
            let name = doc.scene.key_name_or_id(node.key).to_string();
            let path = scene_node_path(&doc, node.key);
            let hay = format!(
                "{} {} {}",
                name.to_ascii_lowercase(),
                path.to_ascii_lowercase(),
                node.data.type_name().to_ascii_lowercase()
            );
            if !filter.text.iter().all(|needle| hay.contains(needle)) {
                return None;
            }
            let label = format!(
                "{} {}  {}",
                node_type_icon(node.data.node_type),
                editor_view::short_path(&path, 26),
                node.data.type_name()
            );
            Some(InspectorPickerEntry { value: name, label })
        })
        .collect()
}

fn inspector_asset_picker_entries(state: &EditorState) -> Vec<InspectorPickerEntry> {
    let Some(kind) = inspector_picker_asset_kind(state) else {
        return Vec::new();
    };
    let filter = NodePickerFilter::parse(&state.inspector_picker_filter);
    let filters = perro_scene::scene_asset_filters(kind);
    state
        .file_paths
        .iter()
        .filter(|path| {
            !path.ends_with('/')
                && inspector_asset_path_matches(path, filters)
                && (filter.is_empty() || file_path_matches_filter(path, &filter))
        })
        .map(|path| {
            let value = inspector_asset_picker_value(path, kind, state.active_glb_mesh_index);
            let label = format!(
                "{}  {}",
                editor_files::display_kind_label(path),
                editor_view::short_path(&value, 34)
            );
            InspectorPickerEntry { value, label }
        })
        .collect()
}

fn inspector_picker_asset_kind(state: &EditorState) -> Option<perro_scene::SceneAssetKind> {
    if state.inspector_picker_field == "script" {
        return Some(perro_scene::SceneAssetKind::Script);
    }
    let key = state.selected_key?;
    let doc = SceneDoc::parse(&state.doc_text);
    let node = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?;
    let field = perro_scene::scene_inspector_field(node.data.node_type, &state.inspector_picker_field)?;
    match field.kind {
        perro_scene::SceneInspectorValueKind::Asset(kind) => Some(kind),
        _ => None,
    }
}

fn inspector_asset_path_matches(path: &str, filters: &[perro_scene::SceneAssetFilter]) -> bool {
    let ext = path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    filters
        .iter()
        .any(|filter| filter.extensions.iter().any(|item| *item == ext))
}

fn inspector_asset_picker_value(
    path: &str,
    kind: perro_scene::SceneAssetKind,
    glb_mesh_index: usize,
) -> String {
    match kind {
        perro_scene::SceneAssetKind::Mesh if is_gltf_path(path) => {
            format!("{path}:mesh[{glb_mesh_index}]")
        }
        _ => path.to_string(),
    }
}

pub fn node_hierarchy_text(node_type: perro_scene::NodeType) -> String {
    let name = node_type.name();
    if node_type.is_a(perro_scene::NodeType::UiBox) {
        return format!("Node > UiBox > {name}");
    }
    if node_type.is_a(perro_scene::NodeType::Node3D) {
        return format!("Node > Node3D > {name}");
    }
    if node_type.is_a(perro_scene::NodeType::Node2D) {
        return format!("Node > Node2D > {name}");
    }
    format!("Node > {name}")
}

pub fn scene_value_kind(value: &SceneValue) -> &'static str {
    match value {
        SceneValue::Str(_) => "String",
        SceneValue::Key(_) => "Node",
        SceneValue::Bool(_) => "Bool",
        SceneValue::F32(_) => "Number",
        SceneValue::I32(_) => "Number",
        SceneValue::Hashed(_) => "Hash",
        SceneValue::Vec2 { .. } => "Vec2",
        SceneValue::Vec3 { .. } => "Vec3",
        SceneValue::Vec4 { .. } => "Quat",
        SceneValue::Array(_) => "Array",
        SceneValue::Object(_) => "Object",
    }
}

pub fn scene_value_edit_text(value: &SceneValue) -> String {
    match value {
        SceneValue::Str(value) => format!("\"{}\"", value.replace('"', "\\\"")),
        SceneValue::Key(key) => inspector_node_ref_label(key.as_ref()),
        SceneValue::Bool(value) => value.to_string(),
        SceneValue::F32(value) => format_compact_f32(*value),
        SceneValue::I32(value) => value.to_string(),
        SceneValue::Hashed(value) => value.to_string(),
        SceneValue::Vec2 { x, y } => {
            format!("({}, {})", format_compact_f32(*x), format_compact_f32(*y))
        }
        SceneValue::Vec3 { x, y, z } => format!(
            "({}, {}, {})",
            format_compact_f32(*x),
            format_compact_f32(*y),
            format_compact_f32(*z)
        ),
        SceneValue::Vec4 { x, y, z, w } => format!(
            "({}, {}, {}, {})",
            format_compact_f32(*x),
            format_compact_f32(*y),
            format_compact_f32(*z),
            format_compact_f32(*w)
        ),
        SceneValue::Array(values) => {
            let values = values
                .iter()
                .map(scene_value_edit_text)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        SceneValue::Object(fields) => {
            let fields = fields
                .iter()
                .map(|(name, value)| {
                    format!("{} = {}", name.as_ref(), scene_value_edit_text(value))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {fields} }}")
        }
    }
}

pub fn inspector_node_ref_label(value: &str) -> String {
    if value.trim().is_empty() {
        "[Select Node]".to_string()
    } else {
        format!("Node {value}")
    }
}

pub fn format_compact_f32(value: f32) -> String {
    let text = format!("{value:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn asset_user_text(state: &EditorState, path: &str) -> String {
    if state.doc_text.is_empty() || path.ends_with('/') {
        return "users: -".to_string();
    }
    let doc = SceneDoc::parse(&state.doc_text);
    let mut users = Vec::new();
    for node in doc.scene.nodes.iter() {
        if !node_uses_asset_path(node, path) {
            continue;
        }
        users.push(format!(
            "{} : {}",
            doc.scene.key_name_or_id(node.key),
            node.data.type_name()
        ));
        if users.len() >= 4 {
            break;
        }
    }
    let total = doc
        .scene
        .nodes
        .iter()
        .filter(|node| node_uses_asset_path(node, path))
        .count();
    if users.is_empty() {
        "users: -".to_string()
    } else if total > users.len() {
        format!(
            "users: {total}\n{}\n+{} more",
            users.join("\n"),
            total - users.len()
        )
    } else {
        format!("users: {total}\n{}", users.join("\n"))
    }
}

pub fn asset_detail_text(state: &EditorState, path: &str, kind: &str) -> String {
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        return if state.active_glb_path == path && !state.active_glb_summary.is_empty() {
            state.active_glb_summary.clone()
        } else {
            "GLB asset\nopen row -> inspect meshes/materials/animations".to_string()
        };
    }
    if path.ends_with(".panim") {
        return panim_summary(&state.project_root, path);
    }
    if kind == "script" || (kind == "resource" && !path.ends_with(".panim")) {
        return text_asset_preview(&state.project_root, path);
    }
    if kind == "scene"
        && !state.doc_text.is_empty()
        && state.open_paths.get(state.active_open).map(String::as_str) == Some(path)
    {
        let doc = SceneDoc::parse(&state.doc_text);
        return format!(
            "nodes={}\nmode={}",
            doc.scene.nodes.len(),
            editor_scene::root_viewport_mode(&doc)
        );
    }
    format!("{kind} asset")
}

pub fn text_asset_preview(project_root: &str, path: &str) -> String {
    let abs = res_to_abs(project_root, path);
    let Ok(text) = FileMod::load_string(&abs) else {
        return "text preview\nnot readable".to_string();
    };
    let lines = text
        .lines()
        .take(8)
        .map(|line| {
            let line = line.trim_end();
            let mut short = line.chars().take(72).collect::<String>();
            if line.chars().count() > 72 {
                short.push_str("...");
                short
            } else {
                short
            }
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        "text preview\n(empty)".to_string()
    } else {
        format!("text preview\n{}", lines.join("\n"))
    }
}

pub fn animation_drawer_text(state: &EditorState) -> (String, String, String, bool, bool) {
    if !state.active_glb_path.is_empty() {
        return (
            format!(
                "GLB Viewer  {}",
                editor_files::rel_label(&state.active_glb_path)
            ),
            "container refs stay usable in scene fields".to_string(),
            state.active_glb_summary.clone(),
            false,
            false,
        );
    }
    let Some(key) = state.active_anim_player_key else {
        if state.active_anim_path.is_empty() {
            return (
                "Animation".to_string(),
                "select AnimationPlayer or open .panim".to_string(),
                "no live binding".to_string(),
                false,
                false,
            );
        }
        return (
            format!(
                "Animation Data  {}",
                editor_files::rel_label(&state.active_anim_path)
            ),
            ".panim data view\nno scene binding until selected AnimationPlayer references it"
                .to_string(),
            panim_summary(&state.project_root, &state.active_anim_path),
            false,
            false,
        );
    };
    let doc = SceneDoc::parse(&state.doc_text);
    let name = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)
        .map(|node| doc.scene.key_name_or_id(node.key).to_string())
        .unwrap_or_else(|| "AnimationPlayer".to_string());
    let path = if state.active_anim_path.is_empty() {
        selected_node_field_text(&state.doc_text, key, "animation")
            .unwrap_or_else(|| "-".to_string())
    } else {
        state.active_anim_path.clone()
    };
    (
        format!("Animation Player  {name}"),
        format!(
            "player={name}\ncurrent animations:\n{path}\ncreate writes .panim + binds Target to parent node"
        ),
        if path == "-" {
            "no clip bound".to_string()
        } else {
            panim_summary(&state.project_root, &path)
        },
        true,
        true,
    )
}

pub fn gltf_summary(
    path: &str,
    mesh_count: usize,
    material_count: usize,
    animation_count: usize,
    skeleton_count: usize,
    texture_count: usize,
    node_count: usize,
    scene_count: usize,
    mesh_index: usize,
    mat_index: usize,
    anim_index: usize,
) -> String {
    format!(
        "GLB  {}\nselected:\nmesh = {}\nmat = {}\nanim = {}\nmeshes: {}\n{}\nmaterials: {}\n{}\nanimations: {}\n{}\nskins: {}\ntextures: {}\nnodes: {} scenes: {}\nkeys: [] mesh  Shift+[] mat  Ctrl+Shift+[] anim\nconvert:\n- anim -> perro_cli import_anim {} --output res/animations/<clip>.panim --clip {}\n- mesh -> static pipeline emits {}:mesh[index] pmesh entries\n- mat -> static pipeline emits {}:mat[index] pmat refs",
        editor_files::rel_label(path),
        indexed_ref(path, "mesh", mesh_count, mesh_index),
        indexed_ref(path, "mat", material_count, mat_index),
        indexed_ref(path, "animation", animation_count, anim_index),
        mesh_count,
        indexed_refs(path, "mesh", mesh_count),
        material_count,
        indexed_refs(path, "mat", material_count),
        animation_count,
        indexed_refs(path, "animation", animation_count),
        skeleton_count,
        texture_count,
        node_count,
        scene_count,
        editor_files::rel_label(path),
        anim_index,
        path,
        path
    )
}

pub fn indexed_ref(path: &str, kind: &str, count: usize, index: usize) -> String {
    if count == 0 {
        "-".to_string()
    } else {
        format!("{path}:{kind}[{}]", index.min(count - 1))
    }
}

pub fn indexed_refs(path: &str, kind: &str, count: usize) -> String {
    if count == 0 {
        return "-".to_string();
    }
    let shown = count.min(6);
    let mut out = (0..shown)
        .map(|idx| format!("{path}:{kind}[{idx}]"))
        .collect::<Vec<_>>()
        .join("\n");
    if count > shown {
        out.push_str(&format!("\n+{} more", count - shown));
    }
    out
}

pub fn panim_summary(project_root: &str, anim_path: &str) -> String {
    if anim_path.is_empty() || anim_path == "-" {
        return "no .panim".to_string();
    }
    let abs = res_to_abs(project_root, anim_path);
    let Ok(text) = FileMod::load_string(&abs) else {
        return "clip not readable".to_string();
    };
    let mut in_objects = false;
    let mut objects = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[Objects]" {
            in_objects = true;
            continue;
        }
        if trimmed == "[/Objects]" {
            break;
        }
        if in_objects && trimmed.contains('=') {
            objects.push(trimmed.to_string());
        }
        if objects.len() >= 6 {
            break;
        }
    }
    let objects = objects.join("\n");
    let frame_count = text
        .lines()
        .filter(|line| line.trim().starts_with("[Frame"))
        .count();
    format!(
        "frames={frame_count}\nobjects:\n{}",
        if objects.is_empty() { "-" } else { &objects }
    )
}

#[derive(Default)]
pub struct SceneTreeRows {
    pub labels: Vec<String>,
    pub keys: Vec<u32>,
    pub selected_row: Option<usize>,
}

pub fn scene_tree_view(
    doc: &SceneDoc,
    selected_key: Option<u32>,
    filter: &str,
    collapsed_keys: &[u32],
) -> SceneTreeRows {
    let filter = NodePickerFilter::parse(filter);
    if !filter.is_empty() {
        return filtered_scene_tree_view(doc, selected_key, &filter);
    }
    let mut out = SceneTreeRows::default();
    let mut visited = Vec::new();
    let mut roots = Vec::new();

    if let Some(root) = doc.scene.root {
        roots.push(root.as_u32());
    }
    for node in doc.scene.nodes.iter() {
        let key = node.key.as_u32();
        if node.parent.is_none() && !roots.contains(&key) {
            roots.push(key);
        }
    }
    for key in roots {
        push_scene_tree_row(
            doc,
            key,
            0,
            selected_key,
            collapsed_keys,
            &mut visited,
            &mut out,
        );
    }
    for node in doc.scene.nodes.iter() {
        let key = node.key.as_u32();
        if !visited.contains(&key) {
            push_scene_tree_row(
                doc,
                key,
                0,
                selected_key,
                collapsed_keys,
                &mut visited,
                &mut out,
            );
        }
    }
    out
}

pub fn scene_node_path(doc: &SceneDoc, key: SceneKey) -> String {
    let mut parts = Vec::new();
    let mut cursor = Some(key);
    let mut guard = 0;
    while let Some(key) = cursor {
        parts.push(doc.scene.key_name_or_id(key).to_string());
        cursor = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key == key)
            .and_then(|node| node.parent);
        guard += 1;
        if guard > doc.scene.nodes.len() {
            break;
        }
    }
    parts.reverse();
    parts.join("/")
}

pub fn filtered_scene_tree_view(
    doc: &SceneDoc,
    selected_key: Option<u32>,
    filter: &NodePickerFilter,
) -> SceneTreeRows {
    let mut out = SceneTreeRows::default();
    for node in doc.scene.nodes.iter() {
        if out.labels.len() >= MAX_NODES {
            break;
        }
        let name = doc.scene.key_name_or_id(node.key).to_string();
        let type_name = node.data.type_name();
        if !scene_node_matches_filter(doc, node, &name, type_name, filter) {
            continue;
        }
        let row = out.labels.len();
        let key = node.key.as_u32();
        let prefix = if Some(key) == selected_key {
            out.selected_row = Some(row);
            ">"
        } else {
            " "
        };
        let path = scene_node_path(doc, node.key);
        out.labels.push(scene_row_label(
            prefix,
            0,
            &name,
            type_name,
            node_type_icon(node.data.node_type),
            &scene_node_badges(node),
            scene_child_count(doc, key),
            false,
            Some(key) == selected_key,
            Some(&path),
        ));
        out.keys.push(key);
    }
    out
}

pub fn scene_node_matches_filter(
    doc: &SceneDoc,
    node: &SceneNodeEntry,
    name: &str,
    type_name: &str,
    filter: &NodePickerFilter,
) -> bool {
    let path = scene_node_path(doc, node.key);
    let badges = scene_node_badges(node);
    let hay = format!(
        "{} {} {} {} {}",
        name.to_ascii_lowercase(),
        type_name.to_ascii_lowercase(),
        path.to_ascii_lowercase(),
        node_type_search_text(node.data.node_type),
        badges.to_ascii_lowercase()
    );
    filter.text.iter().all(|needle| hay.contains(needle))
        && filter
            .tags
            .iter()
            .all(|tag| scene_node_has_filter_tag(node, tag))
}

pub fn scene_node_has_filter_tag(node: &SceneNodeEntry, tag: &str) -> bool {
    match tag {
        "script" | "scr" => node.script.is_some(),
        "res" | "resource" => selected_node_asset_refs(node)
            .iter()
            .any(|item| !item.starts_with("script:") && !item.starts_with("root_of:")),
        "hidden" | "hide" | "hid" => scene_field_bool(&node.data, "visible") == Some(false),
        "inst" | "instance" => node.root_of.is_some(),
        _ => node_type_has_picker_tag(node.data.node_type, tag),
    }
}

pub fn scene_child_count(doc: &SceneDoc, key: u32) -> usize {
    doc.scene
        .nodes
        .iter()
        .filter(|node| node.parent.map(|parent| parent.as_u32()) == Some(key))
        .count()
}

pub fn push_scene_tree_row(
    doc: &SceneDoc,
    key: u32,
    depth: usize,
    selected_key: Option<u32>,
    collapsed_keys: &[u32],
    visited: &mut Vec<u32>,
    out: &mut SceneTreeRows,
) -> Option<usize> {
    if out.labels.len() >= MAX_NODES || visited.contains(&key) {
        return None;
    }
    let node = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?;
    visited.push(key);
    let row = out.labels.len();
    let prefix = if Some(key) == selected_key {
        out.selected_row = Some(row);
        ">"
    } else {
        " "
    };
    let children = scene_child_count(doc, key);
    out.labels.push(scene_row_label(
        prefix,
        depth,
        &doc.scene.key_name_or_id(node.key).to_string(),
        node.data.type_name(),
        node_type_icon(node.data.node_type),
        &scene_node_badges(node),
        children,
        collapsed_keys.contains(&key),
        Some(key) == selected_key,
        None,
    ));
    out.keys.push(key);
    if children > 0 && collapsed_keys.contains(&key) {
        return Some(row);
    }
    for child in doc
        .scene
        .nodes
        .iter()
        .filter(|child| child.parent.map(|parent| parent.as_u32()) == Some(key))
    {
        let _ = push_scene_tree_row(
            doc,
            child.key.as_u32(),
            depth + 1,
            selected_key,
            collapsed_keys,
            visited,
            out,
        );
    }
    Some(row)
}

pub fn scene_row_label(
    prefix: &str,
    depth: usize,
    name: &str,
    type_name: &str,
    icon: &str,
    badges: &str,
    children: usize,
    collapsed: bool,
    selected: bool,
    parent: Option<&str>,
) -> String {
    let name = editor_view::short_path(name, 20);
    let type_name = editor_view::short_path(type_name, 18);
    let fold = if children == 0 {
        " "
    } else if collapsed {
        ">"
    } else {
        "v"
    };
    let indent = "  ".repeat(depth.min(8));
    let selected = if selected { "*" } else { " " };
    if let Some(parent) = parent {
        format!(
            "{selected}{prefix}{indent}{fold} {icon} {name}  [{type_name}]{badges}  {}",
            editor_view::short_path(parent, 14)
        )
    } else {
        let child_suffix = if children == 0 {
            String::new()
        } else {
            format!("  {children}")
        };
        format!("{selected}{prefix}{indent}{fold} {icon} {name}  [{type_name}]{badges}{child_suffix}")
    }
}

pub fn scene_node_badges(node: &SceneNodeEntry) -> String {
    let mut out = Vec::new();
    if scene_field_bool(&node.data, "visible") == Some(false) {
        out.push("hid");
    }
    if node.root_of.is_some() {
        out.push("inst");
    }
    if node.script.is_some() {
        out.push("scr");
    }
    if selected_node_asset_refs(node)
        .iter()
        .any(|item| !item.starts_with("script:") && !item.starts_with("root_of:"))
    {
        out.push("res");
    }
    if out.is_empty() {
        String::new()
    } else {
        format!(" {}", out.join(" "))
    }
}

pub fn picker_rows(state: &EditorState, filter: &str, offset: usize) -> Vec<String> {
    picker_node_types(state, filter)
        .into_iter()
        .skip(offset)
        .take(MAX_NODE_PICKER_ROWS)
        .map(|node_type| picker_node_row(state, node_type))
        .collect()
}

pub fn filtered_file_paths(state: &EditorState) -> Vec<String> {
    let filter = NodePickerFilter::parse(&state.file_filter);
    let mut paths = state
        .file_paths
        .iter()
        .filter(|path| {
            if state.activity_mode == "glb" && !is_gltf_path(path) {
                return false;
            }
            let visible = if filter.is_empty() && state.activity_mode != "glb" {
                file_path_tree_visible(path, &state.file_expanded_paths)
            } else {
                true
            };
            visible && (filter.is_empty() || file_path_matches_filter(path, &filter))
        })
        .cloned()
        .collect::<Vec<_>>();
    if filter.is_empty() && state.activity_mode != "glb" {
        paths.insert(0, "res://".to_string());
    }
    paths
}

pub fn file_row_label(path: &str) -> String {
    let depth = file_path_depth(path);
    let name = file_base_name(path);
    let label = format!("{}{}", "  ".repeat(depth.min(6)), name);
    editor_view::short_path(&label, 30)
}

pub fn file_row_label_for_filter(path: &str, filtered: bool) -> String {
    if filtered {
        return editor_view::short_path(&editor_files::rel_label(path), 30);
    }
    file_row_label(path)
}

pub fn file_row_state_prefix(path: &str, open_paths: &[String], dirty_scene_paths: &[String]) -> &'static str {
    let dirty = dirty_scene_paths.iter().any(|dirty| dirty == path);
    let open = open_paths.iter().any(|open| open == path);
    if dirty {
        "* "
    } else if open {
        "> "
    } else {
        ""
    }
}

pub fn file_row_icon(path: &str, expanded_paths: &[String]) -> &'static str {
    if path.ends_with('/') {
        if expanded_paths.iter().any(|item| item == path) {
            "v"
        } else {
            ">"
        }
    } else {
        editor_files::display_kind_label(path)
    }
}

pub fn file_path_tree_visible(path: &str, expanded_paths: &[String]) -> bool {
    let parent = parent_res_folder(path);
    if parent.is_empty() {
        return true;
    }
    file_ancestor_folders(path)
        .into_iter()
        .all(|folder| expanded_paths.iter().any(|item| item == &folder))
}

pub fn file_ancestor_folders(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = parent_res_folder(path);
    while !cursor.is_empty() {
        out.push(cursor.clone());
        cursor = parent_res_folder(&cursor);
    }
    out.reverse();
    out
}

pub fn file_path_depth(path: &str) -> usize {
    file_ancestor_folders(path).len()
}

pub fn file_base_name(path: &str) -> String {
    let rel = editor_files::rel_label(path);
    if rel.is_empty() {
        return "res://".to_string();
    }
    rel.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(rel.as_str())
        .to_string()
}

pub fn file_path_matches_filter(path: &str, filter: &NodePickerFilter) -> bool {
    let hay = format!(
        "{} {} {} {}",
        path.to_ascii_lowercase(),
        editor_files::rel_label(path).to_ascii_lowercase(),
        editor_files::kind_label(path).to_ascii_lowercase(),
        editor_files::display_kind_label(path).to_ascii_lowercase()
    );
    filter.text.iter().all(|needle| hay.contains(needle))
        && filter.tags.iter().all(|tag| file_path_has_tag(path, tag))
}

pub fn file_path_has_tag(path: &str, tag: &str) -> bool {
    let kind = editor_files::kind_label(path);
    let badge = editor_files::display_kind_label(path).to_ascii_lowercase();
    match tag {
        "dir" | "folder" => path.ends_with('/'),
        "scene" | "scn" => kind == "scene",
        "script" | "rs" => kind == "script",
        "img" | "image" => kind == "image",
        "audio" | "aud" => kind == "audio",
        "mesh" => kind == "mesh",
        "glb" | "gltf" => badge == "glb",
        "anim" | "panim" => path.ends_with(".panim"),
        "mat" | "pmat" => path.ends_with(".pmat"),
        "res" | "resource" => kind == "resource",
        _ => badge.contains(tag) || kind.contains(tag) || path.to_ascii_lowercase().contains(tag),
    }
}

pub fn file_panel_title(state: &EditorState) -> String {
    if state.activity_mode == "glb" {
        return "GLB Files".to_string();
    }
    if state.active_asset_path.ends_with('/') {
        format!(
            "FileSystem  {}",
            editor_view::short_path(&state.active_asset_path, 24)
        )
    } else {
        "FileSystem  res://".to_string()
    }
}

pub fn file_path_in_scope(path: &str, scope: &str) -> bool {
    if scope.is_empty() {
        let rel = editor_files::rel_label(path);
        if path.ends_with('/') {
            return rel.trim_end_matches('/').matches('/').count() == 0;
        }
        return !rel.contains('/');
    }
    let Some(rest) = path.strip_prefix(scope) else {
        return false;
    };
    !rest.is_empty() && !rest.trim_end_matches('/').contains('/')
}

pub fn file_path_in_scope_or_descendant(path: &str, scope: &str) -> bool {
    if scope.is_empty() {
        return true;
    }
    path.strip_prefix(scope).is_some_and(|rest| !rest.is_empty())
}

pub fn parent_res_folder(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    let Some((head, _tail)) = trimmed.rsplit_once('/') else {
        return String::new();
    };
    if head == "res:" || head == "res://" {
        String::new()
    } else {
        format!("{head}/")
    }
}

pub fn picker_parent_text(state: &EditorState) -> String {
    if state.doc_text.is_empty() {
        return "target: -".to_string();
    }
    let doc = SceneDoc::parse(&state.doc_text);
    let Some(key) = state
        .selected_key
        .or_else(|| doc.scene.root.map(|key| key.as_u32()))
    else {
        return "target: scene root".to_string();
    };
    let Some(node) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) else {
        return "target: scene root".to_string();
    };
    if state.add_node_as_sibling {
        let parent = node
            .parent
            .map(|key| doc.scene.key_name_or_id(key).to_string())
            .unwrap_or_else(|| "scene root".to_string());
        let kind = picker_parent_node_kind(state).unwrap_or("Root");
        return format!(
            "as sibling of {}  parent: {parent}  kind={kind}  Tab toggles",
            doc.scene.key_name_or_id(node.key)
        );
    }
    format!(
        "as child of {} ({})  kind={}  Tab toggles",
        doc.scene.key_name_or_id(node.key),
        node.data.type_name(),
        node_type_kind(node.data.node_type)
    )
}

pub fn node_type_icon(node_type: perro_scene::NodeType) -> &'static str {
    match node_type.name() {
        "Sprite2D" => "[SPR]",
        "Camera2D" | "Camera3D" => "[CAM]",
        "MeshInstance3D" | "MultiMeshInstance3D" => "[MSH]",
        "PointLight2D" | "SpotLight2D" | "RayLight2D" | "AmbientLight2D" | "PointLight3D"
        | "SpotLight3D" | "RayLight3D" | "AmbientLight3D" => "[LGT]",
        "AudioPlayer2D"
        | "AudioStreamPlayer2D"
        | "AudioArea2D"
        | "AudioPlayer3D"
        | "AudioStreamPlayer3D"
        | "AudioArea3D" => "[AUD]",
        "PhysicsBody2D" | "StaticBody2D" | "RigidBody2D" | "CharacterBody2D" | "Area2D"
        | "CollisionShape2D" | "PhysicsBody3D" | "StaticBody3D" | "RigidBody3D"
        | "CharacterBody3D" | "Area3D" | "CollisionShape3D" => "[PHY]",
        _ if node_type.is_a(perro_scene::NodeType::UiBox) => "[UI]",
        _ if node_type.is_a(perro_scene::NodeType::Node2D) => "[2D]",
        _ if node_type.is_a(perro_scene::NodeType::Node3D) => "[3D]",
        _ if node_type.name().ends_with("Resource") => "[RES]",
        _ => "[NOD]",
    }
}

pub fn find_position_text(data: &SceneNodeData) -> Option<String> {
    find_scene_value_text(data, "position")
}

pub fn scene_value_components(data: &SceneNodeData, field: &str) -> Vec<String> {
    scene_value_override_components(data, field)
        .or_else(|| default_scene_value_components(data.node_type, field))
        .unwrap_or_default()
}

pub fn scene_rotation_deg_components(data: &SceneNodeData) -> Vec<String> {
    if let Some(values) = scene_value_override_components(data, "rotation_deg") {
        return values;
    }
    if let Some(values) = scene_value_override_components(data, "rotation")
        && values.len() == 4
    {
        let parsed = values
            .iter()
            .filter_map(|value| value.parse::<f32>().ok())
            .collect::<Vec<_>>();
        if let [x, y, z, w] = parsed.as_slice() {
            return quat_to_euler_deg_components(*x, *y, *z, *w);
        }
    }
    vec!["0".to_string(), "0".to_string(), "0".to_string()]
}

pub fn quat_to_euler_deg_components(x: f32, y: f32, z: f32, w: f32) -> Vec<String> {
    let len = (x * x + y * y + z * z + w * w).sqrt();
    if len <= 0.0 {
        return vec!["0".to_string(), "0".to_string(), "0".to_string()];
    }
    let x = x / len;
    let y = y / len;
    let z = z / len;
    let w = w / len;
    let sinr_cosp = 2.0 * (w * x + y * z);
    let cosr_cosp = 1.0 - 2.0 * (x * x + y * y);
    let roll = sinr_cosp.atan2(cosr_cosp);
    let sinp = 2.0 * (w * y - z * x);
    let pitch = if sinp.abs() >= 1.0 {
        sinp.signum() * std::f32::consts::FRAC_PI_2
    } else {
        sinp.asin()
    };
    let siny_cosp = 2.0 * (w * z + x * y);
    let cosy_cosp = 1.0 - 2.0 * (y * y + z * z);
    let yaw = siny_cosp.atan2(cosy_cosp);
    let to_deg = 180.0 / std::f32::consts::PI;
    vec![
        format_compact_f32(roll * to_deg),
        format_compact_f32(pitch * to_deg),
        format_compact_f32(yaw * to_deg),
    ]
}

pub fn scene_value_override_components(data: &SceneNodeData, field: &str) -> Option<Vec<String>> {
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            return Some(match value {
                SceneValue::F32(value) => vec![format_compact_f32(*value)],
                SceneValue::Vec2 { x, y } => {
                    vec![format_compact_f32(*x), format_compact_f32(*y)]
                }
                SceneValue::Vec3 { x, y, z } => vec![
                    format_compact_f32(*x),
                    format_compact_f32(*y),
                    format_compact_f32(*z),
                ],
                SceneValue::Vec4 { x, y, z, w } => vec![
                    format_compact_f32(*x),
                    format_compact_f32(*y),
                    format_compact_f32(*z),
                    format_compact_f32(*w),
                ],
                _ => Vec::new(),
            });
        }
    }
    data.base_ref()
        .and_then(|base| scene_value_override_components(base, field))
}

pub fn default_scene_value_components(
    node_type: perro_scene::NodeType,
    field: &str,
) -> Option<Vec<String>> {
    perro_scene::default_scene_field_value_by_name(node_type, field)
        .map(|value| scene_value_components_from_value(&value))
}

pub fn scene_value_components_from_value(value: &SceneValue) -> Vec<String> {
    match value {
        SceneValue::F32(value) => vec![format_compact_f32(*value)],
        SceneValue::I32(value) => vec![value.to_string()],
        SceneValue::Vec2 { x, y } => {
            vec![format_compact_f32(*x), format_compact_f32(*y)]
        }
        SceneValue::Vec3 { x, y, z } => vec![
            format_compact_f32(*x),
            format_compact_f32(*y),
            format_compact_f32(*z),
        ],
        SceneValue::Vec4 { x, y, z, w } => vec![
            format_compact_f32(*x),
            format_compact_f32(*y),
            format_compact_f32(*z),
            format_compact_f32(*w),
        ],
        _ => Vec::new(),
    }
}

pub fn find_vec2_value(data: &SceneNodeData, field: &str) -> Option<Vector2> {
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            return match value {
                SceneValue::Vec2 { x, y } => Some(Vector2::new(*x, *y)),
                SceneValue::Vec3 { x, y, .. } => Some(Vector2::new(*x, *y)),
                _ => None,
            };
        }
    }
    data.base_ref()
        .and_then(|base| find_vec2_value(base, field))
}

pub fn find_vec3_value(data: &SceneNodeData, field: &str) -> Option<Vector3> {
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            return match value {
                SceneValue::Vec3 { x, y, z } => Some(Vector3::new(*x, *y, *z)),
                SceneValue::Vec2 { x, y } => Some(Vector3::new(*x, *y, 0.0)),
                _ => None,
            };
        }
    }
    data.base_ref()
        .and_then(|base| find_vec3_value(base, field))
}

pub fn find_scene_value_text(data: &SceneNodeData, field: &str) -> Option<String> {
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            return match value {
                SceneValue::F32(value) => Some(format!("{value:.2}")),
                SceneValue::Vec2 { x, y } => Some(format!("({x:.2}, {y:.2})")),
                SceneValue::Vec3 { x, y, z } => Some(format!("({x:.2}, {y:.2}, {z:.2})")),
                _ => None,
            };
        }
    }
    data.base_ref()
        .and_then(|base| find_scene_value_text(base, field))
}

pub fn unique_node_name(doc: &SceneDoc, prefix: &str) -> String {
    for idx in 1..1000 {
        let name = format!("{prefix}_{idx}");
        if !doc.scene.key_names.iter().any(|item| item.as_ref() == name) {
            return name;
        }
    }
    format!("{prefix}_x")
}

pub fn sanitize_node_name(text: &str) -> String {
    let mut out = String::new();
    for ch in text.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '-' || ch == '.' {
            out.push('_');
        }
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

pub fn parse_project_name(text: &str) -> Option<String> {
    let mut in_project = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[project]" {
            in_project = true;
            continue;
        }
        if in_project && trimmed.starts_with('[') {
            return None;
        }
        if in_project && trimmed.starts_with("name") {
            let (_, value) = trimmed.split_once('=')?;
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

pub fn abs_to_res(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root.join("res")).ok()?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    Some(format!("res://{}", rel.trim_start_matches('/')))
}

pub fn res_to_abs(root: &str, res_path: &str) -> String {
    let rel = res_path.trim_start_matches("res://");
    Path::new(root)
        .join("res")
        .join(rel)
        .to_string_lossy()
        .to_string()
}

pub fn is_gltf_path(path: &str) -> bool {
    path.ends_with(".glb") || path.ends_with(".gltf")
}

pub fn rewrite_project_res_paths(doc: &SceneDoc, project_root: &str) -> SceneDoc {
    let mut doc = doc.clone();
    for node in doc.scene.nodes.to_mut().iter_mut() {
        node.script = None;
        node.clear_script = true;
        if let Some(root_of) = node.root_of.as_mut()
            && root_of.starts_with("res://")
        {
            *root_of = Cow::Owned(res_to_abs(project_root, root_of));
        }
        rewrite_project_res_data(&mut node.data, project_root);
        for (_, value) in node.script_vars.to_mut().iter_mut() {
            rewrite_project_res_value(value, project_root);
        }
    }
    doc
}

pub fn rewrite_project_res_data(data: &mut SceneNodeData, project_root: &str) {
    for (_, value) in data.fields.to_mut().iter_mut() {
        rewrite_project_res_value(value, project_root);
    }
    if let Some(base) = data.base.as_mut() {
        match base {
            perro_scene::SceneNodeDataBase::Borrowed(_) => {}
            perro_scene::SceneNodeDataBase::Owned(base) => {
                rewrite_project_res_data(base, project_root)
            }
        }
    }
}

pub fn rewrite_project_res_value(value: &mut SceneValue, project_root: &str) {
    match value {
        SceneValue::Str(path) if path.starts_with("res://") => {
            *path = Cow::Owned(res_to_abs(project_root, path));
        }
        SceneValue::Object(fields) => {
            for (_, value) in fields.to_mut().iter_mut() {
                rewrite_project_res_value(value, project_root);
            }
        }
        SceneValue::Array(values) => {
            for value in values.to_mut().iter_mut() {
                rewrite_project_res_value(value, project_root);
            }
        }
        _ => {}
    }
}

pub fn suffix_index(name: &str, prefix: &str) -> Option<usize> {
    name.strip_prefix(prefix)?.parse::<usize>().ok()
}

pub fn middle_index(name: &str, prefix: &str, suffix: &str) -> Option<usize> {
    name.strip_prefix(prefix)?
        .strip_suffix(suffix)?
        .parse::<usize>()
        .ok()
}

pub fn set_log<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, text: &str) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.log = text.to_string();
    });
    set_label(ctx, "log_text", text);
}

pub fn set_label<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    text: &str,
) {
    if let Some(id) = find_named(ctx, name) {
        let text = text.to_string();
        let _ = with_node_mut!(ctx.run, UiLabel, id, |node| {
            node.set_text(text);
        });
    }
}

pub fn set_text_box<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    text: &str,
) {
    let focused = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.focused_inspector_box == name
    });
    if focused {
        return;
    }
    if let Some(id) = find_named(ctx, name) {
        let text = text.to_string();
        let _ = with_node_mut!(ctx.run, UiTextBox, id, |node| {
            if node.text.as_ref() != text {
                node.set_text(text);
            }
        });
    }
}

pub fn read_text_box<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
) -> Option<String> {
    let id = find_named(ctx, name)?;
    Some(with_node!(ctx.run, UiTextBox, id, |node| node
        .text
        .to_string()))
}

pub fn set_button_fill<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    fill: &str,
) {
    let Some(color) = Color::from_hex(fill) else {
        return;
    };
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiButton, id, |node| {
            node.style.fill = color;
        });
    }
}

pub fn set_image_tint<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    tint: &str,
) {
    let Some(color) = Color::from_hex(tint) else {
        return;
    };
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiImage, id, |node| {
            node.tint = color;
        });
    }
}

pub fn set_label_size_ratio<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    size: (f32, f32),
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiLabel, id, |node| {
            node.layout.size = UiVector2::ratio(size.0, size.1);
        });
    }
}

pub fn set_label_text_ratio<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    ratio: f32,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiLabel, id, |node| {
            node.text_size_ratio = ratio;
        });
    }
}

pub fn set_text_box_text_ratio<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    ratio: f32,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiTextBox, id, |node| {
            node.text_size_ratio = ratio;
        });
    }
}

pub fn set_text_box_padding<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    x: f32,
    y: f32,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiTextBox, id, |node| {
            node.padding = UiRect::symmetric(x, y);
        });
    }
}

pub fn set_ui_box_padding<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    padding: UiRect,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_base_node_mut!(ctx.run, UiBox, id, |node| {
            node.layout.padding = padding;
        });
    }
}

pub fn set_ui_box_z_index<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    z_index: i32,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_base_node_mut!(ctx.run, UiBox, id, |node| {
            node.layout.z_index = z_index;
        });
    }
}

pub fn set_hlayout_spacing<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    spacing: f32,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiHLayout, id, |node| {
            node.inner.spacing = spacing;
        });
    }
}

pub fn set_button_row_style<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    hover: &str,
    pressed: &str,
) {
    let Some(hover_color) = Color::from_hex(hover) else {
        return;
    };
    let Some(pressed_color) = Color::from_hex(pressed) else {
        return;
    };
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiButton, id, |node| {
            node.style.fill = hover_color;
            node.style.stroke = hover_color;
            node.style.stroke_width = 0.0;
            node.style.corner_radius = 0.0;
            node.hover_style.fill = hover_color;
            node.hover_style.stroke = hover_color;
            node.hover_style.stroke_width = 0.0;
            node.hover_style.corner_radius = 0.0;
            node.pressed_style.fill = pressed_color;
            node.pressed_style.stroke = pressed_color;
            node.pressed_style.stroke_width = 0.0;
            node.pressed_style.corner_radius = 0.0;
        });
    }
}

pub fn apply_viewport_mode<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, mode: &str) {
    let window_aspect = viewport_window_aspect(ctx);
    let stream_size = viewport_stream_size_ratio(window_aspect);
    set_grid_visible(ctx, "viewport_grid", false);
    set_ui_center_size(ctx, "viewport_stream_2d", stream_size);
    set_ui_center_size(ctx, "viewport_stream_3d", stream_size);
    set_ui_center_size(ctx, "viewport_click_layer", stream_size);
    set_camera_stream_visible(ctx, "viewport_stream_2d", mode == "2D");
    set_camera_stream_visible(ctx, "viewport_stream_3d", mode == "3D");
    set_panel_display(ctx, "viewport_canvas_overlay", mode == "UI" || mode == "2D");
    set_panel_display(ctx, "canvas_origin_x", mode == "2D");
    set_panel_display(ctx, "canvas_origin_y", mode == "2D");
    apply_viewport_canvas(ctx);
}

pub fn apply_editor_gizmos<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    gizmo: &editor_gizmos::GizmoView,
    mode: &str,
) {
    let show_2d = mode == "2D";
    let show_3d = mode == "3D";
    set_panel_visible(ctx, "selected_outline", gizmo.selected && !show_3d);
    set_panel_visible(ctx, "camera2d_gizmo", gizmo.camera_2d && show_2d);
    set_panel_visible(ctx, "camera3d_gizmo", false);
    if gizmo.selected && !show_3d {
        set_panel_size(ctx, "selected_outline", gizmo.outline_size);
    }
}

pub fn apply_selected_ui_overlay<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    rect: Option<EditorUiRect>,
) {
    let Some(rect) = rect else {
        set_resize_handles_visible(ctx, false);
        return;
    };
    set_resize_handles_visible(ctx, true);
    let window_aspect = viewport_window_aspect(ctx);
    let (canvas_w, canvas_h) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.viewport_mode == "UI" {
            let zoom = state.ui_canvas_zoom.max(0.25);
            ui_canvas_size_ratio(window_aspect, zoom)
        } else {
            viewport_stream_size_ratio(window_aspect)
        }
    });
    if let Some(id) = find_named(ctx, "selected_outline") {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.layout.anchor = UiAnchor::Center;
            node.layout.size = UiVector2::ratio(rect.size.x * canvas_w, rect.size.y * canvas_h);
            node.transform.position = UiVector2::percent(50.0, 50.0);
            node.transform.pivot = UiVector2::percent(50.0, 50.0);
            node.transform.translation = Vector2::new(
                (rect.center.x - 0.5) * canvas_w,
                (rect.center.y - 0.5) * canvas_h,
            );
            node.transform.self_translation = Vector2::ZERO;
            node.transform.scale = Vector2::ONE;
            node.transform.rotation = rect.rotation;
        });
    }
}

pub fn set_resize_handles_visible<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    visible: bool,
) {
    for name in [
        "resize_nw",
        "resize_n",
        "resize_ne",
        "resize_w",
        "resize_e",
        "resize_sw",
        "resize_s",
        "resize_se",
    ] {
        set_panel_visible(ctx, name, visible);
    }
}

pub fn set_panel_visible<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    visible: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

pub fn set_panel_display<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    visible: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.visible = visible;
            node.input_enabled = false;
        });
    }
}

pub fn set_ui_display<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    visible: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_base_node_mut!(ctx.run, UiBox, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

pub fn set_ui_box_size<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    size: (f32, f32),
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_base_node_mut!(ctx.run, UiBox, id, |node| {
            node.layout.size = UiVector2::ratio(size.0, size.1);
        });
    }
}

pub fn set_ui_center_size<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    size: (f32, f32),
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_base_node_mut!(ctx.run, UiBox, id, |node| {
            node.layout.anchor = UiAnchor::Center;
            node.layout.size = UiVector2::ratio(size.0, size.1);
            node.transform.position = UiVector2::percent(50.0, 50.0);
            node.transform.pivot = UiVector2::percent(50.0, 50.0);
            node.transform.translation = Vector2::ZERO;
            node.transform.self_translation = Vector2::ZERO;
            node.transform.scale = Vector2::ONE;
        });
    }
}

pub fn set_grid_visible<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    visible: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiGrid, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

pub fn set_camera_stream_visible<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    visible: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiCameraStream, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

pub fn set_viewport_stream_camera<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    camera: NodeID,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiCameraStream, id, |node| {
            node.stream.camera = camera;
        });
    }
}

pub fn set_panel_size<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    size: (f32, f32),
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.layout.size = UiVector2::ratio(size.0, size.1);
        });
    }
}

pub fn apply_viewport_canvas<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let (mode, pan_x, pan_y, zoom) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.viewport_mode == "2D" {
            let zoom = state.cam2_zoom.max(0.05);
            (
                state.viewport_mode.clone(),
                -state.cam2_x * zoom / 960.0,
                state.cam2_y * zoom / 540.0,
                zoom,
            )
        } else {
            (
                state.viewport_mode.clone(),
                0.0,
                0.0,
                state.ui_canvas_zoom.max(0.25),
            )
        }
    });
    let show = mode == "UI" || mode == "2D";
    set_panel_display(ctx, "viewport_canvas_overlay", show);
    if !show {
        return;
    }

    let window_aspect = viewport_window_aspect(ctx);
    let (canvas_w, canvas_h) = if mode == "UI" {
        ui_canvas_size_ratio(window_aspect, zoom)
    } else {
        viewport_stream_size_ratio(window_aspect)
    };
    set_ui_center_size(ctx, "viewport_canvas_overlay", (canvas_w, canvas_h));
    apply_ui_preview_canvas_transform(ctx, &mode, zoom);

    let spacing = if mode == "UI" {
        0.25
    } else {
        (0.125 * zoom).clamp(0.03, 0.4)
    };
    for i in 0..9 {
        let offset = (i as f32 - 4.0) * spacing;
        set_canvas_line(
            ctx,
            &format!("canvas_v_{i}"),
            true,
            wrap_grid_offset(offset + pan_x, spacing),
            false,
        );
        set_canvas_line(
            ctx,
            &format!("canvas_h_{i}"),
            false,
            wrap_grid_offset(offset + pan_y, spacing),
            false,
        );
    }
    set_panel_display(ctx, "canvas_origin_x", mode == "2D");
    set_panel_display(ctx, "canvas_origin_y", mode == "2D");
    if mode == "2D" {
        set_canvas_line(ctx, "canvas_origin_x", false, pan_y, true);
        set_canvas_line(ctx, "canvas_origin_y", true, pan_x, true);
    }
}

pub fn apply_ui_preview_canvas_transform<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    mode: &str,
    zoom: f32,
) {
    if mode != "UI" {
        return;
    }
    let root = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.preview_root != 0).then(|| NodeID::from_u64(state.preview_root))
    });
    let Some(root) = root else {
        return;
    };
    let window_aspect = viewport_window_aspect(ctx);
    let canvas_size = ui_canvas_size_ratio(window_aspect, 1.0);
    let _ = with_base_node_mut!(ctx.run, UiBox, root, |node| {
        node.layout.anchor = UiAnchor::Center;
        node.layout.size = UiVector2::ratio(canvas_size.0, canvas_size.1);
        node.transform.position = UiVector2::percent(50.0, 50.0);
        node.transform.pivot = UiVector2::percent(50.0, 50.0);
        node.transform.translation = Vector2::ZERO;
        node.transform.self_translation = Vector2::ZERO;
        node.transform.scale = Vector2::new(zoom, zoom);
    });
}

pub fn viewport_window_aspect<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> f32 {
    let viewport = ctx.res.viewport_size();
    viewport.x / viewport.y.max(0.0001)
}

pub fn viewport_stream_size_ratio(window_aspect: f32) -> (f32, f32) {
    const MAIN_PADDING: f32 = 0.0025;
    const MAIN_SPACING: f32 = 0.0025;
    const SPLIT_CONTENT_W: f32 = 1.0 - (MAIN_PADDING * 2.0) - (MAIN_SPACING * 3.0);
    const SPLIT_CONTENT_H: f32 = 0.944 - (0.003 * 2.0);
    const CENTER_W: f32 = 0.596;
    const VIEWPORT_PANEL_H: f32 = 0.828;
    const MAX_W: f32 = 0.98;
    const MAX_H: f32 = 0.92;
    const ASPECT: f32 = 16.0 / 9.0;

    let panel_aspect =
        window_aspect * (SPLIT_CONTENT_W * CENTER_W) / (SPLIT_CONTENT_H * VIEWPORT_PANEL_H);
    let h_for_w = MAX_W * panel_aspect / ASPECT;
    if h_for_w <= MAX_H {
        (MAX_W, h_for_w)
    } else {
        (MAX_H * ASPECT / panel_aspect, MAX_H)
    }
}

pub fn ui_canvas_size_ratio(window_aspect: f32, zoom: f32) -> (f32, f32) {
    const MAIN_PADDING: f32 = 0.0025;
    const MAIN_SPACING: f32 = 0.0025;
    const SPLIT_CONTENT_W: f32 = 1.0 - (MAIN_PADDING * 2.0) - (MAIN_SPACING * 3.0);
    const SPLIT_CONTENT_H: f32 = 0.944 - (0.003 * 2.0);
    const CENTER_W: f32 = 0.596;
    const VIEWPORT_PANEL_H: f32 = 0.828;
    const BASE_W: f32 = 0.98;
    const ASPECT: f32 = 16.0 / 9.0;

    let panel_aspect =
        window_aspect * (SPLIT_CONTENT_W * CENTER_W) / (SPLIT_CONTENT_H * VIEWPORT_PANEL_H);
    let w = BASE_W * zoom.max(0.25);
    (w, w * panel_aspect / ASPECT)
}

pub fn wrap_grid_offset(offset: f32, spacing: f32) -> f32 {
    if spacing <= 0.0 {
        return offset;
    }
    let half = spacing * 4.0;
    let width = spacing * 9.0;
    (offset + half).rem_euclid(width) - half
}

pub fn set_canvas_line<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    vertical: bool,
    offset: f32,
    origin: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.visible = offset.abs() <= 0.55 || origin;
            node.input_enabled = false;
            node.layout.size = if vertical {
                UiVector2::ratio(if origin { 0.003 } else { 0.0015 }, 1.0)
            } else {
                UiVector2::ratio(1.0, if origin { 0.003 } else { 0.0015 })
            };
            node.transform.translation = if vertical {
                Vector2::new(offset, 0.0)
            } else {
                Vector2::new(0.0, offset)
            };
        });
    }
}

pub fn apply_scene_list_layout<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(list_id) = find_named(ctx, "scene_rows") else {
        return;
    };
    let _ = with_node_mut!(ctx.run, UiList, list_id, |list| {
        list.indent = 12.0;
        list.v_spacing = 0.0008;
    });
    let _ = with_node_mut!(ctx.run, UiVLayout, list_id, |list| {
        list.inner.spacing = 0.0008;
    });
    for idx in 0..MAX_NODES {
        let row_name = format!("scene_row_{idx}");
        let row_label = format!("scene_row_{idx}_label");
        set_button_size(ctx, &row_name, (1.0, 0.053));
        set_button_row_style(ctx, &row_name, "#00000000", "#333842");
        set_label_size_ratio(ctx, &row_label, (1.0, 1.0));
        set_label_text_ratio(ctx, &row_label, 0.35);
    }
}

pub fn apply_file_tree_layout<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(list_id) = find_named(ctx, "file_rows") else {
        return;
    };
    let _ = with_node_mut!(ctx.run, UiList, list_id, |list| {
        list.indent = 11.0;
        list.v_spacing = 0.0008;
    });
    let _ = with_node_mut!(ctx.run, UiVLayout, list_id, |list| {
        list.inner.spacing = 0.0008;
    });
    for idx in 0..MAX_FILES {
        let row_name = format!("file_row_{idx}");
        let row_label = format!("file_row_{idx}_label");
        set_button_size(ctx, &row_name, (1.0, 0.052));
        set_button_row_style(ctx, &row_name, "#00000000", "#333842");
        set_label_size_ratio(ctx, &row_label, (1.0, 1.0));
        set_label_text_ratio(ctx, &row_label, 0.34);
    }
}

pub fn set_button_size<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    size: (f32, f32),
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiButton, id, |node| {
            node.layout.size = UiVector2::ratio(size.0, size.1);
        });
    }
}

pub fn set_checkbox_checked<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    checked: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiCheckbox, id, |node| {
            node.checked = checked;
        });
    }
}

pub fn set_add_node_popup<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    visible: bool,
) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.add_node_popup_open = visible;
    });
    if let Some(id) = find_named(ctx, "add_node_popup") {
        let _ = with_node_mut!(ctx.run, UiVLayout, id, |node| {
            node.layout.z_index = 200;
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

pub fn set_inspector_picker<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    visible: bool,
) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.inspector_picker_open = visible;
        if !visible {
            state.inspector_picker_field.clear();
            state.inspector_picker_kind.clear();
            state.inspector_picker_offset = 0;
            state.inspector_picker_filter.clear();
        }
    });
    let mut has_picker_popup = false;
    if let Some(id) = find_named(ctx, "inspector_pick_popup") {
        has_picker_popup = true;
        let _ = with_node_mut!(ctx.run, UiVLayout, id, |node| {
            node.layout.z_index = 200;
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
    if let Some(id) = find_named(ctx, "add_node_popup") {
        let _ = with_node_mut!(ctx.run, UiVLayout, id, |node| {
            node.layout.z_index = 200;
            node.visible = visible && !has_picker_popup;
            node.input_enabled = visible && !has_picker_popup;
        });
    } else if visible && !has_picker_popup {
        set_log(ctx, "picker fail\nmissing popup");
    }
}

pub fn set_project_manager<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    visible: bool,
) {
    if let Some(id) = find_named(ctx, "project_manager") {
        let _ = with_node_mut!(ctx.run, UiVLayout, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

pub fn find_named<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
) -> Option<NodeID> {
    let mut stack = vec![ctx.id];
    while let Some(id) = stack.pop() {
        if get_node_name!(ctx.run, id).as_deref() == Some(name) {
            return Some(id);
        }
        stack.extend(get_children!(ctx.run, id));
    }
    None
}

pub fn save_recent_projects(recent: &[String]) {
    let list = recent
        .iter()
        .map(|item| format!("\"{}\"", json_escape(item)))
        .collect::<Vec<_>>()
        .join(",");
    let text = format!("{{\"recent\":[{list}]}}");
    let _ = FileMod::save_string(RECENT_PROJECTS_PATH, &text);
}

pub fn load_recent_projects() -> Vec<String> {
    let text = FileMod::load_string(RECENT_PROJECTS_PATH).unwrap_or_default();
    let mut out = Vec::new();
    for path in parse_recent_projects(&text) {
        if !out.iter().any(|item| item == &path) && validate_project_root(Path::new(&path)).is_ok()
        {
            out.push(path);
        }
    }
    out.truncate(MAX_RECENT);
    save_recent_projects(&out);
    out
}

pub fn parse_recent_projects(text: &str) -> Vec<String> {
    let Some(start) = text.find('[') else {
        return Vec::new();
    };
    let Some(end) = text[start..].find(']') else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut item = String::new();
    let mut in_string = false;
    let mut escape = false;
    for ch in text[start + 1..start + end].chars() {
        if escape {
            item.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape = true;
            continue;
        }
        if ch == '"' {
            if in_string {
                if !item.is_empty() {
                    out.push(item.clone());
                }
                item.clear();
            }
            in_string = !in_string;
            continue;
        }
        if in_string {
            item.push(ch);
        }
    }
    out
}

pub fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

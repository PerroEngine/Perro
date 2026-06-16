use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_app_editor_manager_rs as editor_manager;
use crate::scripts_app_editor_project_rs as editor_project;
use crate::scripts_assets_editor_assets_rs::*;
use crate::scripts_assets_editor_file_watch_rs as editor_file_watch;
use crate::scripts_assets_editor_files_rs as editor_files;
use crate::scripts_editor_main_rs::{
    EditorState, FILE_WATCH_INTERVAL_FRAMES, MAX_FILES, MAX_INSPECTOR_PICKER_ROWS,
    MAX_NODE_PICKER_ROWS, MAX_NODES, MAX_RECENT, MAX_TABS, RECENT_PROJECTS_PATH, cached_scene_doc,
    cached_scene_node,
};
use crate::scripts_scene_editor_animation_rs::*;
use crate::scripts_scene_editor_gizmos_rs as editor_gizmos;
use crate::scripts_scene_editor_nav_rs::*;
use crate::scripts_scene_editor_nodes_rs::*;
use crate::scripts_scene_editor_scene_deps_rs as editor_scene_deps;
use crate::scripts_scene_editor_scene_rs as editor_scene;
use crate::scripts_scene_editor_viewport_rs::*;
use crate::scripts_ui_bitmask_rs::{ensure_inspector_bitmask_grid, update_inspector_bitmask_grid};
use crate::scripts_ui_editor_inspector_values_rs::*;
use crate::scripts_ui_editor_view_rs as editor_view;
use crate::scripts_ui_inspector_value_row_rs::{
    apply_inspector_value_row_panel, ensure_inspector_value_row, hide_inspector_value_rows_from,
    inspector_value_row_inner, place_inspector_value_row,
};
use perro_api::prelude::*;
use perro_api::scene::{
    SceneDoc, SceneFieldName, SceneKey, SceneNodeData, SceneNodeEntry, SceneValue, SceneValueKey,
};
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
struct CachedFilteredFiles {
    key: String,
    paths: Vec<String>,
}

static FILTERED_FILE_CACHE: OnceLock<Mutex<Option<CachedFilteredFiles>>> = OnceLock::new();
static EDITOR_TREE_ICON_CACHE: OnceLock<Mutex<Vec<(String, TextureID)>>> = OnceLock::new();

pub fn refresh_all<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let view = with_state!(ctx.run, EditorState, ctx.id, EditorView::from_state);

    set_label(
        ctx,
        "project_status",
        &format!("{}  {}", view.project_name, view.project_root),
    );
    set_label(ctx, "status_bar", &view.status);
    set_label(ctx, "log_text", &view.log);
    apply_script_reload_popup(ctx, view.script_schema_reloading);
    set_label(ctx, "viewport_label", &view.viewport);
    let glb_mode = view.activity_mode == "glb";
    set_button_fill(
        ctx,
        "activity_scene_button",
        if glb_mode { "#2A2F36" } else { "#4D84D1" },
    );
    set_button_fill(
        ctx,
        "activity_glb_button",
        if glb_mode { "#4D84D1" } else { "#2A2F36" },
    );
    set_image_tint(
        ctx,
        "activity_scene_icon",
        if glb_mode { "#A7AFB9" } else { "#D7DBE0" },
    );
    set_image_tint(
        ctx,
        "activity_glb_icon",
        if glb_mode { "#D7DBE0" } else { "#A7AFB9" },
    );
    set_button_fill(
        ctx,
        "mode_ui_button",
        if view.viewport_mode == "UI" {
            "#4D84D1"
        } else {
            "#2A2F36"
        },
    );
    set_button_fill(
        ctx,
        "mode_2d_button",
        if view.viewport_mode == "2D" {
            "#4D84D1"
        } else {
            "#2A2F36"
        },
    );
    set_button_fill(
        ctx,
        "mode_3d_button",
        if view.viewport_mode == "3D" {
            "#4D84D1"
        } else {
            "#2A2F36"
        },
    );
    set_ui_display(ctx, "bottom_tab_bar", true);
    set_button_fill(
        ctx,
        "bottom_log_button",
        if view.anim_drawer_open {
            "#2A2F36"
        } else {
            "#4D84D1"
        },
    );
    set_button_fill(
        ctx,
        "bottom_anim_button",
        if view.anim_drawer_open {
            "#4D84D1"
        } else {
            "#2A2F36"
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
    set_ui_node_size(ctx, "scene_tools_row", (1.0, 0.032));
    set_ui_node_size(ctx, "scene_rows", (1.0, if glb_mode { 0.0 } else { 0.312 }));
    set_ui_node_size(ctx, "file_action_row", (1.0, 0.034));
    set_ui_node_size(ctx, "file_tools_row", (1.0, 0.032));
    set_ui_node_size(ctx, "file_ops_row", (1.0, 0.032));
    set_ui_node_size(
        ctx,
        "file_rows",
        (1.0, if glb_mode { 0.776 } else { 0.296 }),
    );

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

    apply_file_tree_layout(ctx);
    set_file_tree_list(ctx, &view);

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
                "#4D84D1"
            } else {
                "#2A2F36"
            },
        );
    }

    apply_scene_list_layout(ctx);
    set_scene_tree_list(ctx, &view);
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
    set_ui_node_size(
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
        view.inspector.asset_actions,
    );
    set_ui_display(ctx, "asset_action_row", view.inspector.asset_selected);
    set_ui_display(ctx, "asset_use_button", view.inspector.asset_use_action);
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
    let vars_closed = false;
    let show_transform = false;
    let show_transform_body = false;
    set_label(ctx, "inspector_pos_label", "Transform");
    set_row_state(
        ctx,
        "inspector_pos",
        false,
        inspector_disclosure(transform_closed),
    );
    set_ui_display(ctx, "inspector_pos", show_transform);
    set_text_box(
        ctx,
        "inspector_position_box",
        &view.inspector.pos.join(", "),
    );
    set_ui_display(ctx, "inspector_position_label", show_transform_body);
    set_label(ctx, "inspector_position_header_label", "Position");
    apply_component_row(
        ctx,
        "inspector_position",
        &["x", "y", "z", "w"],
        &view.inspector.pos,
        show_transform_body,
    );
    set_component_row_input_type(ctx, "inspector_position", UiTextInputType::SignedFloat);
    set_label(ctx, "inspector_rotation_header_label", "Rotation");
    set_ui_display(ctx, "inspector_rotation_label", show_transform_body);
    set_ui_display(
        ctx,
        "inspector_rotation_mode_row",
        view.inspector.rotation_mode_buttons && show_transform_body,
    );
    set_button_fill(
        ctx,
        "inspector_rotation_quat_button",
        if view.inspector.rotation_mode == "quat" {
            "#4D84D1"
        } else {
            "#2A2F36"
        },
    );
    set_button_fill(
        ctx,
        "inspector_rotation_euler_button",
        if view.inspector.rotation_mode == "euler" {
            "#4D84D1"
        } else {
            "#2A2F36"
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
        show_transform_body,
    );
    set_component_row_input_type(ctx, "inspector_rotation", UiTextInputType::SignedFloat);
    set_label(ctx, "inspector_scale_header_label", "Scale");
    set_ui_display(ctx, "inspector_scale_label", show_transform_body);
    set_text_box(ctx, "inspector_scale_box", &view.inspector.scale.join(", "));
    apply_component_row(
        ctx,
        "inspector_scale",
        &["x", "y", "z", "w"],
        &view.inspector.scale,
        show_transform_body,
    );
    set_component_row_input_type(ctx, "inspector_scale", UiTextInputType::SignedFloat);
    set_label(ctx, "inspector_vars_label", "Fields");
    set_row_state(
        ctx,
        "inspector_vars",
        false,
        inspector_disclosure(vars_closed),
    );
    set_ui_display(
        ctx,
        "inspector_vars",
        false,
    );
    set_ui_display(ctx, "inspector_vars_box", false);
    set_text_box(ctx, "inspector_vars_box", &view.inspector.vars_text);
    let row_parents = inspector_row_parent_indices(&view.inspector.script_vars);
    let row_base_heights = inspector_row_base_heights(&view.inspector.script_vars);
    let row_subtree_heights =
        inspector_row_subtree_heights(&view.inspector.script_vars, &row_base_heights);
    for idx in 0..view.inspector.script_vars.len() {
        ensure_inspector_value_row(ctx, idx);
        let row = view.inspector.script_vars.get(idx);
        let parent_idx = row_parents.get(idx).copied().flatten();
        place_inspector_value_row(ctx, idx, parent_idx);
        if let Some(row_id) = inspector_value_row_inner(ctx, idx) {
            ensure_inspector_bitmask_grid(ctx, idx, row_id);
        }
        set_ui_display(
            ctx,
            &format!("inspector_var_row_{idx}"),
            view.inspector.node_actions && row.is_some() && !vars_closed,
        );
        set_label(
            ctx,
            &format!("inspector_var_{idx}_name"),
            row.map(inspector_row_display_name)
                .as_deref()
                .unwrap_or("-"),
        );
        let bitmask_row = row.is_some_and(|item| item.kind == "BitMask");
        let component_row =
            row.is_some_and(|item| !item.components.is_empty() || item.kind == "Color");
        let section_row = row.is_some_and(|item| item.source == "section");
        set_label(
            ctx,
            &format!("inspector_var_{idx}_type"),
            row.map(|item| item.kind.as_str()).unwrap_or("-"),
        );
        set_ui_display(
            ctx,
            &format!("inspector_var_{idx}_type"),
            view.inspector.node_actions
                && !bitmask_row
                && !component_row
                && !section_row
                && !vars_closed,
        );
        set_text_box(
            ctx,
            &format!("inspector_var_{idx}_value"),
            row.map(|item| item.value.as_str()).unwrap_or(""),
        );
        set_text_box_input_type(
            ctx,
            &format!("inspector_var_{idx}_value"),
            row.map(inspector_row_input_type)
                .unwrap_or(UiTextInputType::Any),
        );
        for component in 0..4 {
            let box_name = format!("inspector_var_{idx}_{component}_box");
            let component_value = row.and_then(|item| item.components.get(component));
            set_text_box(
                ctx,
                &box_name,
                component_value.map(String::as_str).unwrap_or(""),
            );
            set_text_box_input_type(
                ctx,
                &box_name,
                row.map(inspector_row_component_input_type)
                    .unwrap_or(UiTextInputType::Any),
            );
            set_ui_display(
                ctx,
                &box_name,
                view.inspector.node_actions
                    && row.is_some_and(|item| item.components.get(component).is_some())
                    && row.is_none_or(|item| item.kind != "Color")
                    && !vars_closed,
            );
            set_text_box_interactive(
                ctx,
                &box_name,
                view.inspector.node_actions
                    && row.is_some_and(|item| item.components.get(component).is_some())
                    && row.is_none_or(|item| item.kind != "Color")
                    && !vars_closed,
            );
        }
        let swatch_name = format!("inspector_var_{idx}_color_swatch");
        let color_preview = row.and_then(|item| item.color_preview.as_deref());
        set_ui_display(
            ctx,
            &swatch_name,
            view.inspector.node_actions && color_preview.is_some() && !vars_closed,
        );
        if let Some(color) = color_preview {
            set_color_picker_value(ctx, &swatch_name, color);
        }
        let bool_row = row.is_some_and(|item| item.kind == "Bool");
        let enum_row = row.is_some_and(|item| !item.enum_options.is_empty());
        let dropdown_name = format!("inspector_var_{idx}_dropdown");
        let picker_button_name = format!("inspector_var_{idx}_pick_button");
        let picker_row = row.is_some_and(|item| {
            item.expandable
                || (item.source != "section"
                    && (item.kind.starts_with("Node") || item.kind.starts_with("Asset(")))
        }) && find_named(ctx, &picker_button_name).is_some();
        let quat_row = row.is_some_and(|item| item.kind == "Quat");
        for name in [
            format!("inspector_var_{idx}_quat_button"),
            format!("inspector_var_{idx}_euler_button"),
        ] {
            set_ui_display(ctx, &name, false);
        }
        let quat_mode_name = format!("inspector_var_{idx}_quat_mode");
        set_ui_display(
            ctx,
            &quat_mode_name,
            view.inspector.node_actions && quat_row && !vars_closed,
        );
        if quat_row {
            let options = vec!["Quat".to_string(), "Euler".to_string()];
            let selected = if view.inspector.rotation_mode == "euler" {
                "Euler"
            } else {
                "Quat"
            };
            set_dropdown_options(ctx, &quat_mode_name, &options, selected);
        }
        set_ui_display(
            ctx,
            &format!("inspector_var_{idx}_value"),
            view.inspector.node_actions
                && row.is_some()
                && !section_row
                && !picker_row
                && !enum_row
                && !bool_row
                && !bitmask_row
                && !component_row
                && !vars_closed,
        );
        set_text_box_interactive(
            ctx,
            &format!("inspector_var_{idx}_value"),
            view.inspector.node_actions
                && row.is_some_and(|item| item.editable)
                && !vars_closed,
        );
        set_ui_display(
            ctx,
            &dropdown_name,
            view.inspector.node_actions && enum_row && !vars_closed,
        );
        if let Some(row) = row
            && enum_row
        {
            set_dropdown_options(ctx, &dropdown_name, &row.enum_options, &row.value);
        }
        let checkbox_name = format!("inspector_var_{idx}_check");
        set_ui_display(
            ctx,
            &checkbox_name,
            view.inspector.node_actions && bool_row && !vars_closed,
        );
        set_ui_display(
            ctx,
            &format!("inspector_var_{idx}_bitmask_grid"),
            view.inspector.node_actions && bitmask_row && !vars_closed,
        );
        apply_inspector_row_tree_layout(
            ctx,
            idx,
            parent_idx,
            &row_base_heights,
            &row_subtree_heights,
        );
        if let Some(row) = row
            && bitmask_row
        {
            update_inspector_bitmask_grid(ctx, idx, scene_value_bitmask_from_text(&row.value));
        }
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
        if let Some(row) = row {
            let has_children = row_subtree_heights
                .get(idx)
                .zip(row_base_heights.get(idx))
                .is_some_and(|(total, base)| *total > *base);
            apply_inspector_value_row_panel(ctx, idx, row.depth, &row.source, has_children);
            apply_inspector_value_row_text_layout(ctx, idx, row);
        }
        let add_name = format!("inspector_var_{idx}_add_button");
        let remove_name = format!("inspector_var_{idx}_remove_button");
        set_ui_display(
            ctx,
            &add_name,
            view.inspector.node_actions
                && row.is_some_and(|item| item.source != "section" && item.addable)
                && !vars_closed,
        );
        set_ui_display(
            ctx,
            &remove_name,
            view.inspector.node_actions
                && row.is_some_and(|item| item.source != "section" && item.removable)
                && !vars_closed,
        );
    }
    hide_inspector_value_rows_from(ctx, view.inspector.script_vars.len());
    set_ui_display(ctx, "inspector_pick_popup", view.inspector_picker_open);
    set_label(ctx, "inspector_pick_title", &view.inspector_picker_title);
    set_text_box(
        ctx,
        "inspector_pick_filter_box",
        &view.inspector_picker_filter,
    );
    set_label(
        ctx,
        "inspector_pick_page_label",
        &view.inspector_picker_page,
    );
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

fn set_component_row_input_type<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    prefix: &str,
    input_type: UiTextInputType,
) {
    for idx in 0..4 {
        set_text_box_input_type(ctx, &format!("{prefix}_{idx}_box"), input_type);
    }
}

fn inspector_row_input_type(row: &InspectorValueRow) -> UiTextInputType {
    match row.kind.as_str() {
        "F32" | "Unit" => UiTextInputType::SignedFloat,
        "I32" => UiTextInputType::SignedInteger,
        "U32" | "BitMask" => UiTextInputType::UnsignedInteger,
        _ => UiTextInputType::Any,
    }
}

fn inspector_row_component_input_type(row: &InspectorValueRow) -> UiTextInputType {
    match row.kind.as_str() {
        "Vec2" | "Vec3" | "Vec4" | "Quat" | "F32" | "Unit" | "UnitVector2" | "UnitVector3"
        | "UnitVector4" => {
            UiTextInputType::SignedFloat
        }
        "I32" | "IVec2" | "IVec3" | "IVec4" => UiTextInputType::SignedInteger,
        "U32" | "UVec2" | "UVec3" | "UVec4" | "BitMask" => UiTextInputType::UnsignedInteger,
        _ => UiTextInputType::Any,
    }
}

pub fn refresh_status<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let view = with_state!(ctx.run, EditorState, ctx.id, EditorView::from_state);
    set_label(
        ctx,
        "project_status",
        &format!("{}  {}", view.project_name, view.project_root),
    );
    set_label(ctx, "status_bar", &view.status);
    set_label(ctx, "log_text", &view.log);
    set_label(ctx, "viewport_label", &view.viewport);
    apply_script_reload_popup(ctx, view.script_schema_reloading);
}

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RowIndicator {
    #[default]
    None,
    Collapsed,
    Expanded,
}

pub fn inspector_disclosure(collapsed: bool) -> RowIndicator {
    if collapsed {
        RowIndicator::Collapsed
    } else {
        RowIndicator::Expanded
    }
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
        set_ui_node_z_index(ctx, name, 200);
    }

    set_ui_node_size(ctx, "inspector_panel", (0.20, 1.0));
    set_ui_node_size(ctx, "inspector_content", (1.0, 1.12));
    set_label_text_ratio(ctx, "inspector_title", 0.34);
    set_label_text_ratio(ctx, "inspector_name", 0.30);
    set_label_text_ratio(ctx, "inspector_type", 0.28);
    set_label_text_ratio(ctx, "inspector_parent", 0.27);
    set_label_text_ratio(ctx, "inspector_script_top", 0.27);
    set_label_text_ratio(ctx, "inspector_pos_label", 0.31);
    set_label_text_ratio(ctx, "inspector_position_header_label", 0.30);
    set_label_text_ratio(ctx, "inspector_rotation_header_label", 0.31);
    set_label_text_ratio(ctx, "inspector_scale_header_label", 0.31);
    set_label_text_ratio(ctx, "inspector_vars_label", 0.31);

    for name in [
        "asset_action_row",
        "inspector_position_row",
        "inspector_rotation_mode_row",
        "inspector_rotation_row",
        "inspector_scale_row",
    ] {
        set_ui_node_size(ctx, name, (1.0, 0.024));
    }
    for name in [
        "inspector_position_row",
        "inspector_rotation_row",
        "inspector_scale_row",
    ] {
        set_ui_node_padding(ctx, name, UiRect::new(0.030, 0.0, 0.0, 0.0));
        set_hlayout_spacing(ctx, name, 0.002);
    }

    for name in [
        "inspector_rotation_quat_button",
        "inspector_rotation_euler_button",
    ] {
        set_button_size(ctx, name, (0.42, 0.62));
    }

    for name in [
        "inspector_name_box",
        "inspector_position_box",
        "inspector_rotation_box",
        "inspector_scale_box",
    ] {
        set_ui_node_size(ctx, name, (1.0, 0.024));
        set_text_box_text_ratio(ctx, name, 0.62);
        set_text_box_padding(ctx, name, 4.0, 1.0);
    }

    for prefix in [
        "inspector_position",
        "inspector_rotation",
        "inspector_scale",
    ] {
        for idx in 0..4 {
            let name = format!("{prefix}_{idx}_box");
            set_ui_node_size(ctx, &name, (0.185, 0.72));
            set_text_box_text_ratio(ctx, &name, 0.62);
            set_text_box_padding(ctx, &name, 4.0, 1.0);
        }
    }

    let mut idx = 0;
    while find_named(ctx, &format!("inspector_var_row_{idx}")).is_some() {
        set_ui_node_size(ctx, &format!("inspector_var_row_{idx}"), (0.985, 0.031));
        set_ui_node_size(
            ctx,
            &format!("inspector_var_row_{idx}_header"),
            (1.0, 1.0),
        );
        set_ui_node_size(ctx, &format!("inspector_var_row_{idx}_stack"), (1.0, 1.0));
        set_ui_node_size(ctx, &format!("inspector_var_row_{idx}_inner"), (1.0, 1.0));
        set_ui_node_size(
            ctx,
            &format!("inspector_var_row_{idx}_children"),
            (1.0, 0.0),
        );
        set_ui_node_size(ctx, &format!("inspector_var_{idx}_value"), (0.50, 0.70));
        set_text_box_text_ratio(ctx, &format!("inspector_var_{idx}_value"), 0.62);
        set_text_box_padding(ctx, &format!("inspector_var_{idx}_value"), 5.0, 1.0);
        set_hlayout_spacing(ctx, &format!("inspector_var_row_{idx}_inner"), 0.002);
        set_ui_node_size(ctx, &format!("inspector_var_{idx}_check"), (0.055, 0.50));
        set_button_size(
            ctx,
            &format!("inspector_var_{idx}_pick_button"),
            (0.42, 0.62),
        );
        set_button_size(
            ctx,
            &format!("inspector_var_{idx}_quat_button"),
            (0.075, 0.62),
        );
        set_button_size(
            ctx,
            &format!("inspector_var_{idx}_euler_button"),
            (0.075, 0.62),
        );
        set_label_text_ratio(ctx, &format!("inspector_var_{idx}_name"), 0.35);
        set_label_text_ratio(ctx, &format!("inspector_var_{idx}_type"), 0.28);
        idx += 1;
    }
}

fn apply_inspector_dynamic_layout<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    inspector: &InspectorViewData,
) {
    let asset_button_w = if inspector.glb_asset_actions {
        0.25
    } else if inspector.asset_actions {
        0.50
    } else {
        1.0
    };
    set_button_size(ctx, "asset_use_button", (asset_button_w, 0.62));
    for name in ["asset_glb_anim_button", "asset_glb_mat_button"] {
        set_button_size(ctx, name, (0.25, 0.62));
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
    node_icons: Vec<String>,
    scene_disclosures: Vec<RowIndicator>,
    scene_depths: Vec<usize>,
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
    script_schema_reloading: bool,
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
    transform_fields: Vec<&'static str>,
    script: String,
    vars_text: String,
    script_vars: Vec<InspectorValueRow>,
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
            glb_asset_actions: false,
            pos_label: "Transform".to_string(),
            pos: Vec::new(),
            rotation_label: "Rotation".to_string(),
            rotation: Vec::new(),
            rotation_components: ["x", "y", "z", "w"],
            rotation_mode: "quat".to_string(),
            rotation_mode_buttons: false,
            scale_label: "Scale".to_string(),
            scale: Vec::new(),
            transform_fields: Vec::new(),
            script: "Script  -".to_string(),
            vars_text: String::new(),
            script_vars: Vec::new(),
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
        let transform_fields = scene_node_transform_fields(node.data.node_type);
        let has_position = transform_fields.contains(&"position");
        let has_rotation = transform_fields.contains(&"rotation");
        let has_scale = transform_fields.contains(&"scale");
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
        view.transform_fields = transform_fields;
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
        view.rotation_mode_buttons = is_3d && has_rotation;
        view.scale = scene_value_components(&node.data, "scale");
        view.script = format!("Script  {script}");
        view.collapsed_sections = state.inspector_collapsed_sections.clone();
        let script_fields = inspector_script_var_fields_for_node(state, node);
        view.vars_text = script_vars_edit_text(&script_fields);
        view.script_vars = inspector_display_rows_for_node(state, node);
        view.apply_asset_actions(state);
        view
    }

    fn for_asset(state: &EditorState) -> Self {
        let mut view = Self::default();
        if state.active_asset_path.is_empty() {
            return view;
        }
        if state.active_asset_path.ends_with(".scn") {
            return view;
        }
        let asset = asset_inspector_text(state);
        view.title = "Asset".to_string();
        view.name = "Name".to_string();
        view.name_edit = asset_edit_name(&state.active_asset_path);
        view.kind = asset.kind;
        view.parent = format!("{}\n{}", asset.path, asset.size);
        view.script = format!(
            "State  {}\n{}\n{}",
            asset.state, asset.detail, asset.actions
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
        self.glb_asset_actions = self.asset_actions && is_gltf_path(path);
    }
}

impl EditorView {
    fn from_state(state: &EditorState) -> Self {
        let mut nodes = Vec::new();
        let mut node_icons = Vec::new();
        let mut scene_disclosures = Vec::new();
        let mut scene_depths = Vec::new();
        let mut selected_row = None;
        let mut inspector = InspectorViewData::for_asset(state);
        let mut gizmo = editor_gizmos::GizmoView::default();
        let mut selected_ui_rect = None;
        let mut glb_title = "GLB Viewer".to_string();
        let mut glb_summary = "select .glb asset".to_string();

        if !state.doc_text.is_empty() {
            let doc = cached_scene_doc(&state.doc_text);
            let tree = scene_tree_view(
                &doc,
                state.selected_key,
                &state.scene_filter,
                &state.collapsed_scene_keys,
            );
            gizmo = editor_gizmos::gizmo_view(&doc, state.selected_key);
            selected_ui_rect = state.selected_key.and_then(|key| doc_ui_rect(&doc, key));
            nodes = tree.labels;
            node_icons = tree.icons;
            scene_disclosures = tree.disclosures;
            scene_depths = tree.depths;
            selected_row = tree.selected_row;

            if state.sidebar_mode != "files"
                && let Some(key) = state.selected_key
                && let Some(node) = cached_scene_node(&state.doc_text, key)
            {
                inspector = InspectorViewData::for_node(&doc, &node, state);
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
        let inspector_picker_page = (state.inspector_picker_offset / MAX_INSPECTOR_PICKER_ROWS) + 1;
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
            node_icons,
            scene_disclosures,
            scene_depths,
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
            script_schema_reloading: state.script_schema_reload_frames > 0,
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
        cached_scene_doc(&state.doc_text).scene.nodes.len()
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
        "scene" => String::new(),
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

pub fn inspector_scene_value_fields_for_node(
    node: &SceneNodeEntry,
) -> Vec<(SceneFieldName, SceneValue)> {
    perro_scene::scene_node_fields(node.data.node_type)
        .into_iter()
        .filter(inspector_generic_scene_field)
        .filter_map(|field| {
            let value = scene_field_value(&node.data, field.name)
                .cloned()
                .or(field.default)
                .or_else(|| {
                    perro_scene::default_scene_field_value_by_name(node.data.node_type, field.name)
                })
                .or_else(|| Some(field.ty.default_value()))?;
            Some((
                SceneFieldName::from_name(field.name.to_string()),
                coerce_scene_value_to_kind(value, &node_field_type_label(&field.ty)),
            ))
        })
        .collect()
}

fn scene_node_transform_fields(node_type: perro_scene::NodeType) -> Vec<&'static str> {
    perro_scene::scene_node_fields(node_type)
        .into_iter()
        .filter_map(|field| match field.name {
            "position" | "rotation" | "scale" => Some(field.name),
            _ => None,
        })
        .collect()
}

pub fn inspector_generic_scene_field(field: &perro_scene::SceneNodeField) -> bool {
    let _ = field;
    true
}

pub fn inspector_value_fields_for_node(
    scene_fields: &[(SceneFieldName, SceneValue)],
    script_fields: &[(SceneFieldName, SceneValue)],
) -> Vec<(SceneFieldName, SceneValue)> {
    let mut out = Vec::new();
    if !script_fields.is_empty() {
        out.push((
            SceneFieldName::from_name("script_vars".to_string()),
            SceneValue::Object(Cow::Owned(script_fields.to_vec())),
        ));
    }
    out.extend_from_slice(scene_fields);
    out
}

fn inspector_var_button_label(row: &InspectorValueRow) -> String {
    row.value.clone()
}

fn inspector_row_display_name(row: &InspectorValueRow) -> String {
    let indent = row.name.chars().take_while(|ch| ch.is_whitespace()).count();
    let label = row.name.trim();
    if row.source == "section" {
        return format!("{}{}", " ".repeat(indent), label);
    }
    format!("{}{}", " ".repeat(indent), title_case_label(label))
}

fn title_case_label(label: &str) -> String {
    label
        .replace('_', " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!(
                "{}{}",
                first.to_uppercase().collect::<String>(),
                chars.as_str()
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn inspector_row_parent_indices(rows: &[InspectorValueRow]) -> Vec<Option<usize>> {
    let mut stack: Vec<usize> = Vec::new();
    let mut out = Vec::with_capacity(rows.len());
    for (idx, row) in rows.iter().enumerate() {
        while stack
            .last()
            .is_some_and(|parent| rows[*parent].depth >= row.depth)
        {
            stack.pop();
        }
        out.push(stack.last().copied());
        stack.push(idx);
    }
    out
}

fn inspector_row_base_heights(rows: &[InspectorValueRow]) -> Vec<f32> {
    rows.iter()
        .map(|row| {
            if row.kind == "BitMask" {
                0.100
            } else if row.kind == "Color" {
                0.044
            } else if !row.components.is_empty() {
                0.040
            } else if row.source == "section" {
                0.030
            } else {
                0.027
            }
        })
        .collect()
}

fn apply_inspector_value_row_text_layout<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    row: &InspectorValueRow,
) {
    let name_ratio = if row.source == "section" {
        0.38
    } else if row.kind == "BitMask" {
        0.13
    } else if !row.components.is_empty() || row.kind == "Color" {
        0.24
    } else {
        0.35
    };
    set_label_text_ratio(ctx, &format!("inspector_var_{idx}_name"), name_ratio);
    for component in 0..4 {
        let box_name = format!("inspector_var_{idx}_{component}_box");
        set_ui_node_size(&mut *ctx, &box_name, (0.15, 0.72));
        set_text_box_text_ratio(&mut *ctx, &box_name, 0.50);
        set_text_box_padding(&mut *ctx, &box_name, 3.0, 1.0);
    }
}

fn inspector_row_subtree_heights(rows: &[InspectorValueRow], base_heights: &[f32]) -> Vec<f32> {
    let mut out = base_heights.to_vec();
    for idx in (0..rows.len()).rev() {
        let depth = rows[idx].depth;
        let mut next = idx + 1;
        while next < rows.len() && rows[next].depth > depth {
            if rows[next].depth == depth + 1 {
                out[idx] += out[next];
            }
            next += 1;
        }
    }
    out
}

fn apply_inspector_row_tree_layout<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    parent_idx: Option<usize>,
    base_heights: &[f32],
    subtree_heights: &[f32],
) {
    let base = base_heights.get(idx).copied().unwrap_or(0.031);
    let total = subtree_heights.get(idx).copied().unwrap_or(base).max(base);
    let child_total = (total - base).max(0.0);
    let parent_child_total = parent_idx
        .and_then(|parent| {
            let parent_base = base_heights.get(parent).copied()?;
            let parent_total = subtree_heights.get(parent).copied()?;
            Some((parent_total - parent_base).max(0.0))
        })
        .unwrap_or(1.0)
        .max(0.0001);
    let root_h = if parent_idx.is_some() {
        total / parent_child_total
    } else {
        total
    };
    let row_w = if parent_idx.is_some() { 1.0 } else { 0.985 };
    set_ui_node_size(ctx, &format!("inspector_var_row_{idx}"), (row_w, root_h));
    set_ui_node_size(ctx, &format!("inspector_var_row_{idx}_stack"), (1.0, 1.0));
    set_ui_node_size(
        ctx,
        &format!("inspector_var_row_{idx}_inner"),
        (1.0, base / total),
    );
    set_ui_node_size(
        ctx,
        &format!("inspector_var_row_{idx}_header"),
        (1.0, 1.0),
    );
    set_ui_node_size(
        ctx,
        &format!("inspector_var_row_{idx}_children"),
        (1.0, child_total / total),
    );
    set_ui_display(
        ctx,
        &format!("inspector_var_row_{idx}_children"),
        child_total > 0.0,
    );
}

#[derive(Clone)]
pub struct InspectorPickerEntry {
    pub value: String,
    pub label: String,
}

pub fn inspector_picker_entries(state: &EditorState) -> Vec<InspectorPickerEntry> {
    match state.inspector_picker_kind.as_str() {
        "node" | "script_node" | "value_node" => inspector_node_picker_entries(state),
        "script_enum" | "value_enum" => inspector_enum_picker_entries(state),
        "asset" | "value_asset" => inspector_asset_picker_entries(state),
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
        "script_node" | "value_node" => "Pick Node".to_string(),
        "script_enum" | "value_enum" => "Pick Enum".to_string(),
        "value_asset" => "Pick Asset".to_string(),
        "asset" => format!("Pick Asset  {}", state.inspector_picker_field),
        _ => "Pick".to_string(),
    }
}

fn inspector_enum_picker_entries(state: &EditorState) -> Vec<InspectorPickerEntry> {
    let Ok(row_idx) = state.inspector_picker_field.parse::<usize>() else {
        return Vec::new();
    };
    let Some(key) = state.selected_key else {
        return Vec::new();
    };
    let doc = cached_scene_doc(&state.doc_text);
    let Some(node) = cached_scene_node(&state.doc_text, key) else {
        return Vec::new();
    };
    let rows = if state.inspector_picker_kind == "value_enum" {
        inspector_display_rows_for_node(state, &node)
    } else {
        inspector_script_var_rows_for_node(state, &node)
    };
    let Some(row) = rows.get(row_idx).cloned() else {
        return Vec::new();
    };
    let filter = state.inspector_picker_filter.to_ascii_lowercase();
    row.enum_options
        .into_iter()
        .filter(|value| filter.is_empty() || value.to_ascii_lowercase().contains(&filter))
        .map(|value| InspectorPickerEntry {
            label: value.clone(),
            value,
        })
        .collect()
}

fn inspector_node_picker_entries(state: &EditorState) -> Vec<InspectorPickerEntry> {
    if state.doc_text.is_empty() {
        return Vec::new();
    }
    let filter = NodePickerFilter::parse(&state.inspector_picker_filter);
    let doc = cached_scene_doc(&state.doc_text);
    let allowed = inspector_picker_node_ref_types(state);
    doc.scene
        .nodes
        .iter()
        .filter_map(|node| {
            if !node_ref_type_allows(&allowed, node.data.node_type) {
                return None;
            }
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

fn inspector_picker_node_ref_types(state: &EditorState) -> Vec<String> {
    let Ok(row_idx) = state.inspector_picker_field.parse::<usize>() else {
        return Vec::new();
    };
    let Some(key) = state.selected_key else {
        return Vec::new();
    };
    let doc = cached_scene_doc(&state.doc_text);
    let Some(node) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) else {
        return Vec::new();
    };
    let rows = if state.inspector_picker_kind.starts_with("value_") {
        inspector_display_rows_for_node(state, node)
    } else {
        inspector_script_var_rows_for_node(state, node)
    };
    rows.get(row_idx)
        .and_then(|row| node_ref_types_from_kind(&row.kind))
        .unwrap_or_default()
}

fn node_ref_types_from_kind(kind: &str) -> Option<Vec<String>> {
    let inner = kind.strip_prefix("Node(")?.strip_suffix(')')?;
    Some(
        inner
            .split('|')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn node_ref_type_allows(allowed: &[String], node_type: perro_scene::NodeType) -> bool {
    allowed.is_empty()
        || allowed.iter().any(|name| {
            node_type_from_hint_name(name)
                .is_some_and(|allowed_type| node_type.is_a(allowed_type))
        })
}

fn node_type_from_hint_name(name: &str) -> Option<perro_scene::NodeType> {
    use std::str::FromStr;
    perro_scene::NodeType::from_str(name).ok()
}

fn inspector_asset_picker_entries(state: &EditorState) -> Vec<InspectorPickerEntry> {
    let Some(kind) = inspector_picker_asset_kind(state) else {
        return Vec::new();
    };
    let filter = NodePickerFilter::parse(&state.inspector_picker_filter);
    let filters = editor_asset_filters(kind);
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
    if state.inspector_picker_kind == "value_asset" {
        let row_idx = state.inspector_picker_field.parse::<usize>().ok()?;
        let key = state.selected_key?;
        let doc = cached_scene_doc(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        let row = inspector_display_rows_for_node(state, node)
            .get(row_idx)?
            .clone();
        return asset_kind_from_row_kind(&row.kind);
    }
    if state.inspector_picker_field == "script" {
        return Some(perro_scene::SceneAssetKind::Script);
    }
    let key = state.selected_key?;
    let doc = cached_scene_doc(&state.doc_text);
    let node = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?;
    let field = perro_scene::scene_node_field(node.data.node_type, &state.inspector_picker_field)?;
    match field.ty {
        perro_scene::NodeFieldType::Asset(kind) => Some(kind),
        _ => None,
    }
}

fn asset_kind_from_row_kind(kind: &str) -> Option<perro_scene::SceneAssetKind> {
    let name = kind.strip_prefix("Asset(")?.strip_suffix(')')?;
    match name {
        "Scene" => Some(perro_scene::SceneAssetKind::Scene),
        "Script" => Some(perro_scene::SceneAssetKind::Script),
        "Texture" => Some(perro_scene::SceneAssetKind::Texture),
        "Mesh" => Some(perro_scene::SceneAssetKind::Mesh),
        "Model" => Some(perro_scene::SceneAssetKind::Model),
        "Material" => Some(perro_scene::SceneAssetKind::Material),
        "Animation" => Some(perro_scene::SceneAssetKind::Animation),
        "AnimationTree" => Some(perro_scene::SceneAssetKind::AnimationTree),
        "Skeleton" => Some(perro_scene::SceneAssetKind::Skeleton),
        "ParticleProfile" => Some(perro_scene::SceneAssetKind::ParticleProfile),
        "TileSet" => Some(perro_scene::SceneAssetKind::TileSet),
        "UiStyle" => Some(perro_scene::SceneAssetKind::UiStyle),
        _ => None,
    }
}

fn inspector_asset_path_matches(path: &str, filters: &[EditorAssetFilter]) -> bool {
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
    if node_type.is_a(perro_scene::NodeType::UiNode) {
        return format!("Node > UiNode > {name}");
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
        SceneValue::IVec2 { .. } => "IVec2",
        SceneValue::IVec3 { .. } => "IVec3",
        SceneValue::IVec4 { .. } => "IVec4",
        SceneValue::UVec2 { .. } => "UVec2",
        SceneValue::UVec3 { .. } => "UVec3",
        SceneValue::UVec4 { .. } => "UVec4",
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
        SceneValue::IVec2 { x, y } => format!("({x}, {y})"),
        SceneValue::IVec3 { x, y, z } => format!("({x}, {y}, {z})"),
        SceneValue::IVec4 { x, y, z, w } => format!("({x}, {y}, {z}, {w})"),
        SceneValue::UVec2 { x, y } => format!("({x}, {y})"),
        SceneValue::UVec3 { x, y, z } => format!("({x}, {y}, {z})"),
        SceneValue::UVec4 { x, y, z, w } => format!("({x}, {y}, {z}, {w})"),
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
    let value = value.trim();
    let bare = value.trim_start_matches('@');
    if value.is_empty() || matches!(bare, "null" | "none" | "-") {
        "Select Node".to_string()
    } else {
        format!("Node {value}")
    }
}

pub fn format_compact_f32(value: f32) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

pub fn asset_user_text(state: &EditorState, path: &str) -> String {
    if state.doc_text.is_empty() || path.ends_with('/') {
        return "users: -".to_string();
    }
    let doc = cached_scene_doc(&state.doc_text);
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
        let doc = cached_scene_doc(&state.doc_text);
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
    let doc = cached_scene_doc(&state.doc_text);
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

#[allow(clippy::too_many_arguments)]
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
    pub icons: Vec<String>,
    pub disclosures: Vec<RowIndicator>,
    pub keys: Vec<u32>,
    pub depths: Vec<usize>,
    pub selected_row: Option<usize>,
}

pub struct SceneDocIndex {
    node_indices: Vec<(u32, usize)>,
    children: Vec<Vec<u32>>,
    roots: Vec<u32>,
}

impl SceneDocIndex {
    pub fn new(doc: &SceneDoc) -> Self {
        let mut node_indices = Vec::with_capacity(doc.scene.nodes.len());
        for (idx, node) in doc.scene.nodes.iter().enumerate() {
            node_indices.push((node.key.as_u32(), idx));
        }
        node_indices.sort_by_key(|(key, _)| *key);

        let mut children = vec![Vec::new(); doc.scene.nodes.len()];
        let mut roots = Vec::new();
        if let Some(root) = doc.scene.root {
            roots.push(root.as_u32());
        }
        for node in doc.scene.nodes.iter() {
            let key = node.key.as_u32();
            if let Some(parent) = node.parent {
                if let Ok(pos) =
                    node_indices.binary_search_by_key(&parent.as_u32(), |(item_key, _)| *item_key)
                {
                    let parent_idx = node_indices[pos].1;
                    children[parent_idx].push(key);
                }
            } else if !roots.contains(&key) {
                roots.push(key);
            }
        }

        Self {
            node_indices,
            children,
            roots,
        }
    }

    pub fn node<'a>(&self, doc: &'a SceneDoc, key: u32) -> Option<&'a SceneNodeEntry> {
        self.node_indices
            .binary_search_by_key(&key, |(item_key, _)| *item_key)
            .ok()
            .map(|pos| self.node_indices[pos].1)
            .and_then(|idx| doc.scene.nodes.get(idx))
    }

    pub fn child_keys(&self, key: u32) -> &[u32] {
        let Ok(pos) = self
            .node_indices
            .binary_search_by_key(&key, |(item_key, _)| *item_key)
        else {
            return &[];
        };
        let idx = self.node_indices[pos].1;
        self.children.get(idx).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn child_count(&self, key: u32) -> usize {
        self.child_keys(key).len()
    }

    pub fn roots(&self) -> &[u32] {
        &self.roots
    }

    pub fn path(&self, doc: &SceneDoc, key: SceneKey) -> String {
        let mut parts = Vec::new();
        let mut cursor = Some(key);
        let mut guard = 0;
        while let Some(key) = cursor {
            parts.push(doc.scene.key_name_or_id(key).to_string());
            cursor = self.node(doc, key.as_u32()).and_then(|node| node.parent);
            guard += 1;
            if guard > doc.scene.nodes.len() {
                break;
            }
        }
        parts.reverse();
        parts.join("/")
    }
}

pub fn scene_tree_view(
    doc: &SceneDoc,
    selected_key: Option<u32>,
    filter: &str,
    collapsed_keys: &[u32],
) -> SceneTreeRows {
    let filter = NodePickerFilter::parse(filter);
    let index = SceneDocIndex::new(doc);
    if !filter.is_empty() {
        return filtered_scene_tree_view_indexed(doc, &index, selected_key, &filter);
    }
    let mut out = SceneTreeRows::default();
    let mut visited = Vec::new();
    for key in index.roots().iter().copied() {
        push_scene_tree_row(
            doc,
            &index,
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
                &index,
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
    SceneDocIndex::new(doc).path(doc, key)
}

pub fn filtered_scene_tree_view(
    doc: &SceneDoc,
    selected_key: Option<u32>,
    filter: &NodePickerFilter,
) -> SceneTreeRows {
    let index = SceneDocIndex::new(doc);
    filtered_scene_tree_view_indexed(doc, &index, selected_key, filter)
}

fn filtered_scene_tree_view_indexed(
    doc: &SceneDoc,
    index: &SceneDocIndex,
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
        if Some(key) == selected_key {
            out.selected_row = Some(row);
        }
        let path = index.path(doc, node.key);
        let children = index.child_count(key);
        out.labels.push(scene_row_label(
            0,
            &name,
            type_name,
            &scene_node_badges(node),
            children,
            Some(&path),
        ));
        out.icons.push(scene_node_icon(node));
        out.disclosures.push(scene_row_disclosure(children, false));
        out.keys.push(key);
        out.depths.push(0);
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
    SceneDocIndex::new(doc).child_count(key)
}

#[allow(clippy::too_many_arguments)]
pub fn push_scene_tree_row(
    doc: &SceneDoc,
    index: &SceneDocIndex,
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
    let node = index.node(doc, key)?;
    visited.push(key);
    let row = out.labels.len();
    if Some(key) == selected_key {
        out.selected_row = Some(row);
    }
    let children = index.child_count(key);
    let collapsed = collapsed_keys.contains(&key);
    out.labels.push(scene_row_label(
        depth,
        doc.scene.key_name_or_id(node.key).as_ref(),
        node.data.type_name(),
        &scene_node_badges(node),
        children,
        None,
    ));
    out.icons.push(scene_node_icon(node));
    out.disclosures
        .push(scene_row_disclosure(children, collapsed));
    out.keys.push(key);
    out.depths.push(depth);
    if children > 0 && collapsed {
        return Some(row);
    }
    for child_key in index.child_keys(key).iter().copied() {
        let _ = push_scene_tree_row(
            doc,
            index,
            child_key,
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
    depth: usize,
    name: &str,
    type_name: &str,
    badges: &str,
    children: usize,
    parent: Option<&str>,
) -> String {
    let name = editor_view::short_path(name, 20);
    let type_name = editor_view::short_path(type_name, 18);
    let indent = "  ".repeat(depth.min(8));
    if let Some(parent) = parent {
        format!(
            "{indent}     {name}  [{type_name}]{badges}  {}",
            editor_view::short_path(parent, 14)
        )
    } else {
        let child_suffix = if children == 0 {
            String::new()
        } else {
            format!("  {children}")
        };
        format!("{indent}     {name}  [{type_name}]{badges}{child_suffix}")
    }
}

pub fn scene_row_disclosure(children: usize, collapsed: bool) -> RowIndicator {
    if children == 0 {
        RowIndicator::None
    } else if collapsed {
        RowIndicator::Collapsed
    } else {
        RowIndicator::Expanded
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
    let cache_key = filtered_file_cache_key(state);
    let cache = FILTERED_FILE_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(cache) = cache.lock()
        && let Some(cached) = cache.as_ref()
        && cached.key == cache_key
    {
        return cached.paths.clone();
    }

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
    if let Ok(mut cache) = cache.lock() {
        *cache = Some(CachedFilteredFiles {
            key: cache_key,
            paths: paths.clone(),
        });
    }
    paths
}

fn filtered_file_cache_key(state: &EditorState) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}",
        state.activity_mode,
        state.file_filter,
        state.file_scope,
        state.file_expanded_paths.join("\n"),
        state.file_paths.join("\n")
    )
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

pub fn file_icon(path: &str) -> &'static str {
    match editor_files::kind_label(path) {
        "folder" => "[DIR]",
        "scene" => "[SCN]",
        "script" => "[RS]",
        "image" => "[IMG]",
        "audio" => "[AUD]",
        "mesh" => "[MSH]",
        "resource" if path.ends_with(".panim") => "[ANI]",
        "resource" if path.ends_with(".panimtree") => "[ATR]",
        "resource" if path.ends_with(".pmat") => "[MAT]",
        "resource" if path.ends_with(".uistyle") => "[STY]",
        "resource" if path.ends_with(".ppart") => "[PRT]",
        "resource" if path.ends_with(".ptileset") => "[TIL]",
        "resource" if path.ends_with(".pskel2d") || path.ends_with(".pskel3d") => "[SKL]",
        "resource" => "[RES]",
        _ => "[OTH]",
    }
}

pub fn scene_field_value<'a>(data: &'a SceneNodeData, field: &str) -> Option<&'a SceneValue> {
    data.fields
        .iter()
        .find(|(name, _)| name.as_ref() == field)
        .map(|(_, value)| value)
        .or_else(|| {
            data.base_ref()
                .and_then(|base| scene_field_value(base, field))
        })
}

pub fn file_row_state_prefix(
    path: &str,
    open_paths: &[String],
    dirty_scene_paths: &[String],
) -> &'static str {
    let dirty = dirty_scene_paths.iter().any(|dirty| dirty == path);
    if dirty { "* " } else { "" }
}

pub fn file_row_disclosure(path: &str, expanded_paths: &[String]) -> RowIndicator {
    if path.ends_with('/') {
        if expanded_paths.iter().any(|item| item == path) {
            RowIndicator::Expanded
        } else {
            RowIndicator::Collapsed
        }
    } else {
        RowIndicator::None
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
    path.strip_prefix(scope)
        .is_some_and(|rest| !rest.is_empty())
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
    let doc = cached_scene_doc(&state.doc_text);
    let Some(key) = state
        .selected_key
        .or_else(|| doc.scene.root.map(|key| key.as_u32()))
    else {
        return "target: scene root".to_string();
    };
    let Some(node) = cached_scene_node(&state.doc_text, key) else {
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
        _ if node_type.is_a(perro_scene::NodeType::UiNode) => "[UI]",
        _ if node_type.is_a(perro_scene::NodeType::Node2D) => "[2D]",
        _ if node_type.is_a(perro_scene::NodeType::Node3D) => "[3D]",
        _ if node_type.name().ends_with("Resource") => "[RES]",
        _ => "[NOD]",
    }
}

pub fn scene_node_icon(node: &SceneNodeEntry) -> String {
    node_custom_icon_path(node).unwrap_or_else(|| node_type_icon(node.data.node_type).to_string())
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
        SceneValue::IVec2 { x, y } => vec![x.to_string(), y.to_string()],
        SceneValue::IVec3 { x, y, z } => vec![x.to_string(), y.to_string(), z.to_string()],
        SceneValue::IVec4 { x, y, z, w } => {
            vec![x.to_string(), y.to_string(), z.to_string(), w.to_string()]
        }
        SceneValue::UVec2 { x, y } => vec![x.to_string(), y.to_string()],
        SceneValue::UVec3 { x, y, z } => vec![x.to_string(), y.to_string(), z.to_string()],
        SceneValue::UVec4 { x, y, z, w } => {
            vec![x.to_string(), y.to_string(), z.to_string(), w.to_string()]
        }
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

pub fn inspector_var_component_row(name: &str) -> Option<usize> {
    let rest = name.strip_prefix("inspector_var_")?;
    let (row, component) = rest.split_once('_')?;
    let component = component.strip_suffix("_box")?;
    if !matches!(component, "0" | "1" | "2" | "3") {
        return None;
    }
    row.parse::<usize>().ok()
}

pub fn set_log<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, text: &str) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.log = text.to_string();
    });
    set_label(ctx, "log_text", text);
}

pub fn tick_script_schema_reload<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.script_schema_reload_frames == 0 {
            return false;
        }
        state.script_schema_reload_frames = state.script_schema_reload_frames.saturating_sub(1);
        true
    })
    .unwrap_or(false);
    if changed {
        let visible = with_state!(ctx.run, EditorState, ctx.id, |state| {
            state.script_schema_reload_frames > 0
        });
        apply_script_reload_popup(ctx, visible);
    }
}

pub fn apply_script_reload_popup<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    visible: bool,
) {
    ensure_script_reload_popup(ctx);
    set_panel_display(ctx, "script_reload_popup", visible);
    set_ui_display(ctx, "script_reload_popup_label", visible);
}

fn ensure_script_reload_popup<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    if find_named(ctx, "script_reload_popup").is_some() {
        return;
    }
    let panel = ctx.run.Nodes().create::<UiPanel>();
    let label = ctx.run.Nodes().create::<UiLabel>();
    let _ = ctx.run.Nodes().set_node_name(panel, "script_reload_popup");
    let _ = ctx
        .run
        .Nodes()
        .set_node_name(label, "script_reload_popup_label");
    let _ = ctx.run.Nodes().reparent(ctx.id, panel);
    let _ = ctx.run.Nodes().reparent(panel, label);
    let _ = with_node_mut!(ctx.run, UiPanel, panel, |node| {
        node.layout.anchor = UiAnchor::Center;
        node.layout.size = UiVector2::ratio(0.16, 0.052);
        node.layout.z_index = 500;
        node.transform.position = UiVector2::percent(50.0, 50.0);
        node.transform.pivot = UiVector2::percent(50.0, 50.0);
        node.style.fill = Color::from_hex("#23272DF2").unwrap_or(node.style.fill);
        node.style.stroke = Color::from_hex("#6BA0EA").unwrap_or(node.style.stroke);
        node.style.stroke_width = 1.0;
        node.style.corner_radius = 0.06;
        node.visible = false;
        node.input_enabled = false;
    });
    let _ = with_node_mut!(ctx.run, UiLabel, label, |node| {
        node.layout.anchor = UiAnchor::Center;
        node.layout.size = UiVector2::ratio(1.0, 1.0);
        node.transform.position = UiVector2::percent(50.0, 50.0);
        node.transform.pivot = UiVector2::percent(50.0, 50.0);
        node.text = Cow::Borrowed("Reloading scripts");
        node.text_size_ratio = 0.36;
        node.color = Color::from_hex("#D7DBE0").unwrap_or(node.color);
        node.visible = false;
        node.input_enabled = false;
    });
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

pub fn set_text_box_interactive<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    interactive: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiTextBox, id, |node| {
            node.base.input_enabled = interactive;
            node.editable = interactive;
        });
    }
}

pub fn set_text_box_input_type<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    input_type: UiTextInputType,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiTextBox, id, |node| {
            node.input_type = input_type;
        });
    }
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

pub fn set_panel_fill<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    fill: &str,
) {
    let Some(color) = Color::from_hex(fill) else {
        return;
    };
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.style.fill = color;
        });
    }
}

pub fn set_color_picker_value<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    fill: &str,
) {
    let Some(color) = Color::from_hex(fill) else {
        return;
    };
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiColorPicker, id, |node| {
            node.color = color;
        });
    }
}

pub fn read_color_picker_value<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
) -> Option<String> {
    let id = find_named(ctx, name)?;
    let [r, g, b, a] = with_node!(ctx.run, UiColorPicker, id, |node| node.color.to_rgba());
    Some(format!(
        "({}, {}, {}, {})",
        format_compact_f32(r),
        format_compact_f32(g),
        format_compact_f32(b),
        format_compact_f32(a)
    ))
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

pub fn set_ui_node_padding<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    padding: UiRect,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_base_node_mut!(ctx.run, UiNode, id, |node| {
            node.layout.padding = padding;
        });
    }
}

pub fn set_ui_node_z_index<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    z_index: i32,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_base_node_mut!(ctx.run, UiNode, id, |node| {
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

pub fn set_row_state<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    selected: bool,
    indicator: RowIndicator,
) {
    set_button_fill(ctx, name, if selected { "#4D84D1" } else { "#00000000" });
    set_indicator_shape(ctx, &format!("{name}_indicator"), indicator);
}

pub fn set_indicator_shape<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    indicator: RowIndicator,
) {
    let Some(id) = find_named(ctx, name) else {
        return;
    };
    let _ = with_node_mut!(ctx.run, UiShape, id, |node| {
        node.visible = indicator != RowIndicator::None;
        node.transform.rotation = match indicator {
            RowIndicator::Expanded => std::f32::consts::FRAC_PI_2,
            RowIndicator::Collapsed | RowIndicator::None => 0.0,
        };
    });
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
        let _ = with_base_node_mut!(ctx.run, UiNode, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

pub fn set_ui_node_size<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    size: (f32, f32),
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_base_node_mut!(ctx.run, UiNode, id, |node| {
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
        let _ = with_base_node_mut!(ctx.run, UiNode, id, |node| {
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
        (1.0, 1.0)
    };
    set_ui_center_size(ctx, "viewport_canvas_overlay", (canvas_w, canvas_h));
    apply_canvas_overlay_style(ctx, &mode);
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

pub fn apply_canvas_overlay_style<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    mode: &str,
) {
    if let Some(id) = find_named(ctx, "viewport_canvas_overlay") {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.style.fill = Color::from_hex("#00000000").unwrap_or(node.style.fill);
            node.style.stroke = if mode == "UI" {
                Color::from_hex("#D7DBE0EE").unwrap_or(node.style.stroke)
            } else {
                Color::from_hex("#6E7680AA").unwrap_or(node.style.stroke)
            };
            node.style.stroke_width = if mode == "UI" { 2.0 } else { 1.0 };
            node.style.corner_radius = 0.0;
            node.clip_children = false;
        });
    }
    if let Some(id) = find_named(ctx, "viewport_click_layer") {
        let _ = with_node_mut!(ctx.run, UiButton, id, |node| {
            node.style.corner_radius = 0.0;
            node.hover_style.corner_radius = 0.0;
            node.pressed_style.corner_radius = 0.0;
        });
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
    let _ = with_base_node_mut!(ctx.run, UiNode, root, |node| {
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
    let _ = with_node_mut!(ctx.run, UiTreeList, list_id, |tree| {
        tree.indent = 12.0;
        tree.v_spacing = 0.0008;
        tree.row_height = 23.0;
    });
}

pub fn set_scene_tree_list<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    view: &EditorView,
) {
    let Some(list_id) = find_named(ctx, "scene_rows") else {
        return;
    };
    let mut parents = Vec::<usize>::new();
    let mut items = Vec::new();
    for (idx, label) in view.nodes.iter().enumerate() {
        let depth = view.scene_depths.get(idx).copied().unwrap_or(0);
        while parents.len() > depth {
            parents.pop();
        }
        let mut item = UiTreeListItem::new(label.trim_start().to_string())
            .with_id(format!("scene:{idx}"))
            .with_value(variant!(idx as i32))
            .with_icon(editor_tree_icon_texture(
                ctx,
                view.node_icons.get(idx).map(String::as_str).unwrap_or(""),
            ));
        item.parent = parents.last().copied();
        item.open = !matches!(
            view.scene_disclosures
                .get(idx)
                .copied()
                .unwrap_or(RowIndicator::None),
            RowIndicator::Collapsed
        );
        item.selectable = true;
        items.push(item);
        if !matches!(
            view.scene_disclosures
                .get(idx)
                .copied()
                .unwrap_or(RowIndicator::None),
            RowIndicator::None
        ) {
            parents.push(idx);
        }
    }
    let _ = with_node_mut!(ctx.run, UiTreeList, list_id, |tree| {
        if tree.items != items {
            tree.items = items;
        }
        if tree.selected_index != view.selected_row {
            tree.selected_index = view.selected_row;
        }
        tree.indent = 12.0;
        tree.row_height = 23.0;
        tree.v_spacing = 0.0008;
        tree.icon_size = 13.0;
        tree.toggle_size = 10.0;
        tree.line_color = Color::from_hex("#343A43").unwrap_or(tree.line_color);
        tree.triangle_color = Color::from_hex("#5EA868").unwrap_or(tree.triangle_color);
        tree.text_color = Color::from_hex("#D7DBE0").unwrap_or(tree.text_color);
        tree.row_style.fill = Color::TRANSPARENT;
        tree.row_style.stroke = Color::TRANSPARENT;
        tree.row_hover_style.fill =
            Color::from_hex("#323842").unwrap_or(tree.row_hover_style.fill);
        tree.selected_style.fill = Color::from_hex("#4D84D1").unwrap_or(tree.selected_style.fill);
        tree.selected_signals = vec![signal!("editor_scene_tree_selected")];
        tree.toggled_signals = vec![signal!("editor_scene_tree_toggled")];
    });
}

pub fn apply_file_tree_layout<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(list_id) = find_named(ctx, "file_rows") else {
        return;
    };
    let _ = with_node_mut!(ctx.run, UiTreeList, list_id, |tree| {
        tree.indent = 11.0;
        tree.v_spacing = 0.0008;
        tree.row_height = 22.0;
    });
}

pub fn set_file_tree_list<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    view: &EditorView,
) {
    let Some(list_id) = find_named(ctx, "file_rows") else {
        return;
    };
    let mut parents = Vec::<usize>::new();
    let mut items = Vec::new();
    for (idx, path) in view.file_paths.iter().enumerate() {
        let has_root = view.file_paths.first().is_some_and(|root| root == "res://");
        let depth = if path == "res://" || !view.file_filter.is_empty() {
            0
        } else if has_root {
            file_path_depth(path) + 1
        } else {
            file_path_depth(path)
        };
        while parents.len() > depth {
            parents.pop();
        }
        let label = format!(
            "{}{}",
            file_row_state_prefix(path, &view.open_paths, &view.dirty_scene_paths),
            file_row_label_for_filter(path, !view.file_filter.is_empty()),
        );
        let mut item = UiTreeListItem::new(label)
            .with_id(format!("file:{idx}"))
            .with_value(variant!(idx as i32))
            .with_icon(editor_tree_icon_texture(ctx, file_icon(path)));
        item.parent = parents.last().copied();
        item.open = !matches!(
            file_row_disclosure(path, &view.file_expanded_paths),
            RowIndicator::Collapsed
        );
        item.selectable = true;
        items.push(item);
        if path.ends_with('/') {
            parents.push(idx);
        }
    }
    let selected_index = view
        .file_paths
        .iter()
        .position(|path| path == &view.active_asset_path);
    let _ = with_node_mut!(ctx.run, UiTreeList, list_id, |tree| {
        if tree.items != items {
            tree.items = items;
        }
        if tree.selected_index != selected_index {
            tree.selected_index = selected_index;
        }
        tree.indent = 11.0;
        tree.row_height = 22.0;
        tree.v_spacing = 0.0008;
        tree.icon_size = 13.0;
        tree.toggle_size = 10.0;
        tree.line_color = Color::from_hex("#343A43").unwrap_or(tree.line_color);
        tree.triangle_color = Color::from_hex("#5A91DD").unwrap_or(tree.triangle_color);
        tree.text_color = Color::from_hex("#D7DBE0").unwrap_or(tree.text_color);
        tree.row_style.fill = Color::TRANSPARENT;
        tree.row_style.stroke = Color::TRANSPARENT;
        tree.row_hover_style.fill =
            Color::from_hex("#323842").unwrap_or(tree.row_hover_style.fill);
        tree.selected_style.fill = Color::from_hex("#4D84D1").unwrap_or(tree.selected_style.fill);
        tree.selected_signals = vec![signal!("editor_file_tree_selected")];
        tree.toggled_signals = vec![signal!("editor_file_tree_toggled")];
    });
}

fn editor_tree_icon_texture<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    icon: &str,
) -> TextureID {
    let path = if icon.starts_with("res://") {
        icon
    } else {
        match icon {
            "[DIR]" => "res://icons/nodes/node.png",
            "[SCN]" => "res://icons/activity/scene.png",
            "[RS]" => "res://icons/nodes/resource.png",
            "[IMG]" => "res://icons/nodes/resource.png",
            "[AUD]" => "res://icons/nodes/audio.png",
            "[MSH]" => "res://icons/nodes/mesh_3d.png",
            "[ANI]" | "[ATR]" | "[MAT]" | "[STY]" | "[PRT]" | "[TIL]" | "[SKL]" | "[RES]" => {
                "res://icons/nodes/resource.png"
            }
            "Node2D" => "res://icons/nodes/node_2d.png",
            "Node3D" => "res://icons/nodes/node_3d.png",
            "Ui" => "res://icons/nodes/ui_node.png",
            "Sprite2D" => "res://icons/nodes/sprite_2d.png",
            "Mesh3D" => "res://icons/nodes/mesh_3d.png",
            "Camera" => "res://icons/nodes/camera.png",
            "Light" => "res://icons/nodes/light.png",
            "Physics" => "res://icons/nodes/physics.png",
            "Audio" => "res://icons/nodes/audio.png",
            "Resource" => "res://icons/nodes/resource.png",
            _ => "res://icons/nodes/node.png",
        }
    };
    let cache = EDITOR_TREE_ICON_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(cache) = cache.lock()
        && let Some((_, texture)) = cache.iter().find(|(cached_path, _)| cached_path == path)
    {
        return *texture;
    }
    let texture = texture_load!(ctx.res, path);
    if let Ok(mut cache) = cache.lock() {
        if let Some((_, cached_texture)) = cache
            .iter_mut()
            .find(|(cached_path, _)| cached_path == path)
        {
            *cached_texture = texture;
        } else {
            cache.push((path.to_string(), texture));
        }
    }
    texture
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

pub fn read_checkbox_checked<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
) -> Option<bool> {
    let id = find_named(ctx, name)?;
    Some(with_node!(ctx.run, UiCheckbox, id, |node| node.checked))
}

pub fn set_dropdown_options<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    options: &[String],
    selected: &str,
) {
    if let Some(id) = find_named(ctx, name) {
        let selected_index = options
            .iter()
            .position(|item| item == selected)
            .unwrap_or_default();
        let next_options = options
            .iter()
            .map(|item| UiDropdownOption::new(item.clone(), variant!(item.clone())))
            .collect::<Vec<_>>();
        let _ = with_node_mut!(ctx.run, UiDropdown, id, |node| {
            let same_options = node.options.len() == next_options.len()
                && node
                    .options
                    .iter()
                    .zip(next_options.iter())
                    .all(|(a, b)| a.label == b.label && a.value == b.value);
            if !same_options && !node.open {
                node.options = next_options;
            }
            if !node.open && node.selected_index != selected_index {
                node.selected_index = selected_index;
            }
        });
    }
}

pub fn read_dropdown_value<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
) -> Option<String> {
    let id = find_named(ctx, name)?;
    with_node!(ctx.run, UiDropdown, id, |node| {
        node.options
            .get(node.selected_index)
            .and_then(|option| option.value.as_str())
            .map(str::to_string)
    })
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
    let cached = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state
            .editor_name_cache_names
            .iter()
            .position(|cached_name| cached_name == name)
            .and_then(|idx| state.editor_name_cache_ids.get(idx).copied())
    });
    if let Some(raw) = cached {
        let id = NodeID::from_u64(raw);
        if get_node_name!(ctx.run, id).as_deref() == Some(name) {
            return Some(id);
        }
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            if let Some(idx) = state
                .editor_name_cache_ids
                .iter()
                .position(|cached_id| *cached_id == raw)
            {
                state.editor_name_cache_ids.remove(idx);
                if idx < state.editor_name_cache_names.len() {
                    state.editor_name_cache_names.remove(idx);
                }
            }
        });
    }

    let mut stack = vec![ctx.id];
    while let Some(id) = stack.pop() {
        if get_node_name!(ctx.run, id).as_deref() == Some(name) {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                if let Some(idx) = state
                    .editor_name_cache_names
                    .iter_mut()
                    .position(|cached_name| cached_name == name)
                {
                    if let Some(cached_id) = state.editor_name_cache_ids.get_mut(idx) {
                        *cached_id = id.as_u64();
                    }
                } else {
                    state.editor_name_cache_names.push(name.to_string());
                    state.editor_name_cache_ids.push(id.as_u64());
                }
            });
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

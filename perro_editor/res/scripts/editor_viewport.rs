use crate::scripts_editor_animation_rs::*;
use crate::scripts_editor_app_rs as editor_app;
use crate::scripts_editor_assets_rs::*;
use crate::scripts_editor_file_watch_rs as editor_file_watch;
use crate::scripts_editor_files_rs as editor_files;
use crate::scripts_editor_gizmos_rs as editor_gizmos;
use crate::scripts_editor_manager_rs as editor_manager;
use crate::scripts_editor_nav_rs::*;
use crate::scripts_editor_nodes_rs::*;
use crate::scripts_editor_project_rs as editor_project;
use crate::scripts_editor_scene_deps_rs as editor_scene_deps;
use crate::scripts_editor_scene_rs as editor_scene;
use crate::scripts_editor_ui_rs::*;
use crate::scripts_editor_view_rs as editor_view;
use crate::scripts_main_rs::{
    EditorState, FILE_WATCH_INTERVAL_FRAMES, MAX_FILES, MAX_NODE_PICKER_ROWS, MAX_NODES,
    MAX_RECENT, MAX_TABS, RECENT_PROJECTS_PATH,
};
use perro_api::prelude::*;
use perro_api::scene::{
    SceneDoc, SceneFieldName, SceneKey, SceneNodeData, SceneNodeEntry, SceneValue, SceneValueKey,
};
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
pub fn set_mode<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, mode: &str) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode = mode.to_string();
        if mode == "3D" {
            reset_freecam(state);
        } else if mode == "2D" {
            reset_freecam_2d(state);
        }
        state.log = format!("mode {mode}");
    });
    apply_viewport_mode(ctx, mode);
    apply_freecam(ctx);
    apply_freecam_2d(ctx);
    refresh_all(ctx);
}

pub fn zoom_active_viewport<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, dir: i32) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let factor = if dir > 0 { 1.25 } else { 0.8 };
        if state.viewport_mode == "2D" {
            state.cam2_zoom = (state.cam2_zoom * factor).clamp(0.05, 40.0);
            state.log = format!("zoom 2d\n{:.2}", state.cam2_zoom);
        } else if state.viewport_mode == "UI" {
            state.ui_canvas_zoom = (state.ui_canvas_zoom * factor).clamp(0.25, 12.0);
            state.ui_canvas_x = 0.0;
            state.ui_canvas_y = 0.0;
            state.log = format!("zoom ui\n{:.2}", state.ui_canvas_zoom);
        } else {
            state.log = "zoom\nuse 2d/ui viewport".to_string();
        }
    });
    apply_freecam_2d(ctx);
    refresh_all(ctx);
}

pub fn reset_active_viewport_zoom<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.viewport_mode == "2D" {
            state.cam2_zoom = 1.0;
            state.log = "zoom 2d\nreset".to_string();
        } else if state.viewport_mode == "UI" {
            state.ui_canvas_zoom = 1.0;
            state.ui_canvas_x = 0.0;
            state.ui_canvas_y = 0.0;
            state.log = "zoom ui\nreset".to_string();
        } else {
            state.log = "zoom\nuse 2d/ui viewport".to_string();
        }
    });
    apply_freecam_2d(ctx);
    apply_viewport_canvas(ctx);
    refresh_all(ctx);
}

#[derive(Clone, Copy, Debug)]
pub struct ViewportPointer {
    uv: Vector2,
    ndc: Vector2,
}

#[derive(Clone, Copy, Debug)]
pub struct ViewportRay3D {
    origin: Vector3,
    direction: Vector3,
}

pub fn handle_viewport_click<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(pointer) = viewport_pointer(ctx) else {
        return;
    };
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    match mode.as_str() {
        "UI" => {
            let _ = pick_preview_ui(ctx);
            set_log(
                ctx,
                &format!(
                    "ui canvas click\nuv=({:.3}, {:.3}) ndc=({:.3}, {:.3})",
                    pointer.uv.x, pointer.uv.y, pointer.ndc.x, pointer.ndc.y
                ),
            );
        }
        "2D" => {
            if let Some(world) = stream_pointer_world_2d(ctx, pointer) {
                let place = if viewport_shift_down(ctx) {
                    snap_vec2(world, 16.0)
                } else {
                    world
                };
                if viewport_alt_down(ctx) && duplicate_selected_node_at(ctx, Some(place), None) {
                    return;
                }
                if place_selected_2d(ctx, world) {
                    return;
                }
                set_log(
                    ctx,
                    &format!(
                        "2d stream click\nuv=({:.3}, {:.3}) world=({:.2}, {:.2})",
                        pointer.uv.x, pointer.uv.y, world.x, world.y
                    ),
                );
            }
        }
        "3D" => {
            if let Some(ray) = stream_pointer_ray_3d(ctx, pointer) {
                if let Some(point) = ray_ground_point(ray) {
                    let place = if viewport_shift_down(ctx) {
                        snap_vec3(point, 1.0)
                    } else {
                        point
                    };
                    if viewport_alt_down(ctx) && duplicate_selected_node_at(ctx, None, Some(place))
                    {
                        return;
                    }
                    if place_selected_3d(ctx, point) {
                        return;
                    }
                }
                set_log(
                    ctx,
                    &format!(
                        "3d stream click\norigin=({:.2}, {:.2}, {:.2}) dir=({:.3}, {:.3}, {:.3})",
                        ray.origin.x,
                        ray.origin.y,
                        ray.origin.z,
                        ray.direction.x,
                        ray.direction.y,
                        ray.direction.z
                    ),
                );
            }
        }
        _ => {}
    }
}

pub fn viewport_alt_down<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> bool {
    key_down!(ctx.ipt, KeyCode::AltLeft) || key_down!(ctx.ipt, KeyCode::AltRight)
}

pub fn viewport_shift_down<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> bool {
    key_down!(ctx.ipt, KeyCode::ShiftLeft) || key_down!(ctx.ipt, KeyCode::ShiftRight)
}

pub fn duplicate_selected_node_at<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    pos2: Option<Vector2>,
    pos3: Option<Vector3>,
) -> bool {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let subtree_keys = collect_scene_subtree_keys(&doc, key);
        if subtree_keys.is_empty() {
            return false;
        }
        let mut map = Vec::new();
        let mut clones = Vec::new();
        for old_key in subtree_keys.iter().copied() {
            let Some(source) = doc
                .scene
                .nodes
                .iter()
                .find(|node| node.key.as_u32() == old_key)
                .cloned()
            else {
                continue;
            };
            let new_key = doc.scene.key_names.len() as u32;
            let source_name = doc.scene.key_name_or_id(source.key).to_string();
            let new_name = unique_node_name(&doc, &format!("{source_name}_copy"));
            doc.scene.key_names.to_mut().push(Cow::Owned(new_name));
            map.push((old_key, new_key));
            clones.push(source);
        }
        if clones.is_empty() {
            return false;
        }
        for mut node in clones {
            let old_key = node.key.as_u32();
            let Some(new_key) = mapped_scene_key(&map, old_key) else {
                continue;
            };
            node.key = SceneKey::new(new_key);
            if let Some(parent) = node.parent
                && let Some(new_parent) = mapped_scene_key(&map, parent.as_u32())
            {
                node.parent = Some(SceneKey::new(new_parent));
            }
            if old_key == key {
                if let Some(point) = pos2
                    && node.data.node_type.is_a(perro_scene::NodeType::Node2D)
                {
                    set_scene_vec2(&mut node.data, "position", point);
                }
                if let Some(point) = pos3
                    && node.data.node_type.is_a(perro_scene::NodeType::Node3D)
                {
                    set_scene_vec3(&mut node.data, "position", point);
                }
            }
            node.children = Cow::Owned(Vec::new());
            doc.scene.nodes.to_mut().push(node);
        }
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.selected_key = mapped_scene_key(&map, key);
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("alt-place copy\nadd {} node", map.len());
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
    changed
}

pub fn place_selected_2d<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    world: Vector2,
) -> bool {
    let snap = key_down!(ctx.ipt, KeyCode::ShiftLeft) || key_down!(ctx.ipt, KeyCode::ShiftRight);
    let world = if snap { snap_vec2(world, 16.0) } else { world };
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        if !node.data.node_type.is_a(perro_scene::NodeType::Node2D) {
            return false;
        }
        set_scene_vec2(&mut node.data, "position", world);
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = if snap {
            format!("place 2d\npos=({:.2}, {:.2})\nsnap=16", world.x, world.y)
        } else {
            format!("place 2d\npos=({:.2}, {:.2})", world.x, world.y)
        };
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
    changed
}

pub fn place_selected_3d<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    point: Vector3,
) -> bool {
    let snap = key_down!(ctx.ipt, KeyCode::ShiftLeft) || key_down!(ctx.ipt, KeyCode::ShiftRight);
    let point = if snap { snap_vec3(point, 1.0) } else { point };
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        if !node.data.node_type.is_a(perro_scene::NodeType::Node3D) {
            return false;
        }
        set_scene_vec3(&mut node.data, "position", point);
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = if snap {
            format!(
                "place 3d\npos=({:.2}, {:.2}, {:.2})\nsnap=1",
                point.x, point.y, point.z
            )
        } else {
            format!(
                "place 3d\npos=({:.2}, {:.2}, {:.2})",
                point.x, point.y, point.z
            )
        };
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
    changed
}

pub fn snap_vec2(value: Vector2, grid: f32) -> Vector2 {
    Vector2::new(
        (value.x / grid).round() * grid,
        (value.y / grid).round() * grid,
    )
}

pub fn snap_vec3(value: Vector3, grid: f32) -> Vector3 {
    Vector3::new(
        (value.x / grid).round() * grid,
        (value.y / grid).round() * grid,
        (value.z / grid).round() * grid,
    )
}

pub fn snap_f32(value: f32, grid: f32) -> f32 {
    if grid <= 0.0 {
        value
    } else {
        (value / grid).round() * grid
    }
}

pub fn ray_ground_point(ray: ViewportRay3D) -> Option<Vector3> {
    if ray.direction.y.abs() < 0.0001 {
        return None;
    }
    let t = -ray.origin.y / ray.direction.y;
    if !t.is_finite() || t < 0.0 {
        return None;
    }
    Some(ray.origin + ray.direction * t)
}

pub fn viewport_pointer<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
) -> Option<ViewportPointer> {
    let mouse = mouse_position!(ctx.ipt);
    let viewport = ctx.res.viewport_size();
    if viewport.x <= 0.0 || viewport.y <= 0.0 {
        return None;
    }

    let x = mouse.x;
    let y = mouse.y;
    let window_aspect = viewport.x / viewport.y.max(0.0001);
    let rect = viewport_stream_rect_ratio(window_aspect);
    let center_x = rect.0;
    let center_y = rect.1;
    let size_x = rect.2;
    let size_y = rect.3;
    let min_x = center_x - size_x * 0.5;
    let max_x = center_x + size_x * 0.5;
    let min_y = center_y - size_y * 0.5;
    let max_y = center_y + size_y * 0.5;
    if x < min_x || x > max_x || y < min_y || y > max_y {
        return None;
    }
    let uv = Vector2::new((x - min_x) / size_x, (y - min_y) / size_y);
    let mut ndc = Vector2::new(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);

    let (half_w, half_h) = stream_half_ndc(window_aspect);
    if half_w <= 0.0 || half_h <= 0.0 {
        return None;
    }
    if ndc.x.abs() > half_w || ndc.y.abs() > half_h {
        return None;
    }
    ndc.x /= half_w;
    ndc.y /= half_h;

    Some(ViewportPointer {
        uv,
        ndc,
    })
}

pub fn viewport_stream_rect_ratio(window_aspect: f32) -> (f32, f32, f32, f32) {
    const TOP_BAR_H: f32 = 0.034;
    const ROOT_SPACING: f32 = 0.0;
    const MAIN_SPLIT_H: f32 = 0.944;
    const MAIN_PADDING: f32 = 0.004;
    const MAIN_SPACING: f32 = 0.004;
    const ACTIVITY_W: f32 = 0.024;
    const LEFT_W: f32 = 0.132;
    const CENTER_W: f32 = 0.705;
    const VIEWPORT_PANEL_H: f32 = 0.828;
    const SCENE_TABS_H: f32 = 0.042;
    const CENTER_STACK_SPACING: f32 = 0.004;

    let split_content_w = 1.0 - (MAIN_PADDING * 2.0) - (MAIN_SPACING * 3.0);
    let split_content_h = MAIN_SPLIT_H - (MAIN_PADDING * 2.0);
    let activity_w = split_content_w * ACTIVITY_W;
    let left_w = split_content_w * LEFT_W;
    let center_w = split_content_w * CENTER_W;
    let center_h = split_content_h * VIEWPORT_PANEL_H;
    let center_x = MAIN_PADDING
        + activity_w
        + MAIN_SPACING
        + left_w
        + MAIN_SPACING
        + (center_w * 0.5);

    let panel_center_y = TOP_BAR_H
        + ROOT_SPACING
        + MAIN_PADDING
        + SCENE_TABS_H
        + CENTER_STACK_SPACING
        + (center_h * 0.5);
    let (stream_ratio_w, stream_ratio_h) = viewport_stream_size_ratio(window_aspect);
    let size_x = (center_w * stream_ratio_w).clamp(0.0, 0.9999);
    let size_y = (center_h * stream_ratio_h).clamp(0.0, 0.9999);
    (center_x, panel_center_y, size_x, size_y)
}

pub fn stream_half_ndc(window_aspect: f32) -> (f32, f32) {
    let (stream_ratio_w, stream_ratio_h) = viewport_stream_size_ratio(window_aspect);
    let stream_aspect = window_aspect * stream_ratio_w / stream_ratio_h;
    const CAMERA_ASPECT: f32 = 16.0 / 9.0;
    if stream_aspect >= CAMERA_ASPECT {
        (1.0, CAMERA_ASPECT / stream_aspect)
    } else {
        (stream_aspect / CAMERA_ASPECT, 1.0)
    }
}

pub fn stream_pointer_world_2d<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    pointer: ViewportPointer,
) -> Option<Vector2> {
    let camera = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.preview_camera_2d != 0).then(|| NodeID::from_u64(state.preview_camera_2d))
    })
    .or_else(|| find_named(ctx, "editor_camera_2d"))?;
    let global = ctx.run.Nodes().get_global_transform_2d(camera)?;
    let zoom = with_node!(ctx.run, Camera2D, camera, |node| node.zoom).max(0.0001);
    let local = Vector2::new(pointer.ndc.x * 480.0 / zoom, pointer.ndc.y * 270.0 / zoom);
    let sin = global.rotation.sin();
    let cos = global.rotation.cos();
    Some(Vector2::new(
        global.position.x + local.x * cos - local.y * sin,
        global.position.y + local.x * sin + local.y * cos,
    ))
}

pub fn stream_pointer_ray_3d<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    pointer: ViewportPointer,
) -> Option<ViewportRay3D> {
    let camera = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.preview_camera_3d != 0).then(|| NodeID::from_u64(state.preview_camera_3d))
    })
    .or_else(|| find_named(ctx, "editor_camera_3d"))?;
    let global = ctx.run.Nodes().get_global_transform_3d(camera)?;
    let projection = with_node!(ctx.run, Camera3D, camera, |node| node.projection.clone());
    let aspect = 16.0 / 9.0;
    let local_dir = match projection {
        CameraProjection::Perspective { fov_y_degrees, .. } => {
            let tan_y = (fov_y_degrees.to_radians() * 0.5).tan();
            Vector3::new(pointer.ndc.x * aspect * tan_y, pointer.ndc.y * tan_y, -1.0).normalized()
        }
        CameraProjection::Orthographic { .. } => Vector3::new(0.0, 0.0, -1.0),
        CameraProjection::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            ..
        } => {
            let x = left + (pointer.uv.x * (right - left));
            let y = bottom + ((1.0 - pointer.uv.y) * (top - bottom));
            Vector3::new(x, y, -near.max(0.001)).normalized()
        }
    };
    let local_origin = match projection {
        CameraProjection::Orthographic { size, .. } => Vector3::new(
            pointer.ndc.x * size * aspect * 0.5,
            pointer.ndc.y * size * 0.5,
            0.0,
        ),
        _ => Vector3::ZERO,
    };
    let origin_offset = global.rotation.rotate_vector3(local_origin);
    Some(ViewportRay3D {
        origin: global.position + origin_offset,
        direction: global.rotation.rotate_vector3(local_dir).normalized(),
    })
}

pub fn poll_project_diffs<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let action = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.project_root.is_empty() {
            return None;
        }
        state.file_watch_frame = state.file_watch_frame.wrapping_add(1);
        if state.file_watch_frame % FILE_WATCH_INTERVAL_FRAMES != 0 {
            return None;
        }

        let root = PathBuf::from(&state.project_root);
        let next = editor_file_watch::scan_project(root.as_path());
        let changed = editor_file_watch::changed_paths(&state.project_file_sigs, &next);
        if changed.is_empty() {
            state.project_file_sigs = next;
            return None;
        }
        state.project_file_sigs = next;

        let res_changed = changed
            .iter()
            .any(|path| editor_file_watch::is_under_res(&root, path));
        let changed_scenes = changed
            .iter()
            .filter_map(|path| editor_file_watch::abs_scene_to_res(&root, path))
            .collect::<Vec<_>>();
        Some((root, res_changed, changed_scenes))
    })
    .flatten();

    let Some((root, res_changed, changed_scenes)) = action else {
        return;
    };

    if res_changed && let Ok(paths) = scan_res_paths(root.as_path()) {
        let scene_paths = paths
            .iter()
            .filter(|path| path.ends_with(".scn"))
            .cloned()
            .collect::<Vec<_>>();
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.file_paths = paths;
            state.scene_paths = scene_paths;
        });
    }

    if changed_scenes.is_empty() {
        refresh_all(ctx);
        return;
    }

    let reload = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let active = state.open_paths.get(state.active_open).cloned();
        let affects_preview = changed_scenes
            .iter()
            .any(|path| state.preview_scene_paths.iter().any(|item| item == path));
        let affects_open = active
            .as_ref()
            .is_some_and(|path| changed_scenes.iter().any(|item| item == path));

        if (affects_preview || affects_open) && state.dirty {
            for path in changed_scenes.iter() {
                if !state.dirty_scene_paths.iter().any(|item| item == path) {
                    state.dirty_scene_paths.push(path.clone());
                }
            }
            state.log = "external change pending".to_string();
            return None;
        }

        if affects_open {
            return active;
        }
        if affects_preview {
            state.log = format!("reload preview deps\n{}", changed_scenes.join("\n"));
            return Some(String::new());
        }
        state.log = format!("project file change\n{}", changed_scenes.join("\n"));
        None
    })
    .flatten();

    match reload {
        Some(path) if path.is_empty() => {
            rebuild_preview(ctx);
            refresh_all(ctx);
        }
        Some(path) => {
            reload_scene_path(ctx, &path);
        }
        None => refresh_all(ctx),
    }
}

pub fn reload_scene_path<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    scene_path: &str,
) {
    let root = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.project_root.clone()
    });
    let abs = res_to_abs(&root, scene_path);
    let text = match FileMod::load_string(&abs) {
        Ok(text) => text,
        Err(err) => {
            set_log(ctx, &format!("reload scene fail\n{scene_path}\n{err}"));
            return;
        }
    };
    let doc = SceneDoc::parse(&text);
    let first_key = doc.scene.nodes.first().map(|node| node.key.as_u32());
    let mode = editor_scene::root_viewport_mode(&doc);
    let normalized = doc.to_text();
    let same = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.doc_text == normalized
    });
    if same {
        return;
    }
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.doc_text = normalized;
        state.selected_key = first_key;
        state.viewport_mode = mode.to_string();
        if mode == "3D" {
            reset_freecam(state);
        } else if mode == "2D" {
            reset_freecam_2d(state);
        }
        state.dirty = false;
        state.dirty_scene_paths.retain(|path| path != scene_path);
        state.log = format!("reload scene\n{scene_path}");
    });
    rebuild_preview(ctx);
    refresh_all(ctx);
}

pub fn rebuild_preview<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    clear_preview(ctx);
    let (root, active, doc_text, serial) = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.preview_serial = state.preview_serial.wrapping_add(1);
        (
            state.project_root.clone(),
            state.open_paths.get(state.active_open).cloned(),
            state.doc_text.clone(),
            state.preview_serial,
        )
    })
    .unwrap_or_else(|| (String::new(), None, String::new(), 0));
    let Some(active) = active else {
        return;
    };
    if root.is_empty() || doc_text.is_empty() {
        return;
    }

    let deps = editor_scene_deps::collect_scene_deps(Path::new(&root), &active, &doc_text);
    let mut log = None;
    if let Some(err) = deps.error.clone() {
        log = Some(format!("preview deps fail\n{err}"));
    }
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.preview_scene_paths = deps.paths;
        if let Some(log) = log {
            state.log = log;
        }
    });
    load_preview_scene(ctx, &active, &doc_text, serial);
}

pub fn clear_preview<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let root = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let root = state.preview_root;
        state.preview_root = 0;
        state.preview_camera_2d = 0;
        state.preview_camera_3d = 0;
        state.preview_node_ids.clear();
        state.preview_node_keys.clear();
        root
    })
    .unwrap_or(0);
    if root != 0 {
        let _ = ctx.run.Nodes().remove_node(NodeID::from_u64(root));
    }
}

pub fn load_preview_scene<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    path: &str,
    doc_text: &str,
    serial: u64,
) {
    let project_root = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.project_root.clone()
    });
    let preview_text =
        rewrite_project_res_paths(&SceneDoc::parse(doc_text), &project_root).to_text();
    let preview_path = PathBuf::from(&project_root)
        .join(".perro")
        .join(format!("editor_preview_{serial}.scn"));
    if let Err(err) = FileMod::save_string(preview_path.to_string_lossy().as_ref(), &preview_text) {
        set_log(ctx, &format!("preview write fail\n{path}\n{err}"));
        return;
    }

    let root = match ctx
        .run
        .Scene()
        .load(preview_path.to_string_lossy().to_string())
    {
        Ok(root) => root,
        Err(err) => {
            set_log(ctx, &format!("preview load fail\n{path}\n{err}"));
            return;
        }
    };
    attach_preview_to_viewport(ctx, root);
    disable_preview_runtime_input(ctx, root);

    let doc_text = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.doc_text.clone()
    });
    let (node_ids, keys, preview_camera_2d, preview_camera_3d) = if doc_text.is_empty() {
        (Vec::new(), Vec::new(), 0, 0)
    } else {
        let doc = SceneDoc::parse(&doc_text);
        add_preview_env(ctx, root, &doc);
        let preview_camera_2d = if editor_scene::has_2d(&doc) {
            let name = format!("__editor_preview_camera_2d_{serial}");
            let camera = create_node!(ctx.run, Camera2D, name, tags![], root);
            set_viewport_stream_camera(ctx, "viewport_stream_2d", camera);
            Some(camera)
        } else {
            None
        };
        let preview_camera_3d = if editor_scene::has_3d(&doc) {
            let name = format!("__editor_preview_camera_3d_{serial}");
            let camera = create_node!(ctx.run, Camera3D, name, tags![], root);
            set_viewport_stream_camera(ctx, "viewport_stream_3d", camera);
            Some(camera)
        } else {
            None
        };
        if let Some(camera) = preview_camera_2d {
            let _ = with_node_mut!(ctx.run, Camera2D, camera, |node| {
                node.active = false;
                node.zoom = 1.0;
            });
        }
        if let Some(camera) = preview_camera_3d {
            let _ = with_node_mut!(ctx.run, Camera3D, camera, |node| {
                node.active = false;
            });
        }
        let doc_keys = preview_doc_order(&doc);
        let node_ids = preview_runtime_order(ctx, root, doc_keys.len());
        (
            node_ids.into_iter().map(NodeID::as_u64).collect::<Vec<_>>(),
            doc_keys,
            preview_camera_2d.map(NodeID::as_u64).unwrap_or(0),
            preview_camera_3d.map(NodeID::as_u64).unwrap_or(0),
        )
    };

    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.preview_root = root.as_u64();
        state.preview_node_ids = node_ids;
        state.preview_node_keys = keys;
        state.preview_camera_2d = preview_camera_2d;
        state.preview_camera_3d = preview_camera_3d;
    });
    apply_freecam(ctx);
    apply_freecam_2d(ctx);
}

pub fn add_preview_env<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
    doc: &SceneDoc,
) {
    if !editor_scene::has_3d(doc) {
        return;
    }
    if !editor_scene::has_type(doc, perro_scene::NodeType::AmbientLight3D) {
        let light = create_node!(
            ctx.run,
            AmbientLight3D,
            "__editor_preview_ambient",
            tags![],
            root
        );
        let _ = with_node_mut!(ctx.run, AmbientLight3D, light, |node| {
            node.intensity = 0.35;
        });
    }
    if !editor_scene::has_type(doc, perro_scene::NodeType::Sky3D) {
        let _ = create_node!(ctx.run, Sky3D, "__editor_preview_sky", tags![], root);
    }
}

pub fn attach_preview_to_viewport<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
) {
    let Some(panel) = find_named(ctx, "viewport_panel") else {
        return;
    };
    if ctx
        .run
        .Nodes()
        .with_base_node::<UiBox, _, _>(root, |_| ())
        .is_some()
    {
        let _ = ctx.run.Nodes().reparent(panel, root);
        let canvas_size = ui_canvas_size_ratio(viewport_window_aspect(ctx), 1.0);
        let _ = with_base_node_mut!(ctx.run, UiBox, root, |node| {
            node.layout.anchor = UiAnchor::Center;
            node.layout.size = UiVector2::ratio(canvas_size.0, canvas_size.1);
            node.transform.position = UiVector2::percent(50.0, 50.0);
            node.transform.pivot = UiVector2::percent(50.0, 50.0);
            node.transform.translation = Vector2::ZERO;
            node.transform.self_translation = Vector2::ZERO;
            node.transform.scale = Vector2::ONE;
            node.input_enabled = false;
        });
    }
}

pub fn preview_doc_order(doc: &SceneDoc) -> Vec<u32> {
    let mut out = Vec::new();
    if let Some(root) = doc.scene.root {
        push_doc_order(doc, root.as_u32(), &mut out);
    }
    for node in doc.scene.nodes.iter() {
        let key = node.key.as_u32();
        if !out.contains(&key) {
            push_doc_order(doc, key, &mut out);
        }
    }
    out
}

pub fn push_doc_order(doc: &SceneDoc, key: u32, out: &mut Vec<u32>) {
    if out.contains(&key) {
        return;
    }
    out.push(key);
    for child in doc
        .scene
        .nodes
        .iter()
        .filter(|child| child.parent.map(|parent| parent.as_u32()) == Some(key))
    {
        push_doc_order(doc, child.key.as_u32(), out);
    }
}

pub fn preview_runtime_order<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
    limit: usize,
) -> Vec<NodeID> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        out.push(id);
        if out.len() >= limit {
            break;
        }
        let mut children = ctx.run.Nodes().get_children(id);
        children.reverse();
        stack.extend(children);
    }
    out
}

pub fn disable_preview_runtime_input<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let _ = with_node_mut!(ctx.run, UiButton, id, |node| {
            node.input_enabled = false;
            node.disabled = true;
        });
        let _ = with_node_mut!(ctx.run, UiImageButton, id, |node| {
            node.input_enabled = false;
            node.disabled = true;
        });
        let _ = with_node_mut!(ctx.run, UiTextBox, id, |node| {
            node.base.input_enabled = false;
        });
        let _ = with_node_mut!(ctx.run, UiTextBlock, id, |node| {
            node.base.input_enabled = false;
        });
        let _ = with_node_mut!(ctx.run, UiScrollContainer, id, |node| {
            node.input_enabled = false;
        });
        let _ = with_node_mut!(ctx.run, Button2D, id, |node| {
            node.input_enabled = false;
            node.disabled = true;
        });
        let _ = with_node_mut!(ctx.run, ImageButton2D, id, |node| {
            node.input_enabled = false;
            node.disabled = true;
        });
        let _ = with_node_mut!(ctx.run, Camera2D, id, |node| {
            node.active = false;
        });
        let _ = with_node_mut!(ctx.run, Camera3D, id, |node| {
            node.active = false;
        });

        stack.extend(ctx.run.Nodes().get_children(id));
    }
}

pub fn update_preview_pick<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    if !mouse_pressed!(ctx.ipt, MouseButton::Left) {
        return;
    }
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    if mode != "UI" {
        return;
    }
    let pointer = viewport_pointer(ctx);
    if let Some((handle, pointer)) =
        pointer.and_then(|pointer| pick_resize_handle(ctx, pointer).map(|handle| (handle, pointer)))
    {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            if let Some(key) = state.selected_key {
                state.ui_drag_key = Some(key);
                state.ui_drag_mode = handle.to_string();
                state.ui_drag_last_x = pointer.uv.x;
                state.ui_drag_last_y = pointer.uv.y;
                state.log = format!("resize node\n{handle}");
            }
        });
        refresh_all(ctx);
        return;
    }
    if let Some(pointer) = pointer
        && pick_rotation_zone(ctx, pointer).is_some()
    {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            if let Some(key) = state.selected_key {
                state.ui_drag_key = Some(key);
                state.ui_drag_mode = "rotate".to_string();
                state.ui_drag_last_x = pointer.uv.x;
                state.ui_drag_last_y = pointer.uv.y;
                state.log = "rotate node".to_string();
            }
        });
        refresh_all(ctx);
        return;
    }
    let Some(key) = pick_preview_ui(ctx) else {
        return;
    };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.selected_key = Some(key);
        state.ui_drag_key = Some(key);
        state.ui_drag_mode = "move".to_string();
        if let Some(pointer) = pointer {
            state.ui_drag_last_x = pointer.uv.x;
            state.ui_drag_last_y = pointer.uv.y;
        }
        state.log = format!("select node\nkey={key}");
    });
    refresh_all(ctx);
}

pub fn update_ui_drag<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    if mouse_released!(ctx.ipt, MouseButton::Left) {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.ui_drag_key = None;
            state.ui_drag_mode.clear();
        });
        return;
    }
    if !mouse_down!(ctx.ipt, MouseButton::Left) {
        return;
    }
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    if mode != "UI" {
        return;
    }
    let Some(pointer) = viewport_pointer(ctx) else {
        return;
    };
    let drag = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.ui_drag_key?;
        if state.ui_drag_mode.is_empty() {
            return None;
        }
        let delta = Vector2::new(
            pointer.uv.x - state.ui_drag_last_x,
            state.ui_drag_last_y - pointer.uv.y,
        );
        let mode = state.ui_drag_mode.clone();
        state.ui_drag_last_x = pointer.uv.x;
        state.ui_drag_last_y = pointer.uv.y;
        if delta.x.abs() < 0.0001 && delta.y.abs() < 0.0001 {
            return None;
        }
        Some((key, mode, delta))
    })
    .flatten();
    let Some((key, mode, root_delta)) = drag else {
        return;
    };
    let snap = viewport_shift_down(ctx);
    if mode == "move" {
        move_doc_ui_node(ctx, key, root_delta, snap);
    } else if mode == "rotate" {
        rotate_doc_ui_node(ctx, key, root_delta, snap);
    } else {
        resize_doc_ui_node(ctx, key, &mode, root_delta, snap);
    }
}

pub fn update_editor_cursor<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let icon = editor_cursor_icon(ctx);
    ctx.run.Window().set_cursor_icon(icon);
}

pub fn editor_cursor_icon<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> CursorIcon {
    let drag = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.ui_drag_mode.clone()
    });
    if !drag.is_empty() {
        return if drag == "move" {
            CursorIcon::Grabbing
        } else if drag == "rotate" {
            CursorIcon::AllResize
        } else {
            resize_cursor_icon(&drag)
        };
    }

    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    if mode != "UI" {
        return CursorIcon::Default;
    }
    let Some(pointer) = viewport_pointer(ctx) else {
        return CursorIcon::Default;
    };
    if let Some(handle) = pick_resize_handle(ctx, pointer) {
        return resize_cursor_icon(handle);
    }
    if pick_rotation_zone(ctx, pointer).is_some() {
        return CursorIcon::AllResize;
    }
    if pick_preview_ui(ctx).is_some() {
        return CursorIcon::Grab;
    }
    CursorIcon::Default
}

pub fn resize_cursor_icon(handle: &str) -> CursorIcon {
    match handle {
        "resize_n" => CursorIcon::NResize,
        "resize_s" => CursorIcon::SResize,
        "resize_e" => CursorIcon::EResize,
        "resize_w" => CursorIcon::WResize,
        "resize_ne" => CursorIcon::NeResize,
        "resize_nw" => CursorIcon::NwResize,
        "resize_se" => CursorIcon::SeResize,
        "resize_sw" => CursorIcon::SwResize,
        _ => CursorIcon::AllResize,
    }
}

pub fn move_doc_ui_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    key: u32,
    root_delta: Vector2,
    snap: bool,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(parent_rect) = doc_ui_parent_rect(&doc, key) else {
            return false;
        };
        if parent_rect.size.x <= 0.0 || parent_rect.size.y <= 0.0 {
            return false;
        }
        let delta = Vector2::new(
            root_delta.x / parent_rect.size.x,
            root_delta.y / parent_rect.size.y,
        );
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        let current = scene_field_vec2(&node.data, "translation_ratio").unwrap_or(Vector2::ZERO);
        let next = if snap {
            snap_vec2(current + delta, 0.01)
        } else {
            current + delta
        };
        set_scene_vec2(&mut node.data, "translation_ratio", next);
        state.log = if snap {
            "move ui\nsnap=0.01".to_string()
        } else {
            "move ui".to_string()
        };
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

pub fn resize_doc_ui_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    key: u32,
    handle: &str,
    root_delta: Vector2,
    snap: bool,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(parent_rect) = doc_ui_parent_rect(&doc, key) else {
            return false;
        };
        let Some(rect) = doc_ui_rect(&doc, key) else {
            return false;
        };
        if parent_rect.size.x <= 0.0 || parent_rect.size.y <= 0.0 {
            return false;
        }

        let mut min = rect.center - rect.size * 0.5;
        let mut max = rect.center + rect.size * 0.5;
        let (sx, sy) = resize_handle_sign(handle);
        if sx < 0.0 {
            min.x += root_delta.x;
        } else if sx > 0.0 {
            max.x += root_delta.x;
        }
        if sy < 0.0 {
            min.y += root_delta.y;
        } else if sy > 0.0 {
            max.y += root_delta.y;
        }
        let min_size = Vector2::new(0.02, 0.02);
        if max.x - min.x < min_size.x {
            if sx < 0.0 {
                min.x = max.x - min_size.x;
            } else {
                max.x = min.x + min_size.x;
            }
        }
        if max.y - min.y < min_size.y {
            if sy < 0.0 {
                min.y = max.y - min_size.y;
            } else {
                max.y = min.y + min_size.y;
            }
        }
        let new_size = max - min;
        let new_center = min + new_size * 0.5;
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        let anchor_text =
            scene_field_str(&node.data, "anchor").unwrap_or_else(|| "center".to_string());
        let anchor = scene_anchor_dir(&anchor_text);
        let anchor_point = parent_rect.center
            + Vector2::new(
                parent_rect.size.x * 0.5 * anchor.x,
                parent_rect.size.y * 0.5 * anchor.y,
            );
        let inward = Vector2::new(new_size.x * 0.5 * anchor.x, new_size.y * 0.5 * anchor.y);
        let mut translation = Vector2::new(
            (new_center.x - anchor_point.x + inward.x) / parent_rect.size.x,
            (new_center.y - anchor_point.y + inward.y) / parent_rect.size.y,
        );
        let mut size_ratio = Vector2::new(
            new_size.x / parent_rect.size.x,
            new_size.y / parent_rect.size.y,
        );
        if snap {
            translation = snap_vec2(translation, 0.01);
            size_ratio = snap_vec2(size_ratio, 0.01);
        }
        set_scene_vec2(&mut node.data, "size_ratio", size_ratio);
        set_scene_vec2(&mut node.data, "translation_ratio", translation);
        state.log = if snap {
            "resize ui\nsnap=0.01".to_string()
        } else {
            "resize ui".to_string()
        };
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

pub fn rotate_doc_ui_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    key: u32,
    root_delta: Vector2,
    snap: bool,
) {
    let (prev, curr) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (
            Vector2::new(
                state.ui_drag_last_x - root_delta.x,
                1.0 - state.ui_drag_last_y - root_delta.y,
            ),
            Vector2::new(state.ui_drag_last_x, 1.0 - state.ui_drag_last_y),
        )
    });
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(rect) = doc_ui_rect(&doc, key) else {
            return false;
        };
        let prev_angle = (prev.y - rect.center.y).atan2(prev.x - rect.center.x);
        let curr_angle = (curr.y - rect.center.y).atan2(curr.x - rect.center.x);
        let delta = curr_angle - prev_angle;
        if !delta.is_finite() || delta.abs() < 0.0001 {
            return false;
        }
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        let current = scene_field_f32(&node.data, "rotation").unwrap_or(0.0);
        let next = if snap {
            snap_f32(current + delta, std::f32::consts::TAU / 24.0)
        } else {
            current + delta
        };
        set_scene_f32(&mut node.data, "rotation", next);
        state.log = if snap {
            "rotate ui\nsnap=15deg".to_string()
        } else {
            "rotate ui".to_string()
        };
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

pub fn pick_preview_ui<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> Option<u32> {
    let pointer = viewport_pointer(ctx)?;
    let doc_text = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.doc_text.clone()
    });
    if doc_text.is_empty() {
        return None;
    }
    let doc = SceneDoc::parse(&doc_text);
    let point = Vector2::new(pointer.uv.x, 1.0 - pointer.uv.y);
    pick_doc_ui_node(&doc, point)
}

pub fn pick_resize_handle<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    pointer: ViewportPointer,
) -> Option<&'static str> {
    let (doc_text, selected) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.doc_text.clone(), state.selected_key)
    });
    let key = selected?;
    let doc = SceneDoc::parse(&doc_text);
    let rect = doc_ui_rect(&doc, key)?;
    let point = Vector2::new(pointer.uv.x, 1.0 - pointer.uv.y);
    resize_handles(rect)
        .into_iter()
        .find(|(_, center)| {
            (point.x - center.x).abs() <= 0.018 && (point.y - center.y).abs() <= 0.018
        })
        .map(|(name, _)| name)
}

pub fn pick_rotation_zone<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    pointer: ViewportPointer,
) -> Option<&'static str> {
    let (doc_text, selected) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.doc_text.clone(), state.selected_key)
    });
    let key = selected?;
    let doc = SceneDoc::parse(&doc_text);
    let rect = doc_ui_rect(&doc, key)?;
    let point = Vector2::new(pointer.uv.x, 1.0 - pointer.uv.y);
    let min = rect.center - rect.size * 0.5;
    let max = rect.center + rect.size * 0.5;
    let zones = [
        ("rotate_nw", Vector2::new(min.x - 0.035, max.y + 0.035)),
        ("rotate_ne", Vector2::new(max.x + 0.035, max.y + 0.035)),
        ("rotate_sw", Vector2::new(min.x - 0.035, min.y - 0.035)),
        ("rotate_se", Vector2::new(max.x + 0.035, min.y - 0.035)),
    ];
    zones
        .into_iter()
        .find(|(_, center)| {
            (point.x - center.x).abs() <= 0.045 && (point.y - center.y).abs() <= 0.045
        })
        .map(|(name, _)| name)
}

pub fn resize_handles(rect: EditorUiRect) -> [(&'static str, Vector2); 8] {
    let min = rect.center - rect.size * 0.5;
    let max = rect.center + rect.size * 0.5;
    let mid_x = rect.center.x;
    let mid_y = rect.center.y;
    [
        ("resize_nw", Vector2::new(min.x, max.y)),
        ("resize_n", Vector2::new(mid_x, max.y)),
        ("resize_ne", Vector2::new(max.x, max.y)),
        ("resize_w", Vector2::new(min.x, mid_y)),
        ("resize_e", Vector2::new(max.x, mid_y)),
        ("resize_sw", Vector2::new(min.x, min.y)),
        ("resize_s", Vector2::new(mid_x, min.y)),
        ("resize_se", Vector2::new(max.x, min.y)),
    ]
}

pub fn resize_handle_sign(handle: &str) -> (f32, f32) {
    match handle {
        "resize_nw" => (-1.0, 1.0),
        "resize_n" => (0.0, 1.0),
        "resize_ne" => (1.0, 1.0),
        "resize_w" => (-1.0, 0.0),
        "resize_e" => (1.0, 0.0),
        "resize_sw" => (-1.0, -1.0),
        "resize_s" => (0.0, -1.0),
        "resize_se" => (1.0, -1.0),
        _ => (0.0, 0.0),
    }
}

#[derive(Clone, Copy)]
pub struct EditorUiRect {
    pub center: Vector2,
    pub size: Vector2,
    pub rotation: f32,
}

impl EditorUiRect {
    fn contains(self, point: Vector2) -> bool {
        let half = self.size * 0.5;
        point.x >= self.center.x - half.x
            && point.x <= self.center.x + half.x
            && point.y >= self.center.y - half.y
            && point.y <= self.center.y + half.y
    }
}

pub fn pick_doc_ui_node(doc: &SceneDoc, point: Vector2) -> Option<u32> {
    let root_rect = EditorUiRect {
        center: Vector2::new(0.5, 0.5),
        size: Vector2::ONE,
        rotation: 0.0,
    };
    let mut hit = None;
    if let Some(root) = doc.scene.root {
        pick_doc_ui_node_inner(doc, root.as_u32(), root_rect, point, &mut hit);
    }
    for node in doc.scene.nodes.iter() {
        if node.parent.is_none()
            && doc.scene.root.map(|root| root.as_u32()) != Some(node.key.as_u32())
        {
            pick_doc_ui_node_inner(doc, node.key.as_u32(), root_rect, point, &mut hit);
        }
    }
    hit
}

pub fn pick_doc_ui_node_inner(
    doc: &SceneDoc,
    key: u32,
    parent_rect: EditorUiRect,
    point: Vector2,
    hit: &mut Option<u32>,
) {
    let Some(node) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) else {
        return;
    };
    let Some(rect) = editor_ui_rect(&node.data, parent_rect) else {
        return;
    };
    if rect.contains(point) {
        *hit = Some(key);
    }
    for child in doc
        .scene
        .nodes
        .iter()
        .filter(|child| child.parent.map(|parent| parent.as_u32()) == Some(key))
    {
        pick_doc_ui_node_inner(doc, child.key.as_u32(), rect, point, hit);
    }
}

pub fn editor_ui_rect(data: &SceneNodeData, parent: EditorUiRect) -> Option<EditorUiRect> {
    if !data.type_name().starts_with("Ui") {
        return None;
    }
    if scene_field_bool(data, "visible") == Some(false) {
        return None;
    }
    let anchor_text = scene_field_str(data, "anchor").unwrap_or_else(|| "center".to_string());
    let anchor = scene_anchor_dir(&anchor_text);
    let size_ratio = scene_field_vec2(data, "size_ratio").unwrap_or(Vector2::ZERO);
    let translation = scene_field_vec2(data, "translation_ratio").unwrap_or(Vector2::ZERO);
    let rotation = scene_field_f32(data, "rotation").unwrap_or(0.0);
    let size = Vector2::new(parent.size.x * size_ratio.x, parent.size.y * size_ratio.y);
    if size.x <= 0.0 || size.y <= 0.0 {
        return None;
    }
    let anchor_point = parent.center
        + Vector2::new(
            parent.size.x * 0.5 * anchor.x,
            parent.size.y * 0.5 * anchor.y,
        );
    let inward = Vector2::new(size.x * 0.5 * anchor.x, size.y * 0.5 * anchor.y);
    let offset = Vector2::new(parent.size.x * translation.x, parent.size.y * translation.y);
    Some(EditorUiRect {
        center: anchor_point - inward + offset,
        size,
        rotation,
    })
}

pub fn doc_ui_parent_rect(doc: &SceneDoc, key: u32) -> Option<EditorUiRect> {
    let root_rect = EditorUiRect {
        center: Vector2::new(0.5, 0.5),
        size: Vector2::ONE,
        rotation: 0.0,
    };
    let node = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?;
    let Some(parent) = node.parent else {
        return Some(root_rect);
    };
    doc_ui_rect(doc, parent.as_u32()).or(Some(root_rect))
}

pub fn doc_ui_rect(doc: &SceneDoc, key: u32) -> Option<EditorUiRect> {
    let root_rect = EditorUiRect {
        center: Vector2::new(0.5, 0.5),
        size: Vector2::ONE,
        rotation: 0.0,
    };
    let node = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?;
    let parent = node
        .parent
        .and_then(|parent| doc_ui_rect(doc, parent.as_u32()))
        .unwrap_or(root_rect);
    editor_ui_rect(&node.data, parent)
}

pub fn scene_anchor_dir(anchor: &str) -> Vector2 {
    match anchor {
        "left" => Vector2::new(-1.0, 0.0),
        "right" => Vector2::new(1.0, 0.0),
        "top" => Vector2::new(0.0, 1.0),
        "bottom" => Vector2::new(0.0, -1.0),
        "top_left" | "top-left" => Vector2::new(-1.0, 1.0),
        "top_right" | "top-right" => Vector2::new(1.0, 1.0),
        "bottom_left" | "bottom-left" => Vector2::new(-1.0, -1.0),
        "bottom_right" | "bottom-right" => Vector2::new(1.0, -1.0),
        _ => Vector2::ZERO,
    }
}

pub fn scene_field(data: &SceneNodeData, field: &str) -> Option<SceneValue> {
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            return Some(value.clone());
        }
    }
    data.base_ref().and_then(|base| scene_field(base, field))
}

pub fn scene_field_bool(data: &SceneNodeData, field: &str) -> Option<bool> {
    scene_field(data, field)?.as_bool()
}

pub fn scene_field_str(data: &SceneNodeData, field: &str) -> Option<String> {
    scene_field(data, field)?.as_str().map(str::to_string)
}

pub fn selected_node_type_name(doc_text: &str, key: u32) -> Option<String> {
    let doc = SceneDoc::parse(doc_text);
    doc.scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)
        .map(|node| node.data.type_name().to_string())
}

pub fn selected_node_viewport_mode(doc_text: &str, key: u32) -> Option<&'static str> {
    let doc = SceneDoc::parse(doc_text);
    let node_type = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?
        .data
        .node_type;
    viewport_mode_for_node_type(node_type)
}

pub fn viewport_mode_for_node_type(node_type: perro_scene::NodeType) -> Option<&'static str> {
    if node_type.is_a(perro_scene::NodeType::UiBox) {
        Some("UI")
    } else if node_type.is_a(perro_scene::NodeType::Node3D) {
        Some("3D")
    } else if node_type.is_a(perro_scene::NodeType::Node2D) {
        Some("2D")
    } else {
        None
    }
}

pub fn selected_node_field_text(doc_text: &str, key: u32, field: &str) -> Option<String> {
    let doc = SceneDoc::parse(doc_text);
    let node = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?;
    scene_field_value_text(&node.data, field)
}

pub fn scene_field_value_text(data: &SceneNodeData, field: &str) -> Option<String> {
    match scene_field(data, field)? {
        SceneValue::Str(value) => Some(value.to_string()),
        SceneValue::Key(key) => Some(key.to_string()),
        SceneValue::F32(value) => Some(value.to_string()),
        SceneValue::I32(value) => Some(value.to_string()),
        _ => None,
    }
}

pub fn scene_field_vec2(data: &SceneNodeData, field: &str) -> Option<Vector2> {
    scene_field(data, field)?
        .as_vec2()
        .map(|(x, y)| Vector2::new(x, y))
}

pub fn scene_field_f32(data: &SceneNodeData, field: &str) -> Option<f32> {
    scene_field(data, field)?.as_f32()
}

pub fn set_scene_vec2(data: &mut SceneNodeData, field: &str, value: Vector2) {
    let name = SceneFieldName::from_name(field.to_string());
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == field {
            *field_value = SceneValue::Vec2 {
                x: value.x,
                y: value.y,
            };
            return;
        }
    }
    data.fields.to_mut().push((
        name,
        SceneValue::Vec2 {
            x: value.x,
            y: value.y,
        },
    ));
}

pub fn set_scene_f32(data: &mut SceneNodeData, field: &str, value: f32) {
    let name = SceneFieldName::from_name(field.to_string());
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == field {
            *field_value = SceneValue::F32(value);
            return;
        }
    }
    data.fields.to_mut().push((name, SceneValue::F32(value)));
}

pub fn set_scene_bool(data: &mut SceneNodeData, field: &str, value: bool) {
    let name = SceneFieldName::from_name(field.to_string());
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == field {
            *field_value = SceneValue::Bool(value);
            return;
        }
    }
    data.fields.to_mut().push((name, SceneValue::Bool(value)));
}

pub fn set_scene_vec3(data: &mut SceneNodeData, field: &str, value: Vector3) {
    let name = SceneFieldName::from_name(field.to_string());
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == field {
            *field_value = SceneValue::Vec3 {
                x: value.x,
                y: value.y,
                z: value.z,
            };
            return;
        }
    }
    data.fields.to_mut().push((
        name,
        SceneValue::Vec3 {
            x: value.x,
            y: value.y,
            z: value.z,
        },
    ));
}

pub fn set_scene_vec4(data: &mut SceneNodeData, field: &str, x: f32, y: f32, z: f32, w: f32) {
    let name = SceneFieldName::from_name(field.to_string());
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == field {
            *field_value = SceneValue::Vec4 { x, y, z, w };
            return;
        }
    }
    data.fields
        .to_mut()
        .push((name, SceneValue::Vec4 { x, y, z, w }));
}

pub fn set_scene_string(data: &mut SceneNodeData, field: &str, value: String) {
    let name = SceneFieldName::from_name(field.to_string());
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == field {
            *field_value = SceneValue::Str(Cow::Owned(value));
            return;
        }
    }
    data.fields
        .to_mut()
        .push((name, SceneValue::Str(Cow::Owned(value))));
}

pub fn set_scene_binding(data: &mut SceneNodeData, object: &str, node_name: &str) {
    let name = SceneFieldName::Bindings;
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == "bindings" {
            match field_value {
                SceneValue::Object(fields) => {
                    for (binding_name, binding_value) in fields.to_mut().iter_mut() {
                        if binding_name.as_ref() == object {
                            *binding_value =
                                SceneValue::Key(SceneValueKey::from(node_name.to_string()));
                            return;
                        }
                    }
                    fields.to_mut().push((
                        SceneFieldName::from_name(object.to_string()),
                        SceneValue::Key(SceneValueKey::from(node_name.to_string())),
                    ));
                }
                _ => {
                    *field_value = SceneValue::Object(Cow::Owned(vec![(
                        SceneFieldName::from_name(object.to_string()),
                        SceneValue::Key(SceneValueKey::from(node_name.to_string())),
                    )]));
                }
            }
            return;
        }
    }
    data.fields.to_mut().push((
        name,
        SceneValue::Object(Cow::Owned(vec![(
            SceneFieldName::from_name(object.to_string()),
            SceneValue::Key(SceneValueKey::from(node_name.to_string())),
        )])),
    ));
}

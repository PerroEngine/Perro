use crate::scripts::editor::main::{EditorState, cached_scene_doc_shared};
use crate::scripts::scene::editor_viewport::*;
use crate::scripts::scene::{editor_batch, editor_selection as selection};
use crate::scripts::ui::editor_ui::{
    editor_layout, find_named, refresh_selection_panels, set_log, set_panel_display,
};
use perro_api::prelude::*;
use perro_api::scene::{SceneDoc, SceneValue};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy)]
struct Pose {
    key: u32,
    global: Transform3D,
    parent: Transform3D,
}
struct Drag {
    owner: u64,
    path: String,
    source: String,
    doc: SceneDoc,
    selected: Vec<u32>,
    poses: Vec<Pose>,
    pivot: Transform3D,
    start: Vector3,
    uv: Vector2,
    normal: Vector3,
    axis: Option<Vector3>,
    mode: String,
    tool: String,
    changed: bool,
    last_sample: Option<(Vector3, f32, f32)>,
}
static DRAG: OnceLock<Mutex<Option<Drag>>> = OnceLock::new();

fn project<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    point: Vector3,
    mode: &str,
) -> Option<Vector2> {
    let (raw, layout) = with_state!(ctx.run, EditorState, ctx.id, |s| (
        if mode == "2D" {
            s.preview_camera_2d
        } else {
            s.preview_camera_3d
        },
        editor_layout(s)
    ))?;
    let camera = if raw != 0 {
        NodeID::from_u64(raw)
    } else {
        find_named(
            ctx,
            if mode == "2D" {
                "editor_camera_2d"
            } else {
                "editor_camera_3d"
            },
        )?
    };
    let ndc = if mode == "2D" {
        let t = ctx.run.Nodes().get_global_transform_2d(camera)?;
        let zoom = with_node!(ctx.run, Camera2D, camera, |n| n.zoom)?;
        let (s, c) = t.rotation.sin_cos();
        let p = point - Vector3::new(t.position.x, t.position.y, 0.0);
        Vector2::new(
            (p.x * c + p.y * s) * zoom / 480.0,
            (-p.x * s + p.y * c) * zoom / 270.0,
        )
    } else {
        let t = ctx.run.Nodes().get_global_transform_3d(camera)?;
        let p = t.rotation.inverse().rotate_vector3(point - t.position);
        let projection = with_node!(ctx.run, Camera3D, camera, |n| n.projection.clone())?;
        match projection {
            CameraProjection::Perspective { fov_y_degrees, .. } => {
                if p.z >= -0.001 {
                    return None;
                }
                let tan = (fov_y_degrees.to_radians() * 0.5).tan();
                Vector2::new(p.x / (-p.z * tan * 16.0 / 9.0), p.y / (-p.z * tan))
            }
            CameraProjection::Orthographic { size, .. } => {
                Vector2::new(p.x / (size * 16.0 / 9.0 * 0.5), p.y / (size * 0.5))
            }
            CameraProjection::Frustum {
                left,
                right,
                bottom,
                top,
                near,
                ..
            } => {
                if p.z >= -0.001 {
                    return None;
                }
                Vector2::new(
                    ((p.x * near / -p.z - left) / (right - left)) * 2.0 - 1.0,
                    ((p.y * near / -p.z - bottom) / (top - bottom)) * 2.0 - 1.0,
                )
            }
        }
    };
    let viewport = ctx.res.viewport_size();
    let aspect = viewport.x / viewport.y.max(1.0);
    let rect = viewport_stream_rect_ratio(aspect, layout);
    let half = stream_half_ndc(aspect);
    if ndc.x.abs() > 1.2 || ndc.y.abs() > 1.2 {
        return None;
    }
    Some(Vector2::new(
        rect.0 + ndc.x * half.0 * rect.2 * 0.5,
        rect.1 - ndc.y * half.1 * rect.3 * 0.5,
    ))
}

fn line<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    a: Vector2,
    b: Vector2,
) {
    let viewport = ctx.res.viewport_size();
    let dx = (b.x - a.x) * viewport.x;
    let dy = (a.y - b.y) * viewport.y;
    let width = (dx * dx + dy * dy).sqrt() / viewport.x.max(1.0);
    if let Some(id) = find_named(ctx, name) {
        let translation = Vector2::new((a.x + b.x) * 0.5 - 0.5, 0.5 - (a.y + b.y) * 0.5);
        let rotation = dy.atan2(dx);
        let size = UiVector2::ratio(width, 3.0 / viewport.y.max(1.0));
        if with_node!(ctx.run, UiPanel, id, |n| n.visible
            && n.layout.size == size
            && n.transform.translation == translation
            && n.transform.rotation == rotation)
        .unwrap_or(false)
        {
            return;
        }
        let _ = with_node_mut!(ctx.run, UiPanel, id, |n| {
            n.visible = true;
            n.input_enabled = false;
            n.layout.anchor = UiAnchor::Center;
            n.layout.size = size;
            n.transform.position = UiVector2::percent(50.0, 50.0);
            n.transform.pivot = UiVector2::percent(50.0, 50.0);
            n.transform.translation = translation;
            n.transform.rotation = rotation;
        });
    }
}

/// Draw true projected axes; return the axis under the pointer for drag capture.
fn hide<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str) {
    if let Some(id) = find_named(ctx, name)
        && with_node!(ctx.run, UiPanel, id, |n| n.visible).unwrap_or(false)
    {
        set_panel_display(ctx, name, false);
    }
}
fn draw<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> Option<Vector3> {
    let data = with_state!(ctx.run, EditorState, ctx.id, |s| {
        if s.viewport_mode == "UI" || s.animation_tool_open || s.viewport_tool == "select" {
            return None;
        }
        let key = s.selected_key?;
        let id = *s
            .preview_node_ids
            .get(s.preview_node_keys.iter().position(|k| *k == key)?)?;
        Some((s.viewport_mode.clone(), id, s.viewport_local))
    })
    .unwrap_or_default();
    let data = data.and_then(|(mode, id, local)| {
        let pose = global(ctx, id, &mode)?;
        let origin = project(ctx, pose.position, &mode)?;
        Some((mode, local, pose, origin))
    });
    let Some((mode, local, pose, origin)) = data else {
        for name in ["spatial_axis_x", "spatial_axis_y", "spatial_axis_z"] {
            hide(ctx, name);
        }
        return None;
    };
    let mouse = mouse_position!(ctx.ipt);
    let viewport = ctx.res.viewport_size();
    let mut picked = None;
    for (name, axis) in [
        ("spatial_axis_x", Vector3::new(1.0, 0.0, 0.0)),
        ("spatial_axis_y", Vector3::new(0.0, 1.0, 0.0)),
        ("spatial_axis_z", Vector3::new(0.0, 0.0, 1.0)),
    ] {
        if mode == "2D" && axis.z != 0.0 {
            hide(ctx, name);
            continue;
        }
        let axis = if local {
            pose.rotation.rotate_vector3(axis)
        } else {
            axis
        };
        let Some(unit) = project(ctx, pose.position + axis, &mode) else {
            hide(ctx, name);
            continue;
        };
        let dx = (unit.x - origin.x) * viewport.x;
        let dy = (unit.y - origin.y) * viewport.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.01 {
            hide(ctx, name);
            continue;
        }
        let end = Vector2::new(
            origin.x + (unit.x - origin.x) * 70.0 / len,
            origin.y + (unit.y - origin.y) * 70.0 / len,
        );
        line(ctx, name, origin, end);
        let mx = (mouse.x - origin.x) * viewport.x;
        let my = (mouse.y - origin.y) * viewport.y;
        let ux = dx / len;
        let uy = dy / len;
        let along = mx * ux + my * uy;
        let across = (mx * uy - my * ux).abs();
        if (8.0..=78.0).contains(&along) && across < 8.0 {
            picked = Some(axis);
        }
    }
    picked
}

fn dot(a: Vector3, b: Vector3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

pub fn select_at_pointer<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(pointer) = viewport_pointer(ctx) else {
        return;
    };
    let (mode, keys, ids) = with_state!(ctx.run, EditorState, ctx.id, |s| (
        s.viewport_mode.clone(),
        s.preview_node_keys.clone(),
        s.preview_node_ids.clone()
    ))
    .unwrap_or_default();
    let key = if mode == "3D" {
        stream_pointer_ray_3d(ctx, pointer).and_then(|ray| pick_preview_3d(ctx, ray))
    } else {
        let mouse = mouse_position!(ctx.ipt);
        let viewport = ctx.res.viewport_size();
        let mut best = None;
        let mut distance = 24.0_f32 * 24.0;
        for (key, id) in keys.iter().zip(ids) {
            if let Some(p) = global(ctx, id, &mode).and_then(|t| project(ctx, t.position, &mode)) {
                let dx = (p.x - mouse.x) * viewport.x;
                let dy = (p.y - mouse.y) * viewport.y;
                let d = dx * dx + dy * dy;
                if d < distance {
                    best = Some(*key);
                    distance = d;
                }
            }
        }
        best
    };
    let ctrl =
        key_down!(ctx.ipt, KeyCode::ControlLeft) || key_down!(ctx.ipt, KeyCode::ControlRight);
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |s| {
        if let Some(key) = key {
            selection::click(s, key, ctrl, false, &[]);
        } else if !ctrl {
            selection::replace(s, Vec::new());
        }
    });
    refresh_selection_panels(ctx);
}
fn axis_rotation(axis: Vector3, angle: f32) -> Quaternion {
    let axis = axis.normalized();
    let (s, c) = (angle * 0.5).sin_cos();
    Quaternion::new(axis.x * s, axis.y * s, axis.z * s, c)
}
fn embed(t: Transform2D) -> Transform3D {
    Transform3D {
        position: Vector3::new(t.position.x, t.position.y, 0.0),
        scale: Vector3::new(t.scale.x, t.scale.y, 1.0),
        rotation: Quaternion::from_euler_xyz(0.0, 0.0, t.rotation),
    }
}
fn global<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    id: u64,
    mode: &str,
) -> Option<Transform3D> {
    if mode == "2D" {
        ctx.run
            .Nodes()
            .get_global_transform_2d(NodeID::from_u64(id))
            .map(embed)
    } else {
        ctx.run
            .Nodes()
            .get_global_transform_3d(NodeID::from_u64(id))
    }
}
fn hit<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    pointer: ViewportPointer,
    mode: &str,
    pivot: Vector3,
    normal: Vector3,
) -> Option<Vector3> {
    if mode == "2D" {
        return stream_pointer_world_2d(ctx, pointer).map(|p| Vector3::new(p.x, p.y, 0.0));
    }
    let ray = stream_pointer_ray_3d(ctx, pointer)?;
    let d = dot(ray.direction, normal);
    if d.abs() < 1e-6 {
        return None;
    }
    let t = dot(pivot - ray.origin, normal) / d;
    (t.is_finite() && t >= 0.0).then_some(ray.origin + ray.direction * t)
}

pub fn active() -> bool {
    DRAG.get()
        .and_then(|m| m.lock().ok())
        .is_some_and(|d| d.is_some())
}

pub fn cancel<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> bool {
    let Some(slot) = DRAG.get() else {
        return false;
    };
    let Ok(mut slot) = slot.lock() else {
        return false;
    };
    let Some(drag) = slot.take() else {
        return false;
    };
    drop(slot);
    if drag.owner == ctx.id.as_u64() {
        rebuild_preview(ctx);
        refresh_selection_panels(ctx);
    }
    true
}

/// Gesture samples are evaluated from initial poses, never accumulated snapped deltas.
fn transformed(
    pose: Pose,
    pivot: Transform3D,
    delta: Vector3,
    axis: Vector3,
    angle: f32,
    scale: f32,
    tool: &str,
) -> Transform3D {
    let mut next = pose.global;
    match tool {
        "move" => next.position += delta,
        "rotate" => {
            let rotation = axis_rotation(axis, angle);
            next.position =
                pivot.position + rotation.rotate_vector3(next.position - pivot.position);
            next.rotation = rotation * next.rotation;
        }
        "scale" => {
            next.position = pivot.position + (next.position - pivot.position) * scale;
            next.scale *= scale;
        }
        _ => {}
    }
    Transform3D::inverse_compose(pose.parent, next)
}

pub fn update<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let picked_axis = draw(ctx);
    if key_pressed!(ctx.ipt, KeyCode::Escape) && cancel(ctx) {
        return;
    }
    let slot = DRAG.get_or_init(|| Mutex::new(None));
    let Ok(mut slot) = slot.lock() else {
        return;
    };
    if let Some(drag) = slot.as_mut() {
        let valid = with_state!(ctx.run, EditorState, ctx.id, |s| s.doc_text == drag.source
            && s.open_paths
                .get(s.active_open)
                .map(String::as_str)
                .unwrap_or("")
                == drag.path
            && s.viewport_mode == drag.mode)
        .unwrap_or(false);
        if !valid {
            slot.take();
            drop(slot);
            rebuild_preview(ctx);
            return;
        }
        if mouse_released!(ctx.ipt, MouseButton::Left) || !mouse_down!(ctx.ipt, MouseButton::Left) {
            let Some(drag) = slot.take() else {
                return;
            };
            drop(slot);
            if drag.changed {
                let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |s| editor_batch::commit(
                    s,
                    &drag.doc,
                    drag.selected
                ));
                refresh_selection_panels(ctx);
            }
            return;
        }
        let Some(pointer) = viewport_pointer(ctx) else {
            return;
        };
        let Some(point) = hit(ctx, pointer, &drag.mode, drag.pivot.position, drag.normal) else {
            return;
        };
        let mut delta = point - drag.start;
        if let Some(axis) = drag.axis {
            delta = axis * dot(delta, axis);
        }
        let amount = (pointer.uv.x - drag.uv.x) - (pointer.uv.y - drag.uv.y);
        let mut angle = amount * std::f32::consts::TAU;
        let mut scale = (1.0 + amount * 2.0).max(0.01);
        let shift = viewport_shift_down(ctx);
        let snap = with_state!(ctx.run, EditorState, ctx.id, |s| viewport_snap_active(
            s, shift
        ))
        .unwrap_or(false);
        if snap {
            let step = if drag.mode == "2D" { 16.0 } else { 1.0 };
            delta = if let Some(axis) = drag.axis {
                axis * snap_f32(dot(delta, axis), step)
            } else {
                snap_vec3(delta, step)
            };
            angle = snap_f32(angle, 15.0_f32.to_radians());
            scale = snap_f32(scale, 0.1).max(0.1);
        }
        let sample = (delta, angle, scale);
        if drag.last_sample == Some(sample) {
            return;
        }
        drag.last_sample = Some(sample);
        let changed = match drag.tool.as_str() {
            "move" => dot(delta, delta) > 1e-10,
            "rotate" => angle.abs() > 1e-6,
            "scale" => (scale - 1.0).abs() > 1e-6,
            _ => false,
        };
        if !changed {
            if drag.changed {
                drag.doc = (*cached_scene_doc_shared(&drag.source)).clone();
                drag.changed = false;
                drop(slot);
                rebuild_preview(ctx);
            }
            return;
        }
        let axis = drag.axis.unwrap_or(if drag.mode == "2D" {
            Vector3::new(0.0, 0.0, 1.0)
        } else {
            drag.normal
        });
        for pose in &drag.poses {
            let next = transformed(*pose, drag.pivot, delta, axis, angle, scale, &drag.tool);
            let Some(node) = drag
                .doc
                .scene
                .nodes
                .to_mut()
                .iter_mut()
                .find(|n| n.key.as_u32() == pose.key)
            else {
                continue;
            };
            let fields = if drag.mode == "2D" {
                let rotation = 2.0 * next.rotation.z.atan2(next.rotation.w);
                vec![
                    (
                        "position",
                        SceneValue::Vec2 {
                            x: next.position.x,
                            y: next.position.y,
                        },
                    ),
                    ("rotation", SceneValue::F32(rotation)),
                    (
                        "scale",
                        SceneValue::Vec2 {
                            x: next.scale.x,
                            y: next.scale.y,
                        },
                    ),
                ]
            } else {
                vec![
                    (
                        "position",
                        SceneValue::Vec3 {
                            x: next.position.x,
                            y: next.position.y,
                            z: next.position.z,
                        },
                    ),
                    (
                        "rotation",
                        SceneValue::Vec4 {
                            x: next.rotation.x,
                            y: next.rotation.y,
                            z: next.rotation.z,
                            w: next.rotation.w,
                        },
                    ),
                    (
                        "scale",
                        SceneValue::Vec3 {
                            x: next.scale.x,
                            y: next.scale.y,
                            z: next.scale.z,
                        },
                    ),
                ]
            };
            for (name, value) in fields {
                if name == "rotation" && drag.tool != "rotate"
                    || name == "scale" && drag.tool != "scale"
                {
                    continue;
                }
                if let Some((_, current)) = node
                    .data
                    .fields
                    .to_mut()
                    .iter_mut()
                    .find(|(field, _)| field.as_ref() == name)
                {
                    *current = value.clone();
                } else {
                    node.data.fields.to_mut().push((
                        perro_api::scene::SceneFieldName::from_name(name.to_string()),
                        value.clone(),
                    ));
                }
                let _ = sync_preview_field_for_key(ctx, pose.key, name, &value);
            }
        }
        drag.changed = true;
        return;
    }
    if !mouse_pressed!(ctx.ipt, MouseButton::Left) {
        return;
    }
    let Some(pointer) = viewport_pointer(ctx) else {
        return;
    };
    let data = with_state!(ctx.run, EditorState, ctx.id, |s| {
        if s.viewport_mode != "2D" && s.viewport_mode != "3D" {
            return None;
        }
        Some((
            s.viewport_mode.clone(),
            s.viewport_tool.clone(),
            selection::keys(s),
            s.selected_key?,
            s.doc_text.clone(),
            s.open_paths.get(s.active_open).cloned().unwrap_or_default(),
            s.preview_node_keys.clone(),
            s.preview_node_ids.clone(),
            s.viewport_local,
        ))
    })
    .unwrap_or_default();
    let Some((mode, tool, selected, primary, text, path, keys, ids, local)) = data else {
        return;
    };
    if !matches!(tool.as_str(), "move" | "rotate" | "scale") {
        return;
    }
    let doc = cached_scene_doc_shared(&text);
    let id_for = |key| {
        keys.iter()
            .position(|k| *k == key)
            .and_then(|i| ids.get(i))
            .copied()
    };
    let Some(pivot) = id_for(primary).and_then(|id| global(ctx, id, &mode)) else {
        return;
    };
    let normal = if mode == "2D" {
        Vector3::new(0.0, 0.0, 1.0)
    } else {
        let Some(ray) = stream_pointer_ray_3d(ctx, pointer) else {
            return;
        };
        ray.direction
    };
    let Some(start) = hit(ctx, pointer, &mode, pivot.position, normal) else {
        return;
    };
    let mut poses = Vec::new();
    for key in selection::roots(&doc, &selected) {
        let Some(global_pose) = id_for(key).and_then(|id| global(ctx, id, &mode)) else {
            set_log(ctx, "transform fail\nselect compatible spatial nodes");
            return;
        };
        let parent = doc
            .scene
            .nodes
            .iter()
            .find(|n| n.key.as_u32() == key)
            .and_then(|n| n.parent)
            .and_then(|p| id_for(p.as_u32()))
            .and_then(|id| global(ctx, id, &mode))
            .unwrap_or(Transform3D::IDENTITY);
        if parent.scale.x.abs() < 1e-6 || parent.scale.y.abs() < 1e-6 || parent.scale.z.abs() < 1e-6
        {
            set_log(ctx, "transform fail\nzero-scale parent");
            return;
        }
        if tool != "move"
            && ((parent.scale.x - parent.scale.y).abs() > 1e-5
                || (mode == "3D" && (parent.scale.x - parent.scale.z).abs() > 1e-5))
        {
            set_log(
                ctx,
                "transform fail\nrotate/scale needs uniform parent scale",
            );
            return;
        }
        poses.push(Pose {
            key,
            global: global_pose,
            parent,
        });
    }
    let axis = if key_down!(ctx.ipt, KeyCode::KeyX) {
        Some(Vector3::new(1.0, 0.0, 0.0))
    } else if key_down!(ctx.ipt, KeyCode::KeyY) {
        Some(Vector3::new(0.0, 1.0, 0.0))
    } else if key_down!(ctx.ipt, KeyCode::KeyZ) && mode == "3D" {
        Some(Vector3::new(0.0, 0.0, 1.0))
    } else {
        None
    };
    let axis = picked_axis.or_else(|| {
        axis.map(|a| {
            if local {
                pivot.rotation.rotate_vector3(a)
            } else {
                a
            }
        })
    });
    *slot = Some(Drag {
        owner: ctx.id.as_u64(),
        path,
        source: text,
        doc: (*doc).clone(),
        selected,
        poses,
        pivot,
        start,
        uv: pointer.uv,
        normal,
        axis,
        mode,
        tool,
        changed: false,
        last_sample: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn move_under_rotated_parent_uses_local_coordinates() {
        let parent = Transform3D {
            rotation: Quaternion::from_euler_xyz(0.0, 0.0, std::f32::consts::FRAC_PI_2),
            ..Transform3D::IDENTITY
        };
        let pose = Pose {
            key: 1,
            global: Transform3D::IDENTITY,
            parent,
        };
        let next = transformed(
            pose,
            Transform3D::IDENTITY,
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::ZERO,
            0.0,
            1.0,
            "move",
        );
        let round_trip = Transform3D::compose(parent, next);
        assert!((round_trip.position.x - 1.0).abs() < 1e-5);
        assert!(round_trip.position.y.abs() < 1e-5);
    }
}

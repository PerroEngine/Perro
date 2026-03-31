use crate::prelude::*;
use perro_animation::{
    AnimationBoneSelector, AnimationEase, AnimationEvent, AnimationEventScope,
    AnimationInterpolation, AnimationObjectTrack, AnimationParam, AnimationTrackValue,
};
use perro_nodes::animation_player::{AnimationObjectBinding, AnimationPlaybackType};
use perro_nodes::{
    AmbientLight3D, AnimationPlayer, Camera3D, MeshInstance3D, Node2D, Node3D, PointLight3D,
    RayLight3D, Skeleton3D, SpotLight3D, Sprite2D,
};
use perro_runtime_context::perro_structs::{Quaternion, Vector2, Vector3};
use perro_runtime_context::perro_variant::Variant;
use perro_scene::{
    Camera3DField, Light3DField, MeshInstance3DField, Node2DField, Node3DField, NodeField,
    PointLight3DField, SpotLight3DField, Sprite2DField, resolve_node_field,
};
use std::collections::HashMap;
use std::sync::Arc;

type SelfNodeType = AnimationPlayer;

pub fn internal_update<RT, R, IP>(
    ctx: &mut RuntimeContext<'_, RT>,
    res: &ResourceContext<'_, R>,
    _ipt: &InputContext<'_, IP>,
    self_id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    let animation_id = with_node!(ctx, SelfNodeType, self_id, |player| player.animation);
    if animation_id.is_nil() {
        return;
    }

    let Some(clip) = res.Animations().get(animation_id) else {
        return;
    };
    if clip.object_tracks.is_empty() {
        return;
    }

    let delta_seconds = delta_time!(ctx).max(0.0);
    let Some(step) = step_animation_player(ctx, self_id, &clip, delta_seconds) else {
        return;
    };
    if !step.should_apply {
        return;
    }

    apply_clip_frame(ctx, res, &clip, step.frame, &step.bindings);
}

pub fn internal_fixed_update<RT, R, IP>(
    _ctx: &mut RuntimeContext<'_, RT>,
    _res: &ResourceContext<'_, R>,
    _ipt: &InputContext<'_, IP>,
    _self_id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
}

struct AnimationStep {
    frame: u32,
    bindings: Vec<AnimationObjectBinding>,
    should_apply: bool,
}

fn step_animation_player<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    self_id: NodeID,
    clip: &Arc<perro_animation::AnimationClip>,
    delta_seconds: f32,
) -> Option<AnimationStep>
where
    RT: RuntimeAPI + ?Sized,
{
    with_node_mut!(ctx, SelfNodeType, self_id, |player| {
        let previous_frame = player.current_frame;
        let frame_count = clip.frame_count();

        if !player.paused {
            player.internal.playback_frame = advance_playback_frame(
                player.internal.playback_frame,
                delta_seconds * clip.fps.max(0.0) * player.speed,
                frame_count,
                player.playback_type,
                &mut player.internal.boomerang_direction,
            );
            player.current_frame = playback_frame_to_frame(
                player.internal.playback_frame,
                frame_count,
                player.playback_type,
            );
        } else {
            player.current_frame =
                clamp_frame(player.current_frame, frame_count, player.playback_type);
            player.internal.playback_frame = player.current_frame as f32;
        }

        let binding_hash = hash_bindings(&player.bindings);
        let frame_changed = player.current_frame != previous_frame;
        let binding_changed = binding_hash != player.internal.last_binding_hash;
        let animation_changed = player.animation != player.internal.last_applied_animation;
        let frame_unapplied = player.current_frame != player.internal.last_applied_frame;
        let should_apply = animation_changed || frame_changed || binding_changed || frame_unapplied;

        if should_apply {
            player.internal.last_applied_animation = player.animation;
            player.internal.last_applied_frame = player.current_frame;
            player.internal.last_binding_hash = binding_hash;
        }

        AnimationStep {
            frame: player.current_frame,
            bindings: if should_apply {
                player.bindings.to_vec()
            } else {
                Vec::new()
            },
            should_apply,
        }
    })
}

fn apply_clip_frame<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    res: &ResourceContext<'_, impl ResourceAPI + ?Sized>,
    clip: &Arc<perro_animation::AnimationClip>,
    frame: u32,
    bindings: &[AnimationObjectBinding],
) where
    RT: RuntimeAPI + ?Sized,
{
    for track in clip.object_tracks.iter() {
        for binding in bindings
            .iter()
            .filter(|b| b.object.as_ref() == track.object.as_ref())
        {
            apply_track(ctx, res, binding.node, track, frame);
        }
    }

    apply_frame_events(ctx, clip, frame, bindings);
}

fn apply_frame_events<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    clip: &Arc<perro_animation::AnimationClip>,
    frame: u32,
    bindings: &[AnimationObjectBinding],
) where
    RT: RuntimeAPI + ?Sized,
{
    if clip.frame_events.is_empty() {
        return;
    }

    let binding_map: HashMap<&str, NodeID> = bindings
        .iter()
        .map(|b| (b.object.as_ref(), b.node))
        .collect();
    let object_type_map: HashMap<&str, &str> = clip
        .objects
        .iter()
        .map(|o| (o.name.as_ref(), o.node_type.as_ref()))
        .collect();

    for entry in clip
        .frame_events
        .iter()
        .filter(|entry| entry.frame == frame)
    {
        match &entry.event {
            AnimationEvent::EmitSignal { name, params } => {
                let values = resolve_event_params(ctx, params, &binding_map, &object_type_map);
                let signal_id = signal!(name.as_ref());
                let _ = signal_emit!(ctx, signal_id, &values);
            }
            AnimationEvent::SetVar { name, value } => {
                let Some(target) = scope_target_node(&entry.scope, &binding_map) else {
                    continue;
                };
                let resolved = resolve_animation_param(ctx, value, &binding_map, &object_type_map);
                set_var!(ctx, target, name.as_ref(), resolved);
            }
            AnimationEvent::CallMethod { name, params } => {
                let Some(target) = scope_target_node(&entry.scope, &binding_map) else {
                    continue;
                };
                let values = resolve_event_params(ctx, params, &binding_map, &object_type_map);
                let _ = call_method!(ctx, target, name.as_ref(), &values);
            }
        }
    }
}

fn scope_target_node(
    scope: &AnimationEventScope,
    binding_map: &HashMap<&str, NodeID>,
) -> Option<NodeID> {
    match scope {
        AnimationEventScope::Global => None,
        AnimationEventScope::Object(object) => binding_map.get(object.as_ref()).copied(),
    }
}

fn resolve_event_params<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    params: &[AnimationParam],
    binding_map: &HashMap<&str, NodeID>,
    object_type_map: &HashMap<&str, &str>,
) -> Vec<Variant>
where
    RT: RuntimeAPI + ?Sized,
{
    params
        .iter()
        .map(|param| resolve_animation_param(ctx, param, binding_map, object_type_map))
        .collect()
}

fn resolve_animation_param<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    param: &AnimationParam,
    binding_map: &HashMap<&str, NodeID>,
    object_type_map: &HashMap<&str, &str>,
) -> Variant
where
    RT: RuntimeAPI + ?Sized,
{
    match param {
        AnimationParam::Bool(v) => (*v).into(),
        AnimationParam::I32(v) => (*v).into(),
        AnimationParam::U32(v) => (*v).into(),
        AnimationParam::F32(v) => (*v).into(),
        AnimationParam::Vec2(v) => Vector2::new(v[0], v[1]).into(),
        AnimationParam::Vec3(v) => Vector3::new(v[0], v[1], v[2]).into(),
        AnimationParam::Vec4(v) => {
            let mut q = Quaternion::new(v[0], v[1], v[2], v[3]);
            q.normalize();
            q.into()
        }
        AnimationParam::String(v) => v.as_ref().into(),
        AnimationParam::Transform2D(v) => (*v).into(),
        AnimationParam::Transform3D(v) => (*v).into(),
        AnimationParam::ObjectNode(object) => binding_map
            .get(object.as_ref())
            .copied()
            .map(Variant::from)
            .unwrap_or(Variant::Null),
        AnimationParam::ObjectField { object, field } => {
            let Some(node_id) = binding_map.get(object.as_ref()).copied() else {
                return Variant::Null;
            };
            let Some(node_type_name) = object_type_map.get(object.as_ref()).copied() else {
                return Variant::Null;
            };
            let Some(field) = resolve_node_field(node_type_name, field.as_ref()) else {
                return Variant::Null;
            };
            read_node_field_variant(ctx, node_id, field).unwrap_or(Variant::Null)
        }
    }
}

fn read_node_field_variant<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    node_id: NodeID,
    field: NodeField,
) -> Option<Variant>
where
    RT: RuntimeAPI + ?Sized,
{
    match field {
        NodeField::Node2D(Node2DField::Position) => with_base_node!(ctx, Node2D, node_id, |node| {
            Variant::from(node.transform.position)
        }),
        NodeField::Node2D(Node2DField::Rotation) => with_base_node!(ctx, Node2D, node_id, |node| {
            Variant::from(node.transform.rotation)
        }),
        NodeField::Node2D(Node2DField::Scale) => with_base_node!(ctx, Node2D, node_id, |node| {
            Variant::from(node.transform.scale)
        }),
        NodeField::Node2D(Node2DField::Visible) => {
            with_base_node!(ctx, Node2D, node_id, |node| Variant::from(node.visible))
        }
        NodeField::Node2D(Node2DField::ZIndex) => {
            with_base_node!(ctx, Node2D, node_id, |node| Variant::from(node.z_index))
        }
        NodeField::Node3D(Node3DField::Position) => with_base_node!(ctx, Node3D, node_id, |node| {
            Variant::from(node.transform.position)
        }),
        NodeField::Node3D(Node3DField::Rotation) => with_base_node!(ctx, Node3D, node_id, |node| {
            Variant::from(node.transform.rotation)
        }),
        NodeField::Node3D(Node3DField::Scale) => with_base_node!(ctx, Node3D, node_id, |node| {
            Variant::from(node.transform.scale)
        }),
        NodeField::Node3D(Node3DField::Visible) => {
            with_base_node!(ctx, Node3D, node_id, |node| Variant::from(node.visible))
        }
        NodeField::Sprite2D(Sprite2DField::Texture) => {
            with_base_node!(ctx, Sprite2D, node_id, |node| Variant::from(node.texture))
        }
        NodeField::Camera3D(Camera3DField::Zoom) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| {
                match camera.projection {
                    perro_nodes::CameraProjection::Perspective { fov_y_degrees, .. } => {
                        Variant::from((60.0 / fov_y_degrees.max(0.001)).max(0.001))
                    }
                    _ => Variant::from(1.0_f32),
                }
            })
        }
        NodeField::Camera3D(Camera3DField::PerspectiveFovYDegrees) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Perspective { fov_y_degrees, .. } => {
                    Variant::from(fov_y_degrees)
                }
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::PerspectiveNear) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Perspective { near, .. } => Variant::from(near),
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::PerspectiveFar) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Perspective { far, .. } => Variant::from(far),
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::OrthographicSize) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Orthographic { size, .. } => Variant::from(size),
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::OrthographicNear) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Orthographic { near, .. } => Variant::from(near),
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::OrthographicFar) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Orthographic { far, .. } => Variant::from(far),
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::FrustumLeft) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Frustum { left, .. } => Variant::from(left),
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::FrustumRight) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Frustum { right, .. } => Variant::from(right),
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::FrustumBottom) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Frustum { bottom, .. } => Variant::from(bottom),
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::FrustumTop) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Frustum { top, .. } => Variant::from(top),
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::FrustumNear) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Frustum { near, .. } => Variant::from(near),
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::FrustumFar) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| match camera.projection {
                perro_nodes::CameraProjection::Frustum { far, .. } => Variant::from(far),
                _ => Variant::Null,
            })
        }
        NodeField::Camera3D(Camera3DField::Active) => {
            with_base_node!(ctx, Camera3D, node_id, |camera| Variant::from(
                camera.active
            ))
        }
        NodeField::Light3D(Light3DField::Color) => with_base_node!(ctx, RayLight3D, node_id, |n| {
            Variant::from(Vector3::new(n.color[0], n.color[1], n.color[2]))
        })
        .or_else(|| {
            with_base_node!(ctx, PointLight3D, node_id, |n| Variant::from(Vector3::new(
                n.color[0], n.color[1], n.color[2]
            )))
        })
        .or_else(|| {
            with_base_node!(ctx, SpotLight3D, node_id, |n| Variant::from(Vector3::new(
                n.color[0], n.color[1], n.color[2]
            )))
        })
        .or_else(|| {
            with_base_node!(ctx, AmbientLight3D, node_id, |n| Variant::from(
                Vector3::new(n.color[0], n.color[1], n.color[2])
            ))
        }),
        NodeField::Light3D(Light3DField::Intensity) => {
            with_base_node!(ctx, RayLight3D, node_id, |n| Variant::from(n.intensity))
                .or_else(|| {
                    with_base_node!(ctx, PointLight3D, node_id, |n| Variant::from(n.intensity))
                })
                .or_else(|| {
                    with_base_node!(ctx, SpotLight3D, node_id, |n| Variant::from(n.intensity))
                })
                .or_else(|| {
                    with_base_node!(ctx, AmbientLight3D, node_id, |n| Variant::from(n.intensity))
                })
        }
        NodeField::Light3D(Light3DField::Active) => {
            with_base_node!(ctx, RayLight3D, node_id, |n| { Variant::from(n.active) })
                .or_else(|| {
                    with_base_node!(ctx, PointLight3D, node_id, |n| Variant::from(n.active))
                })
                .or_else(|| with_base_node!(ctx, SpotLight3D, node_id, |n| Variant::from(n.active)))
                .or_else(|| {
                    with_base_node!(ctx, AmbientLight3D, node_id, |n| Variant::from(n.active))
                })
        }
        NodeField::PointLight3D(PointLight3DField::Range) => {
            with_base_node!(ctx, PointLight3D, node_id, |n| Variant::from(n.range))
        }
        NodeField::SpotLight3D(SpotLight3DField::Range) => {
            with_base_node!(ctx, SpotLight3D, node_id, |n| Variant::from(n.range))
        }
        NodeField::SpotLight3D(SpotLight3DField::InnerAngleRadians) => {
            with_base_node!(ctx, SpotLight3D, node_id, |n| Variant::from(
                n.inner_angle_radians
            ))
        }
        NodeField::SpotLight3D(SpotLight3DField::OuterAngleRadians) => {
            with_base_node!(ctx, SpotLight3D, node_id, |n| Variant::from(
                n.outer_angle_radians
            ))
        }
        _ => None,
    }
}

fn apply_track<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    res: &ResourceContext<'_, impl ResourceAPI + ?Sized>,
    node_id: NodeID,
    track: &AnimationObjectTrack,
    frame: u32,
) where
    RT: RuntimeAPI + ?Sized,
{
    let Some(value) = sample_track_value(track, frame) else {
        return;
    };

    if let Some(bone_target) = &track.bone_target {
        apply_skeleton_bone_track(ctx, node_id, bone_target, &value);
        return;
    }

    match track.field {
        NodeField::Node2D(Node2DField::Position)
        | NodeField::Node2D(Node2DField::Rotation)
        | NodeField::Node2D(Node2DField::Scale) => {
            if let AnimationTrackValue::Transform2D(value) = value {
                let _ = with_base_node_mut!(ctx, Node2D, node_id, |node| {
                    node.transform = value;
                });
            }
        }
        NodeField::Node2D(Node2DField::Visible) => {
            if let AnimationTrackValue::Bool(value) = value {
                let _ = with_base_node_mut!(ctx, Node2D, node_id, |node| {
                    node.visible = value;
                });
            }
        }
        NodeField::Node2D(Node2DField::ZIndex) => {
            if let Some(value) = as_i32_track(&value) {
                let _ = with_base_node_mut!(ctx, Node2D, node_id, |node| {
                    node.z_index = value;
                });
            }
        }
        NodeField::Node3D(Node3DField::Position)
        | NodeField::Node3D(Node3DField::Rotation)
        | NodeField::Node3D(Node3DField::Scale) => {
            if let AnimationTrackValue::Transform3D(value) = value {
                let _ = with_base_node_mut!(ctx, Node3D, node_id, |node| {
                    node.transform = value;
                });
            }
        }
        NodeField::Node3D(Node3DField::Visible) => {
            if let AnimationTrackValue::Bool(value) = value {
                let _ = with_base_node_mut!(ctx, Node3D, node_id, |node| {
                    node.visible = value;
                });
            }
        }
        NodeField::Sprite2D(Sprite2DField::Texture) => {
            if let AnimationTrackValue::AssetPath(path) = value {
                let id = texture_load!(res, path.as_ref());
                let _ = with_base_node_mut!(ctx, Sprite2D, node_id, |node| {
                    node.texture = id;
                });
            }
        }
        NodeField::MeshInstance3D(MeshInstance3DField::Mesh) => {
            if let AnimationTrackValue::AssetPath(path) = value {
                let id = mesh_load!(res, path.as_ref());
                let _ = with_base_node_mut!(ctx, MeshInstance3D, node_id, |node| {
                    node.mesh = id;
                });
            }
        }
        NodeField::MeshInstance3D(MeshInstance3DField::Material) => {
            if let AnimationTrackValue::AssetPath(path) = value {
                let id = material_load!(res, path.as_ref());
                let _ = with_base_node_mut!(ctx, MeshInstance3D, node_id, |node| {
                    if node.surfaces.is_empty() {
                        node.surfaces.push(perro_nodes::MeshSurfaceBinding::default());
                    }
                    node.surfaces[0].material = Some(id);
                });
            }
        }
        NodeField::Camera3D(channel) => {
            if let Some(v) = as_f32_track(&value) {
                let _ = with_base_node_mut!(ctx, Camera3D, node_id, |camera| match channel {
                    Camera3DField::Zoom => apply_camera_zoom(camera, v),
                    Camera3DField::PerspectiveFovYDegrees => {
                        if let perro_nodes::CameraProjection::Perspective {
                            fov_y_degrees, ..
                        } = &mut camera.projection
                        {
                            *fov_y_degrees = v.clamp(10.0, 120.0);
                        }
                    }
                    Camera3DField::PerspectiveNear => {
                        if let perro_nodes::CameraProjection::Perspective { near, far, .. } =
                            &mut camera.projection
                        {
                            *near = v.max(0.001);
                            if *far <= *near {
                                *far = *near + 0.001;
                            }
                        }
                    }
                    Camera3DField::PerspectiveFar => {
                        if let perro_nodes::CameraProjection::Perspective { near, far, .. } =
                            &mut camera.projection
                        {
                            *far = v.max(*near + 0.001);
                        }
                    }
                    Camera3DField::OrthographicSize => {
                        if let perro_nodes::CameraProjection::Orthographic { size, .. } =
                            &mut camera.projection
                        {
                            *size = v.abs().max(0.001);
                        }
                    }
                    Camera3DField::OrthographicNear => {
                        if let perro_nodes::CameraProjection::Orthographic { near, far, .. } =
                            &mut camera.projection
                        {
                            *near = v.max(0.001);
                            if *far <= *near {
                                *far = *near + 0.001;
                            }
                        }
                    }
                    Camera3DField::OrthographicFar => {
                        if let perro_nodes::CameraProjection::Orthographic { near, far, .. } =
                            &mut camera.projection
                        {
                            *far = v.max(*near + 0.001);
                        }
                    }
                    Camera3DField::FrustumLeft => {
                        if let perro_nodes::CameraProjection::Frustum { left, right, .. } =
                            &mut camera.projection
                        {
                            *left = v;
                            if *right <= *left {
                                *right = *left + 0.001;
                            }
                        }
                    }
                    Camera3DField::FrustumRight => {
                        if let perro_nodes::CameraProjection::Frustum { left, right, .. } =
                            &mut camera.projection
                        {
                            *right = v.max(*left + 0.001);
                        }
                    }
                    Camera3DField::FrustumBottom => {
                        if let perro_nodes::CameraProjection::Frustum { bottom, top, .. } =
                            &mut camera.projection
                        {
                            *bottom = v;
                            if *top <= *bottom {
                                *top = *bottom + 0.001;
                            }
                        }
                    }
                    Camera3DField::FrustumTop => {
                        if let perro_nodes::CameraProjection::Frustum { bottom, top, .. } =
                            &mut camera.projection
                        {
                            *top = v.max(*bottom + 0.001);
                        }
                    }
                    Camera3DField::FrustumNear => {
                        if let perro_nodes::CameraProjection::Frustum { near, far, .. } =
                            &mut camera.projection
                        {
                            *near = v.max(0.001);
                            if *far <= *near {
                                *far = *near + 0.001;
                            }
                        }
                    }
                    Camera3DField::FrustumFar => {
                        if let perro_nodes::CameraProjection::Frustum { near, far, .. } =
                            &mut camera.projection
                        {
                            *far = v.max(*near + 0.001);
                        }
                    }
                    Camera3DField::Active
                    | Camera3DField::Projection
                    | Camera3DField::PostProcessing => {}
                });
            }
            if matches!(channel, Camera3DField::Active)
                && let AnimationTrackValue::Bool(active) = value
            {
                let _ = with_base_node_mut!(ctx, Camera3D, node_id, |camera| {
                    camera.active = active;
                });
            }
        }
        NodeField::Light3D(channel) => match channel {
            Light3DField::Color => {
                if let AnimationTrackValue::Vec3(color) = value {
                    let c = color;
                    if with_base_node_mut!(ctx, RayLight3D, node_id, |n| n.color = c).is_none()
                        && with_base_node_mut!(ctx, PointLight3D, node_id, |n| n.color = c)
                            .is_none()
                        && with_base_node_mut!(ctx, SpotLight3D, node_id, |n| n.color = c).is_none()
                    {
                        let _ =
                            with_base_node_mut!(ctx, AmbientLight3D, node_id, |n| { n.color = c });
                    }
                }
            }
            Light3DField::Intensity => {
                if let Some(v) = as_f32_track(&value)
                    && with_base_node_mut!(ctx, RayLight3D, node_id, |n| n.intensity = v).is_none()
                    && with_base_node_mut!(ctx, PointLight3D, node_id, |n| n.intensity = v)
                        .is_none()
                    && with_base_node_mut!(ctx, SpotLight3D, node_id, |n| n.intensity = v).is_none()
                {
                    let _ =
                        with_base_node_mut!(ctx, AmbientLight3D, node_id, |n| { n.intensity = v });
                }
            }
            Light3DField::Active => {
                if let AnimationTrackValue::Bool(v) = value
                    && with_base_node_mut!(ctx, RayLight3D, node_id, |n| n.active = v).is_none()
                    && with_base_node_mut!(ctx, PointLight3D, node_id, |n| n.active = v).is_none()
                    && with_base_node_mut!(ctx, SpotLight3D, node_id, |n| n.active = v).is_none()
                {
                    let _ = with_base_node_mut!(ctx, AmbientLight3D, node_id, |n| { n.active = v });
                }
            }
        },
        NodeField::PointLight3D(PointLight3DField::Range) => {
            if let Some(v) = as_f32_track(&value) {
                let _ = with_base_node_mut!(ctx, PointLight3D, node_id, |node| {
                    node.range = v;
                });
            }
        }
        NodeField::SpotLight3D(channel) => {
            if let Some(v) = as_f32_track(&value) {
                let _ = with_base_node_mut!(ctx, SpotLight3D, node_id, |node| match channel {
                    SpotLight3DField::Range => node.range = v,
                    SpotLight3DField::InnerAngleRadians => node.inner_angle_radians = v,
                    SpotLight3DField::OuterAngleRadians => node.outer_angle_radians = v,
                });
            }
        }
        _ => {}
    }
}

fn apply_skeleton_bone_track<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    node_id: NodeID,
    bone_target: &perro_animation::AnimationBoneTarget,
    value: &AnimationTrackValue,
) where
    RT: RuntimeAPI + ?Sized,
{
    let AnimationTrackValue::Transform3D(rest) = value else {
        return;
    };
    let rest = *rest;
    let _ = with_base_node_mut!(ctx, Skeleton3D, node_id, |skeleton| {
        let bone = match &bone_target.selector {
            AnimationBoneSelector::Index(index) => skeleton.bones.get_mut(*index as usize),
            AnimationBoneSelector::Name(name) => skeleton
                .bones
                .iter_mut()
                .find(|bone| bone.name.as_ref() == name.as_ref()),
        };
        if let Some(bone) = bone {
            bone.rest = rest;
        }
    });
}

#[inline]
fn as_f32_track(value: &AnimationTrackValue) -> Option<f32> {
    match value {
        AnimationTrackValue::F32(v) => Some(*v),
        AnimationTrackValue::I32(v) => Some(*v as f32),
        AnimationTrackValue::U32(v) => Some(*v as f32),
        _ => None,
    }
}

#[inline]
fn as_i32_track(value: &AnimationTrackValue) -> Option<i32> {
    match value {
        AnimationTrackValue::I32(v) => Some(*v),
        AnimationTrackValue::U32(v) => i32::try_from(*v).ok(),
        AnimationTrackValue::F32(v) => {
            if v.is_finite() {
                let rounded = v.round();
                if rounded >= i32::MIN as f32 && rounded <= i32::MAX as f32 {
                    Some(rounded as i32)
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

fn apply_camera_zoom(camera: &mut Camera3D, zoom: f32) {
    let zoom = if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        1.0
    };
    let fov_y_degrees = (60.0 / zoom).clamp(10.0, 120.0);
    if let perro_nodes::CameraProjection::Perspective {
        fov_y_degrees: fov, ..
    } = &mut camera.projection
    {
        *fov = fov_y_degrees;
    }
}

fn sample_track_value(track: &AnimationObjectTrack, frame: u32) -> Option<AnimationTrackValue> {
    if track.keys.is_empty() {
        return None;
    }

    let mut prev_index = None::<usize>;
    let mut next_index = None::<usize>;
    for (index, key) in track.keys.iter().enumerate() {
        if key.frame <= frame {
            prev_index = Some(index);
        } else {
            next_index = Some(index);
            break;
        }
    }

    let prev_index = prev_index.or(Some(0))?;
    let prev_key = &track.keys[prev_index];
    let prev = &prev_key.value;

    let interpolation = prev_key.interpolation;
    let ease = prev_key.ease;
    match interpolation {
        AnimationInterpolation::Step => Some(prev.clone()),
        AnimationInterpolation::Linear => {
            let Some(next_index) = next_index else {
                return Some(prev.clone());
            };
            let next_key = &track.keys[next_index];
            let frame_span = next_key.frame.saturating_sub(prev_key.frame);
            if frame_span == 0 {
                return Some(prev.clone());
            }

            let local = frame.saturating_sub(prev_key.frame);
            let t = (local as f32 / frame_span as f32).clamp(0.0, 1.0);
            let t = ease_sample(ease, t);
            Some(interpolate_values(prev, &next_key.value, t))
        }
    }
}

#[inline]
fn ease_sample(ease: AnimationEase, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match ease {
        AnimationEase::Linear => t,
        AnimationEase::EaseIn => t * t,
        AnimationEase::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
        AnimationEase::EaseInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - ((-2.0 * t + 2.0) * (-2.0 * t + 2.0)) * 0.5
            }
        }
    }
}

fn interpolate_values(
    a: &AnimationTrackValue,
    b: &AnimationTrackValue,
    t: f32,
) -> AnimationTrackValue {
    match (a, b) {
        (AnimationTrackValue::F32(a), AnimationTrackValue::F32(b)) => {
            AnimationTrackValue::F32(lerp_f32(*a, *b, t))
        }
        (AnimationTrackValue::I32(a), AnimationTrackValue::I32(b)) => {
            AnimationTrackValue::I32(lerp_f32(*a as f32, *b as f32, t).round() as i32)
        }
        (AnimationTrackValue::U32(a), AnimationTrackValue::U32(b)) => {
            AnimationTrackValue::U32(lerp_f32(*a as f32, *b as f32, t).round().max(0.0) as u32)
        }
        (AnimationTrackValue::Vec2(a), AnimationTrackValue::Vec2(b)) => {
            AnimationTrackValue::Vec2([lerp_f32(a[0], b[0], t), lerp_f32(a[1], b[1], t)])
        }
        (AnimationTrackValue::Vec3(a), AnimationTrackValue::Vec3(b)) => {
            AnimationTrackValue::Vec3([
                lerp_f32(a[0], b[0], t),
                lerp_f32(a[1], b[1], t),
                lerp_f32(a[2], b[2], t),
            ])
        }
        (AnimationTrackValue::Vec4(a), AnimationTrackValue::Vec4(b)) => {
            AnimationTrackValue::Vec4([
                lerp_f32(a[0], b[0], t),
                lerp_f32(a[1], b[1], t),
                lerp_f32(a[2], b[2], t),
                lerp_f32(a[3], b[3], t),
            ])
        }
        (AnimationTrackValue::Transform2D(a), AnimationTrackValue::Transform2D(b)) => {
            let mut out = *a;
            out.position.x = lerp_f32(a.position.x, b.position.x, t);
            out.position.y = lerp_f32(a.position.y, b.position.y, t);
            out.rotation = lerp_f32(a.rotation, b.rotation, t);
            out.scale.x = lerp_f32(a.scale.x, b.scale.x, t);
            out.scale.y = lerp_f32(a.scale.y, b.scale.y, t);
            AnimationTrackValue::Transform2D(out)
        }
        (AnimationTrackValue::Transform3D(a), AnimationTrackValue::Transform3D(b)) => {
            let mut out = *a;
            out.position.x = lerp_f32(a.position.x, b.position.x, t);
            out.position.y = lerp_f32(a.position.y, b.position.y, t);
            out.position.z = lerp_f32(a.position.z, b.position.z, t);
            out.scale.x = lerp_f32(a.scale.x, b.scale.x, t);
            out.scale.y = lerp_f32(a.scale.y, b.scale.y, t);
            out.scale.z = lerp_f32(a.scale.z, b.scale.z, t);
            out.rotation = a.rotation.to_quat().slerp(b.rotation.to_quat(), t).into();
            AnimationTrackValue::Transform3D(out)
        }
        _ => a.clone(),
    }
}

#[inline]
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn advance_playback_frame(
    current_frame: f32,
    delta_frames: f32,
    frame_count: u32,
    playback_type: AnimationPlaybackType,
    boomerang_direction: &mut f32,
) -> f32 {
    if frame_count <= 1 {
        *boomerang_direction = 1.0;
        return 0.0;
    }

    match playback_type {
        AnimationPlaybackType::Boomerang => advance_boomerang_frame(
            current_frame,
            delta_frames,
            frame_count,
            boomerang_direction,
        ),
        _ => {
            *boomerang_direction = 1.0;
            let next = current_frame + delta_frames;
            normalize_playback_frame(next, frame_count, playback_type)
        }
    }
}

fn advance_boomerang_frame(
    current_frame: f32,
    delta_frames: f32,
    frame_count: u32,
    boomerang_direction: &mut f32,
) -> f32 {
    if frame_count <= 1 {
        *boomerang_direction = 1.0;
        return 0.0;
    }

    let mut dir = if boomerang_direction.is_sign_negative() {
        -1.0
    } else {
        1.0
    };
    let last = frame_count.saturating_sub(1) as f32;
    let mut next = current_frame + delta_frames * dir;

    while next > last {
        next = (2.0 * last) - next;
        dir *= -1.0;
    }
    while next < 0.0 {
        next = -next;
        dir *= -1.0;
    }

    *boomerang_direction = dir;
    next
}

fn playback_frame_to_frame(
    frame: f32,
    frame_count: u32,
    playback_type: AnimationPlaybackType,
) -> u32 {
    if frame_count <= 1 {
        return 0;
    }
    let normalized = normalize_playback_frame(frame, frame_count, playback_type);
    let discrete = normalized.max(0.0).floor() as u32;
    clamp_frame(discrete, frame_count, playback_type)
}

fn clamp_frame(frame: u32, frame_count: u32, playback_type: AnimationPlaybackType) -> u32 {
    if frame_count <= 1 {
        return 0;
    }
    let last = frame_count.saturating_sub(1);
    match playback_type {
        AnimationPlaybackType::Once => frame.min(last),
        AnimationPlaybackType::Loop => frame % frame_count,
        AnimationPlaybackType::Boomerang => {
            let period = last.saturating_mul(2);
            if period == 0 {
                return 0;
            }
            let pos = frame % period;
            if pos <= last { pos } else { period - pos }
        }
    }
}

fn normalize_playback_frame(
    frame: f32,
    frame_count: u32,
    playback_type: AnimationPlaybackType,
) -> f32 {
    if frame_count <= 1 {
        return 0.0;
    }
    let last = frame_count.saturating_sub(1) as f32;
    match playback_type {
        AnimationPlaybackType::Once => frame.clamp(0.0, last),
        AnimationPlaybackType::Loop => frame.rem_euclid(frame_count as f32),
        AnimationPlaybackType::Boomerang => {
            let period = last * 2.0;
            if period <= 0.0 {
                return 0.0;
            }
            let wrapped = frame.rem_euclid(period);
            if wrapped <= last {
                wrapped
            } else {
                period - wrapped
            }
        }
    }
}

fn hash_bindings(bindings: &[AnimationObjectBinding]) -> u64 {
    let mut h = 0x9E37_79B9_7F4A_7C15u64 ^ bindings.len() as u64;
    for binding in bindings {
        h ^= binding.node.as_u64();
        h = h.rotate_left(7).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        for b in binding.object.as_bytes() {
            h ^= *b as u64;
            h = h.rotate_left(5).wrapping_mul(0x94D0_49BB_1331_11EB);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boomerang_keeps_moving_after_turnaround() {
        let mut dir = 1.0_f32;
        let frame_count = 6_u32; // 0..5
        let mut frame = 0.0_f32;
        let mut sampled = Vec::new();

        for _ in 0..20 {
            frame = advance_playback_frame(
                frame,
                1.0,
                frame_count,
                AnimationPlaybackType::Boomerang,
                &mut dir,
            );
            sampled.push(playback_frame_to_frame(
                frame,
                frame_count,
                AnimationPlaybackType::Boomerang,
            ));
        }

        assert_eq!(
            sampled,
            vec![1, 2, 3, 4, 5, 4, 3, 2, 1, 0, 1, 2, 3, 4, 5, 4, 3, 2, 1, 0]
        );
    }
}

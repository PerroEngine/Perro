use super::*;

pub(in super::super) struct AnimationStep {
    pub(super) frame: u32,
    pub(super) bindings: Vec<AnimationObjectBinding>,
    pub(super) should_apply: bool,
    pub(super) event_frames: Vec<u32>,
}

pub(in super::super) fn bindings_fingerprint(bindings: &[AnimationObjectBinding]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bindings.len().hash(&mut hasher);
    for binding in bindings {
        binding.object.hash(&mut hasher);
        binding.node.hash(&mut hasher);
    }
    hasher.finish()
}

pub(in super::super) fn step_animation_player<RT>(
    ctx: &mut RuntimeWindow<'_, RT>,
    id: NodeID,
    animation_id: AnimationID,
    clip: &Arc<perro_animation::AnimationClip>,
    delta_seconds: f32,
) -> Option<AnimationStep>
where
    RT: RuntimeAPI + ?Sized,
{
    with_node_mut!(ctx, SelfNodeType, id, |player| {
        let previous_frame = player.current_frame;
        let previous_playback_frame = player.internal.playback_frame;
        let previous_boomerang_direction = player.internal.boomerang_direction;
        let frame_count = clip.frame_count();
        let delta_frames = delta_seconds * clip.fps.max(0.0) * player.speed;
        let mut event_frames = std::mem::take(&mut player.internal.event_frames_scratch);
        event_frames.clear();

        if !player.paused {
            player.internal.playback_frame = advance_playback_frame(
                player.internal.playback_frame,
                delta_frames,
                frame_count,
                player.playback_type,
                &mut player.internal.boomerang_direction,
            );
            crossed_animation_frames(
                previous_playback_frame,
                delta_frames,
                frame_count,
                player.playback_type,
                previous_boomerang_direction,
                &clip.frame_events,
                &mut event_frames,
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

        let binding_revision = player.bindings_revision;
        let binding_fingerprint = bindings_fingerprint(&player.bindings);
        let frame_changed = player.current_frame != previous_frame;
        let binding_changed = binding_revision != player.internal.last_binding_revision
            || binding_fingerprint != player.internal.last_binding_fingerprint;
        let animation_changed = animation_id != player.internal.last_applied_animation;
        let frame_unapplied = player.current_frame != player.internal.last_applied_frame;
        let should_apply = animation_changed || frame_changed || binding_changed || frame_unapplied;

        if (animation_changed || (player.paused && frame_unapplied))
            && event_frames.last().copied() != Some(player.current_frame)
        {
            event_frames.push(player.current_frame);
        }

        if should_apply {
            player.internal.last_applied_animation = animation_id;
            player.internal.last_applied_frame = player.current_frame;
            player.internal.last_binding_revision = binding_revision;
            player.internal.last_binding_fingerprint = binding_fingerprint;
        }

        let bindings = if should_apply || !event_frames.is_empty() {
            let mut scratch = std::mem::take(&mut player.internal.bindings_scratch);
            scratch.clear();
            scratch.extend_from_slice(&player.bindings);
            scratch
        } else {
            Vec::new()
        };

        AnimationStep {
            frame: player.current_frame,
            bindings,
            should_apply,
            event_frames,
        }
    })
}

pub(in super::super) fn apply_clip_frame<RT>(
    ctx: &mut RuntimeWindow<'_, RT>,
    res: &ResourceWindow<'_, impl ResourceAPI + ?Sized>,
    clip: &Arc<perro_animation::AnimationClip>,
    frame: u32,
    bindings: &[AnimationObjectBinding],
    applied_transforms: &mut Vec<AppliedAnimationTransform>,
) where
    RT: RuntimeAPI + ?Sized,
{
    let mut has_bone_tracks = false;
    for track in clip.object_tracks.iter() {
        if track.bone_target.is_some() {
            // Bone tracks share one skeleton node; apply them in one borrow
            // below instead of one per bone (see `apply_bone_tracks_batched`).
            has_bone_tracks = true;
            continue;
        }
        for binding in bindings
            .iter()
            .filter(|b| b.object.as_ref() == track.object.as_ref())
        {
            apply_track(ctx, res, binding.node, track, frame, applied_transforms);
        }
    }

    if has_bone_tracks {
        apply_bone_tracks_batched(ctx, clip, frame, bindings);
    }
}

/// Apply every bone track of a skeleton under a single mutable borrow and a
/// single `force_rerender`. A humanoid clip drives ~60 bone tracks that all
/// target the same skeleton node; the per-track path (`apply_skeleton_bone_track`)
/// re-borrows the node and re-flags a rerender for each, so batching collapses
/// N heavy borrows + N subtree rerender walks down to 1 each per skeleton.
pub(in super::super) fn apply_bone_tracks_batched<RT>(
    ctx: &mut RuntimeWindow<'_, RT>,
    clip: &Arc<perro_animation::AnimationClip>,
    frame: u32,
    bindings: &[AnimationObjectBinding],
) where
    RT: RuntimeAPI + ?Sized,
{
    for binding in bindings {
        let object = binding.object.as_ref();
        // `with_base_node_mut` only runs the closure on a concrete-type match,
        // so a non-skeleton (or wrong-dimension) node yields `None` and falls
        // through. 3D first, then 2D.
        let applied_3d = with_base_node_mut!(ctx, Skeleton3D, binding.node, |skeleton| {
            let mut any = false;
            for track in clip.object_tracks.iter() {
                let Some(bone_target) = &track.bone_target else {
                    continue;
                };
                if track.object.as_ref() != object {
                    continue;
                }
                if let Some(AnimationTrackValue::Transform3D(pose)) =
                    sample_track_value(track, frame)
                    && apply_bone_pose_3d(skeleton, bone_target, pose, track.transform3d_mask)
                {
                    any = true;
                }
            }
            any
        });
        if let Some(any) = applied_3d {
            if any {
                let _ = ctx.Nodes().force_rerender(binding.node);
            }
            continue;
        }

        let applied_2d = with_base_node_mut!(ctx, Skeleton2D, binding.node, |skeleton| {
            let mut any = false;
            for track in clip.object_tracks.iter() {
                let Some(bone_target) = &track.bone_target else {
                    continue;
                };
                if track.object.as_ref() != object {
                    continue;
                }
                if let Some(AnimationTrackValue::Transform2D(pose)) =
                    sample_track_value(track, frame)
                    && apply_bone_pose_2d(skeleton, bone_target, pose, track.transform2d_mask)
                {
                    any = true;
                }
            }
            any
        });
        if applied_2d == Some(true) {
            let _ = ctx.Nodes().force_rerender(binding.node);
        }
    }
}

/// Cheap presence check used to skip building the event-resolution maps in
/// `apply_frame_events` when nothing fires on `frame`. Split out as its own
/// function so it can be unit-tested without a `RuntimeWindow`.
#[inline]
pub(in super::super) fn frame_has_event(
    frame_events: &[perro_animation::AnimationFrameEvent],
    frame: u32,
) -> bool {
    frame_events.iter().any(|entry| entry.frame == frame)
}

pub(in super::super) fn apply_frame_events<RT>(
    ctx: &mut RuntimeWindow<'_, RT>,
    clip: &Arc<perro_animation::AnimationClip>,
    frame: u32,
    bindings: &[AnimationObjectBinding],
) where
    RT: RuntimeAPI + ?Sized,
{
    // Avoid building the binding/object-type maps when nothing on this
    // frame actually fires — the common case for most advanced frames.
    if !frame_has_event(&clip.frame_events, frame) {
        return;
    }

    let binding_map: HashMap<&str, NodeID> = bindings
        .iter()
        .map(|b| (b.object.as_ref(), b.node))
        .collect();
    let object_type_map: HashMap<&str, &str> = clip
        .objects
        .iter()
        .map(|o| (o.name.as_ref(), o.node_type.as_str()))
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

pub(in super::super) fn scope_target_node(
    scope: &AnimationEventScope,
    binding_map: &HashMap<&str, NodeID>,
) -> Option<NodeID> {
    match scope {
        AnimationEventScope::Global => None,
        AnimationEventScope::Object(object) => binding_map.get(object.as_ref()).copied(),
    }
}

pub(in super::super) fn resolve_event_params<RT>(
    ctx: &mut RuntimeWindow<'_, RT>,
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

pub(in super::super) fn resolve_animation_param<RT>(
    ctx: &mut RuntimeWindow<'_, RT>,
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

pub(in super::super) fn read_node_field_variant<RT>(
    ctx: &mut RuntimeWindow<'_, RT>,
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
            Variant::from(Vector3::new(n.color.r(), n.color.g(), n.color.b()))
        })
        .or_else(|| {
            with_base_node!(ctx, PointLight3D, node_id, |n| Variant::from(Vector3::new(
                n.color.r(),
                n.color.g(),
                n.color.b()
            )))
        })
        .or_else(|| {
            with_base_node!(ctx, SpotLight3D, node_id, |n| Variant::from(Vector3::new(
                n.color.r(),
                n.color.g(),
                n.color.b()
            )))
        })
        .or_else(|| {
            with_base_node!(ctx, AmbientLight3D, node_id, |n| Variant::from(
                Vector3::new(n.color.r(), n.color.g(), n.color.b())
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
        NodeField::Light3D(Light3DField::CastShadows) => {
            with_base_node!(ctx, RayLight3D, node_id, |n| {
                Variant::from(n.cast_shadows)
            })
            .or_else(|| {
                with_base_node!(ctx, PointLight3D, node_id, |n| Variant::from(
                    n.cast_shadows
                ))
            })
            .or_else(|| {
                with_base_node!(ctx, SpotLight3D, node_id, |n| Variant::from(n.cast_shadows))
            })
            .or_else(|| {
                with_base_node!(ctx, AmbientLight3D, node_id, |n| Variant::from(
                    n.cast_shadows
                ))
            })
        }
        NodeField::Light3D(Light3DField::ShadowStrength) => {
            with_base_node!(ctx, RayLight3D, node_id, |n| Variant::from(
                n.shadow_strength
            ))
            .or_else(|| {
                with_base_node!(ctx, PointLight3D, node_id, |n| Variant::from(
                    n.shadow_strength
                ))
            })
            .or_else(|| {
                with_base_node!(ctx, SpotLight3D, node_id, |n| Variant::from(
                    n.shadow_strength
                ))
            })
        }
        NodeField::Light3D(Light3DField::ShadowDepthBias) => {
            with_base_node!(ctx, RayLight3D, node_id, |n| Variant::from(
                n.shadow_depth_bias
            ))
            .or_else(|| {
                with_base_node!(ctx, PointLight3D, node_id, |n| Variant::from(
                    n.shadow_depth_bias
                ))
            })
            .or_else(|| {
                with_base_node!(ctx, SpotLight3D, node_id, |n| Variant::from(
                    n.shadow_depth_bias
                ))
            })
        }
        NodeField::Light3D(Light3DField::ShadowNormalBias) => {
            with_base_node!(ctx, RayLight3D, node_id, |n| Variant::from(
                n.shadow_normal_bias
            ))
            .or_else(|| {
                with_base_node!(ctx, PointLight3D, node_id, |n| Variant::from(
                    n.shadow_normal_bias
                ))
            })
            .or_else(|| {
                with_base_node!(ctx, SpotLight3D, node_id, |n| Variant::from(
                    n.shadow_normal_bias
                ))
            })
        }
        NodeField::Light3D(Light3DField::Shadow) => None,
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

pub(in super::super) fn apply_track<RT>(
    ctx: &mut RuntimeWindow<'_, RT>,
    res: &ResourceWindow<'_, impl ResourceAPI + ?Sized>,
    node_id: NodeID,
    track: &AnimationObjectTrack,
    frame: u32,
    applied_transforms: &mut Vec<AppliedAnimationTransform>,
) where
    RT: RuntimeAPI + ?Sized,
{
    let Some(value) = sample_track_value(track, frame) else {
        return;
    };

    if let Some(bone_target) = &track.bone_target {
        apply_skeleton_bone_track(ctx, node_id, bone_target, track, &value);
        return;
    }

    match track.field {
        NodeField::Node2D(Node2DField::Position)
        | NodeField::Node2D(Node2DField::Rotation)
        | NodeField::Node2D(Node2DField::Scale) => {
            if let AnimationTrackValue::Transform2D(value) = value {
                let _ = with_base_node_mut!(ctx, Node2D, node_id, |node| {
                    let previous = previous_transform_2d(applied_transforms, node_id);
                    node.transform = apply_transform_offset_2d(
                        node.transform,
                        previous,
                        value,
                        track.transform2d_mask,
                    );
                    save_transform_2d(applied_transforms, node_id, value);
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
                    let previous = previous_transform_3d(applied_transforms, node_id);
                    node.transform = apply_transform_offset_3d(
                        node.transform,
                        previous,
                        value,
                        track.transform3d_mask,
                    );
                    save_transform_3d(applied_transforms, node_id, value);
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
                        node.surfaces
                            .push(perro_nodes::MeshSurfaceBinding::default());
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
                    | Camera3DField::RenderMask
                    | Camera3DField::Projection
                    | Camera3DField::PostProcessing
                    | Camera3DField::AudioOptions
                    | Camera3DField::AudioMask => {}
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
                    let c = Color::rgb(color[0], color[1], color[2]);
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
            Light3DField::CastShadows => {
                if let AnimationTrackValue::Bool(v) = value
                    && with_base_node_mut!(ctx, RayLight3D, node_id, |n| n.cast_shadows = v)
                        .is_none()
                    && with_base_node_mut!(ctx, PointLight3D, node_id, |n| n.cast_shadows = v)
                        .is_none()
                    && with_base_node_mut!(ctx, SpotLight3D, node_id, |n| n.cast_shadows = v)
                        .is_none()
                {
                    let _ = with_base_node_mut!(ctx, AmbientLight3D, node_id, |n| {
                        n.cast_shadows = v
                    });
                }
            }
            Light3DField::ShadowStrength => {
                if let Some(v) = as_f32_track(&value)
                    && with_base_node_mut!(ctx, RayLight3D, node_id, |n| n.shadow_strength = v)
                        .is_none()
                    && with_base_node_mut!(ctx, PointLight3D, node_id, |n| n.shadow_strength = v)
                        .is_none()
                {
                    let _ =
                        with_base_node_mut!(ctx, SpotLight3D, node_id, |n| n.shadow_strength = v);
                }
            }
            Light3DField::ShadowDepthBias => {
                if let Some(v) = as_f32_track(&value)
                    && with_base_node_mut!(ctx, RayLight3D, node_id, |n| n.shadow_depth_bias = v)
                        .is_none()
                    && with_base_node_mut!(ctx, PointLight3D, node_id, |n| n.shadow_depth_bias = v)
                        .is_none()
                {
                    let _ =
                        with_base_node_mut!(ctx, SpotLight3D, node_id, |n| n.shadow_depth_bias = v);
                }
            }
            Light3DField::ShadowNormalBias => {
                if let Some(v) = as_f32_track(&value)
                    && with_base_node_mut!(ctx, RayLight3D, node_id, |n| n.shadow_normal_bias = v)
                        .is_none()
                    && with_base_node_mut!(ctx, PointLight3D, node_id, |n| n.shadow_normal_bias = v)
                        .is_none()
                {
                    let _ = with_base_node_mut!(ctx, SpotLight3D, node_id, |n| {
                        n.shadow_normal_bias = v
                    });
                }
            }
            Light3DField::Shadow => {}
            Light3DField::RenderLayers => {}
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

pub(in super::super) fn previous_transform_2d(
    applied_transforms: &[AppliedAnimationTransform],
    node_id: NodeID,
) -> Transform2D {
    applied_transforms
        .iter()
        .find(|entry| entry.node == node_id && entry.kind == AppliedAnimationTransformKind::Node2D)
        .map(|entry| entry.transform_2d)
        .unwrap_or(Transform2D::IDENTITY)
}

pub(in super::super) fn previous_transform_3d(
    applied_transforms: &[AppliedAnimationTransform],
    node_id: NodeID,
) -> Transform3D {
    applied_transforms
        .iter()
        .find(|entry| entry.node == node_id && entry.kind == AppliedAnimationTransformKind::Node3D)
        .map(|entry| entry.transform_3d)
        .unwrap_or(Transform3D::IDENTITY)
}

pub(in super::super) fn save_transform_2d(
    applied_transforms: &mut Vec<AppliedAnimationTransform>,
    node_id: NodeID,
    transform: Transform2D,
) {
    if let Some(entry) = applied_transforms
        .iter_mut()
        .find(|entry| entry.node == node_id && entry.kind == AppliedAnimationTransformKind::Node2D)
    {
        entry.transform_2d = transform;
    } else {
        applied_transforms.push(AppliedAnimationTransform {
            node: node_id,
            kind: AppliedAnimationTransformKind::Node2D,
            transform_2d: transform,
            transform_3d: Transform3D::IDENTITY,
        });
    }
}

pub(in super::super) fn save_transform_3d(
    applied_transforms: &mut Vec<AppliedAnimationTransform>,
    node_id: NodeID,
    transform: Transform3D,
) {
    if let Some(entry) = applied_transforms
        .iter_mut()
        .find(|entry| entry.node == node_id && entry.kind == AppliedAnimationTransformKind::Node3D)
    {
        entry.transform_3d = transform;
    } else {
        applied_transforms.push(AppliedAnimationTransform {
            node: node_id,
            kind: AppliedAnimationTransformKind::Node3D,
            transform_2d: Transform2D::IDENTITY,
            transform_3d: transform,
        });
    }
}

pub(in super::super) fn apply_transform_offset_2d(
    current: Transform2D,
    previous: Transform2D,
    next: Transform2D,
    authored_mask: u8,
) -> Transform2D {
    let mut out = current;
    if authored_mask & ANIMATION_TRANSFORM_MASK_POSITION != 0 {
        out.position = current.position - previous.position + next.position;
    }
    if authored_mask & ANIMATION_TRANSFORM_MASK_ROTATION != 0 {
        out.rotation = current.rotation - previous.rotation + next.rotation;
    }
    if authored_mask & ANIMATION_TRANSFORM_MASK_SCALE != 0 {
        out.scale = Vector2::new(
            scale_channel(current.scale.x, previous.scale.x, next.scale.x),
            scale_channel(current.scale.y, previous.scale.y, next.scale.y),
        );
    }
    out
}

pub(in super::super) fn apply_transform_offset_3d(
    current: Transform3D,
    previous: Transform3D,
    next: Transform3D,
    authored_mask: u8,
) -> Transform3D {
    let mut out = current;
    if authored_mask & ANIMATION_TRANSFORM_MASK_POSITION != 0 {
        out.position = current.position - previous.position + next.position;
    }
    if authored_mask & ANIMATION_TRANSFORM_MASK_ROTATION != 0 {
        out.rotation =
            (current.rotation * previous.rotation.inverse() * next.rotation).normalized();
    }
    if authored_mask & ANIMATION_TRANSFORM_MASK_SCALE != 0 {
        out.scale = Vector3::new(
            scale_channel(current.scale.x, previous.scale.x, next.scale.x),
            scale_channel(current.scale.y, previous.scale.y, next.scale.y),
            scale_channel(current.scale.z, previous.scale.z, next.scale.z),
        );
    }
    out
}

#[inline]
pub(in super::super) fn scale_channel(current: f32, previous: f32, next: f32) -> f32 {
    if previous.abs() <= f32::EPSILON {
        next
    } else {
        current / previous * next
    }
}

pub(in super::super) fn apply_skeleton_bone_track<RT>(
    ctx: &mut RuntimeWindow<'_, RT>,
    node_id: NodeID,
    bone_target: &perro_animation::AnimationBoneTarget,
    track: &AnimationObjectTrack,
    value: &AnimationTrackValue,
) where
    RT: RuntimeAPI + ?Sized,
{
    let applied = match value {
        AnimationTrackValue::Transform2D(pose) => {
            let pose = *pose;
            with_base_node_mut!(ctx, Skeleton2D, node_id, |skeleton| {
                apply_bone_pose_2d(skeleton, bone_target, pose, track.transform2d_mask);
            })
        }
        AnimationTrackValue::Transform3D(pose) => {
            let pose = *pose;
            with_base_node_mut!(ctx, Skeleton3D, node_id, |skeleton| {
                apply_bone_pose_3d(skeleton, bone_target, pose, track.transform3d_mask);
            })
        }
        _ => return,
    };
    if applied.is_some() {
        let _ = ctx.Nodes().force_rerender(node_id);
    }
}

pub(in super::super) fn apply_bone_pose_2d(
    skeleton: &mut Skeleton2D,
    bone_target: &perro_animation::AnimationBoneTarget,
    pose: Transform2D,
    authored_mask: u8,
) -> bool {
    let bone = match &bone_target.selector {
        AnimationBoneSelector::Index(index) => skeleton.bones.get_mut(*index as usize),
        AnimationBoneSelector::Name(name) => skeleton
            .bones
            .iter_mut()
            .find(|bone| bone.name.as_ref() == name.as_ref()),
    };
    if let Some(bone) = bone {
        let mut merged = bone.rest;
        if authored_mask & ANIMATION_TRANSFORM_MASK_POSITION != 0 {
            merged.position = pose.position;
        }
        if authored_mask & ANIMATION_TRANSFORM_MASK_ROTATION != 0 {
            merged.rotation = bone.rest.rotation + pose.rotation;
        }
        if authored_mask & ANIMATION_TRANSFORM_MASK_SCALE != 0 {
            merged.scale = pose.scale;
        }
        bone.pose = merged;
        return true;
    }
    false
}

pub(in super::super) fn apply_bone_pose_3d(
    skeleton: &mut Skeleton3D,
    bone_target: &perro_animation::AnimationBoneTarget,
    pose: Transform3D,
    authored_mask: u8,
) -> bool {
    let bone = match &bone_target.selector {
        AnimationBoneSelector::Index(index) => skeleton.bones.get_mut(*index as usize),
        AnimationBoneSelector::Name(name) => skeleton
            .bones
            .iter_mut()
            .find(|bone| bone.name.as_ref() == name.as_ref()),
    };
    if let Some(bone) = bone {
        let mut merged = bone.rest;
        if authored_mask & ANIMATION_TRANSFORM_MASK_POSITION != 0 {
            merged.position = pose.position;
        }
        if authored_mask & ANIMATION_TRANSFORM_MASK_ROTATION != 0 {
            merged.rotation = (bone.rest.rotation * pose.rotation).normalized();
        }
        if authored_mask & ANIMATION_TRANSFORM_MASK_SCALE != 0 {
            merged.scale = pose.scale;
        }
        bone.pose = merged;
        return true;
    }
    false
}

#[inline]
pub(in super::super) fn as_f32_track(value: &AnimationTrackValue) -> Option<f32> {
    match value {
        AnimationTrackValue::F32(v) => Some(*v),
        AnimationTrackValue::I32(v) => Some(*v as f32),
        AnimationTrackValue::U32(v) => Some(*v as f32),
        _ => None,
    }
}

#[inline]
pub(in super::super) fn as_i32_track(value: &AnimationTrackValue) -> Option<i32> {
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

pub(in super::super) fn apply_camera_zoom(camera: &mut Camera3D, zoom: f32) {
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

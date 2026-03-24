struct ObjectState2D {
    transform: Transform2D,
}

#[derive(Clone, Debug)]
struct ObjectState3D {
    transform: Transform3D,
}

struct TrackAccumulator {
    field: NodeField,
    bone_target: Option<AnimationBoneTarget>,
    interpolation: AnimationInterpolation,
    ease: AnimationEase,
    keys: BTreeMap<u32, TrackKey>,
}

#[derive(Clone, Debug)]
struct TrackKey {
    value: AnimationTrackValue,
    interpolation: AnimationInterpolation,
    ease: AnimationEase,
    transform2d_mask: u8,
    transform3d_mask: u8,
}

const MASK_POS_2D: u8 = 1 << 0;
const MASK_ROT_2D: u8 = 1 << 1;
const MASK_SCALE_2D: u8 = 1 << 2;

const MASK_POS_3D: u8 = 1 << 0;
const MASK_ROT_3D: u8 = 1 << 1;
const MASK_SCALE_3D: u8 = 1 << 2;

fn build_tracks_and_events(
    mut actions: Vec<FrameAction>,
    _object_types: &HashMap<String, String>,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) -> Result<(Vec<AnimationObjectTrack>, Vec<AnimationFrameEvent>), String> {
    let mut frame_events = Vec::<AnimationFrameEvent>::new();

    let mut fields = Vec::<(u32, usize, String, ObjectFieldAction)>::new();
    let mut controls = Vec::<(
        u32,
        usize,
        String,
        String,
        NodeField,
        Option<AnimationBoneTarget>,
        Option<AnimationInterpolation>,
        Option<AnimationEase>,
    )>::new();
    for (sequence, action) in actions.drain(..).enumerate() {
        match action {
            FrameAction::Field {
                frame,
                object,
                field,
            } => fields.push((frame, sequence, object, field)),
            FrameAction::TrackControl {
                frame,
                object,
                channel_key,
                field,
                bone_target,
                interpolation,
                ease,
            } => controls.push((
                frame,
                sequence,
                object,
                channel_key,
                field,
                bone_target,
                interpolation,
                ease,
            )),
            FrameAction::Event {
                frame,
                scope,
                event,
            } => frame_events.push(AnimationFrameEvent {
                frame,
                scope,
                event,
            }),
        }
    }

    fields.sort_by_key(|(frame, sequence, ..)| (*frame, *sequence));
    controls.sort_by_key(|(frame, sequence, ..)| (*frame, *sequence));

    let mut state_2d = HashMap::<String, ObjectState2D>::new();
    let mut state_3d = HashMap::<String, ObjectState3D>::new();
    let mut bone_state_3d = HashMap::<(String, String), ObjectState3D>::new();
    let mut tracks_map = BTreeMap::<(String, String), TrackAccumulator>::new();
    let mut control_index = 0usize;

    for (frame, sequence, object, field) in fields {
        while control_index < controls.len() {
            let (c_frame, c_seq, c_object, channel_key, c_field, c_bone_target, interp, ease) =
                &controls[control_index];
            if *c_frame > frame || (*c_frame == frame && *c_seq > sequence) {
                break;
            }
            apply_track_control(
                &mut tracks_map,
                c_object.clone(),
                channel_key,
                *c_field,
                c_bone_target.clone(),
                *interp,
                *ease,
                default_interpolation,
                default_ease,
            );
            control_index += 1;
        }

        match field {
            ObjectFieldAction::Node2D(action) => {
                apply_node_2d_action(
                    frame,
                    object,
                    action,
                    &mut state_2d,
                    &mut tracks_map,
                    default_interpolation,
                    default_ease,
                );
            }
            ObjectFieldAction::Node3D(action) => {
                apply_node_3d_action(
                    frame,
                    object,
                    action,
                    &mut state_3d,
                    &mut tracks_map,
                    default_interpolation,
                    default_ease,
                );
            }
            ObjectFieldAction::SkeletonBone(action) => {
                apply_skeleton_bone_action(
                    frame,
                    object,
                    action,
                    &mut bone_state_3d,
                    &mut tracks_map,
                    default_interpolation,
                    default_ease,
                );
            }
            ObjectFieldAction::Sprite2D(action) => {
                apply_sprite_2d_action(
                    frame,
                    object,
                    action,
                    &mut tracks_map,
                    default_interpolation,
                    default_ease,
                );
            }
            ObjectFieldAction::MeshInstance3D(action) => {
                apply_mesh_instance_3d_action(
                    frame,
                    object,
                    action,
                    &mut tracks_map,
                    default_interpolation,
                    default_ease,
                );
            }
            ObjectFieldAction::Camera3D(action) => {
                apply_camera_3d_action(
                    frame,
                    object,
                    action,
                    &mut tracks_map,
                    default_interpolation,
                    default_ease,
                );
            }
            ObjectFieldAction::Light3D(action) => {
                apply_light_3d_action(
                    frame,
                    object,
                    action,
                    &mut tracks_map,
                    default_interpolation,
                    default_ease,
                );
            }
            ObjectFieldAction::PointLight3D(action) => {
                apply_point_light_3d_action(
                    frame,
                    object,
                    action,
                    &mut tracks_map,
                    default_interpolation,
                    default_ease,
                );
            }
            ObjectFieldAction::SpotLight3D(action) => {
                apply_spot_light_3d_action(
                    frame,
                    object,
                    action,
                    &mut tracks_map,
                    default_interpolation,
                    default_ease,
                );
            }
        }
    }

    while control_index < controls.len() {
        let (_, _, object, channel_key, field, bone_target, interpolation, ease) =
            &controls[control_index];
        apply_track_control(
            &mut tracks_map,
            object.clone(),
            channel_key,
            *field,
            bone_target.clone(),
            *interpolation,
            *ease,
            default_interpolation,
            default_ease,
        );
        control_index += 1;
    }

    resolve_sparse_transform_components(&mut tracks_map);

    let mut object_tracks = Vec::<AnimationObjectTrack>::new();
    for ((object, _), track) in tracks_map {
        let mut keys = Vec::<AnimationObjectKey>::new();
        for (frame, key) in track.keys {
            keys.push(AnimationObjectKey {
                frame,
                interpolation: key.interpolation,
                ease: key.ease,
                value: key.value,
            });
        }
        object_tracks.push(AnimationObjectTrack {
            object: object.into(),
            field: track.field,
            bone_target: track.bone_target,
            interpolation: track.interpolation,
            ease: track.ease,
            keys: Cow::Owned(keys),
        });
    }

    object_tracks.sort_by(|a, b| a.object.as_ref().cmp(b.object.as_ref()));
    Ok((object_tracks, frame_events))
}

fn apply_node_2d_action(
    frame: u32,
    object: String,
    action: Node2DAction,
    state_2d: &mut HashMap<String, ObjectState2D>,
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    match action {
        Node2DAction::Position(v) => {
            let state = state_2d
                .entry(object.clone())
                .or_insert_with(default_object_state_2d);
            state.transform.position = v;
            insert_track_key(
                tracks_map,
                object,
                "node2d.transform",
                NodeField::Node2D(Node2DField::Position),
                frame,
                AnimationTrackValue::Transform2D(state.transform),
                MASK_POS_2D,
                0,
                default_interpolation,
                default_ease,
            );
        }
        Node2DAction::Rotation(v) => {
            let state = state_2d
                .entry(object.clone())
                .or_insert_with(default_object_state_2d);
            state.transform.rotation = v;
            insert_track_key(
                tracks_map,
                object,
                "node2d.transform",
                NodeField::Node2D(Node2DField::Position),
                frame,
                AnimationTrackValue::Transform2D(state.transform),
                MASK_ROT_2D,
                0,
                default_interpolation,
                default_ease,
            );
        }
        Node2DAction::Scale(v) => {
            let state = state_2d
                .entry(object.clone())
                .or_insert_with(default_object_state_2d);
            state.transform.scale = v;
            insert_track_key(
                tracks_map,
                object,
                "node2d.transform",
                NodeField::Node2D(Node2DField::Position),
                frame,
                AnimationTrackValue::Transform2D(state.transform),
                MASK_SCALE_2D,
                0,
                default_interpolation,
                default_ease,
            );
        }
        Node2DAction::Visible(v) => {
            insert_track_key(
                tracks_map,
                object,
                "node2d.visible",
                NodeField::Node2D(Node2DField::Visible),
                frame,
                AnimationTrackValue::Bool(v),
                0,
                0,
                default_interpolation,
                default_ease,
            );
        }
        Node2DAction::ZIndex(v) => {
            insert_track_key(
                tracks_map,
                object,
                "node2d.z_index",
                NodeField::Node2D(Node2DField::ZIndex),
                frame,
                AnimationTrackValue::I32(v),
                0,
                0,
                default_interpolation,
                default_ease,
            );
        }
    }
}

fn apply_node_3d_action(
    frame: u32,
    object: String,
    action: Node3DAction,
    state_3d: &mut HashMap<String, ObjectState3D>,
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    match action {
        Node3DAction::Position(v) => {
            let state = state_3d
                .entry(object.clone())
                .or_insert_with(default_object_state_3d);
            state.transform.position = v;
            insert_track_key(
                tracks_map,
                object,
                "node3d.transform",
                NodeField::Node3D(Node3DField::Position),
                frame,
                AnimationTrackValue::Transform3D(state.transform),
                0,
                MASK_POS_3D,
                default_interpolation,
                default_ease,
            );
        }
        Node3DAction::Rotation(v) => {
            let state = state_3d
                .entry(object.clone())
                .or_insert_with(default_object_state_3d);
            state.transform.rotation = v;
            insert_track_key(
                tracks_map,
                object,
                "node3d.transform",
                NodeField::Node3D(Node3DField::Position),
                frame,
                AnimationTrackValue::Transform3D(state.transform),
                0,
                MASK_ROT_3D,
                default_interpolation,
                default_ease,
            );
        }
        Node3DAction::Scale(v) => {
            let state = state_3d
                .entry(object.clone())
                .or_insert_with(default_object_state_3d);
            state.transform.scale = v;
            insert_track_key(
                tracks_map,
                object,
                "node3d.transform",
                NodeField::Node3D(Node3DField::Position),
                frame,
                AnimationTrackValue::Transform3D(state.transform),
                0,
                MASK_SCALE_3D,
                default_interpolation,
                default_ease,
            );
        }
        Node3DAction::Visible(v) => {
            insert_track_key(
                tracks_map,
                object,
                "node3d.visible",
                NodeField::Node3D(Node3DField::Visible),
                frame,
                AnimationTrackValue::Bool(v),
                0,
                0,
                default_interpolation,
                default_ease,
            );
        }
    }
}

fn apply_skeleton_bone_action(
    frame: u32,
    object: String,
    action: SkeletonBoneAction,
    bone_state_3d: &mut HashMap<(String, String), ObjectState3D>,
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    match action {
        SkeletonBoneAction::Position(selector, value) => {
            let selector_key = bone_selector_key(&selector);
            let state = bone_state_3d
                .entry((object.clone(), selector_key.clone()))
                .or_insert_with(default_object_state_3d);
            state.transform.position = value;
            insert_track_key_with_bone_target(
                tracks_map,
                object,
                &format!("skeleton3d.bones[{selector_key}].transform"),
                NodeField::Skeleton3D(Skeleton3DField::Skeleton),
                Some(AnimationBoneTarget { selector }),
                frame,
                AnimationTrackValue::Transform3D(state.transform),
                0,
                MASK_POS_3D,
                default_interpolation,
                default_ease,
            );
        }
        SkeletonBoneAction::Rotation(selector, value) => {
            let selector_key = bone_selector_key(&selector);
            let state = bone_state_3d
                .entry((object.clone(), selector_key.clone()))
                .or_insert_with(default_object_state_3d);
            state.transform.rotation = value;
            insert_track_key_with_bone_target(
                tracks_map,
                object,
                &format!("skeleton3d.bones[{selector_key}].transform"),
                NodeField::Skeleton3D(Skeleton3DField::Skeleton),
                Some(AnimationBoneTarget { selector }),
                frame,
                AnimationTrackValue::Transform3D(state.transform),
                0,
                MASK_ROT_3D,
                default_interpolation,
                default_ease,
            );
        }
        SkeletonBoneAction::Scale(selector, value) => {
            let selector_key = bone_selector_key(&selector);
            let state = bone_state_3d
                .entry((object.clone(), selector_key.clone()))
                .or_insert_with(default_object_state_3d);
            state.transform.scale = value;
            insert_track_key_with_bone_target(
                tracks_map,
                object,
                &format!("skeleton3d.bones[{selector_key}].transform"),
                NodeField::Skeleton3D(Skeleton3DField::Skeleton),
                Some(AnimationBoneTarget { selector }),
                frame,
                AnimationTrackValue::Transform3D(state.transform),
                0,
                MASK_SCALE_3D,
                default_interpolation,
                default_ease,
            );
        }
    }
}

fn apply_sprite_2d_action(
    frame: u32,
    object: String,
    action: Sprite2DAction,
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    match action {
        Sprite2DAction::Texture(path) => insert_track_key(
            tracks_map,
            object,
            "sprite2d.texture",
            NodeField::Sprite2D(Sprite2DField::Texture),
            frame,
            AnimationTrackValue::AssetPath(path.into()),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
    }
}

fn apply_mesh_instance_3d_action(
    frame: u32,
    object: String,
    action: MeshInstance3DAction,
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    match action {
        MeshInstance3DAction::Mesh(path) => insert_track_key(
            tracks_map,
            object,
            "mesh_instance3d.mesh",
            NodeField::MeshInstance3D(MeshInstance3DField::Mesh),
            frame,
            AnimationTrackValue::AssetPath(path.into()),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        MeshInstance3DAction::Material(path) => insert_track_key(
            tracks_map,
            object,
            "mesh_instance3d.material",
            NodeField::MeshInstance3D(MeshInstance3DField::Material),
            frame,
            AnimationTrackValue::AssetPath(path.into()),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
    }
}

fn apply_camera_3d_action(
    frame: u32,
    object: String,
    action: Camera3DAction,
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    match action {
        Camera3DAction::Zoom(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.zoom",
            NodeField::Camera3D(Camera3DField::Zoom),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::PerspectiveFovYDegrees(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.perspective_fovy_degrees",
            NodeField::Camera3D(Camera3DField::PerspectiveFovYDegrees),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::PerspectiveNear(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.perspective_near",
            NodeField::Camera3D(Camera3DField::PerspectiveNear),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::PerspectiveFar(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.perspective_far",
            NodeField::Camera3D(Camera3DField::PerspectiveFar),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::OrthographicSize(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.orthographic_size",
            NodeField::Camera3D(Camera3DField::OrthographicSize),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::OrthographicNear(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.orthographic_near",
            NodeField::Camera3D(Camera3DField::OrthographicNear),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::OrthographicFar(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.orthographic_far",
            NodeField::Camera3D(Camera3DField::OrthographicFar),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::FrustumLeft(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_left",
            NodeField::Camera3D(Camera3DField::FrustumLeft),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::FrustumRight(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_right",
            NodeField::Camera3D(Camera3DField::FrustumRight),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::FrustumBottom(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_bottom",
            NodeField::Camera3D(Camera3DField::FrustumBottom),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::FrustumTop(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_top",
            NodeField::Camera3D(Camera3DField::FrustumTop),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::FrustumNear(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_near",
            NodeField::Camera3D(Camera3DField::FrustumNear),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::FrustumFar(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_far",
            NodeField::Camera3D(Camera3DField::FrustumFar),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Camera3DAction::Active(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.active",
            NodeField::Camera3D(Camera3DField::Active),
            frame,
            AnimationTrackValue::Bool(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
    }
}

fn apply_light_3d_action(
    frame: u32,
    object: String,
    action: Light3DAction,
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    match action {
        Light3DAction::Color(v) => insert_track_key(
            tracks_map,
            object,
            "light3d.color",
            NodeField::Light3D(Light3DField::Color),
            frame,
            AnimationTrackValue::Vec3(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Light3DAction::Intensity(v) => insert_track_key(
            tracks_map,
            object,
            "light3d.intensity",
            NodeField::Light3D(Light3DField::Intensity),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        Light3DAction::Active(v) => insert_track_key(
            tracks_map,
            object,
            "light3d.active",
            NodeField::Light3D(Light3DField::Active),
            frame,
            AnimationTrackValue::Bool(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
    }
}

fn apply_point_light_3d_action(
    frame: u32,
    object: String,
    action: PointLight3DAction,
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    match action {
        PointLight3DAction::Range(v) => insert_track_key(
            tracks_map,
            object,
            "point_light3d.range",
            NodeField::PointLight3D(PointLight3DField::Range),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
    }
}

fn apply_spot_light_3d_action(
    frame: u32,
    object: String,
    action: SpotLight3DAction,
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    match action {
        SpotLight3DAction::Range(v) => insert_track_key(
            tracks_map,
            object,
            "spot_light3d.range",
            NodeField::SpotLight3D(SpotLight3DField::Range),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        SpotLight3DAction::InnerAngleRadians(v) => insert_track_key(
            tracks_map,
            object,
            "spot_light3d.inner_angle_radians",
            NodeField::SpotLight3D(SpotLight3DField::InnerAngleRadians),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
        SpotLight3DAction::OuterAngleRadians(v) => insert_track_key(
            tracks_map,
            object,
            "spot_light3d.outer_angle_radians",
            NodeField::SpotLight3D(SpotLight3DField::OuterAngleRadians),
            frame,
            AnimationTrackValue::F32(v),
            0,
            0,
            default_interpolation,
            default_ease,
        ),
    }
}

fn default_object_state_2d() -> ObjectState2D {
    ObjectState2D {
        transform: Transform2D::new(Vector2::ZERO, 0.0, Vector2::ONE),
    }
}

fn default_object_state_3d() -> ObjectState3D {
    ObjectState3D {
        transform: Transform3D::new(Vector3::ZERO, Quaternion::IDENTITY, Vector3::ONE),
    }
}

fn bone_selector_key(selector: &AnimationBoneSelector) -> String {
    match selector {
        AnimationBoneSelector::Index(index) => index.to_string(),
        AnimationBoneSelector::Name(name) => name.to_string(),
    }
}

fn insert_track_key(
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    object: String,
    channel_key: &str,
    field: NodeField,
    frame: u32,
    value: AnimationTrackValue,
    transform2d_mask: u8,
    transform3d_mask: u8,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    insert_track_key_with_bone_target(
        tracks_map,
        object,
        channel_key,
        field,
        None,
        frame,
        value,
        transform2d_mask,
        transform3d_mask,
        default_interpolation,
        default_ease,
    );
}

fn insert_track_key_with_bone_target(
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    object: String,
    channel_key: &str,
    field: NodeField,
    bone_target: Option<AnimationBoneTarget>,
    frame: u32,
    value: AnimationTrackValue,
    transform2d_mask: u8,
    transform3d_mask: u8,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    let entry = tracks_map
        .entry((object, channel_key.to_string()))
        .or_insert_with(|| TrackAccumulator {
            field,
            bone_target,
            interpolation: default_interpolation,
            ease: default_ease,
            keys: BTreeMap::new(),
        });
    if let Some(existing) = entry.keys.get_mut(&frame) {
        existing.value = value;
        existing.interpolation = entry.interpolation;
        existing.ease = entry.ease;
        existing.transform2d_mask |= transform2d_mask;
        existing.transform3d_mask |= transform3d_mask;
    } else {
        entry.keys.insert(
            frame,
            TrackKey {
                value,
                interpolation: entry.interpolation,
                ease: entry.ease,
                transform2d_mask,
                transform3d_mask,
            },
        );
    }
}

fn apply_track_control(
    tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>,
    object: String,
    channel_key: &str,
    field: NodeField,
    bone_target: Option<AnimationBoneTarget>,
    interpolation: Option<AnimationInterpolation>,
    ease: Option<AnimationEase>,
    default_interpolation: AnimationInterpolation,
    default_ease: AnimationEase,
) {
    let entry = tracks_map
        .entry((object, channel_key.to_string()))
        .or_insert_with(|| TrackAccumulator {
            field,
            bone_target,
            interpolation: default_interpolation,
            ease: default_ease,
            keys: BTreeMap::new(),
        });
    if let Some(interpolation) = interpolation {
        entry.interpolation = interpolation;
    }
    if let Some(ease) = ease {
        entry.ease = ease;
    }
}

fn resolve_sparse_transform_components(tracks_map: &mut BTreeMap<(String, String), TrackAccumulator>) {
    for track in tracks_map.values_mut() {
        match track.field {
            NodeField::Node2D(Node2DField::Position) => resolve_sparse_transform2d(track),
            NodeField::Node3D(Node3DField::Position) | NodeField::Skeleton3D(Skeleton3DField::Skeleton) => {
                resolve_sparse_transform3d(track)
            }
            _ => {}
        }
    }
}

fn resolve_sparse_transform2d(track: &mut TrackAccumulator) {
    let snapshot: Vec<(u32, TrackKey)> = track.keys.iter().map(|(f, k)| (*f, k.clone())).collect();
    for (frame, key) in track.keys.iter_mut() {
        let AnimationTrackValue::Transform2D(mut out) = key.value else {
            continue;
        };
        if key.transform2d_mask & MASK_POS_2D == 0
            && let Some(position) = sample_transform2d_position(&snapshot, *frame)
        {
            out.position = position;
        }
        if key.transform2d_mask & MASK_ROT_2D == 0
            && let Some(rotation) = sample_transform2d_rotation(&snapshot, *frame)
        {
            out.rotation = rotation;
        }
        if key.transform2d_mask & MASK_SCALE_2D == 0
            && let Some(scale) = sample_transform2d_scale(&snapshot, *frame)
        {
            out.scale = scale;
        }
        key.value = AnimationTrackValue::Transform2D(out);
    }
}

fn resolve_sparse_transform3d(track: &mut TrackAccumulator) {
    let snapshot: Vec<(u32, TrackKey)> = track.keys.iter().map(|(f, k)| (*f, k.clone())).collect();
    for (frame, key) in track.keys.iter_mut() {
        let AnimationTrackValue::Transform3D(mut out) = key.value else {
            continue;
        };
        if key.transform3d_mask & MASK_POS_3D == 0
            && let Some(position) = sample_transform3d_position(&snapshot, *frame)
        {
            out.position = position;
        }
        if key.transform3d_mask & MASK_ROT_3D == 0
            && let Some(rotation) = sample_transform3d_rotation(&snapshot, *frame)
        {
            out.rotation = rotation;
        }
        if key.transform3d_mask & MASK_SCALE_3D == 0
            && let Some(scale) = sample_transform3d_scale(&snapshot, *frame)
        {
            out.scale = scale;
        }
        key.value = AnimationTrackValue::Transform3D(out);
    }
}

fn sample_transform2d_position(snapshot: &[(u32, TrackKey)], frame: u32) -> Option<Vector2> {
    sample_component_2d(snapshot, frame, MASK_POS_2D, |t| t.position, |a, b, t| {
        Vector2::new(lerp_f32(a.x, b.x, t), lerp_f32(a.y, b.y, t))
    })
}

fn sample_transform2d_rotation(snapshot: &[(u32, TrackKey)], frame: u32) -> Option<f32> {
    sample_component_2d(snapshot, frame, MASK_ROT_2D, |t| t.rotation, |a, b, t| lerp_f32(a, b, t))
}

fn sample_transform2d_scale(snapshot: &[(u32, TrackKey)], frame: u32) -> Option<Vector2> {
    sample_component_2d(snapshot, frame, MASK_SCALE_2D, |t| t.scale, |a, b, t| {
        Vector2::new(lerp_f32(a.x, b.x, t), lerp_f32(a.y, b.y, t))
    })
}

fn sample_transform3d_position(snapshot: &[(u32, TrackKey)], frame: u32) -> Option<Vector3> {
    sample_component_3d(snapshot, frame, MASK_POS_3D, |t| t.position, |a, b, t| {
        Vector3::new(
            lerp_f32(a.x, b.x, t),
            lerp_f32(a.y, b.y, t),
            lerp_f32(a.z, b.z, t),
        )
    })
}

fn sample_transform3d_rotation(snapshot: &[(u32, TrackKey)], frame: u32) -> Option<Quaternion> {
    sample_component_3d(snapshot, frame, MASK_ROT_3D, |t| t.rotation, |a, b, t| {
        a.to_quat().slerp(b.to_quat(), t).into()
    })
}

fn sample_transform3d_scale(snapshot: &[(u32, TrackKey)], frame: u32) -> Option<Vector3> {
    sample_component_3d(snapshot, frame, MASK_SCALE_3D, |t| t.scale, |a, b, t| {
        Vector3::new(
            lerp_f32(a.x, b.x, t),
            lerp_f32(a.y, b.y, t),
            lerp_f32(a.z, b.z, t),
        )
    })
}

fn sample_component_2d<T: Copy>(
    snapshot: &[(u32, TrackKey)],
    frame: u32,
    mask: u8,
    read: impl Fn(Transform2D) -> T,
    lerp: impl Fn(T, T, f32) -> T,
) -> Option<T> {
    let mut prev = None::<(u32, &TrackKey, T)>;
    let mut next = None::<(u32, &TrackKey, T)>;
    for (f, key) in snapshot {
        if key.transform2d_mask & mask == 0 {
            continue;
        }
        let AnimationTrackValue::Transform2D(value) = key.value else {
            continue;
        };
        let comp = read(value);
        if *f <= frame {
            prev = Some((*f, key, comp));
        } else {
            next = Some((*f, key, comp));
            break;
        }
    }
    resolve_sample(frame, prev, next, lerp)
}

fn sample_component_3d<T: Copy>(
    snapshot: &[(u32, TrackKey)],
    frame: u32,
    mask: u8,
    read: impl Fn(Transform3D) -> T,
    lerp: impl Fn(T, T, f32) -> T,
) -> Option<T> {
    let mut prev = None::<(u32, &TrackKey, T)>;
    let mut next = None::<(u32, &TrackKey, T)>;
    for (f, key) in snapshot {
        if key.transform3d_mask & mask == 0 {
            continue;
        }
        let AnimationTrackValue::Transform3D(value) = key.value else {
            continue;
        };
        let comp = read(value);
        if *f <= frame {
            prev = Some((*f, key, comp));
        } else {
            next = Some((*f, key, comp));
            break;
        }
    }
    resolve_sample(frame, prev, next, lerp)
}

fn resolve_sample<T: Copy>(
    frame: u32,
    prev: Option<(u32, &TrackKey, T)>,
    next: Option<(u32, &TrackKey, T)>,
    lerp: impl Fn(T, T, f32) -> T,
) -> Option<T> {
    match (prev, next) {
        (None, None) => None,
        (Some((_, _, value)), None) => Some(value),
        (None, Some((_, _, value))) => Some(value),
        (Some((prev_frame, prev_key, prev_value)), Some((next_frame, _, next_value))) => {
            if prev_frame == next_frame {
                return Some(prev_value);
            }
            match prev_key.interpolation {
                AnimationInterpolation::Step => Some(prev_value),
                AnimationInterpolation::Linear => {
                    let local = frame.saturating_sub(prev_frame);
                    let span = next_frame.saturating_sub(prev_frame).max(1);
                    let t = ease_sample(prev_key.ease, local as f32 / span as f32);
                    Some(lerp(prev_value, next_value, t))
                }
            }
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

#[inline]
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

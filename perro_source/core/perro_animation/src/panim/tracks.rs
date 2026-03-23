struct ObjectState2D {
    transform: Transform2D,
}

#[derive(Clone, Debug)]
struct ObjectState3D {
    transform: Transform3D,
}

struct TrackAccumulator {
    channel: AnimationChannel,
    interpolation: AnimationInterpolation,
    keys: BTreeMap<u32, AnimationTrackValue>,
}

fn build_tracks_and_events(
    mut actions: Vec<FrameAction>,
    _object_types: &HashMap<String, String>,
) -> Result<(Vec<AnimationObjectTrack>, Vec<AnimationFrameEvent>), String> {
    let mut frame_events = Vec::<AnimationFrameEvent>::new();

    let mut fields = Vec::<(u32, String, ObjectFieldAction)>::new();
    for action in actions.drain(..) {
        match action {
            FrameAction::Field {
                frame,
                object,
                field,
            } => fields.push((frame, object, field)),
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

    fields.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut state_2d = HashMap::<String, ObjectState2D>::new();
    let mut state_3d = HashMap::<String, ObjectState3D>::new();
    let mut tracks_map = BTreeMap::<(String, &'static str), TrackAccumulator>::new();

    for (frame, object, field) in fields {
        match field {
            ObjectFieldAction::Node2D(action) => {
                apply_node_2d_action(frame, object, action, &mut state_2d, &mut tracks_map);
            }
            ObjectFieldAction::Node3D(action) => {
                apply_node_3d_action(frame, object, action, &mut state_3d, &mut tracks_map);
            }
            ObjectFieldAction::Camera3D(action) => {
                apply_camera_3d_action(frame, object, action, &mut tracks_map);
            }
            ObjectFieldAction::Light3D(action) => {
                apply_light_3d_action(frame, object, action, &mut tracks_map);
            }
            ObjectFieldAction::PointLight3D(action) => {
                apply_point_light_3d_action(frame, object, action, &mut tracks_map);
            }
            ObjectFieldAction::SpotLight3D(action) => {
                apply_spot_light_3d_action(frame, object, action, &mut tracks_map);
            }
        }
    }

    let mut object_tracks = Vec::<AnimationObjectTrack>::new();
    for ((object, _), track) in tracks_map {
        let mut keys = Vec::<AnimationObjectKey>::new();
        for (frame, value) in track.keys {
            keys.push(AnimationObjectKey { frame, value });
        }
        object_tracks.push(AnimationObjectTrack {
            object: object.into(),
            channel: track.channel,
            interpolation: track.interpolation,
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
    tracks_map: &mut BTreeMap<(String, &'static str), TrackAccumulator>,
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
                AnimationChannel::Node2D(Node2DChannel::Transform),
                frame,
                AnimationTrackValue::Transform2D(state.transform),
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
                AnimationChannel::Node2D(Node2DChannel::Transform),
                frame,
                AnimationTrackValue::Transform2D(state.transform),
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
                AnimationChannel::Node2D(Node2DChannel::Transform),
                frame,
                AnimationTrackValue::Transform2D(state.transform),
            );
        }
        Node2DAction::Visible(v) => {
            insert_track_key(
                tracks_map,
                object,
                "node2d.visible",
                AnimationChannel::Node2D(Node2DChannel::Visible),
                frame,
                AnimationTrackValue::Bool(v),
            );
        }
    }
}

fn apply_node_3d_action(
    frame: u32,
    object: String,
    action: Node3DAction,
    state_3d: &mut HashMap<String, ObjectState3D>,
    tracks_map: &mut BTreeMap<(String, &'static str), TrackAccumulator>,
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
                AnimationChannel::Node3D(Node3DChannel::Transform),
                frame,
                AnimationTrackValue::Transform3D(state.transform),
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
                AnimationChannel::Node3D(Node3DChannel::Transform),
                frame,
                AnimationTrackValue::Transform3D(state.transform),
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
                AnimationChannel::Node3D(Node3DChannel::Transform),
                frame,
                AnimationTrackValue::Transform3D(state.transform),
            );
        }
        Node3DAction::Visible(v) => {
            insert_track_key(
                tracks_map,
                object,
                "node3d.visible",
                AnimationChannel::Node3D(Node3DChannel::Visible),
                frame,
                AnimationTrackValue::Bool(v),
            );
        }
    }
}

fn apply_camera_3d_action(
    frame: u32,
    object: String,
    action: Camera3DAction,
    tracks_map: &mut BTreeMap<(String, &'static str), TrackAccumulator>,
) {
    match action {
        Camera3DAction::Zoom(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.zoom",
            AnimationChannel::Camera3D(Camera3DChannel::Zoom),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::PerspectiveFovYDegrees(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.perspective_fovy_degrees",
            AnimationChannel::Camera3D(Camera3DChannel::PerspectiveFovYDegrees),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::PerspectiveNear(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.perspective_near",
            AnimationChannel::Camera3D(Camera3DChannel::PerspectiveNear),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::PerspectiveFar(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.perspective_far",
            AnimationChannel::Camera3D(Camera3DChannel::PerspectiveFar),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::OrthographicSize(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.orthographic_size",
            AnimationChannel::Camera3D(Camera3DChannel::OrthographicSize),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::OrthographicNear(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.orthographic_near",
            AnimationChannel::Camera3D(Camera3DChannel::OrthographicNear),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::OrthographicFar(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.orthographic_far",
            AnimationChannel::Camera3D(Camera3DChannel::OrthographicFar),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::FrustumLeft(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_left",
            AnimationChannel::Camera3D(Camera3DChannel::FrustumLeft),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::FrustumRight(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_right",
            AnimationChannel::Camera3D(Camera3DChannel::FrustumRight),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::FrustumBottom(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_bottom",
            AnimationChannel::Camera3D(Camera3DChannel::FrustumBottom),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::FrustumTop(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_top",
            AnimationChannel::Camera3D(Camera3DChannel::FrustumTop),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::FrustumNear(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_near",
            AnimationChannel::Camera3D(Camera3DChannel::FrustumNear),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::FrustumFar(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.frustum_far",
            AnimationChannel::Camera3D(Camera3DChannel::FrustumFar),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Camera3DAction::Active(v) => insert_track_key(
            tracks_map,
            object,
            "camera3d.active",
            AnimationChannel::Camera3D(Camera3DChannel::Active),
            frame,
            AnimationTrackValue::Bool(v),
        ),
    }
}

fn apply_light_3d_action(
    frame: u32,
    object: String,
    action: Light3DAction,
    tracks_map: &mut BTreeMap<(String, &'static str), TrackAccumulator>,
) {
    match action {
        Light3DAction::Color(v) => insert_track_key(
            tracks_map,
            object,
            "light3d.color",
            AnimationChannel::Light3D(Light3DChannel::Color),
            frame,
            AnimationTrackValue::Vec3(v),
        ),
        Light3DAction::Intensity(v) => insert_track_key(
            tracks_map,
            object,
            "light3d.intensity",
            AnimationChannel::Light3D(Light3DChannel::Intensity),
            frame,
            AnimationTrackValue::F32(v),
        ),
        Light3DAction::Active(v) => insert_track_key(
            tracks_map,
            object,
            "light3d.active",
            AnimationChannel::Light3D(Light3DChannel::Active),
            frame,
            AnimationTrackValue::Bool(v),
        ),
    }
}

fn apply_point_light_3d_action(
    frame: u32,
    object: String,
    action: PointLight3DAction,
    tracks_map: &mut BTreeMap<(String, &'static str), TrackAccumulator>,
) {
    match action {
        PointLight3DAction::Range(v) => insert_track_key(
            tracks_map,
            object,
            "point_light3d.range",
            AnimationChannel::PointLight3D(PointLight3DChannel::Range),
            frame,
            AnimationTrackValue::F32(v),
        ),
    }
}

fn apply_spot_light_3d_action(
    frame: u32,
    object: String,
    action: SpotLight3DAction,
    tracks_map: &mut BTreeMap<(String, &'static str), TrackAccumulator>,
) {
    match action {
        SpotLight3DAction::Range(v) => insert_track_key(
            tracks_map,
            object,
            "spot_light3d.range",
            AnimationChannel::SpotLight3D(SpotLight3DChannel::Range),
            frame,
            AnimationTrackValue::F32(v),
        ),
        SpotLight3DAction::InnerAngleRadians(v) => insert_track_key(
            tracks_map,
            object,
            "spot_light3d.inner_angle_radians",
            AnimationChannel::SpotLight3D(SpotLight3DChannel::InnerAngleRadians),
            frame,
            AnimationTrackValue::F32(v),
        ),
        SpotLight3DAction::OuterAngleRadians(v) => insert_track_key(
            tracks_map,
            object,
            "spot_light3d.outer_angle_radians",
            AnimationChannel::SpotLight3D(SpotLight3DChannel::OuterAngleRadians),
            frame,
            AnimationTrackValue::F32(v),
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

fn insert_track_key(
    tracks_map: &mut BTreeMap<(String, &'static str), TrackAccumulator>,
    object: String,
    channel_key: &'static str,
    channel: AnimationChannel,
    frame: u32,
    value: AnimationTrackValue,
) {
    let entry = tracks_map
        .entry((object, channel_key))
        .or_insert_with(|| TrackAccumulator {
            channel,
            interpolation: AnimationInterpolation::Step,
            keys: BTreeMap::new(),
        });
    entry.keys.insert(frame, value);
}

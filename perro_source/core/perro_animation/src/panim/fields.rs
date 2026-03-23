fn parse_object_field_action(
    frame: u32,
    object: &str,
    node_type: &str,
    key: &str,
    value: &SceneValue,
    line_no: usize,
) -> Result<FrameAction, String> {
    let resolved = resolve_node_field(node_type, key).ok_or_else(|| {
        format!(
            "line {}: unsupported object key `{}` for node type `{}`",
            line_no, key, node_type
        )
    })?;

    let object_field = match resolved {
        NodeField::Node2D(field) => {
            ObjectFieldAction::Node2D(parse_node_2d_action(field, value, key, line_no)?)
        }
        NodeField::Node3D(field) => {
            ObjectFieldAction::Node3D(parse_node_3d_action(field, value, key, line_no)?)
        }
        NodeField::Sprite2D(Sprite2DField::Texture) => {
            ObjectFieldAction::Sprite2D(Sprite2DAction::Texture(expect_asset_path(
                value, key, line_no,
            )?))
        }
        NodeField::MeshInstance3D(MeshInstance3DField::Mesh) => ObjectFieldAction::MeshInstance3D(
            MeshInstance3DAction::Mesh(expect_asset_path(value, key, line_no)?),
        ),
        NodeField::MeshInstance3D(MeshInstance3DField::Material) => {
            ObjectFieldAction::MeshInstance3D(MeshInstance3DAction::Material(expect_asset_path(
                value, key, line_no,
            )?))
        }
        NodeField::Camera3D(
            field @ (Camera3DField::Zoom
            | Camera3DField::PerspectiveFovYDegrees
            | Camera3DField::PerspectiveNear
            | Camera3DField::PerspectiveFar
            | Camera3DField::OrthographicSize
            | Camera3DField::OrthographicNear
            | Camera3DField::OrthographicFar
            | Camera3DField::FrustumLeft
            | Camera3DField::FrustumRight
            | Camera3DField::FrustumBottom
            | Camera3DField::FrustumTop
            | Camera3DField::FrustumNear
            | Camera3DField::FrustumFar
            | Camera3DField::Active),
        ) => {
            ObjectFieldAction::Camera3D(parse_camera_3d_action(field, value, key, line_no)?)
        }
        NodeField::Light3D(field) => {
            ObjectFieldAction::Light3D(parse_light_3d_action(field, value, key, line_no)?)
        }
        NodeField::PointLight3D(field) => ObjectFieldAction::PointLight3D(
            parse_point_light_3d_action(field, value, key, line_no)?,
        ),
        NodeField::SpotLight3D(field) => {
            ObjectFieldAction::SpotLight3D(parse_spot_light_3d_action(field, value, key, line_no)?)
        }
        _ => {
            return Err(format!(
                "line {}: `{}` is valid for `{}` but not yet animatable in `.panim`",
                line_no, key, node_type
            ));
        }
    };

    Ok(FrameAction::Field {
        frame,
        object: object.to_string(),
        field: object_field,
    })
}

fn parse_node_2d_action(
    field: Node2DField,
    value: &SceneValue,
    key: &str,
    line_no: usize,
) -> Result<Node2DAction, String> {
    Ok(match field {
        Node2DField::Position => Node2DAction::Position(expect_vec2(value, key, line_no)?),
        Node2DField::Rotation => Node2DAction::Rotation(expect_f32(value, key, line_no)?),
        Node2DField::Scale => Node2DAction::Scale(expect_vec2(value, key, line_no)?),
        Node2DField::Visible => Node2DAction::Visible(expect_bool(value, key, line_no)?),
        Node2DField::ZIndex => Node2DAction::ZIndex(expect_i32(value, key, line_no)?),
    })
}

fn parse_track_control_action(
    frame: u32,
    object: &str,
    node_type: &str,
    key: &str,
    value: &SceneValue,
    line_no: usize,
) -> Result<Option<FrameAction>, String> {
    let Some((base_key, control_key)) = key.rsplit_once('.') else {
        return Ok(None);
    };

    let (channel_key, field) = resolve_animatable_channel(node_type, base_key.trim(), line_no)?;
    let control_key = control_key.trim();
    if control_key.eq_ignore_ascii_case("interp")
        || control_key.eq_ignore_ascii_case("interpolation")
    {
        let interpolation = parse_interpolation_value(value, line_no)?;
        return Ok(Some(FrameAction::TrackControl {
            frame,
            object: object.to_string(),
            channel_key,
            field,
            interpolation: Some(interpolation),
            ease: None,
        }));
    }
    if control_key.eq_ignore_ascii_case("ease") || control_key.eq_ignore_ascii_case("easing") {
        let ease = parse_ease_value(value, line_no)?;
        return Ok(Some(FrameAction::TrackControl {
            frame,
            object: object.to_string(),
            channel_key,
            field,
            interpolation: None,
            ease: Some(ease),
        }));
    }
    Ok(None)
}

fn parse_node_3d_action(
    field: Node3DField,
    value: &SceneValue,
    key: &str,
    line_no: usize,
) -> Result<Node3DAction, String> {
    Ok(match field {
        Node3DField::Position => Node3DAction::Position(expect_vec3(value, key, line_no)?),
        Node3DField::Rotation => Node3DAction::Rotation(expect_quat(value, key, line_no)?),
        Node3DField::Scale => Node3DAction::Scale(expect_vec3(value, key, line_no)?),
        Node3DField::Visible => Node3DAction::Visible(expect_bool(value, key, line_no)?),
    })
}

fn parse_camera_3d_action(
    field: Camera3DField,
    value: &SceneValue,
    key: &str,
    line_no: usize,
) -> Result<Camera3DAction, String> {
    Ok(match field {
        Camera3DField::Zoom => Camera3DAction::Zoom(expect_f32(value, key, line_no)?),
        Camera3DField::PerspectiveFovYDegrees => {
            Camera3DAction::PerspectiveFovYDegrees(expect_f32(value, key, line_no)?)
        }
        Camera3DField::PerspectiveNear => {
            Camera3DAction::PerspectiveNear(expect_f32(value, key, line_no)?)
        }
        Camera3DField::PerspectiveFar => {
            Camera3DAction::PerspectiveFar(expect_f32(value, key, line_no)?)
        }
        Camera3DField::OrthographicSize => {
            Camera3DAction::OrthographicSize(expect_f32(value, key, line_no)?)
        }
        Camera3DField::OrthographicNear => {
            Camera3DAction::OrthographicNear(expect_f32(value, key, line_no)?)
        }
        Camera3DField::OrthographicFar => {
            Camera3DAction::OrthographicFar(expect_f32(value, key, line_no)?)
        }
        Camera3DField::FrustumLeft => Camera3DAction::FrustumLeft(expect_f32(value, key, line_no)?),
        Camera3DField::FrustumRight => {
            Camera3DAction::FrustumRight(expect_f32(value, key, line_no)?)
        }
        Camera3DField::FrustumBottom => {
            Camera3DAction::FrustumBottom(expect_f32(value, key, line_no)?)
        }
        Camera3DField::FrustumTop => Camera3DAction::FrustumTop(expect_f32(value, key, line_no)?),
        Camera3DField::FrustumNear => Camera3DAction::FrustumNear(expect_f32(value, key, line_no)?),
        Camera3DField::FrustumFar => Camera3DAction::FrustumFar(expect_f32(value, key, line_no)?),
        Camera3DField::Active => Camera3DAction::Active(expect_bool(value, key, line_no)?),
        Camera3DField::Projection | Camera3DField::PostProcessing => {
            return Err(format!(
                "line {}: `{}` is valid but not animatable in `.panim`",
                line_no, key
            ));
        }
    })
}

fn parse_light_3d_action(
    field: Light3DField,
    value: &SceneValue,
    key: &str,
    line_no: usize,
) -> Result<Light3DAction, String> {
    Ok(match field {
        Light3DField::Color => Light3DAction::Color(expect_color3(value, key, line_no)?),
        Light3DField::Intensity => Light3DAction::Intensity(expect_f32(value, key, line_no)?),
        Light3DField::Active => Light3DAction::Active(expect_bool(value, key, line_no)?),
    })
}

fn parse_point_light_3d_action(
    field: PointLight3DField,
    value: &SceneValue,
    key: &str,
    line_no: usize,
) -> Result<PointLight3DAction, String> {
    Ok(match field {
        PointLight3DField::Range => PointLight3DAction::Range(expect_f32(value, key, line_no)?),
    })
}

fn parse_spot_light_3d_action(
    field: SpotLight3DField,
    value: &SceneValue,
    key: &str,
    line_no: usize,
) -> Result<SpotLight3DAction, String> {
    Ok(match field {
        SpotLight3DField::Range => SpotLight3DAction::Range(expect_f32(value, key, line_no)?),
        SpotLight3DField::InnerAngleRadians => {
            SpotLight3DAction::InnerAngleRadians(expect_f32(value, key, line_no)?)
        }
        SpotLight3DField::OuterAngleRadians => {
            SpotLight3DAction::OuterAngleRadians(expect_f32(value, key, line_no)?)
        }
    })
}

fn expect_f32(value: &SceneValue, key: &str, line_no: usize) -> Result<f32, String> {
    value
        .as_f32()
        .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, key))
}

fn expect_i32(value: &SceneValue, key: &str, line_no: usize) -> Result<i32, String> {
    if let Some(v) = value.as_i32() {
        return Ok(v);
    }
    if let Some(v) = value.as_f32()
        && v.is_finite()
        && v.fract() == 0.0
        && v >= i32::MIN as f32
        && v <= i32::MAX as f32
    {
        return Ok(v as i32);
    }
    Err(format!("line {}: `{}` expects i32", line_no, key))
}

fn expect_bool(value: &SceneValue, key: &str, line_no: usize) -> Result<bool, String> {
    value
        .as_bool()
        .ok_or_else(|| format!("line {}: `{}` expects bool", line_no, key))
}

fn expect_vec2(value: &SceneValue, key: &str, line_no: usize) -> Result<Vector2, String> {
    let (x, y) = value
        .as_vec2()
        .ok_or_else(|| format!("line {}: `{}` expects vec2", line_no, key))?;
    Ok(Vector2::new(x, y))
}

fn expect_vec3(value: &SceneValue, key: &str, line_no: usize) -> Result<Vector3, String> {
    let (x, y, z) = value
        .as_vec3()
        .ok_or_else(|| format!("line {}: `{}` expects vec3", line_no, key))?;
    Ok(Vector3::new(x, y, z))
}

fn expect_color3(value: &SceneValue, key: &str, line_no: usize) -> Result<[f32; 3], String> {
    let (x, y, z) = value
        .as_vec3()
        .ok_or_else(|| format!("line {}: `{}` expects vec3", line_no, key))?;
    Ok([x, y, z])
}

fn expect_quat(value: &SceneValue, key: &str, line_no: usize) -> Result<Quaternion, String> {
    let (x, y, z, w) = value
        .as_vec4()
        .ok_or_else(|| format!("line {}: `{}` expects vec4", line_no, key))?;
    let mut quat = Quaternion::new(x, y, z, w);
    quat.normalize();
    Ok(quat)
}

fn expect_asset_path(value: &SceneValue, key: &str, line_no: usize) -> Result<String, String> {
    match value {
        SceneValue::Str(v) => Ok(v.to_string()),
        SceneValue::Key(v) => Ok(v.to_string()),
        _ => Err(format!(
            "line {}: `{}` expects asset path string/key",
            line_no, key
        )),
    }
}

fn parse_interpolation_value(
    value: &SceneValue,
    line_no: usize,
) -> Result<AnimationInterpolation, String> {
    let raw = as_text(value)
        .ok_or_else(|| format!("line {}: interpolation expects text", line_no))?
        .trim();
    let norm = raw.to_ascii_lowercase().replace(['-', ' '], "_");
    match norm.as_str() {
        "step" => Ok(AnimationInterpolation::Step),
        "interpolate" | "linear" | "lerp" | "slerp" => Ok(AnimationInterpolation::Linear),
        _ => Err(format!(
            "line {}: unknown interpolation `{}` (expected `step` or `interpolate`)",
            line_no, raw
        )),
    }
}

fn parse_ease_value(value: &SceneValue, line_no: usize) -> Result<AnimationEase, String> {
    let raw = as_text(value)
        .ok_or_else(|| format!("line {}: ease expects text", line_no))?
        .trim();
    let norm = raw.to_ascii_lowercase().replace(['-', ' '], "_");
    match norm.as_str() {
        "linear" => Ok(AnimationEase::Linear),
        "ease_in" | "easein" | "in" => Ok(AnimationEase::EaseIn),
        "ease_out" | "easeout" | "out" => Ok(AnimationEase::EaseOut),
        "ease_in_out" | "easeinout" | "in_out" => Ok(AnimationEase::EaseInOut),
        _ => Err(format!(
            "line {}: unknown ease `{}` (expected `linear`, `ease_in`, `ease_out`, `ease_in_out`)",
            line_no, raw
        )),
    }
}

fn resolve_animatable_channel(
    node_type: &str,
    key: &str,
    line_no: usize,
) -> Result<(&'static str, NodeField), String> {
    let resolved = resolve_node_field(node_type, key).ok_or_else(|| {
        format!(
            "line {}: unsupported object key `{}` for node type `{}`",
            line_no, key, node_type
        )
    })?;

    match resolved {
        NodeField::Node2D(Node2DField::Position)
        | NodeField::Node2D(Node2DField::Rotation)
        | NodeField::Node2D(Node2DField::Scale) => {
            Ok(("node2d.transform", NodeField::Node2D(Node2DField::Position)))
        }
        NodeField::Node2D(Node2DField::Visible) => {
            Ok(("node2d.visible", NodeField::Node2D(Node2DField::Visible)))
        }
        NodeField::Node2D(Node2DField::ZIndex) => {
            Ok(("node2d.z_index", NodeField::Node2D(Node2DField::ZIndex)))
        }
        NodeField::Node3D(Node3DField::Position)
        | NodeField::Node3D(Node3DField::Rotation)
        | NodeField::Node3D(Node3DField::Scale) => {
            Ok(("node3d.transform", NodeField::Node3D(Node3DField::Position)))
        }
        NodeField::Node3D(Node3DField::Visible) => {
            Ok(("node3d.visible", NodeField::Node3D(Node3DField::Visible)))
        }
        NodeField::Sprite2D(Sprite2DField::Texture) => {
            Ok(("sprite2d.texture", NodeField::Sprite2D(Sprite2DField::Texture)))
        }
        NodeField::MeshInstance3D(MeshInstance3DField::Mesh) => Ok((
            "mesh_instance3d.mesh",
            NodeField::MeshInstance3D(MeshInstance3DField::Mesh),
        )),
        NodeField::MeshInstance3D(MeshInstance3DField::Material) => Ok((
            "mesh_instance3d.material",
            NodeField::MeshInstance3D(MeshInstance3DField::Material),
        )),
        NodeField::Camera3D(field) => match field {
            Camera3DField::Zoom => Ok(("camera3d.zoom", NodeField::Camera3D(Camera3DField::Zoom))),
            Camera3DField::PerspectiveFovYDegrees => Ok((
                "camera3d.perspective_fovy_degrees",
                NodeField::Camera3D(Camera3DField::PerspectiveFovYDegrees),
            )),
            Camera3DField::PerspectiveNear => Ok((
                "camera3d.perspective_near",
                NodeField::Camera3D(Camera3DField::PerspectiveNear),
            )),
            Camera3DField::PerspectiveFar => Ok((
                "camera3d.perspective_far",
                NodeField::Camera3D(Camera3DField::PerspectiveFar),
            )),
            Camera3DField::OrthographicSize => Ok((
                "camera3d.orthographic_size",
                NodeField::Camera3D(Camera3DField::OrthographicSize),
            )),
            Camera3DField::OrthographicNear => Ok((
                "camera3d.orthographic_near",
                NodeField::Camera3D(Camera3DField::OrthographicNear),
            )),
            Camera3DField::OrthographicFar => Ok((
                "camera3d.orthographic_far",
                NodeField::Camera3D(Camera3DField::OrthographicFar),
            )),
            Camera3DField::FrustumLeft => Ok((
                "camera3d.frustum_left",
                NodeField::Camera3D(Camera3DField::FrustumLeft),
            )),
            Camera3DField::FrustumRight => Ok((
                "camera3d.frustum_right",
                NodeField::Camera3D(Camera3DField::FrustumRight),
            )),
            Camera3DField::FrustumBottom => Ok((
                "camera3d.frustum_bottom",
                NodeField::Camera3D(Camera3DField::FrustumBottom),
            )),
            Camera3DField::FrustumTop => Ok((
                "camera3d.frustum_top",
                NodeField::Camera3D(Camera3DField::FrustumTop),
            )),
            Camera3DField::FrustumNear => Ok((
                "camera3d.frustum_near",
                NodeField::Camera3D(Camera3DField::FrustumNear),
            )),
            Camera3DField::FrustumFar => Ok((
                "camera3d.frustum_far",
                NodeField::Camera3D(Camera3DField::FrustumFar),
            )),
            Camera3DField::Active => {
                Ok(("camera3d.active", NodeField::Camera3D(Camera3DField::Active)))
            }
            Camera3DField::Projection | Camera3DField::PostProcessing => Err(format!(
                "line {}: `{}` is valid but not animatable in `.panim`",
                line_no, key
            )),
        },
        NodeField::Light3D(Light3DField::Color) => {
            Ok(("light3d.color", NodeField::Light3D(Light3DField::Color)))
        }
        NodeField::Light3D(Light3DField::Intensity) => Ok((
            "light3d.intensity",
            NodeField::Light3D(Light3DField::Intensity),
        )),
        NodeField::Light3D(Light3DField::Active) => {
            Ok(("light3d.active", NodeField::Light3D(Light3DField::Active)))
        }
        NodeField::PointLight3D(PointLight3DField::Range) => Ok((
            "point_light3d.range",
            NodeField::PointLight3D(PointLight3DField::Range),
        )),
        NodeField::SpotLight3D(SpotLight3DField::Range) => Ok((
            "spot_light3d.range",
            NodeField::SpotLight3D(SpotLight3DField::Range),
        )),
        NodeField::SpotLight3D(SpotLight3DField::InnerAngleRadians) => Ok((
            "spot_light3d.inner_angle_radians",
            NodeField::SpotLight3D(SpotLight3DField::InnerAngleRadians),
        )),
        NodeField::SpotLight3D(SpotLight3DField::OuterAngleRadians) => Ok((
            "spot_light3d.outer_angle_radians",
            NodeField::SpotLight3D(SpotLight3DField::OuterAngleRadians),
        )),
        _ => Err(format!(
            "line {}: `{}` is valid for `{}` but not yet animatable in `.panim`",
            line_no, key, node_type
        )),
    }
}

enum ObjectFieldAction {
    Node2D(Node2DAction),
    Node3D(Node3DAction),
    Sprite2D(Sprite2DAction),
    MeshInstance3D(MeshInstance3DAction),
    Camera3D(Camera3DAction),
    Light3D(Light3DAction),
    PointLight3D(PointLight3DAction),
    SpotLight3D(SpotLight3DAction),
}

enum Node2DAction {
    Position(Vector2),
    Rotation(f32),
    Scale(Vector2),
    Visible(bool),
    ZIndex(i32),
}

enum Node3DAction {
    Position(Vector3),
    Rotation(Quaternion),
    Scale(Vector3),
    Visible(bool),
}

enum Sprite2DAction {
    Texture(String),
}

enum MeshInstance3DAction {
    Mesh(String),
    Material(String),
}

enum Camera3DAction {
    Zoom(f32),
    PerspectiveFovYDegrees(f32),
    PerspectiveNear(f32),
    PerspectiveFar(f32),
    OrthographicSize(f32),
    OrthographicNear(f32),
    OrthographicFar(f32),
    FrustumLeft(f32),
    FrustumRight(f32),
    FrustumBottom(f32),
    FrustumTop(f32),
    FrustumNear(f32),
    FrustumFar(f32),
    Active(bool),
}

enum Light3DAction {
    Color([f32; 3]),
    Intensity(f32),
    Active(bool),
}

enum PointLight3DAction {
    Range(f32),
}

enum SpotLight3DAction {
    Range(f32),
    InnerAngleRadians(f32),
    OuterAngleRadians(f32),
}

enum FrameAction {
    Field {
        frame: u32,
        object: String,
        field: ObjectFieldAction,
    },
    TrackControl {
        frame: u32,
        object: String,
        channel_key: &'static str,
        field: NodeField,
        interpolation: Option<AnimationInterpolation>,
        ease: Option<AnimationEase>,
    },
    Event {
        frame: u32,
        scope: AnimationEventScope,
        event: AnimationEvent,
    },
}


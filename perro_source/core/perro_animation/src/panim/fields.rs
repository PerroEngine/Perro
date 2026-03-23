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
        NodeField::Node2D(Node2DField::ZIndex) => {
            return Err(format!(
                "line {}: `z_index` is valid for `{}` but not yet animatable",
                line_no, node_type
            ));
        }
        NodeField::Node2D(field) => {
            ObjectFieldAction::Node2D(parse_node_2d_action(field, value, key, line_no)?)
        }
        NodeField::Node3D(field) => {
            ObjectFieldAction::Node3D(parse_node_3d_action(field, value, key, line_no)?)
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
        Node2DField::ZIndex => {
            return Err(format!(
                "line {}: `z_index` is valid but not animatable in `.panim`",
                line_no
            ));
        }
    })
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

enum ObjectFieldAction {
    Node2D(Node2DAction),
    Node3D(Node3DAction),
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
}

enum Node3DAction {
    Position(Vector3),
    Rotation(Quaternion),
    Scale(Vector3),
    Visible(bool),
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
    Event {
        frame: u32,
        scope: AnimationEventScope,
        event: AnimationEvent,
    },
}


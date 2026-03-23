enum PAnimNodeField {
    Node2D(Node2DField),
    Node3D(Node3DField),
    Camera3D(Camera3DField),
    Light3D(Light3DField),
    PointLight3D(PointLight3DField),
    SpotLight3D(SpotLight3DField),
    NotAnimatable(NodeField),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Node2DField {
    Position,
    Rotation,
    Scale,
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Node3DField {
    Position,
    Rotation,
    Scale,
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Camera3DField {
    Zoom,
    PerspectiveFovYDegrees,
    PerspectiveNear,
    PerspectiveFar,
    OrthographicSize,
    OrthographicNear,
    OrthographicFar,
    FrustumLeft,
    FrustumRight,
    FrustumBottom,
    FrustumTop,
    FrustumNear,
    FrustumFar,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Light3DField {
    Color,
    Intensity,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PointLight3DField {
    Range,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpotLight3DField {
    Range,
    InnerAngleRadians,
    OuterAngleRadians,
}

fn resolve_panim_node_field(node_type: &str, field: &str) -> Option<PAnimNodeField> {
    let field = resolve_node_field(node_type, field)?;
    Some(match field {
        NodeField::Position2D => PAnimNodeField::Node2D(Node2DField::Position),
        NodeField::Rotation2D => PAnimNodeField::Node2D(Node2DField::Rotation),
        NodeField::Scale2D => PAnimNodeField::Node2D(Node2DField::Scale),
        NodeField::Visible2D => PAnimNodeField::Node2D(Node2DField::Visible),

        NodeField::Position3D => PAnimNodeField::Node3D(Node3DField::Position),
        NodeField::Rotation3D => PAnimNodeField::Node3D(Node3DField::Rotation),
        NodeField::Scale3D => PAnimNodeField::Node3D(Node3DField::Scale),
        NodeField::Visible3D => PAnimNodeField::Node3D(Node3DField::Visible),

        NodeField::Camera3DZoom => PAnimNodeField::Camera3D(Camera3DField::Zoom),
        NodeField::Camera3DPerspectiveFovYDegrees => {
            PAnimNodeField::Camera3D(Camera3DField::PerspectiveFovYDegrees)
        }
        NodeField::Camera3DPerspectiveNear => {
            PAnimNodeField::Camera3D(Camera3DField::PerspectiveNear)
        }
        NodeField::Camera3DPerspectiveFar => {
            PAnimNodeField::Camera3D(Camera3DField::PerspectiveFar)
        }
        NodeField::Camera3DOrthographicSize => {
            PAnimNodeField::Camera3D(Camera3DField::OrthographicSize)
        }
        NodeField::Camera3DOrthographicNear => {
            PAnimNodeField::Camera3D(Camera3DField::OrthographicNear)
        }
        NodeField::Camera3DOrthographicFar => {
            PAnimNodeField::Camera3D(Camera3DField::OrthographicFar)
        }
        NodeField::Camera3DFrustumLeft => PAnimNodeField::Camera3D(Camera3DField::FrustumLeft),
        NodeField::Camera3DFrustumRight => PAnimNodeField::Camera3D(Camera3DField::FrustumRight),
        NodeField::Camera3DFrustumBottom => PAnimNodeField::Camera3D(Camera3DField::FrustumBottom),
        NodeField::Camera3DFrustumTop => PAnimNodeField::Camera3D(Camera3DField::FrustumTop),
        NodeField::Camera3DFrustumNear => PAnimNodeField::Camera3D(Camera3DField::FrustumNear),
        NodeField::Camera3DFrustumFar => PAnimNodeField::Camera3D(Camera3DField::FrustumFar),
        NodeField::Camera3DActive => PAnimNodeField::Camera3D(Camera3DField::Active),

        NodeField::AmbientLight3DColor
        | NodeField::RayLight3DColor
        | NodeField::PointLight3DColor
        | NodeField::SpotLight3DColor => PAnimNodeField::Light3D(Light3DField::Color),
        NodeField::AmbientLight3DIntensity
        | NodeField::RayLight3DIntensity
        | NodeField::PointLight3DIntensity
        | NodeField::SpotLight3DIntensity => PAnimNodeField::Light3D(Light3DField::Intensity),
        NodeField::AmbientLight3DActive
        | NodeField::RayLight3DActive
        | NodeField::PointLight3DActive
        | NodeField::SpotLight3DActive => PAnimNodeField::Light3D(Light3DField::Active),
        NodeField::PointLight3DRange => PAnimNodeField::PointLight3D(PointLight3DField::Range),
        NodeField::SpotLight3DRange => PAnimNodeField::SpotLight3D(SpotLight3DField::Range),
        NodeField::SpotLight3DInnerAngleRadians => {
            PAnimNodeField::SpotLight3D(SpotLight3DField::InnerAngleRadians)
        }
        NodeField::SpotLight3DOuterAngleRadians => {
            PAnimNodeField::SpotLight3D(SpotLight3DField::OuterAngleRadians)
        }
        other => PAnimNodeField::NotAnimatable(other),
    })
}

fn parse_object_field_action(
    frame: u32,
    object: &str,
    node_type: &str,
    key: &str,
    value: &SceneValue,
    line_no: usize,
) -> Result<FrameAction, String> {
    let resolved = resolve_panim_node_field(node_type, key).ok_or_else(|| {
        format!(
            "line {}: unsupported object key `{}` for node type `{}`",
            line_no, key, node_type
        )
    })?;

    let object_field = match resolved {
        PAnimNodeField::Node2D(field) => {
            ObjectFieldAction::Node2D(parse_node_2d_action(field, value, key, line_no)?)
        }
        PAnimNodeField::Node3D(field) => {
            ObjectFieldAction::Node3D(parse_node_3d_action(field, value, key, line_no)?)
        }
        PAnimNodeField::Camera3D(field) => {
            ObjectFieldAction::Camera3D(parse_camera_3d_action(field, value, key, line_no)?)
        }
        PAnimNodeField::Light3D(field) => {
            ObjectFieldAction::Light3D(parse_light_3d_action(field, value, key, line_no)?)
        }
        PAnimNodeField::PointLight3D(field) => ObjectFieldAction::PointLight3D(
            parse_point_light_3d_action(field, value, key, line_no)?,
        ),
        PAnimNodeField::SpotLight3D(field) => {
            ObjectFieldAction::SpotLight3D(parse_spot_light_3d_action(field, value, key, line_no)?)
        }

        PAnimNodeField::NotAnimatable(NodeField::ZIndex2D) => {
            return Err(format!(
                "line {}: `z_index` is valid for `{}` but not yet animatable",
                line_no, node_type
            ));
        }
        PAnimNodeField::NotAnimatable(_field) => {
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


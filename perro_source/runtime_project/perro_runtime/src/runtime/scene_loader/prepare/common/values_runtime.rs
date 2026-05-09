fn as_bool(value: &SceneValue) -> Option<bool> {
    match value {
        SceneValue::Bool(v) => Some(*v),
        _ => None,
    }
}

fn as_i32(value: &SceneValue) -> Option<i32> {
    match value {
        SceneValue::I32(v) => Some(*v),
        SceneValue::F32(v) => Some(*v as i32),
        _ => None,
    }
}

fn as_u32(value: &SceneValue) -> Option<u32> {
    match value {
        SceneValue::I32(v) if *v >= 0 => Some(*v as u32),
        SceneValue::F32(v) if *v >= 0.0 => Some(*v as u32),
        _ => None,
    }
}

fn as_node_id(value: &SceneValue) -> Option<NodeID> {
    match value {
        SceneValue::I32(v) if *v >= 0 => Some(NodeID::from_u32(*v as u32)),
        SceneValue::F32(v) if *v >= 0.0 => Some(NodeID::from_u32(*v as u32)),
        SceneValue::Key(v) => v
            .as_ref()
            .strip_prefix('#')
            .and_then(|raw| raw.parse::<u32>().ok())
            .map(NodeID::from_u32),
        SceneValue::Str(v) => NodeID::parse_str(v.as_ref()).ok(),
        _ => None,
    }
}

fn as_f32(value: &SceneValue) -> Option<f32> {
    match value {
        SceneValue::F32(v) => Some(*v),
        SceneValue::I32(v) => Some(*v as f32),
        _ => None,
    }
}

fn as_vec2(value: &SceneValue) -> Option<Vector2> {
    match value {
        SceneValue::Vec2 { x, y } => Some(Vector2::new(*x, *y)),
        _ => None,
    }
}

fn as_transform_2d(value: &SceneValue) -> Option<(Vector2, f32, Vector2)> {
    let mut position = Vector2::ZERO;
    let mut rotation = 0.0;
    let mut scale = Vector2::ONE;
    match value {
        SceneValue::Object(fields) => {
            for (name, value) in fields.iter() {
                match name.as_ref() {
                    "position" | "pos" => position = as_vec2(value)?,
                    "rotation" | "rot" => rotation = as_f32(value)?,
                    "scale" => scale = as_vec2(value)?,
                    _ => {}
                }
            }
            Some((position, rotation, scale))
        }
        _ => None,
    }
}

fn as_vec3(value: &SceneValue) -> Option<Vector3> {
    match value {
        SceneValue::Vec3 { x, y, z } => Some(Vector3::new(*x, *y, *z)),
        _ => None,
    }
}

fn as_quat(value: &SceneValue) -> Option<Quaternion> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some(Quaternion::new(*x, *y, *z, *w)),
        _ => None,
    }
}

fn as_str(value: &SceneValue) -> Option<&str> {
    match value {
        SceneValue::Str(v) => Some(v.as_ref()),
        SceneValue::Key(v) => Some(v.as_ref()),
        _ => None,
    }
}



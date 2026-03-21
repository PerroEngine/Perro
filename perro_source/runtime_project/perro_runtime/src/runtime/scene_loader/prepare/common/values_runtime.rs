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



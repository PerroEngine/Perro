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

fn as_bitmask(value: &SceneValue) -> Option<BitMask> {
    match value {
        SceneValue::Array(items) => {
            let mut mask = BitMask::NONE;
            for item in items.iter() {
                let layer = u8::try_from(as_u32(item)?).ok()?;
                mask = mask.union(BitMask::try_layer(layer)?);
            }
            Some(mask)
        }
        _ => as_u32(value).map(BitMask::from_bits),
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



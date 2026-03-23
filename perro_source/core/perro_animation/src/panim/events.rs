fn parse_emit_signal(value: &str, line_no: usize) -> Result<AnimationEvent, String> {
    let value = parse_scene_value(value, line_no)?;
    let fields = as_object(&value).ok_or_else(|| format!("line {}: expected object", line_no))?;

    let mut name = None::<String>;
    let mut params = Vec::<AnimationParam>::new();
    for (k, v) in fields {
        match k.as_ref() {
            "name" => {
                name = as_text(v).map(|s| s.to_string());
            }
            "params" => {
                params = parse_params(v, line_no)?;
            }
            _ => {}
        }
    }
    let name = name.ok_or_else(|| format!("line {}: emit_signal requires `name`", line_no))?;

    Ok(AnimationEvent::EmitSignal {
        name: name.into(),
        params: Cow::Owned(params),
    })
}

fn parse_set_var(value: &str, line_no: usize) -> Result<AnimationEvent, String> {
    let value = parse_scene_value(value, line_no)?;
    let fields = as_object(&value).ok_or_else(|| format!("line {}: expected object", line_no))?;

    let mut name = None::<String>;
    let mut set_value = None::<AnimationParam>;
    for (k, v) in fields {
        match k.as_ref() {
            "name" => {
                name = as_text(v).map(|s| s.to_string());
            }
            "value" => {
                set_value = Some(parse_param(v, line_no)?);
            }
            _ => {}
        }
    }
    let name = name.ok_or_else(|| format!("line {}: set_var requires `name`", line_no))?;
    let set_value =
        set_value.ok_or_else(|| format!("line {}: set_var requires `value`", line_no))?;

    Ok(AnimationEvent::SetVar {
        name: name.into(),
        value: set_value,
    })
}

fn parse_call_method(value: &str, line_no: usize) -> Result<AnimationEvent, String> {
    let value = parse_scene_value(value, line_no)?;
    let fields = as_object(&value).ok_or_else(|| format!("line {}: expected object", line_no))?;

    let mut name = None::<String>;
    let mut params = Vec::<AnimationParam>::new();
    for (k, v) in fields {
        match k.as_ref() {
            "name" => {
                name = as_text(v).map(|s| s.to_string());
            }
            "params" => {
                params = parse_params(v, line_no)?;
            }
            _ => {}
        }
    }
    let name = name.ok_or_else(|| format!("line {}: call_method requires `name`", line_no))?;

    Ok(AnimationEvent::CallMethod {
        name: name.into(),
        params: Cow::Owned(params),
    })
}

fn parse_params(value: &SceneValue, line_no: usize) -> Result<Vec<AnimationParam>, String> {
    let SceneValue::Array(items) = value else {
        return Err(format!("line {}: params must be an array", line_no));
    };

    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        out.push(parse_param(item, line_no)?);
    }
    Ok(out)
}

fn parse_param(value: &SceneValue, line_no: usize) -> Result<AnimationParam, String> {
    match value {
        SceneValue::Bool(v) => Ok(AnimationParam::Bool(*v)),
        SceneValue::I32(v) => Ok(AnimationParam::I32(*v)),
        SceneValue::F32(v) => Ok(AnimationParam::F32(*v)),
        SceneValue::Str(v) => Ok(AnimationParam::String(v.clone())),
        SceneValue::Key(v) => Ok(AnimationParam::String(v.0.clone())),
        SceneValue::Vec2 { x, y } => Ok(AnimationParam::Vec2([*x, *y])),
        SceneValue::Vec3 { x, y, z } => Ok(AnimationParam::Vec3([*x, *y, *z])),
        SceneValue::Vec4 { x, y, z, w } => Ok(AnimationParam::Vec4([*x, *y, *z, *w])),
        SceneValue::Object(fields) => {
            let mut position2 = None;
            let mut rotation2 = None;
            let mut scale2 = None;
            let mut position3 = None;
            let mut rotation3 = None;
            let mut scale3 = None;
            for (k, v) in fields.iter() {
                match k.as_ref() {
                    "position" => {
                        if let Some((x, y)) = v.as_vec2() {
                            position2 = Some(Vector2::new(x, y));
                        }
                        if let Some((x, y, z)) = v.as_vec3() {
                            position3 = Some(Vector3::new(x, y, z));
                        }
                    }
                    "rotation" => {
                        if let Some(r) = v.as_f32() {
                            rotation2 = Some(r);
                        }
                        if let Some((x, y, z, w)) = v.as_vec4() {
                            let mut q = Quaternion::new(x, y, z, w);
                            q.normalize();
                            rotation3 = Some(q);
                        }
                    }
                    "scale" => {
                        if let Some((x, y)) = v.as_vec2() {
                            scale2 = Some(Vector2::new(x, y));
                        }
                        if let Some((x, y, z)) = v.as_vec3() {
                            scale3 = Some(Vector3::new(x, y, z));
                        }
                    }
                    _ => {}
                }
            }
            if let (Some(p), Some(r), Some(s)) = (position2, rotation2, scale2) {
                return Ok(AnimationParam::Transform2D(Transform2D::new(p, r, s)));
            }
            if let (Some(p), Some(r), Some(s)) = (position3, rotation3, scale3) {
                return Ok(AnimationParam::Transform3D(Transform3D::new(p, r, s)));
            }
            Err(format!("line {}: unsupported object param", line_no))
        }
        _ => Err(format!("line {}: unsupported param value", line_no)),
    }
}


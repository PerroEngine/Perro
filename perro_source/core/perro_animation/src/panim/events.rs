fn parse_emit_signal(value: &str, line_no: usize) -> Result<AnimationEvent, String> {
    let value = parse_scene_value_with_refs(value, line_no)?;
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
    let value = parse_scene_value_with_refs(value, line_no)?;
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
    let value = parse_scene_value_with_refs(value, line_no)?;
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
        SceneValue::Str(v) => parse_text_param(v.as_ref(), line_no),
        SceneValue::Key(v) => parse_text_param(v.0.as_ref(), line_no),
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

fn parse_text_param(value: &str, line_no: usize) -> Result<AnimationParam, String> {
    if let Some(rest) = value.strip_prefix('@') {
        return parse_reference_param(rest, line_no);
    }
    Ok(AnimationParam::String(Cow::Owned(value.to_string())))
}

fn parse_reference_param(rest: &str, line_no: usize) -> Result<AnimationParam, String> {
    let Some((object, field)) = rest.split_once('.') else {
        if is_ident(rest) {
            return Ok(AnimationParam::ObjectNode(rest.to_string().into()));
        }
        return Err(format!("line {}: invalid object reference `@{}`", line_no, rest));
    };

    if !is_ident(object) || !is_ident(field) {
        return Err(format!("line {}: invalid object reference `@{}`", line_no, rest));
    }

    Ok(AnimationParam::ObjectField {
        object: object.to_string().into(),
        field: field.to_string().into(),
    })
}

fn is_ident(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn parse_scene_value_with_refs(value: &str, line_no: usize) -> Result<SceneValue, String> {
    match parse_scene_value(value, line_no) {
        Ok(parsed) => Ok(parsed),
        Err(_) => {
            if !value.contains('@') {
                return parse_scene_value(value, line_no);
            }
            let rewritten = rewrite_reference_tokens(value);
            parse_scene_value(&rewritten, line_no)
        }
    }
}

fn rewrite_reference_tokens(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len() + 16);
    let mut i = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            out.push(ch);
            i += 1;
            continue;
        }

        if ch != '@' {
            out.push(ch);
            i += 1;
            continue;
        }

        let mut end = i + 1;
        while end < bytes.len() {
            let c = bytes[end] as char;
            if c.is_ascii_whitespace()
                || matches!(c, ',' | ']' | '}' | ')' | ':' | '=')
            {
                break;
            }
            end += 1;
        }

        if end > i + 1 {
            out.push('"');
            out.push_str(&src[i..end]);
            out.push('"');
            i = end;
            continue;
        }

        out.push(ch);
        i += 1;
    }

    out
}


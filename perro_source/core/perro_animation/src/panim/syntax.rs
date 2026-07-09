fn parse_scene_value(value: &str, line_no: usize) -> Result<SceneValue, String> {
    std::panic::catch_unwind(|| SceneParser::new(value).parse_value_literal())
        .map_err(|_| format!("line {}: invalid value `{}`", line_no, value))
}

fn as_text(value: &SceneValue) -> Option<&str> {
    match value {
        SceneValue::Str(v) => Some(v.as_ref()),
        SceneValue::Key(v) => Some(v.as_ref()),
        _ => None,
    }
}

fn as_object(value: &SceneValue) -> Option<&[SceneObjectField]> {
    match value {
        SceneValue::Object(fields) => Some(fields.as_ref()),
        _ => None,
    }
}

fn parse_frame_header(line: &str) -> Option<(u32, AnimationKeyMode)> {
    let inner = line.strip_prefix("[Frame")?.strip_suffix(']')?;
    let inner = inner.trim();
    if let Some(frame) = inner.strip_suffix('?') {
        return frame.trim().parse::<u32>().ok().map(|f| (f, AnimationKeyMode::Open));
    }
    inner.parse::<u32>().ok().map(|f| (f, AnimationKeyMode::Closed))
}

fn is_frame_footer(line: &str) -> bool {
    line.starts_with("[/Frame") && line.ends_with(']')
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once('=')?;
    Some((k.trim(), v.trim()))
}

fn strip_comment(line: &str) -> &str {
    // Comment markers inside quoted strings (e.g. `"res://path"`) are content.
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_string = !in_string,
            b'\\' if in_string => i += 1,
            b'/' if !in_string && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
            b'#' if !in_string => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

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

fn as_object(value: &SceneValue) -> Option<&[(Cow<'static, str>, SceneValue)]> {
    match value {
        SceneValue::Object(fields) => Some(fields.as_ref()),
        _ => None,
    }
}

fn parse_frame_header(line: &str) -> Option<u32> {
    let inner = line.strip_prefix("[Frame")?.strip_suffix(']')?;
    inner.trim().parse::<u32>().ok()
}

fn is_frame_footer(line: &str) -> bool {
    line.starts_with("[/Frame") && line.ends_with(']')
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once('=')?;
    Some((k.trim(), v.trim()))
}

fn strip_comment(line: &str) -> &str {
    let mut cut = line.len();
    if let Some(i) = line.find("//") {
        cut = cut.min(i);
    }
    if let Some(i) = line.find('#') {
        cut = cut.min(i);
    }
    &line[..cut]
}


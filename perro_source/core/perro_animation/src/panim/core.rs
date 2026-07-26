pub fn parse_panim(source: &str) -> Result<AnimationClip, String> {
    let mut parser = PanimParser::new(source);
    parser.parse()
}

struct PanimParser<'a> {
    lines: Vec<&'a str>,
    index: usize,
    vars: HashMap<String, SceneValue>,
}

impl<'a> PanimParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            lines: source.lines().collect(),
            index: 0,
            vars: HashMap::new(),
        }
    }

    fn parse(&mut self) -> Result<AnimationClip, String> {
        let mut name = Cow::Borrowed("Animation");
        let mut fps = 60.0f32;
        let mut default_interpolation = AnimationInterpolation::Linear;
        let mut default_ease = AnimationEase::Linear;
        let mut objects: Vec<AnimationObject> = Vec::new();
        let mut object_types = HashMap::<String, NodeType>::new();
        let mut frame_actions: Vec<FrameAction> = Vec::new();
        let mut max_frame = 0u32;

        while let Some((line_no, line)) = self.next_line() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            if let Some((name, value_src)) = parse_top_level_var_assign(line) {
                let value = self.parse_value_with_vars(value_src, line_no)?;
                self.vars.insert(name.to_string(), value);
                continue;
            }

            if line.eq_ignore_ascii_case("[Animation]") {
                let (n, f, interp, ease) = self.parse_animation_block(line_no)?;
                name = Cow::Owned(n);
                fps = f;
                default_interpolation = interp;
                default_ease = ease;
                continue;
            }
            if line.eq_ignore_ascii_case("[Objects]") {
                let parsed_objects = self.parse_objects_block(line_no)?;
                object_types.clear();
                for obj in &parsed_objects {
                    object_types.insert(obj.name.to_string(), obj.node_type);
                }
                objects = parsed_objects;
                continue;
            }
            if let Some((frame, key_mode)) = parse_frame_header(line) {
                max_frame = max_frame.max(frame);
                frame_actions.extend(self.parse_frame_block(line_no, frame, key_mode, &object_types)?);
                continue;
            }

            return Err(format!(
                "line {}: unexpected top-level token `{}`",
                line_no, line
            ));
        }

        let (object_tracks, mut frame_events) =
            build_tracks_and_events(
                frame_actions,
                &object_types,
                default_interpolation,
                default_ease,
            )?;
        frame_events.sort_by_key(|e| e.frame);

        let total_frames = max_frame.saturating_add(1).max(1);

        Ok(AnimationClip {
            name,
            fps,
            total_frames,
            objects: Cow::Owned(objects),
            object_tracks: Cow::Owned(object_tracks),
            frame_events: Cow::Owned(frame_events),
        })
    }

    fn parse_animation_block(
        &mut self,
        start_line: usize,
    ) -> Result<(String, f32, AnimationInterpolation, AnimationEase), String> {
        let mut name = String::from("Animation");
        let mut fps = 60.0f32;
        let mut default_interpolation = AnimationInterpolation::Linear;
        let mut default_ease = AnimationEase::Linear;

        while let Some((line_no, line)) = self.next_line() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            if line.eq_ignore_ascii_case("[/Animation]") {
                if !fps.is_finite() || fps <= 0.0 {
                    return Err(format!(
                        "line {}: fps must be a finite positive number",
                        start_line
                    ));
                }
                return Ok((name, fps, default_interpolation, default_ease));
            }

            let Some((k, v)) = split_key_value(line) else {
                return Err(format!("line {}: expected `key = value`", line_no));
            };
            let value = self.parse_value_with_vars(v, line_no)?;
            match k {
                "name" => {
                    name = as_text(&value)
                        .ok_or_else(|| format!("line {}: name must be text", line_no))?
                        .to_string();
                }
                "fps" => {
                    fps = value
                        .as_f32()
                        .ok_or_else(|| format!("line {}: fps must be a number", line_no))?;
                }
                "default_interp" | "default_interpolation" => {
                    default_interpolation = parse_interpolation_value(&value, line_no)?;
                }
                "default_ease" | "default_easing" => {
                    default_ease = parse_ease_value(&value, line_no)?;
                }
                _ => {}
            }
        }

        Err(format!(
            "line {}: missing closing `[/Animation]` block",
            start_line
        ))
    }

    fn parse_objects_block(&mut self, start_line: usize) -> Result<Vec<AnimationObject>, String> {
        let mut objects = Vec::new();
        while let Some((line_no, line)) = self.next_line() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            if line.eq_ignore_ascii_case("[/Objects]") {
                return Ok(objects);
            }

            let rest = line.strip_prefix('@').unwrap_or(line);
            let Some((name, ty)) = rest.split_once('=') else {
                return Err(format!("line {}: expected `Name = NodeType`", line_no));
            };
            let name = name.trim();
            if name.is_empty() {
                return Err(format!("line {}: object name cannot be empty", line_no));
            }
            if name.starts_with('@') {
                return Err(format!(
                    "line {}: object declarations use `Name = NodeType`; refs use `@Name`",
                    line_no
                ));
            }

            let ty_value = self.parse_value_with_vars(ty.trim(), line_no)?;
            let ty = as_text(&ty_value)
                .ok_or_else(|| format!("line {}: object type must be text", line_no))?;
            let node_type = ty
                .parse::<NodeType>()
                .map_err(|_| format!("line {}: unknown object type `{}`", line_no, ty))?;
            objects.push(AnimationObject {
                name: name.to_string().into(),
                node_type,
            });
        }

        Err(format!(
            "line {}: missing closing `[/Objects]` block",
            start_line
        ))
    }

    fn parse_frame_block(
        &mut self,
        start_line: usize,
        frame: u32,
        key_mode: AnimationKeyMode,
        object_types: &HashMap<String, NodeType>,
    ) -> Result<Vec<FrameAction>, String> {
        let mut actions = Vec::new();

        while let Some((line_no, line)) = self.next_line() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            if is_frame_footer(line) {
                return Ok(actions);
            }

            if line.starts_with('@') {
                if line.ends_with('{') {
                    actions.extend(self.parse_object_block(
                        line_no,
                        frame,
                        key_mode,
                        line,
                        object_types,
                    )?);
                    continue;
                }
                if line.ends_with('}') {
                    actions.extend(self.parse_inline_object_block(
                        line_no,
                        frame,
                        key_mode,
                        line,
                        object_types,
                    )?);
                    continue;
                }
            }

            if let Some((k, v)) = split_key_value(line)
                && k == "emit_signal"
            {
                actions.push(FrameAction::Event {
                    frame,
                    scope: AnimationEventScope::Global,
                    event: parse_emit_signal(v, line_no)?,
                });
                continue;
            }

            return Err(format!("line {}: invalid frame entry `{}`", line_no, line));
        }

        Err(format!("line {}: missing frame footer", start_line))
    }

    fn parse_object_block(
        &mut self,
        start_line: usize,
        frame: u32,
        key_mode: AnimationKeyMode,
        header: &str,
        object_types: &HashMap<String, NodeType>,
    ) -> Result<Vec<FrameAction>, String> {
        let object = header
            .strip_prefix('@')
            .and_then(|v| v.strip_suffix('{'))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("line {}: invalid object header", start_line))?
            .to_string();

        let node_type = object_types
            .get(&object)
            .ok_or_else(|| format!("line {}: unknown object `@{}`", start_line, object))?;
        let node_type_name = node_type.as_str();
        let mut actions = Vec::new();
        while let Some((line_no, line)) = self.next_line() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            if line == "}" {
                return Ok(actions);
            }

            let Some((k, v)) = split_key_value(line) else {
                return Err(format!(
                    "line {}: expected `key = value` inside object block",
                    line_no
                ));
            };

            self.parse_object_assignment(
                &mut actions,
                line_no,
                frame,
                key_mode,
                &object,
                node_type_name,
                k,
                v,
            )?;
        }

        Err(format!(
            "line {}: object block `{}` missing closing `}}`",
            start_line, object
        ))
    }

    fn parse_inline_object_block(
        &mut self,
        line_no: usize,
        frame: u32,
        key_mode: AnimationKeyMode,
        line: &str,
        object_types: &HashMap<String, NodeType>,
    ) -> Result<Vec<FrameAction>, String> {
        let open = line
            .find('{')
            .ok_or_else(|| format!("line {}: invalid inline object block", line_no))?;
        let object = line[..open]
            .strip_prefix('@')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| format!("line {}: invalid object header", line_no))?
            .to_string();
        let body = line[open + 1..]
            .strip_suffix('}')
            .map(str::trim)
            .ok_or_else(|| format!("line {}: invalid inline object block", line_no))?;
        let node_type = object_types
            .get(&object)
            .ok_or_else(|| format!("line {}: unknown object `@{}`", line_no, object))?;
        let node_type_name = node_type.as_str();
        let assignments = split_inline_assignments(body, line_no)?;
        if assignments.is_empty() {
            return Err(format!(
                "line {}: inline object block `@{}` has no assignments",
                line_no, object
            ));
        }
        let mut actions = Vec::with_capacity(assignments.len());
        for (key, value) in assignments {
            self.parse_object_assignment(
                &mut actions,
                line_no,
                frame,
                key_mode,
                &object,
                node_type_name,
                key,
                value,
            )?;
        }
        Ok(actions)
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_object_assignment(
        &mut self,
        actions: &mut Vec<FrameAction>,
        line_no: usize,
        frame: u32,
        key_mode: AnimationKeyMode,
        object: &str,
        node_type_name: &str,
        key: &str,
        value_src: &str,
    ) -> Result<(), String> {
        match key {
            "emit_signal" => actions.push(FrameAction::Event {
                frame,
                scope: AnimationEventScope::Object(object.to_string().into()),
                event: parse_emit_signal(value_src, line_no)?,
            }),
            "set_var" => actions.push(FrameAction::Event {
                frame,
                scope: AnimationEventScope::Object(object.to_string().into()),
                event: parse_set_var(value_src, line_no)?,
            }),
            "call_method" => actions.push(FrameAction::Event {
                frame,
                scope: AnimationEventScope::Object(object.to_string().into()),
                event: parse_call_method(value_src, line_no)?,
            }),
            _ => {
                let value = self.parse_value_with_vars(value_src, line_no)?;
                if let Some(action) = parse_track_control_action(
                    frame,
                    object,
                    node_type_name,
                    key,
                    &value,
                    line_no,
                )? {
                    actions.push(action);
                } else {
                    actions.push(parse_object_field_action(
                        frame,
                        key_mode,
                        object,
                        node_type_name,
                        key,
                        &value,
                        line_no,
                    )?);
                }
            }
        }
        Ok(())
    }

    fn next_line(&mut self) -> Option<(usize, &'a str)> {
        if self.index >= self.lines.len() {
            return None;
        }
        let line_no = self.index + 1;
        let line = self.lines[self.index];
        self.index += 1;
        Some((line_no, line))
    }

    fn parse_value_with_vars(&self, value_src: &str, line_no: usize) -> Result<SceneValue, String> {
        let value_src = value_src.trim();
        if let Some(var_name) = value_src.strip_prefix('@')
            && !var_name.is_empty()
            && var_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return self
                .vars
                .get(var_name)
                .cloned()
                .ok_or_else(|| format!("line {}: unknown variable `@{}`", line_no, var_name));
        }
        parse_scene_value(value_src, line_no)
    }
}

fn split_inline_assignments(mut body: &str, line_no: usize) -> Result<Vec<(&str, &str)>, String> {
    let mut out = Vec::new();
    while !body.trim().is_empty() {
        body = body.trim_start();
        let key_end = body
            .find(|ch: char| ch.is_ascii_whitespace() || ch == '=')
            .unwrap_or(body.len());
        let key = body[..key_end].trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.'))
        {
            return Err(format!(
                "line {}: invalid inline object assignment `{}`",
                line_no, body
            ));
        }
        let after_key = body[key_end..].trim_start();
        let Some(value_src) = after_key.strip_prefix('=') else {
            return Err(format!(
                "line {}: expected `=` after inline object key `{}`",
                line_no, key
            ));
        };
        let value_src = value_src.trim_start();
        if value_src.is_empty() {
            return Err(format!(
                "line {}: missing inline object value for `{}`",
                line_no, key
            ));
        }

        let mut paren_depth = 0u32;
        let mut bracket_depth = 0u32;
        let mut brace_depth = 0u32;
        let mut quote = None;
        let mut escaped = false;
        let mut split = value_src.len();
        for (index, ch) in value_src.char_indices() {
            if let Some(active_quote) = quote {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == active_quote {
                    quote = None;
                }
                continue;
            }
            match ch {
                '"' | '\'' => quote = Some(ch),
                '(' => paren_depth = paren_depth.saturating_add(1),
                ')' => paren_depth = paren_depth.saturating_sub(1),
                '[' => bracket_depth = bracket_depth.saturating_add(1),
                ']' => bracket_depth = bracket_depth.saturating_sub(1),
                '{' => brace_depth = brace_depth.saturating_add(1),
                '}' => brace_depth = brace_depth.saturating_sub(1),
                _ if ch.is_ascii_whitespace()
                    && paren_depth == 0
                    && bracket_depth == 0
                    && brace_depth == 0
                    && starts_inline_assignment(value_src[index..].trim_start()) =>
                {
                    split = index;
                    break;
                }
                _ => {}
            }
        }
        let value = value_src[..split].trim();
        if value.is_empty() {
            return Err(format!(
                "line {}: missing inline object value for `{}`",
                line_no, key
            ));
        }
        out.push((key, value));
        body = &value_src[split..];
    }
    Ok(out)
}

fn starts_inline_assignment(src: &str) -> bool {
    let key_end = src
        .find(|ch: char| ch.is_ascii_whitespace() || ch == '=')
        .unwrap_or(src.len());
    key_end > 0
        && src[..key_end]
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.'))
        && src[key_end..].trim_start().starts_with('=')
}

fn parse_top_level_var_assign(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix('@')?;
    let (name, value) = rest.split_once('=')?;
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() || value.is_empty() {
        return None;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    Some((name, value))
}
